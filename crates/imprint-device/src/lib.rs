//! List removable disks, hide system drives, unmount before write.

mod platform;
mod protect;
mod unmount;

pub use platform::list_disks;
pub use protect::{is_system_disk, system_roots};
pub use unmount::{eject, unmount};

use imprint_core::{Result, Settings, TargetDisk};

/// Disks the UI should offer. System drives are omitted unless allowed.
pub fn list_targets(settings: &Settings) -> Result<Vec<TargetDisk>> {
  let mut disks = list_disks()?;
  if settings.hide_system_drives && !settings.allow_system_drives {
    disks.retain(|d| !d.system);
  }
  disks.sort_by(|a, b| a.size.cmp(&b.size).then(a.name.cmp(&b.name)));
  Ok(disks)
}
