//! GPUI views for Imprint. Block-device IO lives in `imprint-flash`.

mod actions;
mod app;
mod rpi;
mod theme;
mod updater;
mod views;
mod widgets;

use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, TextRenderingMode, actions};
use gpui_component::Theme;
use gpui_component::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};
use imprint_core::i18n::{self, t};

use theme::Appearance;

actions!(
  imprint,
  [
    Quit,
    OpenImage,
    SelectTarget,
    StartFlash,
    ToggleSettings,
    About,
    CheckForUpdates,
    OpenRaspberryPi,
    RefreshDrives,
    AppearanceSystem,
    AppearanceLight,
    AppearanceDark,
    CloseAbout
  ]
);

pub use app::{ImprintApp, ImprintShell};

/// Bundle identifier from `Packager.toml` (`identifier`), baked in at compile time.
pub const APP_IDENTIFIER: &str = env!("IMPRINT_APP_IDENTIFIER");
/// Product name from `Packager.toml` (`product-name`), baked in at compile time.
pub const APP_PRODUCT_NAME: &str = env!("IMPRINT_APP_PRODUCT_NAME");

/// Initialize gpui-component and follow the system appearance by default.
pub fn init(cx: &mut App) {
  i18n::init();
  gpui_component::init(cx);
  cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
  Theme::sync_system_appearance(None, cx);
  theme::paint_primary(cx);
  cx.bind_keys([KeyBinding::new("escape", CloseAbout, Some("About"))]);
  install_menus(cx);
}

/// Rebuild the application menu from the active locale.
pub fn install_menus(cx: &mut App) {
  install_menus_with(Appearance::default(), cx);
}

/// Rebuild the application menu, marking the current appearance in View.
pub(crate) fn install_menus_with(appearance: Appearance, cx: &mut App) {
  cx.set_menus([
    Menu::new(t("app.name")).items([
      MenuItem::action(t("menu.about"), About),
      MenuItem::action(t("menu.check_updates"), CheckForUpdates),
      MenuItem::separator(),
      MenuItem::action(t("menu.settings"), ToggleSettings),
      #[cfg(target_os = "macos")]
      MenuItem::separator(),
      #[cfg(target_os = "macos")]
      MenuItem::os_submenu(t("menu.services"), gpui::SystemMenuType::Services),
      MenuItem::separator(),
      MenuItem::action(t("menu.quit"), Quit),
    ]),
    Menu::new(t("menu.file")).items([
      MenuItem::action(t("menu.open_image"), OpenImage),
      MenuItem::action(t("menu.raspberry_pi"), OpenRaspberryPi),
      MenuItem::separator(),
      MenuItem::action(t("menu.select_drive"), SelectTarget),
    ]),
    Menu::new(t("menu.edit")).items([
      MenuItem::os_action(t("menu.undo"), Undo, OsAction::Undo),
      MenuItem::os_action(t("menu.redo"), Redo, OsAction::Redo),
      MenuItem::separator(),
      MenuItem::os_action(t("menu.cut"), Cut, OsAction::Cut),
      MenuItem::os_action(t("menu.copy"), Copy, OsAction::Copy),
      MenuItem::os_action(t("menu.paste"), Paste, OsAction::Paste),
      MenuItem::separator(),
      MenuItem::os_action(t("menu.select_all"), SelectAll, OsAction::SelectAll),
    ]),
    Menu::new(t("menu.view")).items([
      MenuItem::submenu(
        Menu::new(t("settings.appearance")).items([
          MenuItem::action(t("settings.appearance_system"), AppearanceSystem)
            .checked(appearance == Appearance::System),
          MenuItem::action(t("settings.appearance_light"), AppearanceLight)
            .checked(appearance == Appearance::Light),
          MenuItem::action(t("settings.appearance_dark"), AppearanceDark)
            .checked(appearance == Appearance::Dark),
        ]),
      ),
      MenuItem::separator(),
      MenuItem::action(t("menu.refresh_drives"), RefreshDrives),
    ]),
  ]);
}
