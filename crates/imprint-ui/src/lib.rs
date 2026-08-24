//! GPUI views for Imprint. Block-device IO lives in `imprint-flash`.

mod actions;
mod app;
mod theme;
mod widgets;

use gpui::actions;

actions!(
  imprint,
  [Quit, OpenImage, SelectTarget, StartFlash, ToggleSettings]
);

pub use app::ImprintApp;
