//! Open a block device via `/usr/libexec/authopen`.
//!
//! `authopen` is Apple-signed, so the Security Agent may offer Touch ID / Apple
//! Watch instead of a password. The third-party `osascript` path cannot.

use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};

use imprint_core::{Error, Result};
use tracing::info;

const AUTHOPEN: &str = "/usr/libexec/authopen";

pub(crate) fn available() -> bool {
  Path::new(AUTHOPEN).is_file()
}

pub(crate) fn open_device(path: &Path) -> Result<File> {
  let path = path.to_path_buf();
  info!("requesting disk access via authopen for {}", path.display());

  let (reader, writer) = UnixStream::pair().map_err(Error::from)?;
  let mut child = Command::new(AUTHOPEN)
    .arg("-stdoutpipe")
    .arg("-o")
    .arg(libc::O_RDWR.to_string())
    .arg(&path)
    .stdin(Stdio::null())
    .stdout(Stdio::from(OwnedFd::from(writer)))
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| Error::ElevationFailed(format!("authopen: {err}")))?;

  let fd = match recv_fd(&reader) {
    Ok(fd) => Some(fd),
    Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => None,
    Err(err) => {
      let _ = child.kill();
      let _ = child.wait();
      return Err(Error::ElevationFailed(format!("authopen fd: {err}")));
    }
  };

  let output = child
    .wait_with_output()
    .map_err(|err| Error::ElevationFailed(format!("authopen wait: {err}")))?;
  let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

  if !output.status.success() || fd.is_none() {
    return Err(classify_failure(&path, output.status.code(), &stderr));
  }

  let fd = fd.expect("success path has fd");
  // SAFETY: fd is the exclusive descriptor authopen passed via SCM_RIGHTS.
  Ok(unsafe { File::from_raw_fd(fd) })
}

fn classify_failure(path: &Path, code: Option<i32>, stderr: &str) -> Error {
  if looks_like_cancel(stderr) {
    return Error::ElevationCancelled;
  }
  if stderr.to_ascii_lowercase().contains("resource busy") {
    return Error::msg(format!(
      "{} is busy (unmount it in Disk Utility and retry)",
      path.display()
    ));
  }
  let reason = if stderr.is_empty() {
    format!(
      "authopen exited with status {}",
      code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
    )
  } else {
    stderr.to_string()
  };
  Error::ElevationFailed(reason)
}

fn looks_like_cancel(text: &str) -> bool {
  let lower = text.to_ascii_lowercase();
  lower.contains("user canceled")
    || lower.contains("user cancelled")
    || lower.contains("canceled by the user")
    || lower.contains("cancelled by the user")
    || lower.contains("not authorized")
    || text.contains("-128")
    || text.contains("-60006")
}

fn recv_fd(socket: &UnixStream) -> io::Result<RawFd> {
  let mut dummy = [0u8; 8];
  let mut iov = libc::iovec {
    iov_base: dummy.as_mut_ptr() as *mut libc::c_void,
    iov_len: dummy.len(),
  };
  let cmsg_space = unsafe { libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) } as usize;
  let mut cmsg_buf = vec![0u8; cmsg_space];

  loop {
    cmsg_buf.fill(0);
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buf.as_mut_ptr() as *mut libc::c_void;
    msg.msg_controllen = cmsg_buf.len() as _;

    let n = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, 0) };
    if n < 0 {
      let err = io::Error::last_os_error();
      if err.kind() == io::ErrorKind::Interrupted {
        continue;
      }
      return Err(err);
    }
    if n == 0 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "authopen closed the pipe without a file descriptor",
      ));
    }
    if msg.msg_flags & libc::MSG_CTRUNC != 0 {
      return Err(io::Error::other("authopen truncated the file descriptor"));
    }

    let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    if cmsg.is_null() {
      return Err(io::Error::other("authopen sent no control message"));
    }
    unsafe {
      if (*cmsg).cmsg_level != libc::SOL_SOCKET || (*cmsg).cmsg_type != libc::SCM_RIGHTS {
        return Err(io::Error::other("authopen did not pass a file descriptor"));
      }
      let data = libc::CMSG_DATA(cmsg) as *const RawFd;
      return Ok(std::ptr::read_unaligned(data));
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn send_fd(socket: &UnixStream, fd: RawFd) -> io::Result<()> {
    let mut dummy = [0u8; 1];
    let mut iov = libc::iovec {
      iov_base: dummy.as_mut_ptr() as *mut libc::c_void,
      iov_len: dummy.len(),
    };
    let cmsg_space = unsafe { libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) } as usize;
    let mut cmsg_buf = vec![0u8; cmsg_space];
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buf.as_mut_ptr() as *mut libc::c_void;
    msg.msg_controllen = cmsg_buf.len() as _;

    let hdr = unsafe { libc::CMSG_FIRSTHDR(&mut msg) };
    assert!(!hdr.is_null());
    unsafe {
      (*hdr).cmsg_level = libc::SOL_SOCKET;
      (*hdr).cmsg_type = libc::SCM_RIGHTS;
      (*hdr).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) as _;
      std::ptr::write_unaligned(libc::CMSG_DATA(hdr) as *mut RawFd, fd);
      msg.msg_controllen = (*hdr).cmsg_len;
    }

    let n = unsafe { libc::sendmsg(socket.as_raw_fd(), &msg, 0) };
    if n < 0 {
      Err(io::Error::last_os_error())
    } else {
      Ok(())
    }
  }

  #[test]
  fn recv_fd_roundtrip() {
    let (a, b) = UnixStream::pair().unwrap();
    let src = File::open("/dev/null").unwrap();
    send_fd(&b, src.as_raw_fd()).unwrap();
    let received = recv_fd(&a).unwrap();
    assert!(received >= 0);
    let _dup = unsafe { File::from_raw_fd(received) };
    drop(src);
  }

  #[test]
  fn recv_fd_eof() {
    let (a, b) = UnixStream::pair().unwrap();
    drop(b);
    let err = recv_fd(&a).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
  }

  #[test]
  fn cancel_text() {
    assert!(looks_like_cancel("The operation was canceled by the user."));
    assert!(looks_like_cancel("errAuthorizationCanceled (-60006)"));
    assert!(!looks_like_cancel(
      "couldn't open /dev/rdisk4: Resource busy"
    ));
  }

  #[test]
  fn busy_is_not_elevation_failure() {
    let err = classify_failure(Path::new("/dev/rdisk4"), Some(1), "Resource busy");
    match err {
      Error::Message(text) => assert!(text.contains("busy")),
      other => panic!("unexpected {other:?}"),
    }
  }
}
