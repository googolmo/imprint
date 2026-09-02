//! Raspberry Pi Imager catalog, image download, and first-boot customisation.
//!
//! No GPUI and no block-device IO. The flash crate writes the generated boot
//! files onto the first FAT partition after imaging.

mod catalog;
mod config;
mod download;

pub use catalog::{
  Catalog, Device, InitFormat, MatchingType, OFFICIAL_REPO_URL, OsItem, default_device_index,
  fetch_catalog, fetch_subitems, filter_items, os_matches_device,
};
pub use config::{PiCustomization, generate_boot};
pub use download::{cached_path, download_image, image_cache_dir};
