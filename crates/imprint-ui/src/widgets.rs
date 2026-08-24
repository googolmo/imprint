use gpui::{Div, IntoElement, Stateful, div, prelude::*, px, relative};

use crate::theme::THEME;

pub fn card() -> Div {
  div()
    .flex()
    .flex_col()
    .flex_1()
    .p_6()
    .rounded_2xl()
    .bg(THEME.card)
    .border_1()
    .border_color(THEME.line)
    .shadow_lg()
}

pub fn ghost_button(id: &'static str, label: impl Into<String>) -> Stateful<Div> {
  div()
    .id(id)
    .px_4()
    .py_2()
    .rounded_lg()
    .border_1()
    .border_color(THEME.line)
    .text_color(THEME.text)
    .cursor_pointer()
    .hover(|s| s.bg(THEME.card_hover).border_color(THEME.accent))
    .child(label.into())
}

pub fn primary_button(id: &'static str, label: impl Into<String>, enabled: bool) -> Stateful<Div> {
  let color = if enabled {
    THEME.flash
  } else {
    THEME.accent_dim
  };
  let text = if enabled { THEME.text } else { THEME.muted };
  div()
    .id(id)
    .px_8()
    .py_3()
    .rounded_xl()
    .bg(color)
    .text_color(text)
    .shadow_md()
    .when(enabled, |d| {
      d.cursor_pointer().hover(|s| s.bg(THEME.flash_hover))
    })
    .child(label.into())
}

pub fn step_badge(n: u32, active: bool, done: bool) -> impl IntoElement {
  let bg = if done {
    THEME.ok
  } else if active {
    THEME.accent
  } else {
    THEME.accent_dim
  };
  let fg = if done || active {
    THEME.bg
  } else {
    THEME.muted
  };
  div()
    .flex()
    .items_center()
    .justify_center()
    .size(px(28.))
    .rounded_full()
    .bg(bg)
    .text_color(fg)
    .text_sm()
    .child(if done {
      "✓".to_string()
    } else {
      n.to_string()
    })
}

pub fn progress_track(fraction: f32) -> impl IntoElement {
  div()
    .w_full()
    .h(px(10.))
    .rounded_full()
    .bg(THEME.accent_dim)
    .child(
      div()
        .h_full()
        .w(relative(fraction.clamp(0.0, 1.0)))
        .rounded_full()
        .bg(THEME.accent),
    )
}

pub fn kicker(text: impl Into<String>) -> impl IntoElement {
  div().text_xs().text_color(THEME.accent).child(text.into())
}

pub fn muted(text: impl Into<String>) -> impl IntoElement {
  div().text_sm().text_color(THEME.muted).child(text.into())
}
