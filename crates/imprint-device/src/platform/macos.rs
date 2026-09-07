use std::ffi::c_void;
use std::path::PathBuf;
use std::process::Command;
use std::ptr;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use imprint_core::{BusKind, DiskId, Result, TargetDisk};
use plist::Value;

pub fn list() -> Result<Vec<TargetDisk>> {
  let output = Command::new("diskutil").args(["list", "-plist"]).output()?;
  if !output.status.success() {
    return Ok(Vec::new());
  }

  let value: Value =
    plist::from_bytes(&output.stdout).unwrap_or(Value::Dictionary(Default::default()));
  let mut disks = Vec::new();
  let Some(all) = value
    .as_dictionary()
    .and_then(|d| d.get("AllDisksAndPartitions"))
    .and_then(|v| v.as_array())
  else {
    return Ok(disks);
  };

  for entry in all {
    let Some(dict) = entry.as_dictionary() else {
      continue;
    };
    let device = dict
      .get("DeviceIdentifier")
      .and_then(|v| v.as_string())
      .unwrap_or("");
    if device.is_empty() {
      continue;
    }

    let size = dict
      .get("Size")
      .and_then(|v| v.as_unsigned_integer())
      .unwrap_or(0);
    if size == 0 {
      continue;
    }

    let info = diskutil_info(device);
    let content = dict
      .get("Content")
      .and_then(|v| v.as_string())
      .unwrap_or("");
    let mut system =
      device == "disk0" || content.contains("APFS") && info.internal || info.internal;
    if info.removable {
      system = false;
    }

    let bus = if info.protocol.to_ascii_lowercase().contains("usb") {
      BusKind::Usb
    } else if info.protocol.to_ascii_lowercase().contains("secure")
      || info.protocol.to_ascii_lowercase().contains("sd")
    {
      BusKind::Sd
    } else if info.protocol.to_ascii_lowercase().contains("thunderbolt") {
      BusKind::Thunderbolt
    } else if info.internal {
      BusKind::Nvme
    } else {
      BusKind::Unknown
    };

    let name = if info.media_name.is_empty() {
      device.to_string()
    } else {
      info.media_name.clone()
    };

    // Prefer rdisk for faster raw I/O.
    let raw = format!("/dev/r{device}");
    let path = if PathBuf::from(&raw).exists() {
      PathBuf::from(raw)
    } else {
      PathBuf::from(format!("/dev/{device}"))
    };

    disks.push(TargetDisk {
      id: DiskId(device.to_string()),
      name,
      path,
      size,
      bus,
      system,
      description: info.protocol,
    });
  }

  Ok(disks)
}

struct DiskInfo {
  media_name: String,
  protocol: String,
  internal: bool,
  removable: bool,
}

fn diskutil_info(device: &str) -> DiskInfo {
  let output = Command::new("diskutil")
    .args(["info", "-plist", device])
    .output();
  let Ok(output) = output else {
    return DiskInfo {
      media_name: String::new(),
      protocol: String::new(),
      internal: true,
      removable: false,
    };
  };
  let value: Value =
    plist::from_bytes(&output.stdout).unwrap_or(Value::Dictionary(Default::default()));
  let dict = value.as_dictionary();
  let str_field = |key: &str| {
    dict
      .and_then(|d| d.get(key))
      .and_then(|v| v.as_string())
      .unwrap_or("")
      .to_string()
  };
  let bool_field = |key: &str| {
    dict
      .and_then(|d| d.get(key))
      .and_then(|v| v.as_boolean())
      .unwrap_or(false)
  };
  DiskInfo {
    media_name: str_field("MediaName"),
    protocol: str_field("BusProtocol"),
    internal: bool_field("Internal"),
    removable: bool_field("Removable") || bool_field("RemovableMedia"),
  }
}

pub struct Watch {
  runloop: Option<SendPtr>,
  thread: Option<JoinHandle<()>>,
}

struct SendPtr(*mut c_void);

unsafe impl Send for SendPtr {}

struct ChangeFn(Box<dyn Fn() + Send>);

pub fn watch(on_change: Box<dyn Fn() + Send>) -> Watch {
  let (ready_tx, ready_rx) = mpsc::channel();
  let thread = thread::Builder::new()
    .name("imprint-disk-watch".into())
    .spawn(move || da_run(on_change, ready_tx))
    .ok();
  let Some(thread) = thread else {
    return Watch {
      runloop: None,
      thread: None,
    };
  };
  Watch {
    runloop: ready_rx.recv().ok().flatten(),
    thread: Some(thread),
  }
}

fn da_run(on_change: Box<dyn Fn() + Send>, ready: mpsc::Sender<Option<SendPtr>>) {
  unsafe {
    let session = DASessionCreate(ptr::null());
    if session.is_null() {
      let _ = ready.send(None);
      return;
    }
    let ctx = Box::into_raw(Box::new(ChangeFn(on_change)));
    DARegisterDiskAppearedCallback(session, ptr::null(), da_event, ctx.cast());
    DARegisterDiskDisappearedCallback(session, ptr::null(), da_event, ctx.cast());
    DARegisterDiskDescriptionChangedCallback(
      session,
      ptr::null(),
      ptr::null(),
      da_changed,
      ctx.cast(),
    );
    let runloop = CFRunLoopGetCurrent();
    DASessionScheduleWithRunLoop(session, runloop, kCFRunLoopDefaultMode);
    let _ = ready.send(Some(SendPtr(runloop)));
    CFRunLoopRun();
    DASessionUnscheduleFromRunLoop(session, runloop, kCFRunLoopDefaultMode);
    CFRelease(session.cast());
    drop(Box::from_raw(ctx));
  }
}

impl Drop for Watch {
  fn drop(&mut self) {
    if let Some(runloop) = self.runloop.take() {
      unsafe {
        CFRunLoopStop(runloop.0);
      }
    }
    if let Some(thread) = self.thread.take() {
      let _ = thread.join();
    }
  }
}

unsafe extern "C" fn da_event(disk: DADiskRef, ctx: *mut c_void) {
  unsafe {
    if disk_is_partition(disk) {
      return;
    }
    (*ctx.cast::<ChangeFn>()).0();
  }
}

unsafe extern "C" fn da_changed(disk: DADiskRef, _keys: CFArrayRef, ctx: *mut c_void) {
  unsafe {
    if disk_is_partition(disk) {
      return;
    }
    (*ctx.cast::<ChangeFn>()).0();
  }
}

fn disk_is_partition(disk: DADiskRef) -> bool {
  unsafe {
    let name = DADiskGetBSDName(disk);
    if name.is_null() {
      return false;
    }
    std::ffi::CStr::from_ptr(name)
      .to_str()
      .ok()
      .and_then(|s| s.strip_prefix("disk"))
      .is_some_and(|rest| rest.contains('s'))
  }
}

type CFAllocatorRef = *const c_void;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFArrayRef = *const c_void;
type CFRunLoopRef = *mut c_void;
type DASessionRef = *mut c_void;
type DADiskRef = *const c_void;
type DACallback = unsafe extern "C" fn(DADiskRef, *mut c_void);
type DAChangedCallback = unsafe extern "C" fn(DADiskRef, CFArrayRef, *mut c_void);

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
  fn CFRelease(cf: CFTypeRef);
  fn CFRunLoopGetCurrent() -> CFRunLoopRef;
  fn CFRunLoopRun();
  fn CFRunLoopStop(rl: CFRunLoopRef);
  static kCFRunLoopDefaultMode: CFStringRef;
}

#[link(name = "DiskArbitration", kind = "framework")]
unsafe extern "C" {
  fn DASessionCreate(allocator: CFAllocatorRef) -> DASessionRef;
  fn DASessionScheduleWithRunLoop(session: DASessionRef, run_loop: CFRunLoopRef, mode: CFStringRef);
  fn DASessionUnscheduleFromRunLoop(
    session: DASessionRef,
    run_loop: CFRunLoopRef,
    mode: CFStringRef,
  );
  fn DARegisterDiskAppearedCallback(
    session: DASessionRef,
    match_dict: CFDictionaryRef,
    callback: DACallback,
    context: *mut c_void,
  );
  fn DARegisterDiskDisappearedCallback(
    session: DASessionRef,
    match_dict: CFDictionaryRef,
    callback: DACallback,
    context: *mut c_void,
  );
  fn DARegisterDiskDescriptionChangedCallback(
    session: DASessionRef,
    match_dict: CFDictionaryRef,
    watch: CFArrayRef,
    callback: DAChangedCallback,
    context: *mut c_void,
  );
  fn DADiskGetBSDName(disk: DADiskRef) -> *const i8;
}
