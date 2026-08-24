use std::fs;
use std::path::{Path, PathBuf};

use imprint_core::{BusKind, DiskId, Result, TargetDisk};

pub fn list() -> Result<Vec<TargetDisk>> {
  let mut disks = Vec::new();
  let root_dev = root_device();
  let Ok(entries) = fs::read_dir("/sys/block") else {
    return Ok(disks);
  };

  for entry in entries.flatten() {
    let name = entry.file_name();
    let name = name.to_string_lossy();
    if name.starts_with("loop")
      || name.starts_with("ram")
      || name.starts_with("zram")
      || name.starts_with("dm-")
      || name.starts_with("md")
    {
      continue;
    }

    let sys = PathBuf::from("/sys/block").join(name.as_ref());
    let size = read_u64(&sys.join("size")).unwrap_or(0) * 512;
    if size == 0 {
      continue;
    }

    let removable = read_u64(&sys.join("removable")).unwrap_or(0) == 1;
    let model = read_trim(&sys.join("device/model")).unwrap_or_else(|| name.to_string());
    let vendor = read_trim(&sys.join("device/vendor")).unwrap_or_default();
    let bus = classify(&sys, &name, removable);
    let path = PathBuf::from("/dev").join(name.as_ref());
    let system = is_system(&name, &root_dev, removable);

    let description = [vendor.as_str(), model.as_str()]
      .iter()
      .filter(|s| !s.is_empty())
      .map(|s| s.trim())
      .collect::<Vec<_>>()
      .join(" ");

    disks.push(TargetDisk {
      id: DiskId(name.to_string()),
      name: if description.is_empty() {
        name.to_string()
      } else {
        description.clone()
      },
      path,
      size,
      bus,
      system,
      description,
    });
  }

  Ok(disks)
}

fn classify(sys: &Path, name: &str, removable: bool) -> BusKind {
  let uevent = read_trim(&sys.join("device/uevent")).unwrap_or_default();
  let lower = format!("{name} {uevent}").to_ascii_lowercase();
  if lower.contains("usb") {
    BusKind::Usb
  } else if lower.contains("mmc") || name.starts_with("mmcblk") {
    BusKind::Sd
  } else if name.starts_with("nvme") {
    BusKind::Nvme
  } else if removable {
    BusKind::Usb
  } else {
    BusKind::Sata
  }
}

fn is_system(name: &str, root_dev: &Option<String>, removable: bool) -> bool {
  if let Some(root) = root_dev {
    if root == name || root.starts_with(name) {
      return true;
    }
  }
  !removable && !name.starts_with("mmcblk")
}

fn root_device() -> Option<String> {
  let mounts = fs::read_to_string("/proc/self/mountinfo").ok()?;
  for line in mounts.lines() {
    let parts: Vec<&str> = line.split(' ').collect();
    // mountinfo: ... mount_point ... - fstype source
    if let Some(mount_point) = parts.get(4)
      && *mount_point == "/"
    {
      let source = parts.last()?.trim();
      return device_basename(source);
    }
  }
  let mounts = fs::read_to_string("/proc/mounts").ok()?;
  for line in mounts.lines() {
    let mut parts = line.split_whitespace();
    let source = parts.next()?;
    let dest = parts.next()?;
    if dest == "/" {
      return device_basename(source);
    }
  }
  None
}

fn device_basename(source: &str) -> Option<String> {
  let path = Path::new(source);
  let name = path.file_name()?.to_string_lossy().to_string();
  Some(
    name
      .trim_end_matches(|c: char| c.is_ascii_digit())
      .trim_end_matches('p')
      .to_string(),
  )
}

fn read_trim(path: &Path) -> Option<String> {
  fs::read_to_string(path)
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

fn read_u64(path: &Path) -> Option<u64> {
  read_trim(path)?.parse().ok()
}
