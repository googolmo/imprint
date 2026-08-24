use std::sync::LazyLock;

use gpui::{Rgba, rgb, rgba};

/// Dark studio palette — high-contrast teal on navy, Etcher-like 3-step flow.
pub struct Theme {
  pub bg: Rgba,
  pub bg_elevated: Rgba,
  pub card: Rgba,
  pub card_hover: Rgba,
  pub line: Rgba,
  pub text: Rgba,
  pub muted: Rgba,
  pub accent: Rgba,
  pub accent_dim: Rgba,
  pub danger: Rgba,
  pub warn: Rgba,
  pub ok: Rgba,
  pub flash: Rgba,
  pub flash_hover: Rgba,
}

pub static THEME: LazyLock<Theme> = LazyLock::new(|| Theme {
  bg: rgb(0x070b14),
  bg_elevated: rgb(0x0e1524),
  card: rgb(0x121c30),
  card_hover: rgb(0x18243c),
  line: rgba(0x3ee0c933),
  text: rgb(0xe8eef8),
  muted: rgb(0x8b9bb4),
  accent: rgb(0x3ee0c9),
  accent_dim: rgb(0x1a4f4a),
  danger: rgb(0xff6b6b),
  warn: rgb(0xffc857),
  ok: rgb(0x5ee9a0),
  flash: rgb(0xff5a36),
  flash_hover: rgb(0xff7a5c),
});
