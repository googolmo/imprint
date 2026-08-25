//! GPUI views for Imprint. Block-device IO lives in `imprint-flash`.

mod actions;
mod app;
mod theme;
mod widgets;

use gpui::{App, Menu, MenuItem, TextRenderingMode, actions};
use gpui_component::Theme;
use imprint_core::i18n::{self, t};

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
  i18n::init();
  gpui_component::init(cx);
  cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
  Theme::sync_system_appearance(None, cx);
  theme::paint_primary(cx);
  install_menus(cx);
}

/// Rebuild the application menu from the active locale.
pub fn install_menus(cx: &mut App) {
  cx.set_menus([Menu::new(t("app.name")).items([
    MenuItem::action(t("menu.about"), About),
    MenuItem::separator(),
    MenuItem::action(t("menu.settings"), ToggleSettings),
    MenuItem::separator(),
    MenuItem::action(t("menu.open_image"), OpenImage),
    MenuItem::separator(),
    MenuItem::action(t("menu.quit"), Quit),
  ])]);
}
