use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use imprint_core::{
  Error, FlashPhase, FlashProgress, FlashRequest, ImageKind, Result, TargetDisk, format_bytes,
};
use imprint_device::{eject, unmount};
use imprint_image::open_payload;
use tracing::{info, warn};

use crate::privilege::has_block_privileges;
use crate::validate_request;
use crate::verify::verify_target;

pub(crate) const BLOCK: usize = 1024 * 1024;

pub fn flash(
  request: FlashRequest,
  cancel: &AtomicBool,
  on_progress: impl FnMut(FlashProgress),
) -> Result<()> {
  validate_request(&request)?;
  if crate::helper::is_internal_flash() || has_block_privileges() {
    if !has_block_privileges() {
      let label = request
        .targets
        .first()
        .map(|d| d.path.display().to_string())
        .unwrap_or_else(|| "the target disk".into());
      return Err(Error::Privileges(label));
    }
    return flash_in_process(request, cancel, on_progress);
  }
  #[cfg(target_os = "macos")]
  if crate::authopen::available() {
    // Keep this process unprivileged; authopen (Apple-signed) can offer Touch ID.
    return flash_in_process(request, cancel, on_progress);
  }
  crate::helper::flash_elevated(request, cancel, on_progress)
}

pub(crate) fn flash_in_process(
  request: FlashRequest,
  cancel: &AtomicBool,
  mut on_progress: impl FnMut(FlashProgress),
) -> Result<()> {
  let total = request.image.write_size();
  let image = request.image.clone();

  for disk in &request.targets {
    if cancel.load(Ordering::Relaxed) {
      return Err(Error::Cancelled);
    }

    emit(
      &mut on_progress,
      FlashPhase::Preparing,
      0,
      total.max(1),
      0,
      disk,
      format!("Unmounting {}", disk.label()),
    );

    if let Err(err) = unmount(disk) {
      warn!("unmount {err}");
    }

    emit(
      &mut on_progress,
      FlashPhase::Writing,
      0,
      total,
      0,
      disk,
      format!("Writing {} → {}", image.display_name, disk.label()),
    );

    let mut dest = open_device(&disk.path)?;
    if cancel.load(Ordering::Relaxed) {
      return Err(Error::Cancelled);
    }
    let written = write_one(&image, disk, &mut dest, total, cancel, &mut on_progress)?;
    info!("wrote {written} bytes to {}", disk.path.display());

    if request.verify {
      emit(
        &mut on_progress,
        FlashPhase::Verifying,
        0,
        written,
        0,
        disk,
        "Validating written bytes".into(),
      );
      dest.seek(SeekFrom::Start(0))?;
      verify_target(&image, disk, &mut dest, written, cancel, &mut on_progress)?;
    }

    if let Some(boot) = request.boot.as_ref().filter(|b| !b.is_empty()) {
      emit(
        &mut on_progress,
        FlashPhase::Finishing,
        written,
        written.max(1),
        0,
        disk,
        "Applying first-boot configuration".into(),
      );
      dest.seek(SeekFrom::Start(0))?;
      let sector = crate::raw::sector_size(&dest);
      crate::boot::apply_on(&mut dest, boot, sector)?;
      dest.flush()?;
      crate::raw::sync_device(&mut dest)?;
    }

    if request.expand && request.image.kind != ImageKind::Iso {
      emit(
        &mut on_progress,
        FlashPhase::Finishing,
        written,
        written.max(1),
        0,
        disk,
        "Expanding partition to fill the drive".into(),
      );
      let device_bytes = device_len(&mut dest, disk.size);
      dest.seek(SeekFrom::Start(0))?;
      let sector = crate::raw::sector_size(&dest);
      let added = crate::expand::apply_on(&mut dest, device_bytes, sector)?;
      if added > 0 {
        info!(
          "expanded last partition on {} by {}",
          disk.path.display(),
          format_bytes(added)
        );
      }
      dest.flush()?;
      crate::raw::sync_device(&dest)?;
    }
    drop(dest);

    if request.unmount {
      emit(
        &mut on_progress,
        FlashPhase::Finishing,
        written,
        written.max(1),
        0,
        disk,
        "Ejecting".into(),
      );
      if let Err(err) = eject(disk) {
        warn!("eject {err}");
      }
    }
  }

  let last = request.targets.last().cloned().unwrap();
  emit(
    &mut on_progress,
    FlashPhase::Done,
    total.max(1),
    total.max(1),
    0,
    &last,
    "Flash complete".into(),
  );
  Ok(())
}

fn write_one(
  image: &imprint_core::ImageRef,
  disk: &TargetDisk,
  dest: &mut File,
  expected_total: u64,
  cancel: &AtomicBool,
  on_progress: &mut impl FnMut(FlashProgress),
) -> Result<u64> {
  let mut reader = open_payload(image)?;
  let sector = crate::raw::sector_size(dest);
  let chunk = crate::raw::io_chunk(sector);
  let mut buf = vec![0u8; chunk];
  let started = Instant::now();
  let mut last_tick = started;

  let written = write_payload(&mut reader, dest, &mut buf, sector, cancel, |written| {
    let now = Instant::now();
    if now.duration_since(last_tick).as_millis() >= 80 {
      emit_write(on_progress, written, expected_total, false, started, disk);
      last_tick = now;
    }
  })?;

  emit_write(on_progress, written, expected_total, true, started, disk);
  emit(
    on_progress,
    FlashPhase::Finishing,
    written,
    expected_total.max(written).max(1),
    0,
    disk,
    "Syncing".into(),
  );

  dest.flush()?;
  crate::raw::sync_device(dest)?;
  Ok(written)
}

/// Copy the payload with sector-sized device writes. Decompressors (and the
/// kernel) often return short reads; padding those mid-stream inserts zeros
/// into the image. Only the true EOF tail is padded.
fn write_payload(
  reader: &mut impl Read,
  dest: &mut impl Write,
  buf: &mut [u8],
  sector: usize,
  cancel: &AtomicBool,
  mut on_bytes: impl FnMut(u64),
) -> Result<u64> {
  let mut written = 0u64;
  loop {
    if cancel.load(Ordering::Relaxed) {
      return Err(Error::Cancelled);
    }
    let n = crate::raw::read_full(reader, buf)?;
    if n == 0 {
      break;
    }
    let out = crate::raw::padded_write_len(n, sector, buf.len());
    if out > n {
      buf[n..out].fill(0);
    }
    dest.write_all(&buf[..out])?;
    written += n as u64;
    on_bytes(written);
    if n < buf.len() {
      break;
    }
  }
  Ok(written)
}

fn device_len(dest: &mut File, fallback: u64) -> u64 {
  dest
    .seek(SeekFrom::End(0))
    .ok()
    .filter(|&n| n > 0)
    .unwrap_or(0)
    .max(fallback)
}

fn open_device(path: &std::path::Path) -> Result<File> {
  match OpenOptions::new().read(true).write(true).open(path) {
    Ok(mut file) => {
      let _ = file.seek(SeekFrom::Start(0));
      Ok(file)
    }
    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
      #[cfg(target_os = "macos")]
      {
        let mut file = crate::authopen::open_device(path)?;
        let _ = file.seek(SeekFrom::Start(0));
        return Ok(file);
      }
      #[cfg(not(target_os = "macos"))]
      Err(Error::Privileges(path.display().to_string()))
    }
    Err(e) => Err(Error::from(e)),
  }
}

fn emit_write(
  on_progress: &mut impl FnMut(FlashProgress),
  written: u64,
  expected_total: u64,
  at_eof: bool,
  started: Instant,
  disk: &TargetDisk,
) {
  let elapsed = started.elapsed().as_secs_f64().max(0.001);
  let bps = (written as f64 / elapsed) as u64;
  let (done, total) = write_progress(written, expected_total, at_eof);
  emit(
    on_progress,
    FlashPhase::Writing,
    done,
    total,
    bps,
    disk,
    format!("{} written", format_bytes(written)),
  );
}

/// Progress total is the uncompressed payload size. Never report 100% until EOF
/// so a `.img.gz` whose file size was used as a stand-in cannot fill the bar
/// while decompression is still producing bytes.
fn write_progress(written: u64, expected: u64, at_eof: bool) -> (u64, u64) {
  if at_eof {
    (written, expected.max(written))
  } else if expected == 0 {
    (written, 0)
  } else {
    (written.min(expected.saturating_sub(1)), expected)
  }
}

fn emit(
  on_progress: &mut impl FnMut(FlashProgress),
  phase: FlashPhase,
  bytes_done: u64,
  bytes_total: u64,
  bytes_per_sec: u64,
  disk: &TargetDisk,
  message: String,
) {
  on_progress(FlashProgress {
    phase,
    bytes_done,
    bytes_total,
    bytes_per_sec,
    target_label: disk.label(),
    message,
  });
}

#[cfg(test)]
mod tests {
  use super::{write_payload, write_progress};
  use std::io::Read;
  use std::sync::atomic::AtomicBool;

  #[test]
  fn gzip_overrun_stays_below_100_until_eof() {
    let compressed_stand_in = 200 * 1024 * 1024;
    let already_decompressed = 1200 * 1024 * 1024;
    assert_eq!(
      write_progress(already_decompressed, compressed_stand_in, false),
      (compressed_stand_in - 1, compressed_stand_in)
    );
    assert_eq!(
      write_progress(already_decompressed, compressed_stand_in, true),
      (already_decompressed, already_decompressed)
    );
  }

  #[test]
  fn unknown_payload_has_no_total() {
    assert_eq!(write_progress(1_200_000_000, 0, false), (1_200_000_000, 0));
  }

  /// Yields at most `max` bytes per `read`, like gzip/xz decompressors.
  struct ShortReads<'a> {
    data: &'a [u8],
    max: usize,
  }

  impl Read for ShortReads<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
      if self.data.is_empty() {
        return Ok(0);
      }
      let n = self.max.min(buf.len()).min(self.data.len());
      buf[..n].copy_from_slice(&self.data[..n]);
      self.data = &self.data[n..];
      Ok(n)
    }
  }

  #[test]
  fn short_decompressor_reads_are_not_padded_until_eof() {
    // 84144 is the first gzip-sized chunk that produced
    // "wrote 0x9f, read 0x00" when each short read was sector-padded.
    let payload: Vec<u8> = (0u8..=255).cycle().take(200_000).collect();
    let mut dest = Vec::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut reader = ShortReads {
      data: &payload,
      max: 84144,
    };
    let written = write_payload(
      &mut reader,
      &mut dest,
      &mut buf,
      512,
      &AtomicBool::new(false),
      |_| {},
    )
    .unwrap();
    assert_eq!(written, payload.len() as u64);
    assert_eq!(&dest[..payload.len()], payload);
    assert_eq!(dest.len(), crate::raw::align_up(payload.len(), 512));
    assert!(dest[payload.len()..].iter().all(|&b| b == 0));
    assert_eq!(dest[84144], payload[84144]);
    assert_ne!(dest[84144], 0);
  }

  #[test]
  fn eof_tail_is_padded_to_a_sector() {
    let payload: Vec<u8> = (1u8..=200).collect();
    let mut dest = Vec::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut reader = ShortReads {
      data: &payload,
      max: 64,
    };
    let written = write_payload(
      &mut reader,
      &mut dest,
      &mut buf,
      512,
      &AtomicBool::new(false),
      |_| {},
    )
    .unwrap();
    assert_eq!(written, 200);
    assert_eq!(&dest[..200], payload);
    assert_eq!(dest.len(), 512);
    assert!(dest[200..].iter().all(|&b| b == 0));
  }
}
