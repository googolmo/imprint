//! App updates from GitHub Releases via `cargo-packager-updater`.
//!
//! Bake these in at compile time:
//! - `IMPRINT_UPDATER_PUBKEY` — minisign public key from `cargo packager signer generate`
//! - `IMPRINT_UPDATER_ENDPOINT` — URL that serves `latest.json` (typically
//!   `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`)
//!
//! The private key is a secret (`CARGO_PACKAGER_SIGN_PRIVATE_KEY`) used only
//! when packaging. See `scripts/prepare-updater-assets.sh`.

use std::env;
use std::process::Command;
use std::time::Duration;

use cargo_packager_updater::http::header::USER_AGENT;
use cargo_packager_updater::{
  Config, Update, UpdaterBuilder, WindowsConfig, WindowsUpdateInstallMode, url::Url,
};

/// Public key from `IMPRINT_UPDATER_PUBKEY` at compile time.
pub const UPDATER_PUBKEY: &str = env!("IMPRINT_UPDATER_PUBKEY");

/// Update manifest URL from `IMPRINT_UPDATER_ENDPOINT` at compile time.
pub const UPDATE_ENDPOINT: &str = env!("IMPRINT_UPDATER_ENDPOINT");

const USER_AGENT_VALUE: &str = concat!("imprint/", env!("CARGO_PKG_VERSION"));

pub fn updater_config() -> Result<Config, String> {
  Ok(Config {
    endpoints: vec![endpoint_url()?],
    pubkey: UPDATER_PUBKEY.into(),
    windows: Some(WindowsConfig {
      install_mode: Some(WindowsUpdateInstallMode::Passive),
      installer_args: None,
    }),
  })
}

pub fn is_configured() -> bool {
  !UPDATER_PUBKEY.is_empty() && !UPDATE_ENDPOINT.is_empty()
}

pub fn endpoint_url() -> Result<Url, String> {
  Url::parse(UPDATE_ENDPOINT).map_err(|err| format!("invalid IMPRINT_UPDATER_ENDPOINT: {err}"))
}

/// True when this process is a packaged install (`.app`, AppImage, installer).
/// Dev binaries under `target/` are not updated in place.
pub fn is_packaged() -> bool {
  let Ok(exe) = env::current_exe() else {
    return false;
  };
  let path = exe.to_string_lossy();
  if path.contains("/target/debug/")
    || path.contains("/target/release/")
    || path.contains("\\target\\debug\\")
    || path.contains("\\target\\release\\")
    || path.contains("/target/debug\\")
    || path.contains("/target/release\\")
  {
    return false;
  }

  #[cfg(target_os = "macos")]
  {
    return path.contains(".app/Contents/MacOS/");
  }

  #[cfg(any(target_os = "linux", target_os = "freebsd"))]
  {
    return env::var_os("APPIMAGE").is_some();
  }

  #[cfg(windows)]
  {
    true
  }

  #[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    windows
  )))]
  {
    false
  }
}

pub fn check_for_update() -> Result<Option<Update>, String> {
  if !is_configured() {
    return Err(
      "updater is not configured (set IMPRINT_UPDATER_PUBKEY and IMPRINT_UPDATER_ENDPOINT at build)"
        .into(),
    );
  }
  tracing::info!("checking for updates at {UPDATE_ENDPOINT}");
  let version = cargo_packager_updater::semver::Version::parse(env!("CARGO_PKG_VERSION"))
    .map_err(|err| err.to_string())?;
  let updater = UpdaterBuilder::new(version, updater_config()?)
    .timeout(Duration::from_secs(60))
    .header(USER_AGENT, USER_AGENT_VALUE)
    .map_err(|err| err.to_string())?
    .build()
    .map_err(|err| err.to_string())?;
  updater.check().map_err(|err| err.to_string())
}

pub fn relaunch() -> Result<(), String> {
  let exe = env::current_exe().map_err(|err| err.to_string())?;

  #[cfg(target_os = "macos")]
  {
    let app = exe
      .ancestors()
      .find(|path| path.extension().is_some_and(|ext| ext == "app"))
      .ok_or_else(|| "not running from an .app bundle".to_string())?;
    Command::new("open")
      .arg("-n")
      .arg(app)
      .spawn()
      .map_err(|err| err.to_string())?;
    return Ok(());
  }

  #[cfg(not(target_os = "macos"))]
  {
    let program = env::var_os("APPIMAGE")
      .map(std::path::PathBuf::from)
      .unwrap_or(exe);
    Command::new(program)
      .spawn()
      .map_err(|err| err.to_string())?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn baked_endpoint_parses_when_set() {
    if UPDATE_ENDPOINT.is_empty() {
      assert!(!is_configured());
      return;
    }
    let url = endpoint_url().expect("IMPRINT_UPDATER_ENDPOINT must be a valid URL");
    assert_eq!(url.as_str(), UPDATE_ENDPOINT);
    let config = updater_config().expect("updater config");
    assert_eq!(config.pubkey, UPDATER_PUBKEY);
    assert_eq!(config.endpoints.len(), 1);
  }

  #[test]
  fn cargo_test_binary_is_not_packaged() {
    assert!(
      !is_packaged(),
      "unit tests run from target/ and must not look packaged"
    );
  }
}
