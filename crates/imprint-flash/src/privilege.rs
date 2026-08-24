/// True when the process can open raw block devices for writing.
pub fn has_block_privileges() -> bool {
  #[cfg(unix)]
  {
    // SAFETY: geteuid is a POSIX query with no preconditions.
    unsafe { libc::geteuid() == 0 }
  }
  #[cfg(windows)]
  {
    // Best-effort: writing to PhysicalDrive will fail with Access Denied if not elevated.
    true
  }
  #[cfg(not(any(unix, windows)))]
  {
    true
  }
}
