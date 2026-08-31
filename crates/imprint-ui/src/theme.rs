use gpui::{App, Background, Hsla, Window, hsla, linear_color_stop, linear_gradient, px, rgb};
use gpui_component::{ActiveTheme as _, Colorize as _, Theme, ThemeMode, ThemeToken, ThemeTokens};

/// Imprint brand sapphire.
pub const PRIMARY: u32 = 0x0E4BEF;

const H_SAPPHIRE: f32 = 0.617;
const H_CYAN: f32 = 0.515;
const H_VIOLET: f32 = 0.76;
const H_MINT: f32 = 0.45;
const H_CORAL: f32 = 0.985;

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

/// Translucent fills and rims used by cards, wells, and floating panels.
#[derive(Clone, Copy, Debug)]
pub struct Glass {
  pub fill: Hsla,
  pub fill_top: Hsla,
  pub fill_hover: Hsla,
  pub panel: Hsla,
  pub panel_top: Hsla,
  pub border: Hsla,
  pub highlight: Hsla,
  pub glow: Hsla,
  pub shadow: Hsla,
}

pub fn glass(cx: &App) -> Glass {
  glass_for(cx.theme().is_dark())
}

fn glass_for(dark: bool) -> Glass {
  if dark {
    Glass {
      fill: hsla(H_VIOLET, 0.30, 0.16, 0.70),
      fill_top: hsla(H_CYAN, 0.36, 0.52, 0.50),
      fill_hover: hsla(H_CYAN, 0.34, 0.48, 0.56),
      panel: hsla(H_VIOLET, 0.40, 0.12, 0.90),
      panel_top: hsla(H_CYAN, 0.32, 0.42, 0.48),
      border: hsla(H_CYAN, 0.42, 0.78, 0.42),
      highlight: hsla(0.0, 0.0, 1.0, 0.22),
      glow: hsla(H_SAPPHIRE, 0.78, 0.48, 0.14),
      shadow: hsla(0.70, 0.48, 0.03, 0.48),
    }
  } else {
    Glass {
      fill: hsla(H_SAPPHIRE, 0.14, 0.99, 0.86),
      fill_top: hsla(0.0, 0.0, 1.0, 0.94),
      fill_hover: hsla(H_SAPPHIRE, 0.18, 0.98, 0.94),
      panel: hsla(H_SAPPHIRE, 0.16, 0.99, 0.94),
      panel_top: hsla(0.0, 0.0, 1.0, 0.96),
      border: hsla(H_SAPPHIRE, 0.32, 0.50, 0.32),
      highlight: hsla(0.0, 0.0, 1.0, 0.55),
      glow: hsla(H_SAPPHIRE, 0.48, 0.52, 0.08),
      shadow: hsla(H_SAPPHIRE, 0.32, 0.22, 0.14),
    }
  }
}

pub fn glass_fill(cx: &App) -> Background {
  let g = glass(cx);
  linear_gradient(
    158.,
    linear_color_stop(g.fill_top, 0.),
    linear_color_stop(g.fill, 1.),
  )
}

pub fn glass_hover_fill(cx: &App) -> Background {
  let g = glass(cx);
  linear_gradient(
    158.,
    linear_color_stop(g.fill_hover, 0.),
    linear_color_stop(g.fill, 1.),
  )
}

/// Denser wash for large overlays (settings sheet) so type stays readable.
pub fn glass_panel_fill(cx: &App) -> Background {
  let g = glass(cx);
  linear_gradient(
    158.,
    linear_color_stop(g.panel_top, 0.),
    linear_color_stop(g.panel, 1.),
  )
}

pub fn apply_appearance(appearance: Appearance, window: Option<&mut Window>, cx: &mut App) {
  match appearance {
    Appearance::System => Theme::sync_system_appearance(window, cx),
    Appearance::Light => Theme::change(ThemeMode::Light, window, cx),
    Appearance::Dark => Theme::change(ThemeMode::Dark, window, cx),
  }
  paint_primary(cx);
}

/// Re-apply brand color and liquid-glass materials after a theme mode switch.
pub fn paint_primary(cx: &mut App) {
  let primary: Hsla = rgb(PRIMARY).into();
  let cyan = hsla(H_CYAN, 0.82, 0.64, 1.0);
  let hover = primary.lighten(0.12);
  let active = primary.darken(0.10);
  let fg: Hsla = hsla(H_SAPPHIRE, 0.10, 0.98, 1.0);
  {
    let theme = Theme::global_mut(cx);
    theme.font_size = px(15.);
    theme.mono_font_size = px(13.);
    theme.primary = primary;
    theme.primary_hover = hover;
    theme.primary_active = active;
    theme.primary_foreground = fg;
    theme.button_primary = primary;
    theme.button_primary_hover = hover;
    theme.button_primary_active = active;
    theme.button_primary_foreground = fg;
    theme.progress_bar = cyan;
    theme.slider_bar = primary;
    theme.sidebar_primary = primary;
    theme.sidebar_primary_foreground = fg;
    paint_glass(theme);
    theme.tokens = ThemeTokens::from(&theme.colors);
    paint_gradients(theme, primary, hover, active, cyan);
  }
  Theme::sync_base(cx);
}

fn paint_glass(theme: &mut Theme) {
  let dark = theme.is_dark();
  let primary = theme.primary;
  let mint = hsla(H_MINT, 0.72, if dark { 0.58 } else { 0.42 }, 1.0);
  let coral = hsla(H_CORAL, 0.80, if dark { 0.62 } else { 0.42 }, 1.0);

  theme.radius = px(14.);
  theme.radius_lg = px(22.);
  theme.shadow = true;

  if dark {
    let ink = hsla(H_SAPPHIRE, 0.52, 0.09, 0.92);
    let cyan = hsla(H_CYAN, 0.82, 0.64, 1.0);
    theme.foreground = hsla(H_SAPPHIRE, 0.18, 0.97, 1.0);
    theme.muted_foreground = hsla(H_SAPPHIRE, 0.28, 0.76, 1.0);
    theme.accent = cyan;
    theme.accent_foreground = hsla(H_SAPPHIRE, 0.40, 0.10, 1.0);
    theme.link = cyan;
    theme.link_hover = cyan.lighten(0.08);
    theme.link_active = cyan.darken(0.08);
    theme.caret = cyan;
    theme.ring = cyan;
    theme.background = ink;
    theme.title_bar = hsla(H_SAPPHIRE, 0.48, 0.11, 0.42);
    theme.title_bar_border = hsla(H_CYAN, 0.32, 0.70, 0.12);
    theme.status_bar = hsla(H_VIOLET, 0.38, 0.08, 0.55);
    theme.status_bar_border = hsla(H_CYAN, 0.32, 0.70, 0.10);
    theme.popover = hsla(H_SAPPHIRE, 0.40, 0.16, 0.98);
    theme.popover_foreground = theme.foreground;
    theme.border = hsla(H_CYAN, 0.42, 0.72, 0.38);
    theme.input = hsla(H_CYAN, 0.30, 0.70, 0.16);
    theme.secondary = hsla(H_SAPPHIRE, 0.35, 0.55, 0.16);
    theme.secondary_hover = hsla(H_CYAN, 0.40, 0.62, 0.22);
    theme.secondary_active = hsla(H_CYAN, 0.42, 0.62, 0.28);
    theme.secondary_foreground = theme.foreground;
    theme.muted = hsla(H_VIOLET, 0.30, 0.40, 0.16);
    theme.colors.list = hsla(H_SAPPHIRE, 0.35, 0.50, 0.10);
    theme.list_hover = hsla(H_CYAN, 0.40, 0.60, 0.16);
    theme.list_active = Hsla { a: 0.28, ..primary };
    theme.list_active_border = hsla(H_CYAN, 0.70, 0.68, 0.55);
    theme.list_even = hsla(H_VIOLET, 0.30, 0.40, 0.08);
    theme.overlay = hsla(H_SAPPHIRE, 0.40, 0.04, 0.52);
    theme.drop_target = hsla(H_CYAN, 0.62, 0.58, 0.16);
    theme.button = hsla(H_SAPPHIRE, 0.40, 0.60, 0.18);
    theme.button_hover = hsla(H_CYAN, 0.45, 0.62, 0.26);
    theme.button_active = hsla(H_CYAN, 0.50, 0.58, 0.32);
    theme.button_foreground = theme.foreground;
    theme.button_secondary = theme.secondary;
    theme.button_secondary_hover = theme.secondary_hover;
    theme.button_secondary_active = theme.secondary_active;
    theme.button_secondary_foreground = theme.foreground;
  } else {
    let pearl = hsla(H_SAPPHIRE, 0.22, 0.98, 0.96);
    let ink_accent = hsla(H_SAPPHIRE, 0.84, 0.32, 1.0);
    theme.foreground = hsla(H_SAPPHIRE, 0.62, 0.08, 1.0);
    theme.muted_foreground = hsla(H_SAPPHIRE, 0.42, 0.26, 1.0);
    theme.accent = ink_accent;
    theme.accent_foreground = hsla(0.0, 0.0, 1.0, 1.0);
    theme.link = ink_accent;
    theme.link_hover = ink_accent.darken(0.08);
    theme.link_active = ink_accent.darken(0.14);
    theme.caret = theme.primary;
    theme.ring = theme.primary;
    theme.background = pearl;
    theme.title_bar = hsla(H_SAPPHIRE, 0.22, 0.98, 0.88);
    theme.title_bar_border = hsla(H_SAPPHIRE, 0.28, 0.55, 0.18);
    theme.status_bar = hsla(H_SAPPHIRE, 0.16, 0.97, 0.92);
    theme.status_bar_border = hsla(H_SAPPHIRE, 0.30, 0.70, 0.14);
    theme.popover = hsla(0.0, 0.0, 1.0, 1.0);
    theme.popover_foreground = theme.foreground;
    theme.border = hsla(H_SAPPHIRE, 0.32, 0.55, 0.34);
    theme.input = hsla(H_SAPPHIRE, 0.25, 0.98, 0.70);
    theme.secondary = hsla(H_SAPPHIRE, 0.35, 0.96, 0.55);
    theme.secondary_hover = hsla(H_CYAN, 0.30, 0.96, 0.70);
    theme.secondary_active = hsla(H_CYAN, 0.32, 0.94, 0.82);
    theme.secondary_foreground = theme.foreground;
    theme.muted = hsla(H_VIOLET, 0.18, 0.95, 0.55);
    theme.colors.list = hsla(H_SAPPHIRE, 0.28, 0.97, 0.40);
    theme.list_hover = hsla(H_CYAN, 0.28, 0.96, 0.55);
    theme.list_active = Hsla { a: 0.14, ..primary };
    theme.list_active_border = hsla(H_SAPPHIRE, 0.65, 0.52, 0.40);
    theme.list_even = hsla(H_VIOLET, 0.18, 0.97, 0.28);
    theme.overlay = hsla(H_SAPPHIRE, 0.30, 0.20, 0.28);
    theme.drop_target = hsla(H_CYAN, 0.42, 0.72, 0.14);
    theme.button = hsla(H_SAPPHIRE, 0.28, 0.97, 0.70);
    theme.button_hover = hsla(H_CYAN, 0.28, 0.96, 0.84);
    theme.button_active = hsla(H_CYAN, 0.30, 0.94, 0.92);
    theme.button_foreground = theme.foreground;
    theme.button_secondary = theme.secondary;
    theme.button_secondary_hover = theme.secondary_hover;
    theme.button_secondary_active = theme.secondary_active;
    theme.button_secondary_foreground = theme.foreground;
  }

  theme.success = mint;
  theme.success_hover = mint.lighten(0.08);
  theme.success_active = mint.darken(0.08);
  theme.success_foreground = if dark {
    hsla(H_SAPPHIRE, 0.40, 0.08, 1.0)
  } else {
    hsla(0.0, 0.0, 1.0, 1.0)
  };
  theme.danger = coral;
  theme.danger_hover = coral.lighten(0.08);
  theme.danger_active = coral.darken(0.08);
  theme.danger_foreground = hsla(0.0, 0.0, 1.0, 1.0);
  theme.button_success = theme.success;
  theme.button_success_hover = theme.success_hover;
  theme.button_success_active = theme.success_active;
  theme.button_success_foreground = theme.success_foreground;
  theme.button_danger = theme.danger;
  theme.button_danger_hover = theme.danger_hover;
  theme.button_danger_active = theme.danger_active;
  theme.button_danger_foreground = theme.danger_foreground;
  theme.accordion = theme.secondary;
  theme.group_box = theme.secondary;
  theme.group_box_foreground = theme.foreground;
}

fn paint_gradients(theme: &mut Theme, primary: Hsla, hover: Hsla, active: Hsla, cyan: Hsla) {
  let dark = theme.is_dark();
  theme.tokens.background = ThemeToken::new(
    theme.background,
    if dark {
      linear_gradient(
        148.,
        linear_color_stop(hsla(H_SAPPHIRE, 0.55, 0.16, 0.94), 0.),
        linear_color_stop(hsla(H_VIOLET, 0.48, 0.07, 0.96), 1.),
      )
    } else {
      linear_gradient(
        148.,
        linear_color_stop(hsla(H_SAPPHIRE, 0.18, 0.98, 0.97), 0.),
        linear_color_stop(hsla(H_VIOLET, 0.12, 0.97, 0.97), 1.),
      )
    },
  );
  theme.tokens.button_primary = ThemeToken::new(
    primary,
    linear_gradient(
      128.,
      linear_color_stop(primary, 0.),
      linear_color_stop(cyan, 1.),
    ),
  );
  theme.tokens.button_primary_hover = ThemeToken::new(
    hover,
    linear_gradient(
      128.,
      linear_color_stop(hover, 0.),
      linear_color_stop(cyan.lighten(0.10), 1.),
    ),
  );
  theme.tokens.button_primary_active = ThemeToken::new(
    active,
    linear_gradient(
      128.,
      linear_color_stop(active, 0.),
      linear_color_stop(cyan.darken(0.06), 1.),
    ),
  );
  theme.tokens.progress_bar = ThemeToken::new(
    cyan,
    linear_gradient(
      90.,
      linear_color_stop(primary, 0.),
      linear_color_stop(cyan, 1.),
    ),
  );
  let g = glass_for(dark);
  theme.tokens.tab_bar_segmented = ThemeToken::new(
    g.fill,
    linear_gradient(
      158.,
      linear_color_stop(g.fill_top, 0.),
      linear_color_stop(g.fill, 1.),
    ),
  );
}
