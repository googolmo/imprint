use std::path::PathBuf;
use std::process::Command;

use imprint_core::{BusKind, DiskId, Result, TargetDisk};
use plist::Value;

pub fn list() -> Result<Vec<TargetDisk>> {
  let output = Command::new("diskutil").args(["list", "-plist"]).output()?;
  if !output.status.success() {
    return Ok(Vec::new());
  }

  let value: Value =
    plist::from_bytes(&output.stdout).unwrap_or(Value::Dictionary(Default::default()));
  let mut disks = Vec::new();
  let Some(all) = value
    .as_dictionary()
    .and_then(|d| d.get("AllDisksAndPartitions"))
    .and_then(|v| v.as_array())
  else {
    return Ok(disks);
  };

  for entry in all {
    let Some(dict) = entry.as_dictionary() else {
      continue;
    };
    let device = dict
      .get("DeviceIdentifier")
      .and_then(|v| v.as_string())
      .unwrap_or("");
    if device.is_empty() {
      continue;
    }

    let size = dict
      .get("Size")
      .and_then(|v| v.as_unsigned_integer())
      .unwrap_or(0);
    if size == 0 {
      continue;
    }

    let info = diskutil_info(device);
    let content = dict
      .get("Content")
      .and_then(|v| v.as_string())
      .unwrap_or("");
    let mut system =
      device == "disk0" || content.contains("APFS") && info.internal || info.internal;
    if info.removable {
      system = false;
    }

    let bus = if info.protocol.to_ascii_lowercase().contains("usb") {
      BusKind::Usb
    } else if info.protocol.to_ascii_lowercase().contains("secure")
      || info.protocol.to_ascii_lowercase().contains("sd")
    {
      BusKind::Sd
    } else if info.protocol.to_ascii_lowercase().contains("thunderbolt") {
      BusKind::Thunderbolt
    } else if info.internal {
      BusKind::Nvme
    } else {
      BusKind::Unknown
    };

    let name = if info.media_name.is_empty() {
      device.to_string()
    } else {
      info.media_name.clone()
    };

    // Prefer rdisk for faster raw I/O.
    let raw = format!("/dev/r{device}");
    let path = if PathBuf::from(&raw).exists() {
      PathBuf::from(raw)
    } else {
      PathBuf::from(format!("/dev/{device}"))
    };

    disks.push(TargetDisk {
      id: DiskId(device.to_string()),
      name,
      path,
      size,
      bus,
      system,
      description: info.protocol,
    });
  }

  Ok(disks)
}

struct DiskInfo {
  media_name: String,
  protocol: String,
  internal: bool,
  removable: bool,
}

fn diskutil_info(device: &str) -> DiskInfo {
  let output = Command::new("diskutil")
    .args(["info", "-plist", device])
    .output();
  let Ok(output) = output else {
    return DiskInfo {
      media_name: String::new(),
      protocol: String::new(),
      internal: true,
      removable: false,
    };
  };
  let value: Value =
    plist::from_bytes(&output.stdout).unwrap_or(Value::Dictionary(Default::default()));
  let dict = value.as_dictionary();
  let str_field = |key: &str| {
    dict
      .and_then(|d| d.get(key))
      .and_then(|v| v.as_string())
      .unwrap_or("")
      .to_string()
  };
  let bool_field = |key: &str| {
    dict
      .and_then(|d| d.get(key))
      .and_then(|v| v.as_boolean())
      .unwrap_or(false)
  };
  DiskInfo {
    media_name: str_field("MediaName"),
    protocol: str_field("BusProtocol"),
    internal: bool_field("Internal"),
    removable: bool_field("Removable") || bool_field("RemovableMedia"),
  }
}
