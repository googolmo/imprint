use gpui::{App, Div, FontWeight, Styled, div, linear_color_stop, linear_gradient, prelude::*, px};
use gpui_component::{ActiveTheme as _, Colorize as _, Icon, IconName, box_shadow, h_flex};

use crate::theme::{glass, glass_fill, glass_panel_fill};

pub fn section_label(cx: &App, text: impl Into<String>) -> impl gpui::IntoElement {
  div()
    .text_sm()
    .font_weight(FontWeight::SEMIBOLD)
    .text_color(if cx.theme().is_dark() {
      cx.theme().accent
    } else {
      cx.theme().primary
    })
    .child(text.into())
}

pub fn glass_surface(element: Div, cx: &App) -> Div {
  let g = glass(cx);
  element
    .rounded(cx.theme().radius_lg)
    .border_1()
    .border_color(g.border)
    .bg(glass_fill(cx))
    .shadow(vec![
      box_shadow(px(0.), px(-1.), px(1.5), px(0.), g.highlight),
      box_shadow(px(0.), px(10.), px(28.), px(-4.), g.shadow),
      box_shadow(px(0.), px(6.), px(22.), px(0.), g.glow),
    ])
}

/// Denser glass wash for side sheets; cards still use [`glass_surface`].
pub fn glass_panel<E: Styled>(element: E, cx: &App) -> E {
  let g = glass(cx);
  let radius = cx.theme().radius_lg;
  element
    .rounded_tl(radius)
    .rounded_bl(radius)
    .border_color(g.border)
    .bg(glass_panel_fill(cx))
    .shadow(vec![
      box_shadow(px(0.), px(-1.), px(1.5), px(0.), g.highlight),
      box_shadow(px(-12.), px(8.), px(36.), px(-4.), g.shadow),
      box_shadow(px(-10.), px(0.), px(28.), px(0.), g.glow),
    ])
}

pub fn picker_row(cx: &App) -> Div {
  glass_surface(h_flex().w_full().items_center().gap_3().px_4().py_3(), cx)
}

pub fn icon_well(cx: &App, icon: IconName, ready: bool) -> impl gpui::IntoElement {
  let g = glass(cx);
  let wash = if ready {
    linear_gradient(
      135.,
      linear_color_stop(cx.theme().primary.divide(0.35), 0.),
      linear_color_stop(cx.theme().accent.divide(0.28), 1.),
    )
  } else {
    linear_gradient(
      135.,
      linear_color_stop(g.fill_top, 0.),
      linear_color_stop(g.fill, 1.),
    )
  };
  div()
    .flex()
    .items_center()
    .justify_center()
    .size(px(36.))
    .rounded(cx.theme().radius)
    .bg(wash)
    .border_1()
    .border_color(if ready {
      cx.theme().accent.divide(0.55)
    } else {
      g.border
    })
    .child(Icon::new(icon).text_color(if ready {
      cx.theme().accent
    } else {
      cx.theme().muted_foreground
    }))
}

pub fn muted(cx: &App, text: impl Into<String>) -> impl gpui::IntoElement {
  div()
    .text_sm()
    .font_weight(FontWeight::MEDIUM)
    .text_color(cx.theme().muted_foreground)
    .child(text.into())
}

pub fn atmosphere(cx: &App) -> impl gpui::IntoElement {
  let dark = cx.theme().is_dark();
  let sapphire = cx.theme().primary;
  let cyan = gpui::hsla(0.515, 0.82, 0.64, 1.0);
  let violet = gpui::hsla(
    0.76,
    0.62,
    if dark { 0.52 } else { 0.72 },
    if dark { 0.22 } else { 0.05 },
  );
  let sapphire_glow = if dark {
    sapphire.divide(0.28)
  } else {
    sapphire.divide(0.06)
  };
  let cyan_glow = if dark {
    cyan.divide(0.24)
  } else {
    cyan.divide(0.05)
  };

  div()
    .absolute()
    .size_full()
    .overflow_hidden()
    .child(div().absolute().size_full().bg(linear_gradient(
      132.,
      linear_color_stop(sapphire.divide(if dark { 0.16 } else { 0.03 }), 0.),
      linear_color_stop(violet, 1.),
    )))
    .child(
      div()
        .absolute()
        .top(px(-140.))
        .right(px(-70.))
        .w(px(340.))
        .h(px(340.))
        .rounded_full()
        .bg(sapphire_glow)
        .shadow(vec![box_shadow(
          px(0.),
          px(0.),
          px(140.),
          px(50.),
          sapphire_glow,
        )]),
    )
    .child(
      div()
        .absolute()
        .bottom(px(-160.))
        .left(px(-90.))
        .w(px(300.))
        .h(px(300.))
        .rounded_full()
        .bg(violet)
        .shadow(vec![box_shadow(px(0.), px(0.), px(120.), px(40.), violet)]),
    )
    .child(
      div()
        .absolute()
        .top(px(120.))
        .left(px(-80.))
        .w(px(220.))
        .h(px(220.))
        .rounded_full()
        .bg(cyan_glow)
        .shadow(vec![box_shadow(
          px(0.),
          px(0.),
          px(90.),
          px(24.),
          cyan_glow,
        )]),
    )
}
