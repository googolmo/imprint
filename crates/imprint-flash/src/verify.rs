use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use imprint_core::{Error, FlashPhase, FlashProgress, Result, TargetDisk, format_bytes};
use imprint_image::open_payload;

pub fn verify_target(
  image: &imprint_core::ImageRef,
  disk: &TargetDisk,
  dest: &mut File,
  written: u64,
  cancel: &AtomicBool,
  on_progress: &mut impl FnMut(FlashProgress),
) -> Result<()> {
  let mut source = open_payload(image)?;
  let sector = crate::raw::sector_size(dest);
  let chunk = crate::raw::io_chunk(sector);

  let mut src_buf = vec![0u8; chunk];
  let mut dst_buf = vec![0u8; chunk];
  let mut offset = 0u64;
  let started = Instant::now();
  let mut last_tick = started;

  while offset < written {
    if cancel.load(Ordering::Relaxed) {
      return Err(Error::Cancelled);
    }
    let want = ((written - offset) as usize).min(chunk);
    let src_n = crate::raw::read_full(&mut source, &mut src_buf[..want])?;
    if src_n == 0 {
      break;
    }
    // Match the padded write: rdisk reads must also be sector-sized.
    let dest_len = crate::raw::padded_write_len(src_n, sector, dst_buf.len());
    let dst_n = crate::raw::read_full(dest, &mut dst_buf[..dest_len])?;
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
      emit_verify(on_progress, offset, written, started, disk);
      last_tick = now;
    }
  }
  emit_verify(on_progress, offset, written, started, disk);
  Ok(())
}

fn emit_verify(
  on_progress: &mut impl FnMut(FlashProgress),
  offset: u64,
  written: u64,
  started: Instant,
  disk: &TargetDisk,
) {
  let elapsed = started.elapsed().as_secs_f64().max(0.001);
  let bps = (offset as f64 / elapsed) as u64;
  on_progress(FlashProgress {
    phase: FlashPhase::Verifying,
    bytes_done: offset,
    bytes_total: written,
    bytes_per_sec: bps,
    target_label: disk.label(),
    message: format!("Checked {}", format_bytes(offset)),
  });
}
