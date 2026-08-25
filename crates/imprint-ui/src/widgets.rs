use gpui::{App, Div, FontWeight, div, prelude::*};
use gpui_component::{ActiveTheme as _, h_flex};

pub fn section_label(cx: &App, text: impl Into<String>) -> impl gpui::IntoElement {
  div()
    .text_xs()
    .font_weight(FontWeight::MEDIUM)
    .text_color(cx.theme().muted_foreground)
    .child(text.into())
}

pub fn picker_row(cx: &App) -> Div {
  h_flex()
    .w_full()
    .items_center()
    .gap_3()
    .px_3()
    .py_3()
    .rounded(cx.theme().radius)
    .border_1()
    .border_color(cx.theme().border)
    .bg(cx.theme().background)
}

pub fn muted(cx: &App, text: impl Into<String>) -> impl gpui::IntoElement {
  div()
    .text_sm()
    .text_color(cx.theme().muted_foreground)
    .child(text.into())
}
