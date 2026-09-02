use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use imprint_core::{Error, Result};

use crate::catalog::OsItem;

const USER_AGENT: &str = concat!("Imprint/", env!("CARGO_PKG_VERSION"));
const CHUNK: usize = 64 * 1024;

pub fn image_cache_dir() -> PathBuf {
  dirs::cache_dir()
    .unwrap_or_else(std::env::temp_dir)
    .join("imprint")
    .join("rpi")
}

pub fn cache_file_name(url: &str) -> String {
  let without_query = url.split('?').next().unwrap_or(url);
  let last = without_query.split('/').next_back().unwrap_or("image.bin");
  let safe: String = last
    .chars()
    .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    .collect();
  if safe.is_empty() {
    "image.bin".into()
  } else {
    safe
  }
}

pub fn cached_path(os: &OsItem) -> Option<PathBuf> {
  if let Some(path) = os.local_path() {
    return path.is_file().then_some(path);
  }
  let url = os.url.as_deref()?;
  let path = image_cache_dir().join(cache_file_name(url));
  let meta = fs::metadata(&path).ok()?;
  if !meta.is_file() {
    return None;
  }
  if os.image_download_size > 0 && meta.len() != os.image_download_size {
    return None;
  }
  Some(path)
}

pub fn download_image(
  os: &OsItem,
  cancel: &AtomicBool,
  mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf> {
  if let Some(path) = os.local_path() {
    if path.is_file() {
      let len = fs::metadata(&path)?.len();
      on_progress(len, Some(len));
      return Ok(path);
    }
    return Err(Error::Download(format!(
      "custom image not found: {}",
      path.display()
    )));
  }
  let url = os
    .url
    .as_deref()
    .ok_or_else(|| Error::Download("OS has no download URL".into()))?;
  if let Some(path) = cached_path(os) {
    let len = fs::metadata(&path)?.len();
    on_progress(len, Some(len));
    return Ok(path);
  }

  let dir = image_cache_dir();
  fs::create_dir_all(&dir)?;
  let dest = dir.join(cache_file_name(url));
  let part = dest.with_extension(format!(
    "{}.part",
    dest.extension().and_then(|s| s.to_str()).unwrap_or("bin")
  ));

  let agent = ureq::AgentBuilder::new()
    .timeout_connect(std::time::Duration::from_secs(20))
    .timeout_read(std::time::Duration::from_secs(60))
    .user_agent(USER_AGENT)
    .build();
  let response = agent
    .get(url)
    .call()
    .map_err(|err| Error::Download(err.to_string()))?;
  let total = response
    .header("Content-Length")
    .and_then(|s| s.parse().ok())
    .or_else(|| (os.image_download_size > 0).then_some(os.image_download_size));
  let mut reader = response.into_reader();
  let mut file = File::create(&part)?;
  let mut hasher = Sha256::new();
  let mut buf = vec![0u8; CHUNK];
  let mut received = 0u64;
  loop {
    if cancel.load(Ordering::Relaxed) {
      drop(file);
      let _ = fs::remove_file(&part);
      return Err(Error::Cancelled);
    }
    let n = reader
      .read(&mut buf)
      .map_err(|err| Error::Download(err.to_string()))?;
    if n == 0 {
      break;
    }
    file.write_all(&buf[..n])?;
    hasher.update(&buf[..n]);
    received += n as u64;
    on_progress(received, total);
  }
  file.flush()?;
  drop(file);

  if let Some(expected) = os.image_download_sha256.as_deref() {
    let actual = hex_encode(&hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected.trim()) {
      let _ = fs::remove_file(&part);
      return Err(Error::ChecksumMismatch);
    }
  } else if os.image_download_size > 0 && received != os.image_download_size {
    let _ = fs::remove_file(&part);
    return Err(Error::Download(format!(
      "expected {} bytes, got {received}",
      os.image_download_size
    )));
  }

  if dest.exists() {
    fs::remove_file(&dest)?;
  }
  fs::rename(&part, &dest)?;
  Ok(dest)
}

fn hex_encode(bytes: &[u8]) -> String {
  const HEX: &[u8; 16] = b"0123456789abcdef";
  let mut out = String::with_capacity(bytes.len() * 2);
  for &b in bytes {
    out.push(HEX[(b >> 4) as usize] as char);
    out.push(HEX[(b & 0x0f) as usize] as char);
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sanitizes_cache_names() {
    assert_eq!(
      cache_file_name(
        "https://downloads.raspberrypi.com/raspios_arm64/images/foo/2026-06-18-raspios-trixie-arm64.img.xz"
      ),
      "2026-06-18-raspios-trixie-arm64.img.xz"
    );
    assert_eq!(
      cache_file_name("https://example.com/a/b/weird name.img.xz?token=1"),
      "weirdname.img.xz"
    );
  }

  #[test]
  fn cached_path_uses_local_file() {
    let dir = std::env::temp_dir().join("imprint-rpi-local-test");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("custom.img");
    fs::write(&path, b"img").unwrap();
    let os = OsItem::from_local_path(&path);
    assert_eq!(cached_path(&os).as_deref(), Some(path.as_path()));
    let _ = fs::remove_file(&path);
    assert!(cached_path(&os).is_none());
    let _ = fs::remove_dir(&dir);
  }
}
