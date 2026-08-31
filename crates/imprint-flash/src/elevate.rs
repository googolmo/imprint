//! Spawn this binary again with administrator / root rights.

use std::io;
use std::path::Path;
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
use std::process::Child;

use imprint_core::{Error, Result};
use tracing::info;

pub(crate) const INTERNAL_FLASH_FLAG: &str = "--imprint-internal-flash";

pub struct ElevatedChild {
  inner: Inner,
}

enum Inner {
  #[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
  Process(Child),
  #[cfg(windows)]
  Handle(*mut core::ffi::c_void),
}

// SAFETY: the HANDLE is owned exclusively by ElevatedChild and is only used
// from the flash worker thread.
#[cfg(windows)]
unsafe impl Send for ElevatedChild {}

impl ElevatedChild {
  pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
    match &mut self.inner {
      #[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
      Inner::Process(child) => Ok(child.try_wait()?.map(|status| status.code().unwrap_or(1))),
      #[cfg(windows)]
      Inner::Handle(handle) => windows::try_wait(*handle),
    }
  }
}

impl Drop for ElevatedChild {
  fn drop(&mut self) {
    #[cfg(windows)]
    if let Inner::Handle(handle) = self.inner {
      if !handle.is_null() {
        // SAFETY: handle is the exclusive process handle from ShellExecuteExW.
        unsafe { windows::CloseHandle(handle) };
      }
    }
  }
}

pub fn spawn_elevated(exe: &Path, session_dir: &Path) -> Result<ElevatedChild> {
  info!("requesting administrator privileges for {}", exe.display());
  #[cfg(target_os = "macos")]
  {
    return macos::spawn(exe, session_dir);
  }
  #[cfg(any(target_os = "linux", target_os = "freebsd"))]
  {
    return linux::spawn(exe, session_dir);
  }
  #[cfg(windows)]
  {
    return windows::spawn(exe, session_dir);
  }
  #[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    windows
  )))]
  {
    let _ = (exe, session_dir);
    Err(Error::msg(
      "privilege elevation is not supported on this platform",
    ))
  }
}

fn elevation_failed(reason: impl Into<String>) -> Error {
  Error::ElevationFailed(reason.into())
}

#[cfg(target_os = "macos")]
mod macos {
  use std::fs;
  use std::io::Write;
  use std::path::Path;
  use std::process::{Command, Stdio};

  use imprint_core::Result;
  use imprint_core::i18n::t;

  use super::{ElevatedChild, INTERNAL_FLASH_FLAG, Inner, elevation_failed};

  // Run the helper in the foreground of the privileged shell. Backgrounding
  // with `nohup … & echo $!` returns a PID that is already dead by the time
  // the parent waits, which surfaced as "helper exited with status 0".
  const SCRIPT: &str = r#"on run argv
  set exe to item 1 of argv
  set sessionDir to item 2 of argv
  set promptText to item 3 of argv
  set flag to item 4 of argv
  set logFile to sessionDir & "/helper.log"
  set cmd to quoted form of exe & " " & quoted form of flag & " " & quoted form of sessionDir & " >" & quoted form of logFile & " 2>&1"
  with timeout of 86400 seconds
    do shell script cmd with administrator privileges with prompt promptText
  end timeout
end run
"#;

  pub fn spawn(exe: &Path, session_dir: &Path) -> Result<ElevatedChild> {
    let prompt = t("error.privileges_prompt");
    let err_file = fs::File::create(session_dir.join("elevate.err"))
      .map_err(|err| elevation_failed(format!("elevate.err: {err}")))?;
    let mut child = Command::new("osascript")
      .arg("-")
      .arg(exe)
      .arg(session_dir)
      .arg(&prompt)
      .arg(INTERNAL_FLASH_FLAG)
      .stdin(Stdio::piped())
      .stdout(Stdio::null())
      .stderr(Stdio::from(err_file))
      .spawn()
      .map_err(|err| elevation_failed(format!("osascript: {err}")))?;

    if let Some(mut stdin) = child.stdin.take() {
      stdin
        .write_all(SCRIPT.as_bytes())
        .map_err(|err| elevation_failed(format!("osascript stdin: {err}")))?;
    }

    Ok(ElevatedChild {
      inner: Inner::Process(child),
    })
  }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux {
  use std::io;
  use std::path::Path;
  use std::process::Command;

  use imprint_core::{Error, Result};

  use super::{ElevatedChild, INTERNAL_FLASH_FLAG, Inner, elevation_failed};

  pub fn spawn(exe: &Path, session_dir: &Path) -> Result<ElevatedChild> {
    let mut attempts = Vec::new();
    if let Some(pkexec) = first_existing(["pkexec", "/usr/bin/pkexec"]) {
      let mut cmd = Command::new(pkexec);
      cmd.arg(exe).arg(INTERNAL_FLASH_FLAG).arg(session_dir);
      attempts.push(cmd);
    }
    if let Some(sudo) = first_existing(["sudo", "/usr/bin/sudo"]) {
      let mut cmd = Command::new(sudo);
      cmd
        .arg("--")
        .arg(exe)
        .arg(INTERNAL_FLASH_FLAG)
        .arg(session_dir);
      attempts.push(cmd);
    }
    if attempts.is_empty() {
      return Err(elevation_failed(
        "install pkexec (polkit) or sudo to write disks",
      ));
    }

    let mut last_err: Option<io::Error> = None;
    for mut cmd in attempts {
      match cmd.spawn() {
        Ok(child) => {
          return Ok(ElevatedChild {
            inner: Inner::Process(child),
          });
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => last_err = Some(err),
        Err(err) => {
          if err.kind() == io::ErrorKind::PermissionDenied {
            return Err(Error::ElevationCancelled);
          }
          return Err(elevation_failed(err.to_string()));
        }
      }
    }
    Err(elevation_failed(
      last_err
        .map(|err| err.to_string())
        .unwrap_or_else(|| "no elevation helper".into()),
    ))
  }

  fn first_existing(names: impl IntoIterator<Item = &'static str>) -> Option<&'static str> {
    names.into_iter().find(|name| {
      Path::new(name).is_file()
        || Command::new("which")
          .arg(name)
          .output()
          .map(|out| out.status.success())
          .unwrap_or(false)
    })
  }
}

#[cfg(windows)]
mod windows {
  use std::ffi::OsStr;
  use std::io;
  use std::os::windows::ffi::OsStrExt;
  use std::path::Path;
  use std::ptr;

  use imprint_core::{Error, Result};

  use super::{ElevatedChild, INTERNAL_FLASH_FLAG, Inner, elevation_failed};

  pub(super) const WAIT_OBJECT_0: u32 = 0;
  pub(super) const WAIT_TIMEOUT: u32 = 258;
  const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;
  const SW_HIDE: i32 = 0;
  const ERROR_CANCELLED: u32 = 1223;

  #[repr(C)]
  struct ShellExecuteInfoW {
    cb_size: u32,
    f_mask: u32,
    hwnd: *mut core::ffi::c_void,
    lp_verb: *const u16,
    lp_file: *const u16,
    lp_parameters: *const u16,
    lp_directory: *const u16,
    n_show: i32,
    h_inst_app: *mut core::ffi::c_void,
    lp_id_list: *mut core::ffi::c_void,
    lp_class: *const u16,
    hkey_class: *mut core::ffi::c_void,
    dw_hot_key: u32,
    h_icon: *mut core::ffi::c_void,
    h_process: *mut core::ffi::c_void,
  }

  #[link(name = "shell32")]
  extern "system" {
    fn ShellExecuteExW(info: *mut ShellExecuteInfoW) -> i32;
  }
  #[link(name = "kernel32")]
  extern "system" {
    pub(super) fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
    fn WaitForSingleObject(handle: *mut core::ffi::c_void, millis: u32) -> u32;
    fn GetExitCodeProcess(handle: *mut core::ffi::c_void, code: *mut u32) -> i32;
    fn GetLastError() -> u32;
  }

  pub fn spawn(exe: &Path, session_dir: &Path) -> Result<ElevatedChild> {
    let exe_wide = wide(exe.as_os_str());
    let verb = wide(OsStr::new("runas"));
    let params = wide(OsStr::new(&format!(
      "{} {}",
      INTERNAL_FLASH_FLAG,
      quote_windows(&session_dir.to_string_lossy())
    )));

    let mut info = ShellExecuteInfoW {
      cb_size: std::mem::size_of::<ShellExecuteInfoW>() as u32,
      f_mask: SEE_MASK_NOCLOSEPROCESS,
      hwnd: ptr::null_mut(),
      lp_verb: verb.as_ptr(),
      lp_file: exe_wide.as_ptr(),
      lp_parameters: params.as_ptr(),
      lp_directory: ptr::null(),
      n_show: SW_HIDE,
      h_inst_app: ptr::null_mut(),
      lp_id_list: ptr::null_mut(),
      lp_class: ptr::null(),
      hkey_class: ptr::null_mut(),
      dw_hot_key: 0,
      h_icon: ptr::null_mut(),
      h_process: ptr::null_mut(),
    };

    // SAFETY: info points at a fully populated SHELLEXECUTEINFOW; wide strings
    // stay alive for the duration of the call.
    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
      // SAFETY: GetLastError immediately after a failed Win32 call.
      let err = unsafe { GetLastError() };
      if err == ERROR_CANCELLED {
        return Err(Error::ElevationCancelled);
      }
      return Err(elevation_failed(format!("UAC elevation failed ({err})")));
    }
    if info.h_process.is_null() {
      return Err(elevation_failed("UAC did not return a process handle"));
    }
    Ok(ElevatedChild {
      inner: Inner::Handle(info.h_process),
    })
  }

  pub(super) fn try_wait(handle: *mut core::ffi::c_void) -> io::Result<Option<i32>> {
    // SAFETY: handle is the exclusive process handle we own.
    let wait = unsafe { WaitForSingleObject(handle, 0) };
    if wait == WAIT_TIMEOUT {
      return Ok(None);
    }
    if wait != WAIT_OBJECT_0 {
      return Err(io::Error::last_os_error());
    }
    let mut code = 0u32;
    // SAFETY: process has exited; GetExitCodeProcess is valid on this handle.
    let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
    if ok == 0 {
      return Err(io::Error::last_os_error());
    }
    Ok(Some(code as i32))
  }

  fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
  }

  fn quote_windows(value: &str) -> String {
    if value.is_empty() || value.chars().any(|c| c.is_whitespace() || c == '"') {
      format!("\"{}\"", value.replace('"', "\\\""))
    } else {
      value.to_string()
    }
  }
}
