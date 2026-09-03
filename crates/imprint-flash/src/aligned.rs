//! One-sector cache so byte I/O works on `/dev/rdiskN` and `\\.\PhysicalDriveN`,
//! which reject unaligned reads and writes with EINVAL.

use std::io::{self, Read, Seek, SeekFrom, Write};

pub(crate) struct AlignedIo<T: Read + Write + Seek> {
  inner: T,
  sector: u64,
  buf: Vec<u8>,
  loaded: Option<u64>,
  dirty: bool,
  pos: u64,
}

impl<T: Read + Write + Seek> AlignedIo<T> {
  pub(crate) fn new(inner: T, sector: usize) -> Self {
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
