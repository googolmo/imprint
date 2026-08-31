use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How the on-disk file is compressed, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compression {
  Gzip,
  Bzip2,
  Xz,
  Zstd,
  Zip,
}

impl Compression {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Gzip => "gzip",
      Self::Bzip2 => "bzip2",
      Self::Xz => "xz",
      Self::Zstd => "zstd",
      Self::Zip => "zip",
    }
  }
}

/// Kind of payload after decompression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageKind {
  Iso,
  Img,
  Dmg,
  Unknown,
}

impl ImageKind {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Iso => "ISO",
      Self::Img => "IMG",
      Self::Dmg => "DMG",
      Self::Unknown => "image",
    }
  }
}

/// A selected source image. `payload_size` is the uncompressed stream length
/// that will be written to the target (best-effort; `0` means unknown).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
  pub path: PathBuf,
  pub display_name: String,
  pub kind: ImageKind,
  pub compression: Option<Compression>,
  pub file_size: u64,
  pub payload_size: u64,
}

impl ImageRef {
  pub fn from_path(path: impl AsRef<Path>) -> Self {
    let path = path.as_ref().to_path_buf();
    let display_name = path
      .file_name()
      .map(|n| n.to_string_lossy().into_owned())
      .unwrap_or_else(|| path.display().to_string());
    Self {
      path,
      display_name,
      kind: ImageKind::Unknown,
      compression: None,
      file_size: 0,
      payload_size: 0,
    }
  }

  /// Bytes that will land on the target. For compressed images this is the
  /// uncompressed payload, not the `.gz` / `.xz` file length. `0` means unknown.
  pub fn write_size(&self) -> u64 {
    if self.payload_size > 0 {
      self.payload_size
    } else if self.compression.is_none() {
      self.file_size
    } else {
      0
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compressed_file_size_is_not_the_write_size() {
    let image = ImageRef {
      path: PathBuf::from("disk.img.gz"),
      display_name: "disk.img.gz".into(),
      kind: ImageKind::Img,
      compression: Some(Compression::Gzip),
      file_size: 200 * 1024 * 1024,
      payload_size: 0,
    };
    assert_eq!(image.write_size(), 0);

    let known = ImageRef {
      payload_size: 1200 * 1024 * 1024,
      ..image.clone()
    };
    assert_eq!(known.write_size(), 1200 * 1024 * 1024);
  }
}
