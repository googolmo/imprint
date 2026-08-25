use gpui::{App, Hsla, Window, px, rgb};
use gpui_component::{Colorize as _, Theme, ThemeMode, ThemeTokens};

/// Imprint brand blue — used as the gpui-component primary token.
pub const PRIMARY: u32 = 0x0E4BEF;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
  #[default]
  System,
  Light,
  Dark,
}

impl Appearance {
  pub fn as_index(self) -> usize {
    match self {
      Self::System => 0,
      Self::Light => 1,
      Self::Dark => 2,
    }
  }

  pub fn from_index(index: usize) -> Self {
    match index {
      1 => Self::Light,
      2 => Self::Dark,
      _ => Self::System,
    }
  }
}

pub fn apply_appearance(appearance: Appearance, window: Option<&mut Window>, cx: &mut App) {
  match appearance {
    Appearance::System => Theme::sync_system_appearance(window, cx),
    Appearance::Light => Theme::change(ThemeMode::Light, window, cx),
    Appearance::Dark => Theme::change(ThemeMode::Dark, window, cx),
  }
  paint_primary(cx);
}

/// Re-apply brand color and Zed-like density after a theme mode switch.
pub fn paint_primary(cx: &mut App) {
  let primary: Hsla = rgb(PRIMARY).into();
  let hover = primary.lighten(0.1);
  let active = primary.darken(0.12);
  let fg: Hsla = rgb(0xFFFFFF).into();
  {
    let theme = Theme::global_mut(cx);
    theme.font_size = px(14.);
    theme.mono_font_size = px(12.);
    theme.radius = px(4.);
    theme.radius_lg = px(6.);
    theme.primary = primary;
    theme.primary_hover = hover;
    theme.primary_active = active;
    theme.primary_foreground = fg;
    theme.button_primary = primary;
    theme.button_primary_hover = hover;
    theme.button_primary_active = active;
    theme.button_primary_foreground = fg;
    theme.accent = primary;
    theme.accent_foreground = fg;
    theme.progress_bar = primary;
    theme.ring = primary;
    theme.link = primary;
    theme.link_hover = hover;
    theme.link_active = active;
    theme.caret = primary;
    theme.slider_bar = primary;
    theme.sidebar_primary = primary;
    theme.sidebar_primary_foreground = fg;
    theme.tokens = ThemeTokens::from(&theme.colors);
  }
  Theme::sync_base(cx);
}
