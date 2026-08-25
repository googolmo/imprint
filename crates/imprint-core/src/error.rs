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

  /// Translate for the GUI. `Display` stays English for logs and the CLI.
  pub fn localized(&self) -> String {
    use crate::i18n::{t, tr};
    match self {
      Self::Message(message) => message.clone(),
      Self::ImageNotFound(path) => {
        let path = path.display().to_string();
        tr("error.image_not_found", &[("path", &path)])
      }
      Self::UnsupportedImage(path) => {
        let path = path.display().to_string();
        tr("error.unsupported_image", &[("path", &path)])
      }
      Self::NoTarget => t("error.no_target"),
      Self::SystemDisk(disk) => tr("error.system_disk", &[("disk", disk)]),
      Self::TargetTooSmall { disk, have, need } => tr(
        "error.target_too_small",
        &[("disk", disk), ("have", have), ("need", need)],
      ),
      Self::Privileges(path) => tr("error.privileges", &[("path", path)]),
      Self::Cancelled => t("error.cancelled"),
      Self::VerifyMismatch {
        offset,
        expected,
        actual,
      } => {
        let offset = offset.to_string();
        let expected = format!("{expected:#04x}");
        let actual = format!("{actual:#04x}");
        tr(
          "error.verify_mismatch",
          &[
            ("offset", &offset),
            ("expected", &expected),
            ("actual", &actual),
          ],
        )
      }
      Self::Io(err) => err.to_string(),
    }
  }
}
