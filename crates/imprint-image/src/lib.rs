//! Inspect image files and produce a sequential payload reader.

mod inspect;
mod reader;

pub use inspect::inspect;
pub use reader::open_payload;

use imprint_core::{Compression, ImageKind};

const MAGIC_ISO: &[u8] = b"CD001";
const MAGIC_GZIP: [u8; 2] = [0x1f, 0x8b];
const MAGIC_BZIP: [u8; 3] = *b"BZh";
const MAGIC_XZ: [u8; 6] = [0xfd, b'7', b'z', b'X', b'Z', 0x00];
const MAGIC_ZSTD: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const MAGIC_ZIP: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
const MAGIC_DMG: [u8; 4] = *b"koly";

pub fn kind_from_name(name: &str) -> ImageKind {
  let lower = name.to_ascii_lowercase();
  let stem = strip_compression_suffix(&lower);
  if stem.ends_with(".iso") {
    ImageKind::Iso
  } else if stem.ends_with(".img") || stem.ends_with(".raw") || stem.ends_with(".bin") {
    ImageKind::Img
  } else if stem.ends_with(".dmg") {
    ImageKind::Dmg
  } else {
    ImageKind::Unknown
  }
}

pub fn compression_from_name(name: &str) -> Option<Compression> {
  let lower = name.to_ascii_lowercase();
  if lower.ends_with(".gz") || lower.ends_with(".gzip") {
    Some(Compression::Gzip)
  } else if lower.ends_with(".bz2") || lower.ends_with(".bzip2") {
    Some(Compression::Bzip2)
  } else if lower.ends_with(".xz") {
    Some(Compression::Xz)
  } else if lower.ends_with(".zst") || lower.ends_with(".zstd") {
    Some(Compression::Zstd)
  } else if lower.ends_with(".zip") {
    Some(Compression::Zip)
  } else {
    None
  }
}

fn strip_compression_suffix(name: &str) -> &str {
  for suffix in [
    ".gz", ".gzip", ".bz2", ".bzip2", ".xz", ".zst", ".zstd", ".zip",
  ] {
    if let Some(stripped) = name.strip_suffix(suffix) {
      return stripped;
    }
  }
  name
}

pub fn sniff_compression(header: &[u8]) -> Option<Compression> {
  if header.starts_with(&MAGIC_GZIP) {
    Some(Compression::Gzip)
  } else if header.starts_with(&MAGIC_BZIP) {
    Some(Compression::Bzip2)
  } else if header.starts_with(&MAGIC_XZ) {
    Some(Compression::Xz)
  } else if header.starts_with(&MAGIC_ZSTD) {
    Some(Compression::Zstd)
  } else if header.starts_with(&MAGIC_ZIP) {
    Some(Compression::Zip)
  } else {
    None
  }
}

pub fn sniff_kind(header: &[u8]) -> ImageKind {
  if header.len() >= 0x8006 && &header[0x8001..0x8006] == MAGIC_ISO {
    return ImageKind::Iso;
  }
  if header.len() >= 4
    && header[header.len().saturating_sub(512).min(header.len())..]
      .windows(4)
      .any(|w| w == MAGIC_DMG)
  {
    return ImageKind::Dmg;
  }
  if header.windows(5).any(|w| w == MAGIC_ISO) {
    ImageKind::Iso
  } else {
    ImageKind::Unknown
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn names() {
    assert_eq!(kind_from_name("ubuntu-24.04.iso"), ImageKind::Iso);
    assert_eq!(kind_from_name("raspios.img.xz"), ImageKind::Img);
    assert_eq!(
      compression_from_name("disk.img.gz"),
      Some(Compression::Gzip)
    );
    assert_eq!(compression_from_name("os.iso"), None);
  }

  #[test]
  fn gzip_magic() {
    assert_eq!(
      sniff_compression(&[0x1f, 0x8b, 0x08]),
      Some(Compression::Gzip)
    );
  }

  #[test]
  fn gzip_payload_size_uses_isize_not_file_size() {
    use std::io::Write;

    use flate2::Compression as GzLevel;
    use flate2::write::GzEncoder;

    let payload = vec![0u8; 12_345];
    let path = std::env::temp_dir().join(format!(
      "imprint-gzip-isize-{}-{}.img.gz",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
    ));
    {
      let file = std::fs::File::create(&path).expect("temp gzip");
      let mut encoder = GzEncoder::new(file, GzLevel::default());
      encoder.write_all(&payload).expect("gzip write");
      encoder.finish().expect("gzip finish");
    }
    let image = inspect::inspect(&path).expect("inspect gzip");
    let _ = std::fs::remove_file(&path);
    assert_eq!(image.compression, Some(Compression::Gzip));
    assert_eq!(image.payload_size, payload.len() as u64);
    assert_eq!(image.write_size(), payload.len() as u64);
    assert!(image.file_size < image.payload_size);
  }
}
