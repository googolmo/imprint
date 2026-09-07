use std::ffi::CString;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use imprint_core::{BusKind, DiskId, Result, TargetDisk};

pub fn list() -> Result<Vec<TargetDisk>> {
  let mut disks = Vec::new();
  let root_dev = root_device();
  let Ok(entries) = fs::read_dir("/sys/block") else {
    return Ok(disks);
  };

  for entry in entries.flatten() {
    let name = entry.file_name();
    let name = name.to_string_lossy();
    if name.starts_with("loop")
      || name.starts_with("ram")
      || name.starts_with("zram")
      || name.starts_with("dm-")
      || name.starts_with("md")
    {
      continue;
    }

    let sys = PathBuf::from("/sys/block").join(name.as_ref());
    let size = read_u64(&sys.join("size")).unwrap_or(0) * 512;
    if size == 0 {
      continue;
    }

    let removable = read_u64(&sys.join("removable")).unwrap_or(0) == 1;
    let model = read_trim(&sys.join("device/model")).unwrap_or_else(|| name.to_string());
    let vendor = read_trim(&sys.join("device/vendor")).unwrap_or_default();
    let bus = classify(&sys, &name, removable);
    let path = PathBuf::from("/dev").join(name.as_ref());
    let system = is_system(&name, &root_dev, removable);

    let description = [vendor.as_str(), model.as_str()]
      .iter()
      .filter(|s| !s.is_empty())
      .map(|s| s.trim())
      .collect::<Vec<_>>()
      .join(" ");

    disks.push(TargetDisk {
      id: DiskId(name.to_string()),
      name: if description.is_empty() {
        name.to_string()
      } else {
        description.clone()
      },
      path,
      size,
      bus,
      system,
      description,
    });
  }

  Ok(disks)
}

fn classify(sys: &Path, name: &str, removable: bool) -> BusKind {
  let uevent = read_trim(&sys.join("device/uevent")).unwrap_or_default();
  let lower = format!("{name} {uevent}").to_ascii_lowercase();
  if lower.contains("usb") {
    BusKind::Usb
  } else if lower.contains("mmc") || name.starts_with("mmcblk") {
    BusKind::Sd
  } else if name.starts_with("nvme") {
    BusKind::Nvme
  } else if removable {
    BusKind::Usb
  } else {
    BusKind::Sata
  }
}

fn is_system(name: &str, root_dev: &Option<String>, removable: bool) -> bool {
  if let Some(root) = root_dev
    && (root == name || root.starts_with(name))
  {
    return true;
  }
  !removable && !name.starts_with("mmcblk")
}

fn root_device() -> Option<String> {
  let mounts = fs::read_to_string("/proc/self/mountinfo").ok()?;
  for line in mounts.lines() {
    let parts: Vec<&str> = line.split(' ').collect();
    // mountinfo: ... mount_point ... - fstype source
    if let Some(mount_point) = parts.get(4)
      && *mount_point == "/"
    {
      let source = parts.last()?.trim();
      return device_basename(source);
    }
  }
  let mounts = fs::read_to_string("/proc/mounts").ok()?;
  for line in mounts.lines() {
    let mut parts = line.split_whitespace();
    let source = parts.next()?;
    let dest = parts.next()?;
    if dest == "/" {
      return device_basename(source);
    }
  }
  None
}

fn device_basename(source: &str) -> Option<String> {
  let path = Path::new(source);
  let name = path.file_name()?.to_string_lossy().to_string();
  Some(
    name
      .trim_end_matches(|c: char| c.is_ascii_digit())
      .trim_end_matches('p')
      .to_string(),
  )
}

fn read_trim(path: &Path) -> Option<String> {
  fs::read_to_string(path)
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

fn read_u64(path: &Path) -> Option<u64> {
  read_trim(path)?.parse().ok()
}

pub struct Watch {
  wakeup: Option<RawFd>,
  thread: Option<JoinHandle<()>>,
}

pub fn watch(on_change: Box<dyn Fn() + Send>) -> Watch {
  let Ok(path) = CString::new("/sys/block") else {
    return Watch {
      wakeup: None,
      thread: None,
    };
  };
  unsafe {
    let inotify = libc::inotify_init1(libc::IN_CLOEXEC);
    if inotify < 0 {
      return Watch {
        wakeup: None,
        thread: None,
      };
    }
    let mask = libc::IN_CREATE | libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_MOVED_TO;
    if libc::inotify_add_watch(inotify, path.as_ptr(), mask) < 0 {
      libc::close(inotify);
      return Watch {
        wakeup: None,
        thread: None,
      };
    }
    let mut pipe = [0; 2];
    if libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) != 0 {
      libc::close(inotify);
      return Watch {
        wakeup: None,
        thread: None,
      };
    }
    let (rd, wr) = (pipe[0], pipe[1]);
    let thread = thread::Builder::new()
      .name("imprint-disk-watch".into())
      .spawn(move || inotify_loop(inotify, rd, on_change))
      .ok();
    if thread.is_none() {
      libc::close(inotify);
      libc::close(rd);
      libc::close(wr);
      return Watch {
        wakeup: None,
        thread: None,
      };
    }
    Watch {
      wakeup: Some(wr),
      thread,
    }
  }
}

fn inotify_loop(inotify: RawFd, wakeup: RawFd, on_change: Box<dyn Fn() + Send>) {
  let mut buf = [0u8; 4096];
  loop {
    let mut fds = [
      libc::pollfd {
        fd: inotify,
        events: libc::POLLIN,
        revents: 0,
      },
      libc::pollfd {
        fd: wakeup,
        events: libc::POLLIN,
        revents: 0,
      },
    ];
    let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
    if n < 0 {
      if std::io::Error::last_os_error().kind() == ErrorKind::Interrupted {
        continue;
      }
      break;
    }
    if fds[1].revents & libc::POLLIN != 0 {
      break;
    }
    if fds[0].revents & libc::POLLIN != 0 {
      let read = unsafe { libc::read(inotify, buf.as_mut_ptr().cast(), buf.len()) };
      if read > 0 {
        on_change();
      } else if read < 0 {
        break;
      }
    }
  }
  unsafe {
    libc::close(inotify);
    libc::close(wakeup);
  }
}

impl Drop for Watch {
  fn drop(&mut self) {
    if let Some(fd) = self.wakeup.take() {
      let byte = [1u8];
      unsafe {
        libc::write(fd, byte.as_ptr().cast(), 1);
        libc::close(fd);
      }
    }
    if let Some(thread) = self.thread.take() {
      let _ = thread.join();
    }
  }
}
