use std::path::Path;
use std::process::Command;

#[cfg(target_os = "macos")]
use imprint_core::Error;
use imprint_core::{Result, TargetDisk};

/// Unmount every filesystem on `disk` so the raw device can be opened.
pub fn unmount(disk: &TargetDisk) -> Result<()> {
  unmount_path(&disk.path)
}

pub fn eject(disk: &TargetDisk) -> Result<()> {
  eject_path(&disk.path)
}

fn unmount_path(path: &Path) -> Result<()> {
  #[cfg(target_os = "macos")]
  {
    let status = Command::new("diskutil")
      .args(["unmountDisk", "force"])
      .arg(path)
      .status()?;
    if !status.success() {
      return Err(Error::msg(format!(
        "diskutil unmountDisk failed for {}",
        path.display()
      )));
    }
    Ok(())
  }
  #[cfg(target_os = "linux")]
  {
    let _ = Command::new("umount").arg(path).status();
    // Also unmount partitions: /dev/sdb1, /dev/sdb2, …
    if let Some(name) = path.file_name().and_then(|n| n.to_str())
      && let Ok(entries) = std::fs::read_dir("/dev")
    {
      for entry in entries.flatten() {
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        if fname.starts_with(name) && fname != name {
          let _ = Command::new("umount").arg(entry.path()).status();
        }
      }
    }
    Ok(())
  }
  #[cfg(windows)]
  {
    let _ = Command::new("mountvol").arg(path).arg("/p").status();
    Ok(())
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
  {
    let _ = path;
    Ok(())
  }
}

fn eject_path(path: &Path) -> Result<()> {
  #[cfg(target_os = "macos")]
  {
    let status = Command::new("diskutil").arg("eject").arg(path).status()?;
    if !status.success() {
      return Err(Error::msg(format!(
        "diskutil eject failed for {}",
        path.display()
      )));
    }
    Ok(())
  }
  #[cfg(target_os = "linux")]
  {
    let _ = Command::new("eject").arg(path).status();
    Ok(())
  }
  #[cfg(windows)]
  {
    let _ = path;
    Ok(())
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
  {
    let _ = path;
    Ok(())
  }
}
