use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Stable id for a physical disk for the lifetime of one enumeration pass.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiskId(pub String);

impl DiskId {
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl From<&str> for DiskId {
  fn from(value: &str) -> Self {
    Self(value.to_string())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BusKind {
  Usb,
  Sd,
  Thunderbolt,
  Nvme,
  Sata,
  Virtual,
  Unknown,
}

impl BusKind {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Usb => "USB",
      Self::Sd => "SD",
      Self::Thunderbolt => "Thunderbolt",
      Self::Nvme => "NVMe",
      Self::Sata => "SATA",
      Self::Virtual => "Virtual",
      Self::Unknown => "Disk",
    }
  }
}

/// A writable block device. `system` disks are hidden by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetDisk {
  pub id: DiskId,
  pub name: String,
  pub path: PathBuf,
  pub size: u64,
  pub bus: BusKind,
  pub system: bool,
  pub description: String,
}

impl TargetDisk {
  pub fn label(&self) -> String {
    if self.name.is_empty() {
      self.path.display().to_string()
    } else {
      self.name.clone()
    }
  }
}
