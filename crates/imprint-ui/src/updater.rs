//! App updates from GitHub Releases via `cargo-packager-updater`.
//!
//! Bake these in at compile time:
//! - `CARGO_PACKAGER_UPDATER_PUBKEY` — minisign public key from `cargo packager signer generate`
//! - `CARGO_PACKAGER_UPDATER_ENDPOINT` — one or more URLs that serve `latest.json`
//!   (comma, semicolon, or whitespace separated). Later URLs are fallbacks.
//!   Typical GitHub Releases URL:
//!   `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`
//!
//! The private key is a secret (`CARGO_PACKAGER_SIGN_PRIVATE_KEY`) used only
//! when packaging. See `.github/scripts/prepare-updater-assets.py`.

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use cargo_packager_updater::http::header::USER_AGENT;
use cargo_packager_updater::{
  Config, Update, UpdaterBuilder, WindowsConfig, WindowsUpdateInstallMode, url::Url,
};

/// Public key from `CARGO_PACKAGER_UPDATER_PUBKEY` at compile time.
pub const UPDATER_PUBKEY: &str = env!("CARGO_PACKAGER_UPDATER_PUBKEY");

/// Update manifest URL(s) from `CARGO_PACKAGER_UPDATER_ENDPOINT` at compile time.
/// Multiple addresses may be separated by commas, semicolons, or whitespace.
pub const UPDATE_ENDPOINT: &str = env!("CARGO_PACKAGER_UPDATER_ENDPOINT");

const USER_AGENT_VALUE: &str = concat!("imprint/", env!("CARGO_PKG_VERSION"));

fn endpoint_tokens(raw: &str) -> impl Iterator<Item = &str> {
  raw
    .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
    .filter(|part| !part.is_empty())
}

pub fn parse_endpoints(raw: &str) -> Result<Vec<Url>, String> {
  let mut urls = Vec::new();
  for (i, part) in endpoint_tokens(raw).enumerate() {
    urls.push(Url::parse(part).map_err(|err| {
      format!(
        "invalid CARGO_PACKAGER_UPDATER_ENDPOINT entry {}: {part}: {err}",
        i + 1
      )
    })?);
  }
  if urls.is_empty() {
    return Err("CARGO_PACKAGER_UPDATER_ENDPOINT is empty".into());
  }
  Ok(urls)
}

pub fn updater_config() -> Result<Config, String> {
  Ok(Config {
    endpoints: parse_endpoints(UPDATE_ENDPOINT)?,
    pubkey: UPDATER_PUBKEY.into(),
    windows: Some(WindowsConfig {
      install_mode: Some(WindowsUpdateInstallMode::Passive),
      installer_args: None,
    }),
  })
}

pub fn is_configured() -> bool {
  !UPDATER_PUBKEY.is_empty() && endpoint_tokens(UPDATE_ENDPOINT).next().is_some()
}

#[cfg(test)]
pub fn endpoint_urls() -> Result<Vec<Url>, String> {
  parse_endpoints(UPDATE_ENDPOINT)
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
    path.contains(".app/Contents/MacOS/")
  }

  #[cfg(any(target_os = "linux", target_os = "freebsd"))]
  {
    env::var_os("APPIMAGE").is_some()
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
      "updater is not configured (set CARGO_PACKAGER_UPDATER_PUBKEY and CARGO_PACKAGER_UPDATER_ENDPOINT at build)"
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

/// Directory for a one-shot “updated to …” notice after relaunch.
fn notice_dir() -> Option<PathBuf> {
  #[cfg(target_os = "macos")]
  {
    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/imprint"))
  }

  #[cfg(windows)]
  {
    let appdata = env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("imprint"))
  }

  #[cfg(not(any(target_os = "macos", windows)))]
  {
    let base = env::var_os("XDG_DATA_HOME")
      .map(PathBuf::from)
      .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
    Some(base.join("imprint"))
  }
}

fn notice_path() -> Option<PathBuf> {
  Some(notice_dir()?.join("updated-to"))
}

/// Remember the version that was just installed so the next launch can toast.
pub fn mark_update_installed(version: &str) {
  let Some(path) = notice_path() else {
    return;
  };
  if let Some(parent) = path.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  let _ = std::fs::write(path, version);
}

/// Consume the pending post-update notice, if any.
pub fn take_update_notice() -> Option<String> {
  let path = notice_path()?;
  let version = std::fs::read_to_string(&path).ok()?;
  let _ = std::fs::remove_file(&path);
  let version = version.trim().to_string();
  if version.is_empty() {
    None
  } else {
    Some(version)
  }
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
    Ok(())
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
  fn parse_endpoints_accepts_one_or_more_urls() {
    let one = parse_endpoints("https://example.com/latest.json").expect("one");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].as_str(), "https://example.com/latest.json");

    let many = parse_endpoints(
      "https://cdn.example.com/latest.json, https://github.com/o/r/releases/latest/download/latest.json",
    )
    .expect("many");
    assert_eq!(many.len(), 2);
    assert_eq!(many[0].as_str(), "https://cdn.example.com/latest.json");
    assert_eq!(
      many[1].as_str(),
      "https://github.com/o/r/releases/latest/download/latest.json"
    );

    let mixed = parse_endpoints(
      "https://a.example/latest.json;\nhttps://b.example/latest.json https://c.example/latest.json,",
    )
    .expect("mixed");
    assert_eq!(mixed.len(), 3);
  }

  #[test]
  fn parse_endpoints_rejects_empty_and_invalid() {
    assert!(parse_endpoints("").is_err());
    assert!(parse_endpoints("  , ; \n").is_err());
    assert!(parse_endpoints("not-a-url").is_err());
    assert!(parse_endpoints("https://ok.example/latest.json, nope").is_err());
  }

  #[test]
  fn baked_endpoint_parses_when_set() {
    if endpoint_tokens(UPDATE_ENDPOINT).next().is_none() {
      assert!(!is_configured());
      return;
    }
    let urls = endpoint_urls().expect("CARGO_PACKAGER_UPDATER_ENDPOINT must be valid URL(s)");
    let expected: Vec<_> = endpoint_tokens(UPDATE_ENDPOINT).collect();
    assert_eq!(urls.len(), expected.len());
    for (url, raw) in urls.iter().zip(expected) {
      assert_eq!(url.as_str(), raw);
    }
    let config = updater_config().expect("updater config");
    assert_eq!(config.pubkey, UPDATER_PUBKEY);
    assert_eq!(config.endpoints.len(), urls.len());
  }

  #[test]
  fn cargo_test_binary_is_not_packaged() {
    assert!(
      !is_packaged(),
      "unit tests run from target/ and must not look packaged"
    );
  }
}
