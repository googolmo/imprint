fn main() {
  emit("IMPRINT_UPDATER_PUBKEY");
  emit("IMPRINT_UPDATER_ENDPOINT");
}

fn emit(name: &str) {
  println!("cargo:rerun-if-env-changed={name}");
  let value = std::env::var(name).unwrap_or_default();
  let value = value.trim();
  println!("cargo:rustc-env={name}={value}");
}
