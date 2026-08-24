use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("{0}")]
  Message(String),

  #[error("image not found: {0}")]
  ImageNotFound(PathBuf),

  #[error("unsupported image: {0}")]
  UnsupportedImage(PathBuf),

  #[error("no removable target selected")]
  NoTarget,

  #[error("refusing to write to system disk {0}")]
  SystemDisk(String),

  #[error("target {disk} is too small ({have}); image needs {need}")]
  TargetTooSmall {
    disk: String,
    have: String,
    need: String,
  },

  #[error("need administrator / root privileges to write to {0}")]
  Privileges(String),

  #[error("flash cancelled")]
  Cancelled,

  #[error("verification failed at offset {offset}: wrote {expected:#04x}, read {actual:#04x}")]
  VerifyMismatch {
    offset: u64,
    expected: u8,
    actual: u8,
  },

  #[error(transparent)]
  Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
  pub fn msg(text: impl Into<String>) -> Self {
    Self::Message(text.into())
  }
}
