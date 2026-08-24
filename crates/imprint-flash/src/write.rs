use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use imprint_core::{
  Error, FlashPhase, FlashProgress, FlashRequest, Result, TargetDisk, format_bytes,
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
  mut on_progress: impl FnMut(FlashProgress),
) -> Result<()> {
  validate_request(&request)?;
  if !has_block_privileges() {
    let label = request
      .targets
      .first()
      .map(|d| d.path.display().to_string())
      .unwrap_or_else(|| "the target disk".into());
    return Err(Error::Privileges(label));
  }

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
      total.max(1),
      0,
      disk,
      format!("Writing {} → {}", image.display_name, disk.label()),
    );

    let written = write_one(&image, disk, total, cancel, &mut on_progress)?;
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
      verify_target(&image, disk, written, cancel, &mut on_progress)?;
    }

    emit(
      &mut on_progress,
      FlashPhase::Finishing,
      written,
      written.max(1),
      0,
      disk,
      "Syncing".into(),
    );

    if request.unmount {
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
  expected_total: u64,
  cancel: &AtomicBool,
  on_progress: &mut impl FnMut(FlashProgress),
) -> Result<u64> {
  let mut reader = open_payload(image)?;
  let mut dest = open_device(&disk.path)?;
  let mut buf = vec![0u8; BLOCK];
  let mut written = 0u64;
  let started = Instant::now();
  let mut last_tick = started;

  loop {
    if cancel.load(Ordering::Relaxed) {
      return Err(Error::Cancelled);
    }
    let n = reader.read(&mut buf)?;
    if n == 0 {
      break;
    }
    dest.write_all(&buf[..n])?;
    written += n as u64;

    let now = Instant::now();
    if now.duration_since(last_tick).as_millis() >= 80 {
      let elapsed = now.duration_since(started).as_secs_f64().max(0.001);
      let bps = (written as f64 / elapsed) as u64;
      emit(
        on_progress,
        FlashPhase::Writing,
        written,
        expected_total.max(written),
        bps,
        disk,
        format!("{} written", format_bytes(written)),
      );
      last_tick = now;
    }
  }

  dest.flush()?;
  dest.sync_all()?;
  drop(dest);
  Ok(written)
}

fn open_device(path: &std::path::Path) -> Result<File> {
  let mut file = OpenOptions::new()
    .read(true)
    .write(true)
    .open(path)
    .map_err(|e| {
      if e.kind() == std::io::ErrorKind::PermissionDenied {
        Error::Privileges(path.display().to_string())
      } else {
        Error::from(e)
      }
    })?;
  let _ = file.seek(SeekFrom::Start(0));
  Ok(file)
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
