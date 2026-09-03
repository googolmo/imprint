use serde::{Deserialize, Serialize};

use crate::{ImageRef, TargetDisk};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashRequest {
  pub image: ImageRef,
  pub targets: Vec<TargetDisk>,
  pub verify: bool,
  pub unmount: bool,
  /// Files written to the first FAT partition after the image (Raspberry Pi boot).
  #[serde(default)]
  pub boot: Option<BootCustomization>,
  /// Grow the last partition so a smaller image fills unused space on the disk.
  #[serde(default = "default_expand")]
  pub expand: bool,
}

fn default_expand() -> bool {
  true
}

/// Text files dropped onto the imaged FAT boot partition, plus an optional
/// `cmdline.txt` append used by Raspberry Pi `systemd` first-boot scripts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootCustomization {
  pub files: Vec<BootFile>,
  #[serde(default)]
  pub cmdline_append: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootFile {
  pub name: String,
  pub contents: String,
}

impl BootCustomization {
  pub fn is_empty(&self) -> bool {
    self.files.is_empty() && self.cmdline_append.is_none()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashPhase {
  Preparing,
  Writing,
  Verifying,
  Finishing,
  Done,
  Failed,
}

impl FlashPhase {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Preparing => "Preparing",
      Self::Writing => "Flashing",
      Self::Verifying => "Validating",
      Self::Finishing => "Finishing",
      Self::Done => "Done",
      Self::Failed => "Failed",
    }
  }

  pub fn is_terminal(self) -> bool {
    matches!(self, Self::Done | Self::Failed)
  }

  /// Unmount / device flush have no byte counter the UI can show.
  pub fn is_indeterminate(self) -> bool {
    matches!(self, Self::Preparing | Self::Finishing)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashProgress {
  pub phase: FlashPhase,
  pub bytes_done: u64,
  pub bytes_total: u64,
  pub bytes_per_sec: u64,
  pub target_label: String,
  pub message: String,
}

impl FlashProgress {
  pub fn fraction(&self) -> f32 {
    if self.bytes_total == 0 {
      0.0
    } else {
      (self.bytes_done as f32 / self.bytes_total as f32).clamp(0.0, 1.0)
    }
  }

  /// Truncates toward zero so the bar does not read 100% while bytes remain.
  pub fn percent(&self) -> u32 {
    if self.bytes_total == 0 {
      0
    } else if self.bytes_done >= self.bytes_total {
      100
    } else {
      ((self.bytes_done as u128 * 100) / self.bytes_total as u128) as u32
    }
  }

  pub fn is_indeterminate(&self) -> bool {
    if self.phase.is_terminal() {
      return false;
    }
    self.phase.is_indeterminate() || self.bytes_total == 0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn progress(done: u64, total: u64) -> FlashProgress {
    FlashProgress {
      phase: FlashPhase::Writing,
      bytes_done: done,
      bytes_total: total,
      bytes_per_sec: 0,
      target_label: String::new(),
      message: String::new(),
    }
  }

  #[test]
  fn percent_stays_below_100_until_complete() {
    assert_eq!(progress(0, 100).percent(), 0);
    assert_eq!(progress(99, 100).percent(), 99);
    assert_eq!(progress(199, 200).percent(), 99);
    assert_eq!(progress(100, 100).percent(), 100);
    assert_eq!(progress(0, 0).percent(), 0);
  }

  #[test]
  fn preparing_and_finishing_are_indeterminate() {
    assert!(FlashPhase::Preparing.is_indeterminate());
    assert!(FlashPhase::Finishing.is_indeterminate());
    assert!(!FlashPhase::Writing.is_indeterminate());
    assert!(!FlashPhase::Verifying.is_indeterminate());
  }

  #[test]
  fn writing_without_a_total_is_indeterminate() {
    assert!(progress(1_200_000_000, 0).is_indeterminate());
    assert!(!progress(100, 200).is_indeterminate());
  }

  #[test]
  fn expand_defaults_on_when_missing_from_json() {
    let request: FlashRequest = serde_json::from_value(serde_json::json!({
      "image": {
        "path": "/tmp/os.img",
        "display_name": "os.img",
        "kind": "Img",
        "compression": null,
        "file_size": 1,
        "payload_size": 1
      },
      "targets": [],
      "verify": true,
      "unmount": true
    }))
    .unwrap();
    assert!(request.expand);
    assert!(request.boot.is_none());
  }
}
