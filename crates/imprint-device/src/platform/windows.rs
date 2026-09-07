use std::ffi::c_void;
use std::path::PathBuf;
use std::process::Command;
use std::ptr;
use std::sync::Once;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use imprint_core::{BusKind, DiskId, Result, TargetDisk};

pub fn list() -> Result<Vec<TargetDisk>> {
  let script = r#"
Get-CimInstance Win32_DiskDrive | ForEach-Object {
  $iface = $_.InterfaceType
  $media = $_.MediaType
  $sys = if ($iface -eq 'IDE' -or $iface -eq 'SCSI' -or $media -match 'Fixed') { '1' } else { '0' }
  if ($iface -eq 'USB' -or $media -match 'Removable') { $sys = '0' }
  '{0}|{1}|{2}|{3}|{4}' -f $_.Index, $_.Model, $_.Size, $iface, $sys
}
"#;
  let output = Command::new("powershell")
    .args(["-NoProfile", "-Command", script])
    .output()?;
  if !output.status.success() {
    return Ok(Vec::new());
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  let mut disks = Vec::new();
  for line in stdout.lines() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 5 {
      continue;
    }
    let index = parts[0].trim();
    let model = parts[1].trim();
    let size: u64 = parts[2].trim().parse().unwrap_or(0);
    let iface = parts[3].trim();
    let system = parts[4].trim() == "1";
    if size == 0 {
      continue;
    }
    let bus = match iface.to_ascii_uppercase().as_str() {
      "USB" => BusKind::Usb,
      "SD" | "MMC" => BusKind::Sd,
      "SCSI" | "IDE" => BusKind::Sata,
      "NVME" => BusKind::Nvme,
      _ => BusKind::Unknown,
    };
    disks.push(TargetDisk {
      id: DiskId(format!("PhysicalDrive{index}")),
      name: model.to_string(),
      path: PathBuf::from(format!(r"\\.\PhysicalDrive{index}")),
      size,
      bus,
      system,
      description: iface.to_string(),
    });
  }
  Ok(disks)
}

pub struct Watch {
  hwnd: Option<SendHwnd>,
  thread: Option<JoinHandle<()>>,
}

struct SendHwnd(HWND);

unsafe impl Send for SendHwnd {}

struct ChangeFn(Box<dyn Fn() + Send>);

pub fn watch(on_change: Box<dyn Fn() + Send>) -> Watch {
  let (ready_tx, ready_rx) = mpsc::channel();
  let thread = thread::Builder::new()
    .name("imprint-disk-watch".into())
    .spawn(move || win_run(on_change, ready_tx))
    .ok();
  let Some(thread) = thread else {
    return Watch {
      hwnd: None,
      thread: None,
    };
  };
  Watch {
    hwnd: ready_rx.recv().ok().flatten(),
    thread: Some(thread),
  }
}

fn win_run(on_change: Box<dyn Fn() + Send>, ready: mpsc::Sender<Option<SendHwnd>>) {
  unsafe {
    register_class();
    let instance = GetModuleHandleW(ptr::null());
    let class = class_name();
    let hwnd = CreateWindowExW(
      0,
      class.as_ptr(),
      class.as_ptr(),
      0,
      0,
      0,
      0,
      0,
      HWND_MESSAGE,
      ptr::null_mut(),
      instance,
      ptr::null_mut(),
    );
    if hwnd.is_null() {
      let _ = ready.send(None);
      return;
    }
    let ctx = Box::into_raw(Box::new(ChangeFn(on_change)));
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx as isize);
    let _ = ready.send(Some(SendHwnd(hwnd)));
    let mut msg = std::mem::zeroed::<MSG>();
    loop {
      let status = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
      if status <= 0 {
        break;
      }
      TranslateMessage(&msg);
      DispatchMessageW(&msg);
    }
  }
}

impl Drop for Watch {
  fn drop(&mut self) {
    if let Some(hwnd) = self.hwnd.take() {
      unsafe {
        PostMessageW(hwnd.0, WM_CLOSE, 0, 0);
      }
    }
    if let Some(thread) = self.thread.take() {
      let _ = thread.join();
    }
  }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
  unsafe {
    match msg {
      WM_DEVICECHANGE => {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr != 0
          && let Some(cb) = (ptr as *const ChangeFn).as_ref()
        {
          (cb.0)();
        }
        1
      }
      WM_CLOSE => {
        DestroyWindow(hwnd);
        0
      }
      WM_DESTROY => {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        if ptr != 0 {
          drop(Box::from_raw(ptr as *mut ChangeFn));
        }
        PostQuitMessage(0);
        0
      }
      _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
  }
}

fn class_name() -> Vec<u16> {
  "imprint.diskwatch\0".encode_utf16().collect()
}

fn register_class() {
  static ONCE: Once = Once::new();
  ONCE.call_once(|| unsafe {
    let class = Box::leak(class_name().into_boxed_slice());
    let wc = WNDCLASSW {
      style: 0,
      lpfnWndProc: wndproc,
      cbClsExtra: 0,
      cbWndExtra: 0,
      hInstance: GetModuleHandleW(ptr::null()),
      hIcon: ptr::null_mut(),
      hCursor: ptr::null_mut(),
      hbrBackground: ptr::null_mut(),
      lpszMenuName: ptr::null(),
      lpszClassName: class.as_ptr(),
    };
    RegisterClassW(&wc);
  });
}

type HWND = *mut c_void;
type HINSTANCE = *mut c_void;
type HMENU = *mut c_void;
type HICON = *mut c_void;
type HCURSOR = *mut c_void;
type HBRUSH = *mut c_void;

const HWND_MESSAGE: HWND = -3isize as HWND;
const GWLP_USERDATA: i32 = -21;
const WM_CLOSE: u32 = 0x0010;
const WM_DESTROY: u32 = 0x0002;
const WM_DEVICECHANGE: u32 = 0x0219;

#[repr(C)]
struct POINT {
  x: i32,
  y: i32,
}

#[repr(C)]
struct MSG {
  hwnd: HWND,
  message: u32,
  wParam: usize,
  lParam: isize,
  time: u32,
  pt: POINT,
}

#[repr(C)]
struct WNDCLASSW {
  style: u32,
  lpfnWndProc: unsafe extern "system" fn(HWND, u32, usize, isize) -> isize,
  cbClsExtra: i32,
  cbWndExtra: i32,
  hInstance: HINSTANCE,
  hIcon: HICON,
  hCursor: HCURSOR,
  hbrBackground: HBRUSH,
  lpszMenuName: *const u16,
  lpszClassName: *const u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
  fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
}

#[link(name = "user32")]
unsafe extern "system" {
  fn RegisterClassW(class: *const WNDCLASSW) -> u16;
  fn CreateWindowExW(
    ex_style: u32,
    class_name: *const u16,
    window_name: *const u16,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    parent: HWND,
    menu: HMENU,
    instance: HINSTANCE,
    param: *mut c_void,
  ) -> HWND;
  fn DefWindowProcW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
  fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32) -> i32;
  fn TranslateMessage(msg: *const MSG) -> i32;
  fn DispatchMessageW(msg: *const MSG) -> isize;
  fn PostMessageW(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> i32;
  fn DestroyWindow(hwnd: HWND) -> i32;
  fn PostQuitMessage(exit_code: i32);
  fn SetWindowLongPtrW(hwnd: HWND, index: i32, value: isize) -> isize;
  fn GetWindowLongPtrW(hwnd: HWND, index: i32) -> isize;
}
