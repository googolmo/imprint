use serde::{Deserialize, Serialize};

use crate::LocalePref;

/// User-facing options, persisted by the UI later if needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
  pub verify: bool,
  pub unmount_on_success: bool,
  pub hide_system_drives: bool,
  pub allow_system_drives: bool,
  #[serde(default)]
  pub locale: LocalePref,
  /// Grow the last partition after writing so the image fills a larger disk.
  #[serde(default = "default_true")]
  pub expand_to_fill: bool,
}

fn default_true() -> bool {
  true
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      verify: true,
      unmount_on_success: true,
      hide_system_drives: true,
      allow_system_drives: false,
      locale: LocalePref::System,
      expand_to_fill: true,
    }
  }
}
