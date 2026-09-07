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

pub struct Watch {
  #[cfg(target_os = "linux")]
  _inner: linux::Watch,
  #[cfg(target_os = "macos")]
  _inner: macos::Watch,
  #[cfg(windows)]
  _inner: windows::Watch,
}

pub fn watch(on_change: Box<dyn Fn() + Send>) -> Watch {
  #[cfg(target_os = "linux")]
  {
    Watch {
      _inner: linux::watch(on_change),
    }
  }
  #[cfg(target_os = "macos")]
  {
    Watch {
      _inner: macos::watch(on_change),
    }
  }
  #[cfg(windows)]
  {
    Watch {
      _inner: windows::watch(on_change),
    }
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
  {
    let _ = on_change;
    Watch {}
  }
}
