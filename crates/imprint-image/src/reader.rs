use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use imprint_core::{Compression, Error, ImageRef, Result};
use liblzma::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

/// Sequential reader over the uncompressed image payload.
pub fn open_payload(image: &ImageRef) -> Result<Box<dyn Read + Send>> {
  open_path(&image.path, image.compression)
}

pub fn open_path(path: &Path, compression: Option<Compression>) -> Result<Box<dyn Read + Send>> {
  let file = File::open(path)?;
  let buffered = BufReader::with_capacity(1024 * 1024, file);
  match compression {
    None => Ok(Box::new(buffered)),
    Some(Compression::Gzip) => Ok(Box::new(GzDecoder::new(buffered))),
    Some(Compression::Bzip2) => Ok(Box::new(BzDecoder::new(buffered))),
    Some(Compression::Xz) => Ok(Box::new(XzDecoder::new(buffered))),
    Some(Compression::Zstd) => {
      let decoder = ZstdDecoder::new(buffered).map_err(|e| Error::msg(e.to_string()))?;
      Ok(Box::new(decoder))
    }
    Some(Compression::Zip) => {
      let mut archive = ZipArchive::new(buffered).map_err(|e| Error::msg(e.to_string()))?;
      let mut best = 0;
      let mut best_size = 0u64;
      for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| Error::msg(e.to_string()))?;
        if entry.is_dir() {
          continue;
        }
        let name = entry.name().to_ascii_lowercase();
        let size = entry.size();
        let score = if name.ends_with(".iso") || name.ends_with(".img") {
          size + 1
        } else {
          size
        };
        if score >= best_size {
          best_size = score;
          best = i;
        }
      }
      // ZipArchive requires re-opening the file for a Send reader.
      drop(archive);
      let file = File::open(path)?;
      let mut archive =
        ZipArchive::new(BufReader::new(file)).map_err(|e| Error::msg(e.to_string()))?;
      let entry = archive
        .by_index(best)
        .map_err(|e| Error::msg(e.to_string()))?;
      let mut data = Vec::new();
      let mut entry = entry;
      entry
        .read_to_end(&mut data)
        .map_err(|e| Error::msg(e.to_string()))?;
      Ok(Box::new(std::io::Cursor::new(data)))
    }
  }
}
