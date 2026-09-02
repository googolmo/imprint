//! Write first-boot files onto the FAT partition of an imaged disk.

use std::io::{self, Read, Seek, SeekFrom, Write};

use fatfs::{FileSystem, FsOptions};
use imprint_core::{BootCustomization, Error, Result};
use tracing::info;

const SECTOR: u64 = 512;
const MBR_SIGNATURE: [u8; 2] = [0x55, 0xAA];
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
const FAT_TYPES: &[u8] = &[0x01, 0x04, 0x06, 0x0b, 0x0c, 0x0e, 0xef];

pub fn apply_on<T: Read + Write + Seek>(
  dev: &mut T,
  boot: &BootCustomization,
  sector: usize,
) -> Result<()> {
  if boot.is_empty() {
    return Ok(());
  }
  // fatfs issues 1–32 byte reads/writes. Raw devices require sector I/O.
  let mut aligned = AlignedIo::new(dev, sector);
  let (start, len) = find_fat_partition(&mut aligned)?;
  info!(
    "writing {} boot file(s) at LBA offset {start} ({len} bytes)",
    boot.files.len()
  );
  {
    let io = SliceIo {
      inner: &mut aligned,
      start,
      len,
      pos: 0,
    };
    let fs =
      FileSystem::new(io, FsOptions::new()).map_err(|err| Error::BootConfig(err.to_string()))?;
    {
      let root = fs.root_dir();
      for file in &boot.files {
        write_root_file(&root, &file.name, &file.contents)?;
      }
      if let Some(append) = boot.cmdline_append.as_deref().filter(|s| !s.is_empty()) {
        patch_cmdline(&root, append)?;
      }
    }
    fs.unmount()
      .map_err(|err| Error::BootConfig(err.to_string()))?;
  }
  aligned
    .flush()
    .map_err(|err| Error::BootConfig(err.to_string()))?;
  Ok(())
}

fn write_root_file<T: Read + Write + Seek>(
  root: &fatfs::Dir<T>,
  name: &str,
  contents: &str,
) -> Result<()> {
  let mut file = root
    .create_file(name)
    .map_err(|err| Error::BootConfig(format!("{name}: {err}")))?;
  file
    .seek(SeekFrom::Start(0))
    .map_err(|err| Error::BootConfig(err.to_string()))?;
  file
    .truncate()
    .map_err(|err| Error::BootConfig(err.to_string()))?;
  file
    .write_all(contents.as_bytes())
    .map_err(|err| Error::BootConfig(err.to_string()))?;
  file
    .flush()
    .map_err(|err| Error::BootConfig(err.to_string()))?;
  Ok(())
}

fn patch_cmdline<T: Read + Write + Seek>(root: &fatfs::Dir<T>, append: &str) -> Result<()> {
  let mut existing = String::new();
  if let Ok(mut file) = root.open_file("cmdline.txt") {
    let _ = file.read_to_string(&mut existing);
  }
  let trimmed = existing.trim_end();
  if trimmed.contains(append.trim()) {
    return Ok(());
  }
  let new = if trimmed.is_empty() {
    format!("{}\n", append.trim())
  } else {
    format!("{trimmed} {}\n", append.trim())
  };
  write_root_file(root, "cmdline.txt", &new)
}

fn find_fat_partition<T: Read + Seek>(dev: &mut T) -> Result<(u64, u64)> {
  let mut mbr = [0u8; 512];
  dev
    .seek(SeekFrom::Start(0))
    .map_err(|err| Error::BootConfig(err.to_string()))?;
  dev
    .read_exact(&mut mbr)
    .map_err(|err| Error::BootConfig(err.to_string()))?;
  if mbr[510..] == MBR_SIGNATURE {
    if let Some(part) = mbr_fat(&mbr) {
      return Ok(part);
    }
  }
  if let Some(part) = gpt_first(dev) {
    return Ok(part);
  }
  Err(Error::BootConfig(
    "no FAT boot partition found on the imaged disk".into(),
  ))
}

fn mbr_fat(mbr: &[u8]) -> Option<(u64, u64)> {
  let mut fallback = None;
  for i in 0..4 {
    let entry = &mbr[446 + i * 16..446 + (i + 1) * 16];
    let kind = entry[4];
    let lba = u32::from_le_bytes(entry[8..12].try_into().ok()?) as u64;
    let sectors = u32::from_le_bytes(entry[12..16].try_into().ok()?) as u64;
    if lba == 0 || sectors == 0 {
      continue;
    }
    let range = (lba * SECTOR, sectors * SECTOR);
    if FAT_TYPES.contains(&kind) {
      return Some(range);
    }
    if fallback.is_none() {
      fallback = Some(range);
    }
  }
  fallback
}

fn gpt_first<T: Read + Seek>(dev: &mut T) -> Option<(u64, u64)> {
  let mut header = [0u8; 512];
  dev.seek(SeekFrom::Start(SECTOR)).ok()?;
  dev.read_exact(&mut header).ok()?;
  if &header[..8] != GPT_SIGNATURE {
    return None;
  }
  let entry_lba = u64::from_le_bytes(header[72..80].try_into().ok()?);
  let entry_count = u32::from_le_bytes(header[80..84].try_into().ok()?);
  let entry_size = u32::from_le_bytes(header[84..88].try_into().ok()?);
  if entry_size < 48 || entry_count == 0 {
    return None;
  }
  let table_len = entry_count as usize * entry_size as usize;
  let mut table = vec![0u8; table_len.min(128 * 128)];
  dev.seek(SeekFrom::Start(entry_lba * SECTOR)).ok()?;
  dev.read_exact(&mut table).ok()?;
  for chunk in table.chunks(entry_size as usize) {
    if chunk.len() < 48 {
      break;
    }
    if chunk[..16].iter().all(|&b| b == 0) {
      continue;
    }
    let first = u64::from_le_bytes(chunk[32..40].try_into().ok()?);
    let last = u64::from_le_bytes(chunk[40..48].try_into().ok()?);
    if last < first {
      continue;
    }
    return Some((first * SECTOR, (last - first + 1) * SECTOR));
  }
  None
}

/// One-sector cache so fatfs can do byte I/O on `/dev/rdiskN` and
/// `\\.\PhysicalDriveN`, which reject unaligned reads and writes with EINVAL.
struct AlignedIo<T: Read + Write + Seek> {
  inner: T,
  sector: u64,
  buf: Vec<u8>,
  loaded: Option<u64>,
  dirty: bool,
  pos: u64,
}

impl<T: Read + Write + Seek> AlignedIo<T> {
  fn new(inner: T, sector: usize) -> Self {
    let sector = sector.max(1);
    Self {
      inner,
      sector: sector as u64,
      buf: vec![0u8; sector],
      loaded: None,
      dirty: false,
      pos: 0,
    }
  }

  fn align(&self, pos: u64) -> u64 {
    pos - (pos % self.sector)
  }

  fn flush_buf(&mut self) -> io::Result<()> {
    let Some(off) = self.loaded else {
      return Ok(());
    };
    if !self.dirty {
      return Ok(());
    }
    self.inner.seek(SeekFrom::Start(off))?;
    self.inner.write_all(&self.buf)?;
    self.dirty = false;
    Ok(())
  }

  fn load(&mut self, pos: u64) -> io::Result<()> {
    let off = self.align(pos);
    if self.loaded == Some(off) {
      return Ok(());
    }
    self.flush_buf()?;
    self.inner.seek(SeekFrom::Start(off))?;
    self.buf.fill(0);
    let mut filled = 0;
    while filled < self.buf.len() {
      match self.inner.read(&mut self.buf[filled..])? {
        0 => break,
        n => filled += n,
      }
    }
    self.loaded = Some(off);
    self.dirty = false;
    Ok(())
  }
}

impl<T: Read + Write + Seek> Read for AlignedIo<T> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if buf.is_empty() {
      return Ok(0);
    }
    self.load(self.pos)?;
    let loaded = self.loaded.unwrap_or(self.align(self.pos));
    let in_sector = (self.pos - loaded) as usize;
    let n = buf.len().min(self.buf.len() - in_sector);
    buf[..n].copy_from_slice(&self.buf[in_sector..in_sector + n]);
    self.pos += n as u64;
    Ok(n)
  }
}

impl<T: Read + Write + Seek> Write for AlignedIo<T> {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    if buf.is_empty() {
      return Ok(0);
    }
    let aligned = self.align(self.pos);
    let in_sector = (self.pos - aligned) as usize;
    let sector = self.buf.len();
    if in_sector == 0 && buf.len() >= sector {
      self.flush_buf()?;
      self.buf.copy_from_slice(&buf[..sector]);
      self.loaded = Some(aligned);
      self.dirty = true;
      self.pos += sector as u64;
      return Ok(sector);
    }
    self.load(self.pos)?;
    let n = buf.len().min(self.buf.len() - in_sector);
    self.buf[in_sector..in_sector + n].copy_from_slice(&buf[..n]);
    self.dirty = true;
    self.pos += n as u64;
    Ok(n)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.flush_buf()?;
    self.inner.flush()
  }
}

impl<T: Read + Write + Seek> Seek for AlignedIo<T> {
  fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
    self.pos = match from {
      SeekFrom::Start(n) => n,
      SeekFrom::End(n) => {
        let end = self.inner.seek(SeekFrom::End(0))?;
        (end as i64).saturating_add(n).max(0) as u64
      }
      SeekFrom::Current(n) => (self.pos as i64).saturating_add(n).max(0) as u64,
    };
    Ok(self.pos)
  }
}

impl<T: Read + Write + Seek> Drop for AlignedIo<T> {
  fn drop(&mut self) {
    let _ = self.flush_buf();
  }
}

struct SliceIo<'a, T> {
  inner: &'a mut T,
  start: u64,
  len: u64,
  pos: u64,
}

impl<T: Seek> SliceIo<'_, T> {
  fn seek_abs(&mut self) -> io::Result<u64> {
    self.inner.seek(SeekFrom::Start(self.start + self.pos))
  }
}

impl<T: Read + Seek> Read for SliceIo<'_, T> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    let remain = self.len.saturating_sub(self.pos) as usize;
    if remain == 0 {
      return Ok(0);
    }
    let n = buf.len().min(remain);
    self.seek_abs()?;
    let read = self.inner.read(&mut buf[..n])?;
    self.pos += read as u64;
    Ok(read)
  }
}

impl<T: Write + Seek> Write for SliceIo<'_, T> {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    let remain = self.len.saturating_sub(self.pos) as usize;
    if remain == 0 {
      return Ok(0);
    }
    let n = buf.len().min(remain);
    self.seek_abs()?;
    let written = self.inner.write(&buf[..n])?;
    self.pos += written as u64;
    Ok(written)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.inner.flush()
  }
}

impl<T: Seek> Seek for SliceIo<'_, T> {
  fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
    let next = match from {
      SeekFrom::Start(n) => n,
      SeekFrom::End(n) => (self.len as i64).saturating_add(n).max(0) as u64,
      SeekFrom::Current(n) => (self.pos as i64).saturating_add(n).max(0) as u64,
    };
    if next > self.len {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "seek past partition",
      ));
    }
    self.pos = next;
    Ok(self.pos)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Cursor;

  use imprint_core::BootFile;

  #[test]
  fn reads_fat32_lba_from_mbr() {
    let mut mbr = [0u8; 512];
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    // partition 0: type 0x0c, LBA 8192, 131072 sectors
    mbr[446 + 4] = 0x0c;
    mbr[446 + 8..446 + 12].copy_from_slice(&8192u32.to_le_bytes());
    mbr[446 + 12..446 + 16].copy_from_slice(&131072u32.to_le_bytes());
    let (start, len) = mbr_fat(&mbr).unwrap();
    assert_eq!(start, 8192 * 512);
    assert_eq!(len, 131072 * 512);
  }

  #[test]
  fn skips_empty_mbr_slots() {
    let mut mbr = [0u8; 512];
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    mbr[446 + 16 + 4] = 0x0c;
    mbr[446 + 16 + 8..446 + 16 + 12].copy_from_slice(&2048u32.to_le_bytes());
    mbr[446 + 16 + 12..446 + 16 + 16].copy_from_slice(&100u32.to_le_bytes());
    let (start, _) = mbr_fat(&mbr).unwrap();
    assert_eq!(start, 2048 * 512);
  }

  /// Simulates `/dev/rdisk*` / `\\.\PhysicalDrive*`: EINVAL unless I/O is sector-sized.
  struct StrictIo<T> {
    inner: T,
    sector: u64,
  }

  impl<T: Read + Seek> Read for StrictIo<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      self.check(buf.len())?;
      self.inner.read(buf)
    }
  }

  impl<T: Write + Seek> Write for StrictIo<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      self.check(buf.len())?;
      self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
      self.inner.flush()
    }
  }

  impl<T: Seek> Seek for StrictIo<T> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
      self.inner.seek(from)
    }
  }

  impl<T: Seek> StrictIo<T> {
    fn check(&mut self, len: usize) -> io::Result<()> {
      let pos = self.inner.stream_position()?;
      if self.sector > 1 && (pos % self.sector != 0 || len as u64 % self.sector != 0) {
        return Err(io::Error::from_raw_os_error(22));
      }
      Ok(())
    }
  }

  fn imaged_disk() -> Vec<u8> {
    const START: usize = 8192 * 512;
    const LEN: usize = 8 * 1024 * 1024;
    let mut disk = vec![0u8; START + LEN];
    disk[510] = 0x55;
    disk[511] = 0xAA;
    disk[446 + 4] = 0x0c;
    disk[446 + 8..446 + 12].copy_from_slice(&8192u32.to_le_bytes());
    disk[446 + 12..446 + 16].copy_from_slice(&((LEN / 512) as u32).to_le_bytes());
    let mut part = Cursor::new(&mut disk[START..START + LEN]);
    fatfs::format_volume(&mut part, fatfs::FormatVolumeOptions::new()).unwrap();
    disk
  }

  fn read_boot_file(disk: &mut [u8], name: &str) -> String {
    let mut cursor = Cursor::new(disk);
    let (start, len) = find_fat_partition(&mut cursor).unwrap();
    let io = SliceIo {
      inner: &mut cursor,
      start,
      len,
      pos: 0,
    };
    let fs = FileSystem::new(io, FsOptions::new()).unwrap();
    let mut data = String::new();
    fs.root_dir()
      .open_file(name)
      .unwrap()
      .read_to_string(&mut data)
      .unwrap();
    data
  }

  fn sample_boot() -> BootCustomization {
    BootCustomization {
      files: vec![BootFile {
        name: "user-data".into(),
        contents: "#cloud-config\nhostname: lab-pi\n".into(),
      }],
      cmdline_append: Some(
        "systemd.run=/boot/firstrun.sh systemd.run_success_action=reboot".into(),
      ),
    }
  }

  #[test]
  fn aligned_io_covers_unaligned_access() {
    let mut storage = vec![0u8; 4096];
    {
      let mut io = AlignedIo::new(
        StrictIo {
          inner: Cursor::new(&mut storage),
          sector: 512,
        },
        512,
      );
      io.seek(SeekFrom::Start(3)).unwrap();
      io.write_all(b"pi").unwrap();
      io.flush().unwrap();
    }
    assert_eq!(&storage[3..5], b"pi");
  }

  #[test]
  fn apply_on_writes_files_through_sector_strict_device() {
    let mut disk = imaged_disk();
    {
      let mut io = StrictIo {
        inner: Cursor::new(&mut disk),
        sector: 512,
      };
      apply_on(&mut io, &sample_boot(), 512).unwrap();
    }
    assert!(read_boot_file(&mut disk, "user-data").contains("hostname: lab-pi"));
    assert!(read_boot_file(&mut disk, "cmdline.txt").contains("systemd.run=/boot/firstrun.sh"));
  }

  #[test]
  fn apply_on_writes_files_through_4k_device_sectors() {
    let mut disk = imaged_disk();
    {
      let mut io = StrictIo {
        inner: Cursor::new(&mut disk),
        sector: 4096,
      };
      apply_on(&mut io, &sample_boot(), 4096).unwrap();
    }
    assert!(read_boot_file(&mut disk, "user-data").contains("hostname: lab-pi"));
  }
}
