//! Privileged helper: the GUI/CLI stay unprivileged and re-exec as root to write.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use imprint_core::{Error, FlashProgress, FlashRequest, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::elevate::{ElevatedChild, INTERNAL_FLASH_FLAG, spawn_elevated};
use crate::write::flash_in_process;

#[derive(Debug, Serialize, Deserialize)]
struct HelperStatus {
  ok: bool,
  #[serde(default)]
  error: Option<String>,
}

struct Session {
  dir: PathBuf,
}

impl Session {
  fn create() -> Result<Self> {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("imprint-flash-{}-{nanos}", std::process::id()));
    fs::create_dir(&dir)?;
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(Self { dir })
  }

  fn open(dir: impl Into<PathBuf>) -> Self {
    Self { dir: dir.into() }
  }

  fn request_path(&self) -> PathBuf {
    self.dir.join("request.json")
  }

  fn progress_path(&self) -> PathBuf {
    self.dir.join("progress.json")
  }

  fn status_path(&self) -> PathBuf {
    self.dir.join("status.json")
  }

  fn cancel_path(&self) -> PathBuf {
    self.dir.join("cancel")
  }

  fn write_request(&self, request: &FlashRequest) -> Result<()> {
    let bytes = serde_json::to_vec(request).map_err(|err| Error::msg(err.to_string()))?;
    atomic_write(&self.request_path(), &bytes)?;
    Ok(())
  }

  fn read_request(&self) -> Result<FlashRequest> {
    let bytes = fs::read(self.request_path())?;
    serde_json::from_slice(&bytes).map_err(|err| Error::msg(err.to_string()))
  }

  fn write_progress(&self, progress: &FlashProgress) {
    if let Ok(bytes) = serde_json::to_vec(progress) {
      let _ = atomic_write(&self.progress_path(), &bytes);
    }
  }

  fn read_progress(&self) -> Option<FlashProgress> {
    let bytes = fs::read(self.progress_path()).ok()?;
    serde_json::from_slice(&bytes).ok()
  }

  fn write_status(&self, ok: bool, error: Option<String>) {
    let status = HelperStatus { ok, error };
    if let Ok(bytes) = serde_json::to_vec(&status) {
      let _ = atomic_write(&self.status_path(), &bytes);
    }
  }

  fn read_status(&self) -> Option<HelperStatus> {
    let bytes = fs::read(self.status_path()).ok()?;
    serde_json::from_slice(&bytes).ok()
  }

  fn request_cancel(&self) {
    let _ = fs::write(self.cancel_path(), b"1");
  }

  fn elevate_stderr(&self) -> String {
    fs::read_to_string(self.dir.join("elevate.err"))
      .unwrap_or_default()
      .trim()
      .to_string()
  }

  fn helper_log(&self) -> String {
    fs::read_to_string(self.dir.join("helper.log"))
      .unwrap_or_default()
      .trim()
      .to_string()
  }
}

impl Drop for Session {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.dir);
  }
}

pub(crate) fn is_internal_flash() -> bool {
  parse_internal_flash_dir(std::env::args_os()).is_some()
}

/// If this process was launched as the privileged writer, run it and return an exit code.
pub fn run_internal_flash() -> Option<i32> {
  let dir = parse_internal_flash_dir(std::env::args_os())?;
  Some(run_helper(&dir))
}

pub(crate) fn parse_internal_flash_dir<I, S>(args: I) -> Option<PathBuf>
where
  I: IntoIterator<Item = S>,
  S: AsRef<OsStr>,
{
  let mut args = args.into_iter();
  let _exe = args.next()?;
  let flag = args.next()?;
  if flag.as_ref() != OsStr::new(INTERNAL_FLASH_FLAG) {
    return None;
  }
  Some(PathBuf::from(args.next()?.as_ref()))
}

fn run_helper(dir: &Path) -> i32 {
  #[cfg(unix)]
  {
    // SAFETY: umask is process-local; this process exists only to flash.
    unsafe {
      libc::umask(0o022);
    }
  }

  let session = Session::open(dir);
  relax_file_acl(&session.dir.join("helper.log"));
  let status_path = session.status_path();
  let _ = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    let status = HelperStatus {
      ok: false,
      error: Some(info.to_string()),
    };
    if let Ok(bytes) = serde_json::to_vec(&status) {
      let _ = atomic_write(&status_path, &bytes);
    }
  }));

  match run_helper_inner(&session) {
    Ok(()) => {
      session.write_status(true, None);
      // Leak the session so Drop does not delete files the parent still reads.
      std::mem::forget(session);
      0
    }
    Err(err) => {
      session.write_status(false, Some(err.localized()));
      std::mem::forget(session);
      1
    }
  }
}

fn run_helper_inner(session: &Session) -> Result<()> {
  info!("privileged flash helper in {}", session.dir.display());
  if !crate::has_block_privileges() {
    return Err(Error::Privileges(session.dir.display().to_string()));
  }
  let request = session.read_request()?;
  crate::validate_request(&request)?;
  let cancel = Arc::new(AtomicBool::new(false));
  let cancel_watch = cancel.clone();
  let cancel_path = session.cancel_path();
  thread::spawn(move || {
    while !cancel_watch.load(Ordering::Relaxed) {
      if cancel_path.exists() {
        cancel_watch.store(true, Ordering::Relaxed);
        break;
      }
      thread::sleep(Duration::from_millis(50));
    }
  });
  flash_in_process(request, cancel.as_ref(), |progress| {
    session.write_progress(&progress);
  })
}

pub(crate) fn flash_elevated(
  mut request: FlashRequest,
  cancel: &AtomicBool,
  mut on_progress: impl FnMut(FlashProgress),
) -> Result<()> {
  if let Ok(path) = request.image.path.canonicalize() {
    request.image.path = path;
  }
  let session = Session::create()?;
  session.write_request(&request)?;

  let exe = std::env::current_exe()?;
  let exe = fs::canonicalize(&exe).unwrap_or(exe);
  let mut child = spawn_elevated(&exe, &session.dir)?;
  if cancel.load(Ordering::Relaxed) {
    session.request_cancel();
    wait_briefly(&mut child);
    return Err(Error::Cancelled);
  }

  wait_helper(&session, &mut child, cancel, &mut on_progress)
}

fn wait_helper(
  session: &Session,
  child: &mut ElevatedChild,
  cancel: &AtomicBool,
  on_progress: &mut impl FnMut(FlashProgress),
) -> Result<()> {
  let started = Instant::now();
  loop {
    if cancel.load(Ordering::Relaxed) {
      session.request_cancel();
    }
    if let Some(progress) = session.read_progress() {
      on_progress(progress);
    }
    if let Some(status) = session.read_status() {
      let _ = child.try_wait();
      return status_to_result(status, cancel);
    }
    match child.try_wait() {
      Ok(Some(code)) => {
        for _ in 0..40 {
          if let Some(status) = session.read_status() {
            return status_to_result(status, cancel);
          }
          thread::sleep(Duration::from_millis(25));
        }
        if cancel.load(Ordering::Relaxed) {
          return Err(Error::Cancelled);
        }
        let stderr = session.elevate_stderr();
        if looks_like_auth_cancel(&stderr) {
          return Err(Error::ElevationCancelled);
        }
        let log = session.helper_log();
        let mut reason = if stderr.is_empty() {
          format!("privileged helper exited with status {code}")
        } else {
          stderr
        };
        if !log.is_empty() {
          reason = format!("{reason}\n{log}");
        }
        return Err(Error::ElevationFailed(reason));
      }
      Ok(None) => {}
      Err(err) => return Err(err.into()),
    }
    if started.elapsed() > Duration::from_secs(24 * 60 * 60) {
      session.request_cancel();
      return Err(Error::msg("privileged helper timed out"));
    }
    thread::sleep(Duration::from_millis(50));
  }
}

fn status_to_result(status: HelperStatus, cancel: &AtomicBool) -> Result<()> {
  if status.ok {
    Ok(())
  } else if cancel.load(Ordering::Relaxed) {
    Err(Error::Cancelled)
  } else if let Some(error) = status.error {
    Err(Error::msg(error))
  } else {
    Err(Error::msg("privileged helper failed"))
  }
}

fn wait_briefly(child: &mut ElevatedChild) {
  for _ in 0..40 {
    if child.try_wait().ok().flatten().is_some() {
      break;
    }
    thread::sleep(Duration::from_millis(50));
  }
}

fn looks_like_auth_cancel(text: &str) -> bool {
  let lower = text.to_ascii_lowercase();
  lower.contains("user canceled")
    || lower.contains("user cancelled")
    || text.contains("-128")
    || lower.contains("not authorized")
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
  let tmp = path.with_extension("tmp");
  fs::write(&tmp, bytes)?;
  relax_file_acl(&tmp);
  let _ = fs::remove_file(path);
  fs::rename(&tmp, path)?;
  relax_file_acl(path);
  Ok(())
}

fn relax_file_acl(path: &Path) {
  #[cfg(unix)]
  {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
    let Some(dir) = path.parent() else {
      return;
    };
    let Ok(meta) = fs::metadata(dir) else {
      return;
    };
    let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
      return;
    };
    // SAFETY: path is a file we just wrote in the session directory.
    unsafe {
      libc::chown(c_path.as_ptr(), meta.uid(), meta.gid());
    }
  }
  #[cfg(not(unix))]
  {
    let _ = path;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use imprint_core::{BusKind, DiskId, FlashPhase, ImageKind, ImageRef, TargetDisk};

  #[test]
  fn parse_flag_present() {
    let dir = parse_internal_flash_dir(["imprint", INTERNAL_FLASH_FLAG, "/tmp/imprint-flash-1"]);
    assert_eq!(dir.as_deref(), Some(Path::new("/tmp/imprint-flash-1")));
  }

  #[test]
  fn parse_flag_absent() {
    assert_eq!(
      parse_internal_flash_dir(["imprint", "flash", "disk.img"]),
      None
    );
    assert_eq!(parse_internal_flash_dir(["imprint"]), None);
  }

  #[test]
  fn session_request_and_status_roundtrip() {
    let session = Session::create().expect("temp session");
    let request = FlashRequest {
      image: ImageRef {
        path: PathBuf::from("/tmp/os.iso"),
        display_name: "os.iso".into(),
        kind: ImageKind::Iso,
        compression: None,
        file_size: 1024,
        payload_size: 1024,
      },
      targets: vec![TargetDisk {
        id: DiskId("disk4".into()),
        name: "USB".into(),
        path: PathBuf::from("/dev/rdisk4"),
        size: 8 * 1024 * 1024 * 1024,
        bus: BusKind::Usb,
        system: false,
        description: "USB".into(),
      }],
      verify: true,
      unmount: true,
      boot: None,
      expand: true,
    };
    session.write_request(&request).unwrap();
    let loaded = session.read_request().unwrap();
    assert_eq!(loaded.image.display_name, "os.iso");
    assert_eq!(loaded.targets[0].path, PathBuf::from("/dev/rdisk4"));

    session.write_progress(&FlashProgress {
      phase: FlashPhase::Writing,
      bytes_done: 10,
      bytes_total: 100,
      bytes_per_sec: 1,
      target_label: "USB".into(),
      message: "10 B written".into(),
    });
    let progress = session.read_progress().unwrap();
    assert_eq!(progress.bytes_done, 10);

    session.write_status(false, Some("boom".into()));
    let status = session.read_status().unwrap();
    assert!(!status.ok);
    assert_eq!(status.error.as_deref(), Some("boom"));
  }
}
