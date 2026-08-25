//! GPUI views for Imprint. Block-device IO lives in `imprint-flash`.

mod actions;
mod app;
mod theme;
mod widgets;

use gpui::{App, TextRenderingMode, actions};
use gpui_component::Theme;

actions!(
  imprint,
  [
    Quit,
    OpenImage,
    SelectTarget,
    StartFlash,
    ToggleSettings,
    About
  ]
);

pub use app::{ImprintApp, ImprintShell};

/// Initialize gpui-component and follow the system appearance by default.
pub fn init(cx: &mut App) {
  gpui_component::init(cx);
  cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
  Theme::sync_system_appearance(None, cx);
  theme::paint_primary(cx);
}
