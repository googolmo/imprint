//! Raw block-device I/O: sector alignment and a flush that works on `/dev/rdisk*`.

use std::fs::File;
use std::io::{self, Read};

use crate::write::BLOCK;

const DEFAULT_SECTOR: usize = 512;

/// Logical sector size the device requires for read/write lengths and offsets.
pub(crate) fn sector_size(file: &File) -> usize {
  query_sector_size(file)
    .filter(|size| size.is_power_of_two() && *size <= BLOCK)
    .unwrap_or(DEFAULT_SECTOR)
}

/// Transfer size: 1 MiB truncated so every full block is sector-aligned.
pub(crate) fn io_chunk(sector: usize) -> usize {
  let sector = sector.max(1);
  if BLOCK % sector == 0 {
    BLOCK
  } else {
    (BLOCK - (BLOCK % sector)).max(sector)
  }
}

/// Round `len` up to a multiple of `sector`. `0` stays `0`.
pub(crate) fn align_up(len: usize, sector: usize) -> usize {
  if sector <= 1 || len == 0 {
    return len;
  }
  let rem = len % sector;
  if rem == 0 { len } else { len + (sector - rem) }
}

/// Bytes to issue to a raw device for `n` payload bytes in a `buf_len` buffer.
/// Full blocks are already sector-sized; only an EOF tail is padded.
pub(crate) fn padded_write_len(n: usize, sector: usize, buf_len: usize) -> usize {
  if n == 0 || n >= buf_len {
    n
  } else {
    align_up(n, sector).min(buf_len)
  }
}

/// Read until `buf` is full or EOF. Short returns are EOF, not a mid-stream hint.
pub(crate) fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
  let mut filled = 0;
  while filled < buf.len() {
    match reader.read(&mut buf[filled..])? {
      0 => break,
      n => filled += n,
    }
  }
  Ok(filled)
}

/// Flush media. Rust's `File::sync_all` uses `F_FULLFSYNC` on macOS, which
/// returns EINVAL on character devices such as `/dev/rdiskN`.
pub(crate) fn sync_device(file: &File) -> io::Result<()> {
  #[cfg(target_os = "macos")]
  {
    if macos_synchronize_cache(file).is_ok() {
      return Ok(());
    }
    return match fsync_fd(file) {
      Ok(()) => Ok(()),
      Err(err) if is_unsupported_sync(&err) => Ok(()),
      Err(err) => Err(err),
    };
  }
  #[cfg(not(target_os = "macos"))]
  match file.sync_all() {
    Ok(()) => Ok(()),
    Err(err) if is_unsupported_sync(&err) => Ok(()),
    Err(err) => Err(err),
  }
}

fn is_unsupported_sync(err: &io::Error) -> bool {
  if matches!(
    err.kind(),
    io::ErrorKind::InvalidInput | io::ErrorKind::Unsupported
  ) {
    return true;
  }
  #[cfg(unix)]
  {
    let code = err.raw_os_error();
    return code == Some(libc::EINVAL)
      || code == Some(libc::ENOTTY)
      || code == Some(libc::ENODEV)
      || code == Some(libc::ENOTSUP)
      || code == Some(libc::EOPNOTSUPP);
  }
  #[cfg(windows)]
  {
    // ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED, ERROR_INVALID_PARAMETER
    return matches!(err.raw_os_error(), Some(1) | Some(50) | Some(87));
  }
  #[cfg(not(any(unix, windows)))]
  false
}

fn query_sector_size(file: &File) -> Option<usize> {
  #[cfg(target_os = "macos")]
  {
    return macos_sector_size(file);
  }
  #[cfg(target_os = "linux")]
  {
    return linux_sector_size(file);
  }
  #[cfg(target_os = "freebsd")]
  {
    return freebsd_sector_size(file);
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "freebsd")))]
  {
    let _ = file;
    None
  }
}

#[cfg(target_os = "macos")]
fn macos_sector_size(file: &File) -> Option<usize> {
  use std::os::unix::io::AsRawFd;
  // DKIOCGETBLOCKSIZE = _IOR('d', 24, uint32_t)
  const DKIOCGETBLOCKSIZE: libc::c_ulong = 0x4004_6418;
  let mut size: u32 = 0;
  let rc = unsafe { libc::ioctl(file.as_raw_fd(), DKIOCGETBLOCKSIZE, &mut size) };
  (rc == 0 && size > 0).then_some(size as usize)
}

#[cfg(target_os = "macos")]
fn macos_synchronize_cache(file: &File) -> io::Result<()> {
  use std::os::unix::io::AsRawFd;
  // DKIOCSYNCHRONIZECACHE = _IO('d', 22)
  const DKIOCSYNCHRONIZECACHE: libc::c_ulong = 0x2000_6416;
  let rc = unsafe { libc::ioctl(file.as_raw_fd(), DKIOCSYNCHRONIZECACHE) };
  if rc == 0 {
    Ok(())
  } else {
    Err(io::Error::last_os_error())
  }
}

#[cfg(target_os = "macos")]
fn fsync_fd(file: &File) -> io::Result<()> {
  use std::os::unix::io::AsRawFd;
  let rc = unsafe { libc::fsync(file.as_raw_fd()) };
  if rc == 0 {
    Ok(())
  } else {
    Err(io::Error::last_os_error())
  }
}

#[cfg(target_os = "linux")]
fn linux_sector_size(file: &File) -> Option<usize> {
  use std::os::unix::io::AsRawFd;
  // BLKSSZGET = _IO(0x12, 104)
  const BLKSSZGET: libc::c_ulong = 0x1268;
  let mut size: libc::c_int = 0;
  let rc = unsafe { libc::ioctl(file.as_raw_fd(), BLKSSZGET, &mut size) };
  (rc == 0 && size > 0).then_some(size as usize)
}

#[cfg(target_os = "freebsd")]
fn freebsd_sector_size(file: &File) -> Option<usize> {
  use std::os::unix::io::AsRawFd;
  // DIOCGSECTORSIZE = _IOR('d', 128, u_int)
  const DIOCGSECTORSIZE: libc::c_ulong = 0x4004_6480;
  let mut size: libc::c_uint = 0;
  let rc = unsafe { libc::ioctl(file.as_raw_fd(), DIOCGSECTORSIZE, &mut size) };
  (rc == 0 && size > 0).then_some(size as usize)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn align_up_noop_when_aligned() {
    assert_eq!(align_up(0, 512), 0);
    assert_eq!(align_up(512, 512), 512);
    assert_eq!(align_up(2048, 512), 2048);
    assert_eq!(align_up(BLOCK, 512), BLOCK);
    assert_eq!(align_up(BLOCK, 4096), BLOCK);
  }

  #[test]
  fn align_up_pads_short_tail() {
    assert_eq!(align_up(1, 512), 512);
    assert_eq!(align_up(513, 512), 1024);
    assert_eq!(align_up(4095, 4096), 4096);
  }

  #[test]
  fn io_chunk_is_multiple_of_sector() {
    for sector in [512usize, 4096, 8192] {
      let chunk = io_chunk(sector);
      assert_eq!(chunk % sector, 0);
      assert!(chunk >= sector);
      assert!(chunk <= BLOCK);
    }
  }

  #[test]
  fn padded_write_len_only_grows_short_tail() {
    assert_eq!(padded_write_len(BLOCK, 512, BLOCK), BLOCK);
    assert_eq!(padded_write_len(84144, 512, BLOCK), 84480);
    assert_eq!(padded_write_len(512, 512, BLOCK), 512);
    assert_eq!(padded_write_len(0, 512, BLOCK), 0);
  }
}
