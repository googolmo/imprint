use serde::{Deserialize, Serialize};

use crate::{ImageRef, TargetDisk};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashRequest {
  pub image: ImageRef,
  pub targets: Vec<TargetDisk>,
  pub verify: bool,
  pub unmount: bool,
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

  pub fn percent(&self) -> u32 {
    (self.fraction() * 100.0).round() as u32
  }
}
