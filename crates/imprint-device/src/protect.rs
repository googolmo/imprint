use std::path::{Path, PathBuf};

use imprint_core::TargetDisk;

/// Paths that must never be flashed (root volume, ESP, recovery).
pub fn system_roots() -> Vec<PathBuf> {
  let roots = vec![PathBuf::from("/")];
  #[cfg(windows)]
  {
    if let Ok(win) = std::env::var("SystemDrive") {
      roots.push(PathBuf::from(win));
    } else {
      roots.push(PathBuf::from("C:\\"));
    }
  }
  roots
}

pub fn is_system_disk(disk: &TargetDisk) -> bool {
  disk.system
}

#[allow(dead_code)]
pub fn path_looks_system(path: &Path) -> bool {
  let text = path.to_string_lossy().to_ascii_lowercase();
  if text.contains("boot") && text.contains("efi") {
    return true;
  }
  #[cfg(target_os = "macos")]
  {
    // disk0 is almost always the internal Macintosh HD container.
    if text.contains("/dev/disk0") || text.contains("/dev/rdisk0") {
      return true;
    }
  }
  #[cfg(target_os = "linux")]
  {
    if text.contains("nvme0n1") && !text.contains("usb") {
      // Heuristic only; list_disks marks system using mount info.
    }
  }
  false
}
