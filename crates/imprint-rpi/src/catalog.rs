use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use imprint_core::{Error, Result};

/// Official Raspberry Pi Imager v4 repository.
pub const OFFICIAL_REPO_URL: &str =
  "https://downloads.raspberrypi.com/os_list_imagingutility_v4.json";

const USER_AGENT: &str = concat!("Imprint/", env!("CARGO_PKG_VERSION"));

/// Snapshot of [`OFFICIAL_REPO_URL`] compiled into the binary so the OS list
/// can render before a network fetch.
const BUNDLED_CATALOG_JSON: &str = include_str!("../data/os_list_imagingutility_v4.json");

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
  let text = fetch_url_text(OFFICIAL_REPO_URL)?;
  let catalog = parse_catalog(&text)?;
  if let Err(err) = save_local_catalog(&text) {
    tracing::warn!(
      error = %err,
      "could not save Raspberry Pi catalog cache"
    );
  }
  Ok(catalog)
}

pub fn fetch_subitems(item: &mut OsItem) -> Result<()> {
  let Some(url) = item.subitems_url.clone() else {
    return Ok(());
  };
  if !item.subitems.is_empty() {
    return Ok(());
  }
  let nested = parse_catalog(&fetch_url_text(&url)?)?;
  item.subitems = nested.os_list;
  Ok(())
}

/// Catalog to show before a network refresh: last downloaded copy, else the
/// snapshot baked into the binary.
pub fn offline_catalog() -> Catalog {
  load_local_catalog().unwrap_or_else(bundled_catalog)
}

pub fn bundled_catalog() -> Catalog {
  parse_catalog(BUNDLED_CATALOG_JSON).expect("bundled Raspberry Pi catalog JSON must parse")
}

pub fn catalog_cache_path() -> PathBuf {
  dirs::cache_dir()
    .unwrap_or_else(std::env::temp_dir)
    .join("imprint")
    .join("rpi")
    .join("os_list_imagingutility_v4.json")
}

pub fn load_local_catalog() -> Option<Catalog> {
  read_catalog_file(&catalog_cache_path())
}

pub fn save_local_catalog(json: &str) -> Result<()> {
  write_catalog_file(&catalog_cache_path(), json)
}

pub fn default_device_index(devices: &[Device]) -> Option<usize> {
  devices
    .iter()
    .position(|d| d.default)
    .or_else(|| devices.iter().position(|d| !d.tags.is_empty()))
}

pub fn os_matches_device(os: &OsItem, device: &Device) -> bool {
  // A user-picked local file is not catalog metadata; keep it on any model.
  if os.is_local() {
    return true;
  }
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

/// Keep the previously chosen device across a catalog refresh.
pub fn matching_device_index(devices: &[Device], previous: Option<&Device>) -> Option<usize> {
  if let Some(prev) = previous {
    if !prev.tags.is_empty()
      && let Some(ix) = devices.iter().position(|d| d.tags == prev.tags)
    {
      return Some(ix);
    }
    if let Some(ix) = devices.iter().position(|d| d.name == prev.name) {
      return Some(ix);
    }
  }
  default_device_index(devices)
}

/// Whether `path` still names nested categories in `list`.
pub fn os_path_valid(list: &[OsItem], path: &[usize]) -> bool {
  let mut current = list;
  for &ix in path {
    let Some(item) = current.get(ix) else {
      return false;
    };
    if !item.is_category() {
      return false;
    }
    current = item.subitems.as_slice();
  }
  true
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

fn parse_catalog(text: &str) -> Result<Catalog> {
  serde_json::from_str(text).map_err(|err| Error::Catalog(err.to_string()))
}

fn read_catalog_file(path: &Path) -> Option<Catalog> {
  let text = fs::read_to_string(path).ok()?;
  parse_catalog(&text).ok()
}

fn write_catalog_file(path: &Path, json: &str) -> Result<()> {
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir)?;
  }
  let tmp = path.with_extension("json.part");
  fs::write(&tmp, json)?;
  if path.exists() {
    fs::remove_file(path)?;
  }
  fs::rename(&tmp, path)?;
  Ok(())
}

fn fetch_url_text(url: &str) -> Result<String> {
  let agent = ureq::AgentBuilder::new()
    .timeout_connect(std::time::Duration::from_secs(20))
    .timeout_read(std::time::Duration::from_secs(60))
    .user_agent(USER_AGENT)
    .build();
  let response = agent
    .get(url)
    .call()
    .map_err(|err| Error::Catalog(err.to_string()))?;
  response
    .into_string()
    .map_err(|err| Error::Catalog(err.to_string()))
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
    let catalog: Catalog = serde_json::from_str(SAMPLE).unwrap();
    assert!(os_matches_device(&item, &catalog.imager.devices[0]));

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

  #[test]
  fn bundled_catalog_parses() {
    let catalog = bundled_catalog();
    assert!(!catalog.imager.devices.is_empty());
    assert!(!catalog.os_list.is_empty());
    assert!(
      catalog
        .os_list
        .iter()
        .any(|item| item.name.contains("Raspberry Pi OS"))
    );
    assert!(default_device_index(&catalog.imager.devices).is_some());
  }

  #[test]
  fn catalog_file_roundtrip() {
    let dir = std::env::temp_dir().join(format!(
      "imprint-rpi-catalog-{}",
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("os_list_imagingutility_v4.json");
    write_catalog_file(&path, SAMPLE).unwrap();
    let loaded = read_catalog_file(&path).unwrap();
    assert_eq!(loaded.os_list[0].name, "Raspberry Pi OS (64-bit)");
    assert_eq!(loaded.imager.devices.len(), 2);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&dir);
  }

  #[test]
  fn corrupt_catalog_file_is_ignored() {
    let dir = std::env::temp_dir().join(format!(
      "imprint-rpi-catalog-bad-{}",
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("os_list_imagingutility_v4.json");
    fs::write(&path, "not-json").unwrap();
    assert!(read_catalog_file(&path).is_none());
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&dir);
  }

  #[test]
  fn matching_device_index_prefers_tags() {
    let catalog: Catalog = serde_json::from_str(SAMPLE).unwrap();
    let pi5 = catalog.imager.devices[0].clone();
    assert_eq!(
      matching_device_index(&catalog.imager.devices, Some(&pi5)),
      Some(0)
    );
    let renamed = Device {
      name: "Renamed Pi 5".into(),
      tags: pi5.tags.clone(),
      description: String::new(),
      default: false,
      matching_type: MatchingType::Exclusive,
    };
    assert_eq!(
      matching_device_index(&catalog.imager.devices, Some(&renamed)),
      Some(0)
    );
  }

  #[test]
  fn os_path_validates_nested_categories() {
    let catalog: Catalog = serde_json::from_str(SAMPLE).unwrap();
    assert!(os_path_valid(&catalog.os_list, &[]));
    assert!(os_path_valid(&catalog.os_list, &[1]));
    assert!(!os_path_valid(&catalog.os_list, &[0]));
    assert!(!os_path_valid(&catalog.os_list, &[9]));
  }
}
