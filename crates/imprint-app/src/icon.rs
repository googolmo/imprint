//! OS application icon for Dock, app switcher, and taskbar.

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/icon/AppIcon.png");

/// Apply the icon macOS uses in the Dock and Cmd+Tab switcher.
///
/// `cargo run` is not an `.app` bundle, so the bundled `.icns` is ignored until
/// packaging; this sets `NSApplication.applicationIconImage` at runtime.
#[cfg(target_os = "macos")]
pub fn apply_app_icon() {
  use objc2::MainThreadMarker;
  use objc2_app_kit::{NSApplication, NSImage};
  use objc2_foundation::NSData;

  const PNG: &[u8] = include_bytes!("../../../assets/icon/AppIcon-macos.png");

  let Some(mtm) = MainThreadMarker::new() else {
    return;
  };
  let data = NSData::with_bytes(PNG);
  let Some(image) = NSImage::initWithData(mtm.alloc::<NSImage>(), &data) else {
    return;
  };
  let app = NSApplication::sharedApplication(mtm);
  // Copied by AppKit; `None` is allowed by the selector.
  unsafe { app.setApplicationIconImage(Some(&image)) };
}

#[cfg(not(target_os = "macos"))]
pub fn apply_app_icon() {}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
  image::load_from_memory(APP_ICON_PNG)
    .ok()
    .map(|img| std::sync::Arc::new(img.to_rgba8()))
}
