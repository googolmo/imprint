fn main() {
  emit("CARGO_PACKAGER_UPDATER_PUBKEY");
  emit("CARGO_PACKAGER_UPDATER_ENDPOINT");
}

fn emit(name: &str) {
  println!("cargo:rerun-if-env-changed={name}");
  let value = std::env::var(name).unwrap_or_default();
  let value = value.trim();
  println!("cargo:rustc-env={name}={value}");
}
