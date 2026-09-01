use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use imprint_core::{Compression, Error, ImageKind, ImageRef, Result};

use crate::{MAGIC_ZSTD, compression_from_name, kind_from_name, sniff_compression, sniff_kind};

/// Open `path`, sniff magic + extension, and fill `ImageRef`.
pub fn inspect(path: impl AsRef<Path>) -> Result<ImageRef> {
  let path = path.as_ref();
  if !path.is_file() {
    return Err(Error::ImageNotFound(path.to_path_buf()));
  }

  let mut image = ImageRef::from_path(path);
  let meta = std::fs::metadata(path)?;
  image.file_size = meta.len();
  image.kind = kind_from_name(&image.display_name);
  image.compression = compression_from_name(&image.display_name);

  let mut file = File::open(path)?;
  let mut header = [0u8; 64 * 1024];
  let n = file.read(&mut header)?;
  let header = &header[..n];

  if image.compression.is_none() {
    image.compression = sniff_compression(header);
  }

  if image.kind == ImageKind::Unknown && image.compression.is_none() {
    image.kind = sniff_kind(header);
    if image.kind == ImageKind::Unknown {
      // ISO 9660 primary volume descriptor sits at 32 KiB + 1.
      if n > 0x8006 && &header[0x8001..0x8006] == b"CD001" {
        image.kind = ImageKind::Iso;
      } else {
        image.kind = ImageKind::Img;
      }
    }
  }

  image.payload_size = estimate_payload(&mut file, &image, header)?;
  Ok(image)
}

fn estimate_payload(file: &mut File, image: &ImageRef, header: &[u8]) -> Result<u64> {
  match image.compression {
    None => Ok(image.file_size),
    Some(Compression::Gzip) => Ok(gzip_uncompressed_size(file, image.file_size).unwrap_or(0)),
    Some(Compression::Xz) => Ok(xz_uncompressed_size(file, image.file_size).unwrap_or(0)),
    Some(Compression::Zstd) => Ok(zstd_uncompressed_size(header).unwrap_or(0)),
    Some(Compression::Zip) => Ok(zip_uncompressed_size(&image.path).unwrap_or(0)),
    Some(Compression::Bzip2) => Ok(0),
  }
}

/// gzip trailer ISIZE is uncompressed length modulo 2^32.
fn gzip_uncompressed_size(file: &mut File, file_size: u64) -> Option<u64> {
  if file_size < 18 {
    return None;
  }
  file.seek(SeekFrom::End(-4)).ok()?;
  let mut buf = [0u8; 4];
  file.read_exact(&mut buf).ok()?;
  let isize = u32::from_le_bytes(buf) as u64;
  (isize > 0).then_some(isize)
}

/// Frame_Content_Size from the first zstd frame header, when present.
fn zstd_uncompressed_size(header: &[u8]) -> Option<u64> {
  if header.len() < 6 || !header.starts_with(&MAGIC_ZSTD) {
    return None;
  }
  let desc = header[4];
  let fcs_flag = desc >> 6;
  let single_segment = desc & 0b0010_0000 != 0;
  let dict_flag = desc & 0b11;
  let mut pos = 5usize;
  if !single_segment {
    pos += 1;
  }
  pos += match dict_flag {
    0 => 0,
    1 => 1,
    2 => 2,
    3 => 4,
    _ => return None,
  };
  let fcs_bytes = match fcs_flag {
    0 if single_segment => 1,
    0 => return None,
    1 => 2,
    2 => 4,
    3 => 8,
    _ => return None,
  };
  let raw = header.get(pos..pos + fcs_bytes)?;
  let size = match fcs_bytes {
    1 => raw[0] as u64,
    2 => u16::from_le_bytes(raw.try_into().ok()?) as u64 + 256,
    4 => u32::from_le_bytes(raw.try_into().ok()?) as u64,
    8 => u64::from_le_bytes(raw.try_into().ok()?),
    _ => return None,
  };
  (size > 0).then_some(size)
}

/// Sum uncompressed sizes from the xz stream Index (footer at EOF).
fn xz_uncompressed_size(file: &mut File, file_size: u64) -> Option<u64> {
  if file_size < 32 {
    return None;
  }
  file.seek(SeekFrom::End(-12)).ok()?;
  let mut footer = [0u8; 12];
  file.read_exact(&mut footer).ok()?;
  if footer[10] != b'Y' || footer[11] != b'Z' {
    return None;
  }
  let backward = u32::from_le_bytes(footer[4..8].try_into().ok()?);
  let index_size = (backward as u64 + 1).checked_mul(4)?;
  if index_size < 8 || 12 + index_size + 12 > file_size {
    return None;
  }
  file.seek(SeekFrom::End(-12 - index_size as i64)).ok()?;
  let mut index = vec![0u8; index_size as usize];
  file.read_exact(&mut index).ok()?;
  if index.first() != Some(&0) {
    return None;
  }
  let mut pos = 1usize;
  let records = xz_varint(&index, &mut pos)?;
  let mut total = 0u64;
  for _ in 0..records {
    let _unpadded = xz_varint(&index, &mut pos)?;
    total = total.saturating_add(xz_varint(&index, &mut pos)?);
  }
  (total > 0).then_some(total)
}

fn xz_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
  let mut result = 0u64;
  let mut shift = 0;
  for _ in 0..9 {
    let b = *data.get(*pos)?;
    *pos += 1;
    result |= (u64::from(b) & 0x7f) << shift;
    if b & 0x80 == 0 {
      return Some(result);
    }
    shift += 7;
  }
  None
}

fn zip_uncompressed_size(path: &Path) -> Option<u64> {
  let file = File::open(path).ok()?;
  let mut archive = zip::ZipArchive::new(file).ok()?;
  let mut best = 0u64;
  let mut best_score = 0u64;
  for i in 0..archive.len() {
    let entry = archive.by_index(i).ok()?;
    if entry.is_dir() {
      continue;
    }
    let name = entry.name().to_ascii_lowercase();
    let size = entry.size();
    let score = if name.ends_with(".iso") || name.ends_with(".img") || name.ends_with(".raw") {
      size.saturating_add(1)
    } else {
      size
    };
    if score >= best_score {
      best_score = score;
      best = size;
    }
  }
  (best > 0).then_some(best)
}
