use gpui::{App, Background, Hsla, Window, linear_color_stop, linear_gradient, px, rgb};
use gpui_component::{ActiveTheme as _, Colorize as _, Theme, ThemeMode, ThemeToken, ThemeTokens};

/// Raspberry Pi brand red.
pub const RASPBERRY: u32 = 0xC51A4A;

/// Official Catppuccin palette: https://github.com/catppuccin/catppuccin
#[derive(Clone, Copy)]
struct Flavor {
  rosewater: Hsla,
  mauve: Hsla,
  red: Hsla,
  maroon: Hsla,
  peach: Hsla,
  yellow: Hsla,
  green: Hsla,
  teal: Hsla,
  sky: Hsla,
  sapphire: Hsla,
  blue: Hsla,
  lavender: Hsla,
  pink: Hsla,
  text: Hsla,
  subtext1: Hsla,
  subtext0: Hsla,
  overlay2: Hsla,
  overlay1: Hsla,
  overlay0: Hsla,
  surface2: Hsla,
  surface1: Hsla,
  surface0: Hsla,
  base: Hsla,
  mantle: Hsla,
  crust: Hsla,
}

fn hex(color: u32) -> Hsla {
  rgb(color).into()
}

/// Catppuccin Latte — light.
fn latte() -> Flavor {
  Flavor {
    rosewater: hex(0xdc8a78),
    mauve: hex(0x8839ef),
    red: hex(0xd20f39),
    maroon: hex(0xe64553),
    peach: hex(0xfe640b),
    yellow: hex(0xdf8e1d),
    green: hex(0x40a02b),
    teal: hex(0x179299),
    sky: hex(0x04a5e5),
    sapphire: hex(0x209fb5),
    blue: hex(0x1e66f5),
    lavender: hex(0x7287fd),
    pink: hex(0xea76cb),
    text: hex(0x4c4f69),
    subtext1: hex(0x5c5f77),
    subtext0: hex(0x6c6f85),
    overlay2: hex(0x7c7f93),
    overlay1: hex(0x8c8fa1),
    overlay0: hex(0x9ca0b0),
    surface2: hex(0xacb0be),
    surface1: hex(0xbcc0cc),
    surface0: hex(0xccd0da),
    base: hex(0xeff1f5),
    mantle: hex(0xe6e9ef),
    crust: hex(0xdce0e8),
  }
}

/// Catppuccin Mocha — the original dark flavor.
fn mocha() -> Flavor {
  Flavor {
    rosewater: hex(0xf5e0dc),
    mauve: hex(0xcba6f7),
    red: hex(0xf38ba8),
    maroon: hex(0xeba0ac),
    peach: hex(0xfab387),
    yellow: hex(0xf9e2af),
    green: hex(0xa6e3a1),
    teal: hex(0x94e2d5),
    sky: hex(0x89dceb),
    sapphire: hex(0x74c7ec),
    blue: hex(0x89b4fa),
    lavender: hex(0xb4befe),
    pink: hex(0xf5c2e7),
    text: hex(0xcdd6f4),
    subtext1: hex(0xbac2de),
    subtext0: hex(0xa6adc8),
    overlay2: hex(0x9399b2),
    overlay1: hex(0x7f849c),
    overlay0: hex(0x6c7086),
    surface2: hex(0x585b70),
    surface1: hex(0x45475a),
    surface0: hex(0x313244),
    base: hex(0x1e1e2e),
    mantle: hex(0x181825),
    crust: hex(0x11111b),
  }
}

fn flavor(dark: bool) -> Flavor {
  if dark { mocha() } else { latte() }
}

/// Text on a saturated accent: crust on Mocha pastels, base on Latte inks.
fn on_accent(dark: bool, p: Flavor) -> Hsla {
  if dark { p.crust } else { p.base }
}

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
  let p = flavor(dark);
  if dark {
    Glass {
      fill: p.surface0.opacity(0.72),
      fill_top: p.overlay0.opacity(0.38),
      fill_hover: p.surface1.opacity(0.78),
      panel: p.mantle.opacity(0.92),
      panel_top: p.surface0.opacity(0.55),
      border: p.overlay0.opacity(0.42),
      highlight: p.text.opacity(0.16),
      glow: p.mauve.opacity(0.14),
      shadow: p.crust.opacity(0.55),
    }
  } else {
    Glass {
      fill: p.base.opacity(0.88),
      fill_top: hex(0xffffff).opacity(0.92),
      fill_hover: p.mantle.opacity(0.94),
      panel: p.mantle.opacity(0.96),
      panel_top: hex(0xffffff).opacity(0.94),
      border: p.overlay0.opacity(0.48),
      highlight: hex(0xffffff).opacity(0.55),
      glow: p.lavender.opacity(0.10),
      shadow: p.overlay1.opacity(0.22),
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

pub fn raspberry_pi(cx: &App) -> Hsla {
  let color: Hsla = rgb(RASPBERRY).into();
  if cx.theme().is_dark() {
    color.lighten(0.22)
  } else {
    color
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

/// Re-apply Catppuccin colors and glass materials after a theme mode switch.
pub fn paint_primary(cx: &mut App) {
  let dark = Theme::global(cx).is_dark();
  let p = flavor(dark);
  let primary = p.blue;
  let hover = if dark { p.sapphire } else { p.lavender };
  let active = if dark {
    p.lavender
  } else {
    p.blue.darken(0.10)
  };
  let fg = on_accent(dark, p);
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
    theme.progress_bar = p.sapphire;
    theme.slider_bar = primary;
    theme.sidebar_primary = primary;
    theme.sidebar_primary_foreground = fg;
    paint_glass(theme, p);
    theme.tokens = ThemeTokens::from(&theme.colors);
    paint_gradients(theme, p, primary, hover, active);
  }
  Theme::sync_base(cx);
}

fn paint_glass(theme: &mut Theme, p: Flavor) {
  let dark = theme.is_dark();
  let on = on_accent(dark, p);

  theme.radius = px(8.);
  theme.radius_lg = px(12.);
  theme.shadow = true;

  theme.foreground = p.text;
  theme.muted_foreground = p.subtext1;
  theme.accent = p.mauve;
  theme.accent_foreground = on;
  theme.link = p.blue;
  theme.link_hover = p.sapphire;
  theme.link_active = p.lavender;
  theme.caret = p.rosewater;
  theme.ring = p.lavender;
  theme.background = p.base;
  theme.popover = if dark { p.mantle } else { p.base };
  theme.popover_foreground = p.text;
  theme.border = p.surface0;
  theme.window_border = p.surface1;

  theme.red = p.red;
  theme.red_light = p.maroon;
  theme.green = p.green;
  theme.green_light = p.teal;
  theme.blue = p.blue;
  theme.blue_light = p.sapphire;
  theme.yellow = p.yellow;
  theme.yellow_light = p.peach;
  theme.magenta = p.mauve;
  theme.magenta_light = p.pink;
  theme.cyan = p.sapphire;
  theme.cyan_light = p.sky;

  theme.chart_1 = p.blue;
  theme.chart_2 = p.mauve;
  theme.chart_3 = p.green;
  theme.chart_4 = p.peach;
  theme.chart_5 = p.pink;
  theme.chart_bullish = p.green;
  theme.chart_bearish = p.red;

  if dark {
    theme.title_bar = p.mantle;
    theme.title_bar_border = p.surface0;
    theme.status_bar = p.crust;
    theme.status_bar_border = p.surface0;
    theme.input = p.surface0.opacity(0.70);
    theme.secondary = p.surface0.opacity(0.70);
    theme.secondary_hover = p.surface1.opacity(0.80);
    theme.secondary_active = p.surface2.opacity(0.85);
    theme.secondary_foreground = p.text;
    theme.muted = p.surface0.opacity(0.55);
    theme.colors.list = p.surface0.opacity(0.35);
    theme.list_hover = p.surface1.opacity(0.55);
    theme.list_active = p.lavender.opacity(0.18);
    theme.list_active_border = p.lavender.opacity(0.55);
    theme.list_even = p.mantle.opacity(0.55);
    theme.list_head = p.mantle;
    theme.overlay = p.crust.opacity(0.58);
    theme.drop_target = p.blue.opacity(0.16);
    theme.button = p.surface0.opacity(0.70);
    theme.button_hover = p.surface1.opacity(0.85);
    theme.button_active = p.surface2.opacity(0.90);
    theme.button_foreground = p.text;
    theme.sidebar = p.mantle;
    theme.sidebar_foreground = p.text;
    theme.sidebar_border = p.surface0;
    theme.sidebar_accent = p.surface0;
    theme.sidebar_accent_foreground = p.text;
    theme.tab_bar = p.crust;
    theme.tab = p.crust.opacity(0.0);
    theme.tab_foreground = p.subtext0;
    theme.tab_active = p.base;
    theme.tab_active_foreground = p.text;
    theme.scrollbar = p.base.opacity(0.0);
    theme.scrollbar_thumb = p.overlay0;
    theme.scrollbar_thumb_hover = p.overlay1;
    theme.selection = p.overlay2.opacity(0.28);
    theme.switch = p.surface1;
    theme.skeleton = p.surface0;
    theme.tiles = p.mantle;
    theme.table = p.base;
    theme.table_even = p.mantle;
    theme.table_hover = p.surface0.opacity(0.55);
    theme.table_head = p.mantle;
    theme.table_head_foreground = p.subtext0;
    theme.table_row_border = p.surface0.opacity(0.70);
  } else {
    theme.title_bar = p.mantle;
    theme.title_bar_border = p.surface1;
    theme.status_bar = p.crust;
    theme.status_bar_border = p.surface1;
    theme.input = p.crust.opacity(0.80);
    theme.secondary = p.crust;
    theme.secondary_hover = p.surface0;
    theme.secondary_active = p.surface1;
    theme.secondary_foreground = p.text;
    theme.muted = p.crust;
    theme.colors.list = p.crust.opacity(0.55);
    theme.list_hover = p.surface0.opacity(0.70);
    theme.list_active = p.lavender.opacity(0.14);
    theme.list_active_border = p.lavender.opacity(0.45);
    theme.list_even = p.base.opacity(0.70);
    theme.list_head = p.crust;
    theme.overlay = p.text.opacity(0.22);
    theme.drop_target = p.blue.opacity(0.12);
    theme.button = p.crust.opacity(0.85);
    theme.button_hover = p.surface0;
    theme.button_active = p.surface1;
    theme.button_foreground = p.text;
    theme.sidebar = p.mantle;
    theme.sidebar_foreground = p.text;
    theme.sidebar_border = p.surface0;
    theme.sidebar_accent = p.surface0;
    theme.sidebar_accent_foreground = p.text;
    theme.tab_bar = p.crust;
    theme.tab = p.crust.opacity(0.0);
    theme.tab_foreground = p.overlay0;
    theme.tab_active = p.base;
    theme.tab_active_foreground = p.text;
    theme.scrollbar = p.base.opacity(0.0);
    theme.scrollbar_thumb = p.overlay0;
    theme.scrollbar_thumb_hover = p.overlay1;
    theme.selection = p.overlay2.opacity(0.32);
    theme.switch = p.surface1;
    theme.skeleton = p.surface0;
    theme.tiles = p.mantle;
    theme.table = p.base;
    theme.table_even = p.mantle;
    theme.table_hover = p.surface0.opacity(0.70);
    theme.table_head = p.crust;
    theme.table_head_foreground = p.subtext0;
    theme.table_row_border = p.surface0;
  }

  theme.button_secondary = theme.secondary;
  theme.button_secondary_hover = theme.secondary_hover;
  theme.button_secondary_active = theme.secondary_active;
  theme.button_secondary_foreground = theme.foreground;

  theme.success = p.green;
  theme.success_hover = p.green.lighten(0.08);
  theme.success_active = p.green.darken(0.08);
  theme.success_foreground = on;
  theme.danger = p.red;
  theme.danger_hover = p.maroon;
  theme.danger_active = p.red.darken(0.08);
  theme.danger_foreground = on;
  theme.warning = p.yellow;
  theme.warning_hover = p.peach;
  theme.warning_active = p.yellow.darken(0.08);
  theme.warning_foreground = on;
  theme.info = p.sky;
  theme.info_hover = p.sapphire;
  theme.info_active = p.sky.darken(0.08);
  theme.info_foreground = on;
  theme.button_success = theme.success;
  theme.button_success_hover = theme.success_hover;
  theme.button_success_active = theme.success_active;
  theme.button_success_foreground = theme.success_foreground;
  theme.button_danger = theme.danger;
  theme.button_danger_hover = theme.danger_hover;
  theme.button_danger_active = theme.danger_active;
  theme.button_danger_foreground = theme.danger_foreground;
  theme.button_warning = theme.warning;
  theme.button_warning_hover = theme.warning_hover;
  theme.button_warning_active = theme.warning_active;
  theme.button_warning_foreground = theme.warning_foreground;
  theme.button_info = theme.info;
  theme.button_info_hover = theme.info_hover;
  theme.button_info_active = theme.info_active;
  theme.button_info_foreground = theme.info_foreground;
  theme.accordion = theme.secondary;
  theme.group_box = theme.secondary;
  theme.group_box_foreground = theme.foreground;
}

fn paint_gradients(theme: &mut Theme, p: Flavor, primary: Hsla, hover: Hsla, active: Hsla) {
  let dark = theme.is_dark();
  theme.tokens.background = ThemeToken::from(theme.background);
  theme.tokens.button_primary = ThemeToken::new(
    primary,
    linear_gradient(
      128.,
      linear_color_stop(primary, 0.),
      linear_color_stop(p.sapphire, 1.),
    ),
  );
  theme.tokens.button_primary_hover = ThemeToken::new(
    hover,
    linear_gradient(
      128.,
      linear_color_stop(hover, 0.),
      linear_color_stop(p.sky, 1.),
    ),
  );
  theme.tokens.button_primary_active = ThemeToken::new(
    active,
    linear_gradient(
      128.,
      linear_color_stop(active, 0.),
      linear_color_stop(p.sapphire.darken(0.06), 1.),
    ),
  );
  theme.tokens.progress_bar = ThemeToken::new(
    p.sapphire,
    linear_gradient(
      90.,
      linear_color_stop(primary, 0.),
      linear_color_stop(p.sapphire, 1.),
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
