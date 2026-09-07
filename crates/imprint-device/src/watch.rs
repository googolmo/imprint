//! OS disk attach/detach callbacks. The UI re-lists targets when `on_change` fires.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::platform;

/// Keeps the platform watcher alive. Dropping it unregisters the callbacks.
pub struct DiskWatch {
  _platform: platform::Watch,
  _coalesce: Option<JoinHandle<()>>,
  _kick: mpsc::Sender<()>,
}

/// Call `on_change` after disk appear/disappear events settle.
///
/// `on_change` runs on a background thread. Burst events (probe, mount) are
/// coalesced so the UI lists disks once per plug/unplug.
pub fn watch_disks(on_change: impl Fn() + Send + 'static) -> DiskWatch {
  let (kick_tx, kick_rx) = mpsc::channel();
  let coalesce = thread::Builder::new()
    .name("imprint-disk-coalesce".into())
    .spawn(move || coalesce_loop(kick_rx, Box::new(on_change)))
    .ok();
  let notify = {
    let kick_tx = kick_tx.clone();
    Box::new(move || {
      let _ = kick_tx.send(());
    }) as Box<dyn Fn() + Send>
  };
  DiskWatch {
    _platform: platform::watch(notify),
    _coalesce: coalesce,
    _kick: kick_tx,
  }
}

fn coalesce_loop(rx: mpsc::Receiver<()>, on_change: Box<dyn Fn() + Send>) {
  const QUIET: Duration = Duration::from_millis(400);
  loop {
    if rx.recv().is_err() {
      break;
    }
    loop {
      match rx.recv_timeout(QUIET) {
        Ok(()) => continue,
        Err(RecvTimeoutError::Timeout) => {
          on_change();
          break;
        }
        Err(RecvTimeoutError::Disconnected) => return,
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn watch_disks_starts_and_stops() {
    let watch = watch_disks(|| {});
    drop(watch);
  }
}
