/// True when the process can open raw block devices for writing.
pub fn has_block_privileges() -> bool {
  #[cfg(unix)]
  {
    // SAFETY: geteuid is a POSIX query with no preconditions.
    unsafe { libc::geteuid() == 0 }
  }
  #[cfg(windows)]
  {
    windows_is_elevated()
  }
  #[cfg(not(any(unix, windows)))]
  {
    true
  }
}

#[cfg(windows)]
fn windows_is_elevated() -> bool {
  use std::mem::{size_of, zeroed};
  use std::ptr;

  #[repr(C)]
  struct TokenElevation {
    token_is_elevated: u32,
  }

  const TOKEN_QUERY: u32 = 0x0008;
  const TOKEN_ELEVATION: u32 = 20;

  #[link(name = "advapi32")]
  unsafe extern "system" {
    fn OpenProcessToken(
      process: *mut core::ffi::c_void,
      access: u32,
      token: *mut *mut core::ffi::c_void,
    ) -> i32;
    fn GetTokenInformation(
      token: *mut core::ffi::c_void,
      class: u32,
      info: *mut core::ffi::c_void,
      len: u32,
      ret_len: *mut u32,
    ) -> i32;
  }
  #[link(name = "kernel32")]
  unsafe extern "system" {
    fn GetCurrentProcess() -> *mut core::ffi::c_void;
    fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
  }

  // SAFETY: TokenElevation is a DWORD query on the current process token.
  unsafe {
    let mut token = ptr::null_mut();
    if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
      return false;
    }
    let mut elevation: TokenElevation = zeroed();
    let mut size = 0u32;
    let ok = GetTokenInformation(
      token,
      TOKEN_ELEVATION,
      &mut elevation as *mut TokenElevation as *mut core::ffi::c_void,
      size_of::<TokenElevation>() as u32,
      &mut size,
    );
    CloseHandle(token);
    ok != 0 && elevation.token_is_elevated != 0
  }
}
