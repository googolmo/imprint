use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use imprint_core::{Error, FlashPhase, FlashProgress, Result, TargetDisk, format_bytes};
use imprint_image::open_payload;

use crate::write::BLOCK;

pub fn verify_target(
  image: &imprint_core::ImageRef,
  disk: &TargetDisk,
  written: u64,
  cancel: &AtomicBool,
  on_progress: &mut impl FnMut(FlashProgress),
) -> Result<()> {
  let mut source = open_payload(image)?;
  let mut dest = OpenOptions::new().read(true).open(&disk.path)?;
  dest.seek(SeekFrom::Start(0))?;

  let mut src_buf = vec![0u8; BLOCK];
  let mut dst_buf = vec![0u8; BLOCK];
  let mut offset = 0u64;
  let started = Instant::now();
  let mut last_tick = started;

  while offset < written {
    if cancel.load(Ordering::Relaxed) {
      return Err(Error::Cancelled);
    }
    let want = ((written - offset) as usize).min(BLOCK);
    let src_n = read_full(&mut source, &mut src_buf[..want])?;
    let dst_n = read_full(&mut dest, &mut dst_buf[..want])?;
    let n = src_n.min(dst_n);
    if n == 0 {
      break;
    }
    if src_buf[..n] != dst_buf[..n] {
      let pos = src_buf[..n]
        .iter()
        .zip(dst_buf[..n].iter())
        .position(|(a, b)| a != b)
        .unwrap_or(0);
      return Err(Error::VerifyMismatch {
        offset: offset + pos as u64,
        expected: src_buf[pos],
        actual: dst_buf[pos],
      });
    }
    offset += n as u64;

    let now = Instant::now();
    if now.duration_since(last_tick).as_millis() >= 80 {
      let elapsed = now.duration_since(started).as_secs_f64().max(0.001);
      let bps = (offset as f64 / elapsed) as u64;
      on_progress(FlashProgress {
        phase: FlashPhase::Verifying,
        bytes_done: offset,
        bytes_total: written,
        bytes_per_sec: bps,
        target_label: disk.label(),
        message: format!("Checked {}", format_bytes(offset)),
      });
      last_tick = now;
    }
  }
  Ok(())
}

fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> Result<usize> {
  let mut filled = 0;
  while filled < buf.len() {
    match reader.read(&mut buf[filled..])? {
      0 => break,
      n => filled += n,
    }
  }
  Ok(filled)
}
