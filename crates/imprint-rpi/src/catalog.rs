use std::path::{Path, PathBuf};

use serde::Deserialize;

use imprint_core::{Error, Result};

/// Official Raspberry Pi Imager v4 repository.
pub const OFFICIAL_REPO_URL: &str =
  "https://downloads.raspberrypi.com/os_list_imagingutility_v4.json";

const USER_AGENT: &str = concat!("Imprint/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Catalog {
  #[serde(default)]
  pub imager: ImagerMeta,
  #[serde(default)]
  pub os_list: Vec<OsItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ImagerMeta {
  #[serde(default)]
  pub devices: Vec<Device>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
  pub name: String,
  #[serde(default)]
  pub tags: Vec<String>,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub default: bool,
  #[serde(default)]
  pub matching_type: MatchingType,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchingType {
  #[default]
  Inclusive,
  Exclusive,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsItem {
  pub name: String,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub icon: String,
  #[serde(default)]
  pub url: Option<String>,
  #[serde(default)]
  pub extract_size: u64,
  #[serde(default)]
  pub extract_sha256: Option<String>,
  #[serde(default)]
  pub image_download_size: u64,
  #[serde(default)]
  pub image_download_sha256: Option<String>,
  #[serde(default)]
  pub release_date: String,
  #[serde(default)]
  pub init_format: Option<String>,
  #[serde(default)]
  pub devices: Vec<String>,
  #[serde(default)]
  pub capabilities: Vec<String>,
  #[serde(default)]
  pub subitems: Vec<OsItem>,
  #[serde(default)]
  pub subitems_url: Option<String>,
}

impl OsItem {
  pub fn is_image(&self) -> bool {
    self.url.is_some()
  }

  pub fn is_category(&self) -> bool {
    !self.subitems.is_empty() || self.subitems_url.is_some()
  }

  pub fn is_local(&self) -> bool {
    match self.url.as_deref() {
      Some(url) => {
        let trimmed = url.trim();
        !trimmed.starts_with("http://") && !trimmed.starts_with("https://")
      }
      None => false,
    }
  }

  pub fn local_path(&self) -> Option<PathBuf> {
    if self.is_local() {
      self.url.as_ref().map(PathBuf::from)
    } else {
      None
    }
  }

  pub fn from_local_path(path: &Path) -> Self {
    let name = path
      .file_name()
      .and_then(|n| n.to_str())
      .unwrap_or("Custom OS")
      .to_string();
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Self {
      name,
      description: path.display().to_string(),
      icon: String::new(),
      url: Some(path.to_string_lossy().into_owned()),
      extract_size: size,
      extract_sha256: None,
      image_download_size: size,
      image_download_sha256: None,
      release_date: String::new(),
      init_format: Some(InitFormat::CloudInitRpi.as_str().into()),
      devices: Vec::new(),
      capabilities: Vec::new(),
      subitems: Vec::new(),
      subitems_url: None,
    }
  }

  pub fn init_format(&self) -> InitFormat {
    InitFormat::parse(self.init_format.as_deref())
  }

  pub fn set_init_format(&mut self, format: InitFormat) {
    self.init_format = Some(format.as_str().into());
  }

  pub fn write_size(&self) -> u64 {
    self.extract_size
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitFormat {
  None,
  Systemd,
  CloudInit,
  CloudInitRpi,
}

impl InitFormat {
  pub const ALL: [Self; 4] = [
    Self::None,
    Self::Systemd,
    Self::CloudInit,
    Self::CloudInitRpi,
  ];

  pub fn parse(raw: Option<&str>) -> Self {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
      "systemd" => Self::Systemd,
      "cloudinit" | "cloud-init" => Self::CloudInit,
      "cloudinit-rpi" | "cloud-init-rpi" => Self::CloudInitRpi,
      "none" => Self::None,
      _ => Self::None,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::None => "none",
      Self::Systemd => "systemd",
      Self::CloudInit => "cloudinit",
      Self::CloudInitRpi => "cloudinit-rpi",
    }
  }

  pub fn supports_customisation(self) -> bool {
    !matches!(self, Self::None)
  }
}

pub fn fetch_catalog() -> Result<Catalog> {
  fetch_url(OFFICIAL_REPO_URL)
}

pub fn fetch_subitems(item: &mut OsItem) -> Result<()> {
  let Some(url) = item.subitems_url.clone() else {
    return Ok(());
  };
  if !item.subitems.is_empty() {
    return Ok(());
  }
  let nested = fetch_url(&url)?;
  item.subitems = nested.os_list;
  Ok(())
}

pub fn default_device_index(devices: &[Device]) -> Option<usize> {
  devices
    .iter()
    .position(|d| d.default)
    .or_else(|| devices.iter().position(|d| !d.tags.is_empty()))
}

pub fn os_matches_device(os: &OsItem, device: &Device) -> bool {
  if device.tags.is_empty() {
    return true;
  }
  if os.devices.is_empty() {
    return device.matching_type != MatchingType::Exclusive;
  }
  os.devices
    .iter()
    .any(|tag| device.tags.iter().any(|t| t == tag))
}

pub fn filter_items<'a>(items: &'a [OsItem], device: Option<&Device>) -> Vec<(usize, &'a OsItem)> {
  items
    .iter()
    .enumerate()
    .filter(|(_, item)| item_visible(item, device))
    .collect()
}

fn item_visible(item: &OsItem, device: Option<&Device>) -> bool {
  if item.is_category() {
    if item.subitems.is_empty() && item.subitems_url.is_some() {
      return true;
    }
    return item
      .subitems
      .iter()
      .any(|child| item_visible(child, device));
  }
  match device {
    None => true,
    Some(device) => os_matches_device(item, device),
  }
}

fn fetch_url(url: &str) -> Result<Catalog> {
  let agent = ureq::AgentBuilder::new()
    .timeout_connect(std::time::Duration::from_secs(20))
    .timeout_read(std::time::Duration::from_secs(60))
    .user_agent(USER_AGENT)
    .build();
  let response = agent
    .get(url)
    .call()
    .map_err(|err| Error::Catalog(err.to_string()))?;
  let text = response
    .into_string()
    .map_err(|err| Error::Catalog(err.to_string()))?;
  serde_json::from_str(&text).map_err(|err| Error::Catalog(err.to_string()))
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = r#"{
    "imager": {
      "devices": [
        {
          "name": "Raspberry Pi 5",
          "tags": ["pi5-64bit"],
          "matching_type": "exclusive"
        },
        {
          "name": "No filtering",
          "tags": [],
          "matching_type": "inclusive"
        }
      ]
    },
    "os_list": [
      {
        "name": "Raspberry Pi OS (64-bit)",
        "description": "Recommended",
        "url": "https://example.com/rpi.img.xz",
        "extract_size": 1000,
        "image_download_size": 400,
        "release_date": "2026-06-18",
        "init_format": "cloudinit-rpi",
        "devices": ["pi5-64bit", "pi4-64bit"]
      },
      {
        "name": "Other",
        "description": "More images",
        "subitems": [
          {
            "name": "Lite",
            "url": "https://example.com/lite.img.xz",
            "extract_size": 500,
            "init_format": "systemd",
            "devices": ["pi4-32bit"]
          }
        ]
      }
    ]
  }"#;

  #[test]
  fn parses_v4_shape() {
    let catalog: Catalog = serde_json::from_str(SAMPLE).unwrap();
    assert_eq!(catalog.imager.devices.len(), 2);
    assert_eq!(catalog.os_list[0].name, "Raspberry Pi OS (64-bit)");
    assert_eq!(catalog.os_list[0].init_format(), InitFormat::CloudInitRpi);
    assert!(catalog.os_list[1].is_category());
    assert!(catalog.os_list[1].subitems[0].is_image());
  }

  #[test]
  fn exclusive_device_hides_untagged_and_other_tags() {
    let catalog: Catalog = serde_json::from_str(SAMPLE).unwrap();
    let pi5 = &catalog.imager.devices[0];
    let visible = filter_items(&catalog.os_list, Some(pi5));
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].1.name, "Raspberry Pi OS (64-bit)");
    assert!(os_matches_device(&catalog.os_list[0], pi5));
    assert!(!os_matches_device(&catalog.os_list[1].subitems[0], pi5));
  }

  #[test]
  fn no_filtering_shows_everything() {
    let catalog: Catalog = serde_json::from_str(SAMPLE).unwrap();
    let all = &catalog.imager.devices[1];
    let visible = filter_items(&catalog.os_list, Some(all));
    assert_eq!(visible.len(), 2);
  }

  #[test]
  fn init_format_aliases() {
    assert_eq!(InitFormat::parse(Some("cloud-init")), InitFormat::CloudInit);
    assert_eq!(
      InitFormat::parse(Some("cloudinit-rpi")),
      InitFormat::CloudInitRpi
    );
    assert_eq!(InitFormat::parse(Some("none")), InitFormat::None);
    assert!(!InitFormat::None.supports_customisation());
    assert!(InitFormat::Systemd.supports_customisation());
  }

  #[test]
  fn local_custom_os() {
    let item = OsItem::from_local_path(Path::new("/tmp/my-image.img.xz"));
    assert!(item.is_local());
    assert!(item.is_image());
    assert!(!item.is_category());
    assert_eq!(item.name, "my-image.img.xz");
    assert_eq!(
      item.local_path().as_deref(),
      Some(Path::new("/tmp/my-image.img.xz"))
    );
    assert_eq!(item.init_format(), InitFormat::CloudInitRpi);

    let remote = OsItem {
      name: "Remote".into(),
      description: String::new(),
      icon: String::new(),
      url: Some("https://example.com/a.img.xz".into()),
      extract_size: 0,
      extract_sha256: None,
      image_download_size: 0,
      image_download_sha256: None,
      release_date: String::new(),
      init_format: None,
      devices: Vec::new(),
      capabilities: Vec::new(),
      subitems: Vec::new(),
      subitems_url: None,
    };
    assert!(!remote.is_local());
    assert!(remote.local_path().is_none());
  }
}
