fn main() {
  println!("cargo:rerun-if-changed=../../assets/icon/AppIcon.ico");
  println!("cargo:rerun-if-changed=../../assets/icon/AppIcon.icns");
  println!("cargo:rerun-if-changed=../../assets/icon/AppIcon.png");

  #[cfg(target_os = "windows")]
  embed_windows_icon();
}

#[cfg(target_os = "windows")]
fn embed_windows_icon() {
  let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
  let icon = manifest_dir.join("../../assets/icon/AppIcon.ico");
  let icon = std::fs::canonicalize(&icon).unwrap_or(icon);
  let icon_escaped = icon
    .display()
    .to_string()
    .replace('\\', "\\\\")
    .replace('"', "\\\"");
  let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
  let rc_path = out_dir.join("app_icon.rc");
  std::fs::write(&rc_path, format!("1 ICON \"{icon_escaped}\"\n")).unwrap();
  embed_resource::compile(&rc_path, embed_resource::NONE)
    .manifest_optional()
    .ok();
}
