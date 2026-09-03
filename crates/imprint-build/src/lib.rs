//! Helpers for Imprint `build.rs` files. Bakes `Packager.toml` identity into `env!()`.

use std::path::{Path, PathBuf};

/// Read `identifier` and `product-name` from the workspace `Packager.toml`
/// and emit `IMPRINT_APP_IDENTIFIER` / `IMPRINT_APP_PRODUCT_NAME` for `env!()`.
///
/// `CARGO_MANIFEST_DIR` is the crate whose `build.rs` called this (app or ui).
pub fn emit_packager_identity() {
  let packager =
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../Packager.toml");
  println!("cargo:rerun-if-changed={}", packager.display());
  let text = std::fs::read_to_string(&packager)
    .unwrap_or_else(|err| panic!("failed to read {}: {err}", packager.display()));
  println!(
    "cargo:rustc-env=IMPRINT_APP_IDENTIFIER={}",
    packager_field(&text, "identifier", &packager)
  );
  println!(
    "cargo:rustc-env=IMPRINT_APP_PRODUCT_NAME={}",
    packager_field(&text, "product-name", &packager)
  );
}

fn packager_field(text: &str, key: &str, path: &Path) -> String {
  for raw in text.lines() {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
      continue;
    }
    let Some((k, v)) = line.split_once('=') else {
      continue;
    };
    if k.trim() != key {
      continue;
    }
    let v = v.trim();
    let v = v
      .strip_prefix('"')
      .and_then(|s| s.strip_suffix('"'))
      .unwrap_or(v);
    if v.is_empty() {
      panic!("`{key}` in {} is empty", path.display());
    }
    return v.to_string();
  }
  panic!("missing `{key}` in {}", path.display());
}
