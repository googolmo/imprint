//! Shared domain types for Imprint.
//!
//! UI, CLI, and the flash pipeline all speak these structs. Keep this crate
//! free of GPUI and OS-specific IO.

mod error;
mod flash;
pub mod i18n;
mod image;
mod settings;
mod target;

pub use error::{Error, Result};
pub use flash::{BootCustomization, BootFile, FlashPhase, FlashProgress, FlashRequest};
pub use i18n::{Language, LocalePref};
pub use image::{Compression, ImageKind, ImageRef};
pub use settings::Settings;
pub use target::{BusKind, DiskId, TargetDisk};

/// Human-readable byte counts used in both the GUI and CLI.
pub fn format_bytes(bytes: u64) -> String {
  bytesize::ByteSize::b(bytes).to_string()
}
