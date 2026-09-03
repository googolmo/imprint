use imprint_core::{Result, TargetDisk};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub fn list_disks() -> Result<Vec<TargetDisk>> {
  #[cfg(target_os = "linux")]
  {
    linux::list()
  }
  #[cfg(target_os = "macos")]
  {
    macos::list()
  }
  #[cfg(windows)]
  {
    windows::list()
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
  {
    Ok(Vec::new())
  }
}
