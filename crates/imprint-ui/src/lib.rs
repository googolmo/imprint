//! GPUI views for Imprint. Block-device IO lives in `imprint-flash`.

mod actions;
mod app;
mod theme;
mod widgets;

use gpui::{App, actions};
use gpui_component::Theme;

actions!(
  imprint,
  [Quit, OpenImage, SelectTarget, StartFlash, ToggleSettings]
);

pub use app::{ImprintApp, ImprintShell};

/// Initialize gpui-component and follow the system appearance by default.
pub fn init(cx: &mut App) {
  gpui_component::init(cx);
  Theme::sync_system_appearance(None, cx);
  theme::paint_primary(cx);
}
