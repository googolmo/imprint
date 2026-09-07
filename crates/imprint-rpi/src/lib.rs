//! Raspberry Pi Imager catalog, image download, and first-boot customisation.
//!
//! No GPUI and no block-device IO. The flash crate writes the generated boot
//! files onto the first FAT partition after imaging.

mod catalog;
mod config;
mod download;

pub use catalog::{
  Catalog, Device, InitFormat, MatchingType, OFFICIAL_REPO_URL, OsItem, bundled_catalog,
  catalog_cache_path, default_device_index, fetch_catalog, fetch_subitems, filter_items,
  load_local_catalog, matching_device_index, offline_catalog, os_matches_device, os_path_valid,
  save_local_catalog,
};
pub use config::{PiCustomization, generate_boot};
pub use download::{cached_path, download_image, image_cache_dir};
