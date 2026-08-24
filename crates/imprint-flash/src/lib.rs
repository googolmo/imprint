//! Write an image stream to one or more block devices, then optionally verify.

mod privilege;
mod verify;
mod write;

pub use privilege::has_block_privileges;
pub use write::flash;

use imprint_core::{Error, FlashRequest, Result, format_bytes};

pub fn validate_request(request: &FlashRequest) -> Result<()> {
  if request.targets.is_empty() {
    return Err(Error::NoTarget);
  }
  let need = request.image.write_size();
  for disk in &request.targets {
    if disk.system {
      return Err(Error::SystemDisk(disk.label()));
    }
    if need > 0 && disk.size < need {
      return Err(Error::TargetTooSmall {
        disk: disk.label(),
        have: format_bytes(disk.size),
        need: format_bytes(need),
      });
    }
  }
  Ok(())
}
