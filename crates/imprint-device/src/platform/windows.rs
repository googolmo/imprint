use std::path::PathBuf;
use std::process::Command;

use imprint_core::{BusKind, DiskId, Result, TargetDisk};

pub fn list() -> Result<Vec<TargetDisk>> {
  let script = r#"
Get-CimInstance Win32_DiskDrive | ForEach-Object {
  $iface = $_.InterfaceType
  $media = $_.MediaType
  $sys = if ($iface -eq 'IDE' -or $iface -eq 'SCSI' -or $media -match 'Fixed') { '1' } else { '0' }
  if ($iface -eq 'USB' -or $media -match 'Removable') { $sys = '0' }
  '{0}|{1}|{2}|{3}|{4}' -f $_.Index, $_.Model, $_.Size, $iface, $sys
}
"#;
  let output = Command::new("powershell")
    .args(["-NoProfile", "-Command", script])
    .output()?;
  if !output.status.success() {
    return Ok(Vec::new());
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  let mut disks = Vec::new();
  for line in stdout.lines() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 5 {
      continue;
    }
    let index = parts[0].trim();
    let model = parts[1].trim();
    let size: u64 = parts[2].trim().parse().unwrap_or(0);
    let iface = parts[3].trim();
    let system = parts[4].trim() == "1";
    if size == 0 {
      continue;
    }
    let bus = match iface.to_ascii_uppercase().as_str() {
      "USB" => BusKind::Usb,
      "SD" | "MMC" => BusKind::Sd,
      "SCSI" | "IDE" => BusKind::Sata,
      "NVME" => BusKind::Nvme,
      _ => BusKind::Unknown,
    };
    disks.push(TargetDisk {
      id: DiskId(format!("PhysicalDrive{index}")),
      name: model.to_string(),
      path: PathBuf::from(format!(r"\\.\PhysicalDrive{index}")),
      size,
      bus,
      system,
      description: iface.to_string(),
    });
  }
  Ok(disks)
}
