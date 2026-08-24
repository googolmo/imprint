use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use imprint_core::{FlashRequest, Settings, format_bytes};
use imprint_device::list_targets;
use imprint_flash::{flash, has_block_privileges, validate_request};
use imprint_image::inspect;
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "imprint", about = "Flash OS images to USB drives and SD cards")]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand)]
enum Command {
  /// List removable disks (system drives hidden by default)
  Devices {
    #[arg(long)]
    all: bool,
  },
  /// Write an image to a target device
  Flash {
    /// Path to an ISO / IMG / compressed image
    image: PathBuf,
    /// Raw device path, e.g. /dev/rdisk4 or \\\\.\\PhysicalDrive1
    #[arg(long, short)]
    device: PathBuf,
    /// Skip byte-for-byte verification
    #[arg(long)]
    no_verify: bool,
    /// Skip eject after success
    #[arg(long)]
    no_eject: bool,
    /// Really write (required)
    #[arg(long)]
    yes: bool,
  },
}

fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("imprint=info".parse()?))
    .init();

  match Cli::parse().command {
    Command::Devices { all } => {
      let mut settings = Settings::default();
      if all {
        settings.hide_system_drives = false;
        settings.allow_system_drives = true;
      }
      let disks = list_targets(&settings)?;
      if disks.is_empty() {
        println!("No disks found.");
        return Ok(());
      }
      for disk in disks {
        let flag = if disk.system { " SYSTEM" } else { "" };
        println!(
          "{:<18} {:>10}  {:<8}  {}{flag}",
          disk.path.display(),
          format_bytes(disk.size),
          disk.bus.as_str(),
          disk.label()
        );
      }
    }
    Command::Flash {
      image,
      device,
      no_verify,
      no_eject,
      yes,
    } => {
      if !yes {
        bail!("refusing to write without --yes");
      }
      if !has_block_privileges() {
        bail!("need root / Administrator to write to {}", device.display());
      }
      let image = inspect(&image).with_context(|| format!("inspect {}", image.display()))?;
      let settings = Settings {
        hide_system_drives: false,
        allow_system_drives: false,
        ..Settings::default()
      };
      let disks = list_targets(&settings)?;
      let target = disks
        .into_iter()
        .find(|d| d.path == device)
        .with_context(|| {
          format!(
            "device {} is not in the disk list — run `imprint-cli devices --all`",
            device.display()
          )
        })?;
      let request = FlashRequest {
        image,
        targets: vec![target],
        verify: !no_verify,
        unmount: !no_eject,
      };
      validate_request(&request)?;

      let bar = ProgressBar::new(request.image.write_size().max(1));
      bar.set_style(
        ProgressStyle::with_template(
          "{spinner:.green} {msg} {wide_bar:.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec})",
        )?
        .progress_chars("█▉▊▋▌▍▎▏  "),
      );
      let cancel = AtomicBool::new(false);
      ctrlc_stub(&cancel);
      flash(request, &cancel, |p| {
        bar.set_length(p.bytes_total.max(1));
        bar.set_position(p.bytes_done);
        bar.set_message(p.phase.as_str().to_string());
      })?;
      bar.finish_with_message("done");
    }
  }
  Ok(())
}

fn ctrlc_stub(_cancel: &AtomicBool) {
  let _ = Ordering::Relaxed;
}
