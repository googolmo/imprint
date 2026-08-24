use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use imprint_core::{Error, ImageKind, ImageRef, Result};

use crate::{compression_from_name, kind_from_name, sniff_compression, sniff_kind};

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

  image.payload_size = estimate_payload(&mut file, &image)?;
  Ok(image)
}

fn estimate_payload(file: &mut File, image: &ImageRef) -> Result<u64> {
  if image.compression.is_none() {
    return Ok(image.file_size);
  }
  // Compressed payload size is unknown without a full scan. Prefer the file
  // size as a lower bound so the UI can still show something.
  let _ = file.seek(SeekFrom::Start(0));
  Ok(0)
}
