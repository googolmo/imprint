use gpui::{
  App, BoxShadow, Div, FontWeight, Hsla, Pixels, Styled, div, hsla, linear_color_stop,
  linear_gradient, prelude::*, px,
};
use gpui_component::{
  ActiveTheme as _, Colorize as _, Icon, IconName, Sizable as _, box_shadow, h_flex,
};

use crate::theme::{glass, glass_fill, glass_hover_fill, glass_panel_fill};

/// GPUI's blur is gaussian σ (~half of CSS). Any blur on the perimeter reads as
/// a soft fringe, so the rim stays at 0 and drops sit below the surface.
fn glass_rim(cx: &App) -> BoxShadow {
  BoxShadow::new(px(0.), px(1.), glass(cx).highlight).inset()
}

fn glass_drop(cx: &App) -> Vec<BoxShadow> {
  let g = glass(cx);
  vec![
    glass_rim(cx),
    box_shadow(px(0.), px(6.), px(10.), px(-4.), g.shadow),
    box_shadow(px(0.), px(2.), px(3.), px(-1.), g.glow),
  ]
}

pub fn glass_shadows(cx: &App) -> Vec<BoxShadow> {
  glass_drop(cx)
}

pub fn glass_ready_shadows(cx: &App) -> Vec<BoxShadow> {
  let mut shadows = glass_drop(cx);
  shadows.push(box_shadow(
    px(0.),
    px(8.),
    px(12.),
    px(-4.),
    cx.theme().accent.divide(0.22),
  ));
  shadows
}

pub fn glass_primary_shadows(cx: &App) -> Vec<BoxShadow> {
  let mut shadows = glass_drop(cx);
  shadows.push(box_shadow(
    px(0.),
    px(8.),
    px(12.),
    px(-4.),
    cx.theme().primary.divide(0.32),
  ));
  shadows
}

pub fn section_label(cx: &App, text: impl Into<String>) -> impl gpui::IntoElement {
  h_flex()
    .items_center()
    .gap_2()
    .child(
      div()
        .w(px(2.))
        .h(px(11.))
        .rounded_full()
        .bg(if cx.theme().is_dark() {
          cx.theme().accent
        } else {
          cx.theme().primary
        }),
    )
    .child(
      div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(text.into()),
    )
}

pub fn stage_kicker(
  cx: &App,
  step: impl Into<String>,
  label: impl Into<String>,
) -> impl gpui::IntoElement {
  h_flex()
    .items_center()
    .gap_2()
    .child(
      div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if cx.theme().is_dark() {
          cx.theme().accent
        } else {
          cx.theme().primary
        })
        .child(step.into()),
    )
    .child(
      div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label.into()),
    )
}

pub fn glass_surface(element: Div, cx: &App) -> Div {
  let g = glass(cx);
  element
    .rounded(cx.theme().radius_lg)
    .border_1()
    .border_color(g.border)
    .bg(glass_fill(cx))
    .shadow(glass_shadows(cx))
}

/// Denser glass wash for side sheets; cards still use [`glass_surface`].
pub fn glass_panel<E: Styled>(element: E, cx: &App) -> E {
  let g = glass(cx);
  element
    .border_color(g.border)
    .bg(glass_panel_fill(cx))
    .shadow(vec![
      glass_rim(cx),
      box_shadow(px(-4.), px(0.), px(8.), px(-2.), g.shadow),
    ])
}

pub fn icon_well(cx: &App, icon: IconName, ready: bool) -> impl gpui::IntoElement {
  icon_badge(cx, icon, ready, px(40.))
}

pub fn icon_badge(cx: &App, icon: IconName, ready: bool, size: Pixels) -> impl gpui::IntoElement {
  let g = glass(cx);
  let large = size >= px(48.);
  let wash = if ready {
    linear_gradient(
      135.,
      linear_color_stop(cx.theme().primary.divide(0.28), 0.),
      linear_color_stop(cx.theme().accent.divide(0.22), 1.),
    )
  } else {
    linear_gradient(
      135.,
      linear_color_stop(g.fill_top, 0.),
      linear_color_stop(g.fill, 1.),
    )
  };
  let mut shadows = vec![glass_rim(cx)];
  if ready {
    shadows.push(box_shadow(
      px(0.),
      px(4.),
      px(8.),
      px(-2.),
      cx.theme().accent.divide(0.28),
    ));
  }
  div()
    .flex()
    .items_center()
    .justify_center()
    .size(size)
    .rounded(cx.theme().radius)
    .bg(wash)
    .border_1()
    .border_color(if ready {
      cx.theme().accent.divide(0.45)
    } else {
      g.border
    })
    .shadow(shadows)
    .child(
      Icon::new(icon)
        .when(large, |i| i.large())
        .text_color(if ready {
          cx.theme().accent
        } else {
          cx.theme().muted_foreground
        }),
    )
}

pub fn brand_mark(cx: &App, size: Pixels) -> impl gpui::IntoElement {
  let fg: Hsla = cx.theme().primary_foreground;
  let primary = cx.theme().primary;
  let sapphire = cx.theme().cyan;
  div()
    .flex()
    .items_center()
    .justify_center()
    .size(size)
    .rounded(size * 0.30)
    .bg(linear_gradient(
      128.,
      linear_color_stop(primary, 0.),
      linear_color_stop(sapphire, 1.),
    ))
    .shadow(vec![box_shadow(
      px(0.),
      px(2.),
      px(10.),
      px(0.),
      primary.divide(0.40),
    )])
    .child(
      Icon::new(IconName::HardDrive)
        .when(size >= px(40.), |i| i.large())
        .text_color(fg),
    )
}

pub fn stage_connector(cx: &App) -> impl gpui::IntoElement {
  div()
    .flex()
    .h_full()
    .items_center()
    .justify_center()
    .px_1()
    .child(Icon::new(IconName::ChevronRight).text_color(cx.theme().muted_foreground.divide(0.55)))
}

pub fn muted(cx: &App, text: impl Into<String>) -> impl gpui::IntoElement {
  div()
    .text_sm()
    .font_weight(FontWeight::MEDIUM)
    .text_color(cx.theme().muted_foreground)
    .child(text.into())
}

pub fn hover_fill(cx: &App) -> gpui::Background {
  glass_hover_fill(cx)
}

pub fn atmosphere(cx: &App) -> impl gpui::IntoElement {
  let dark = cx.theme().is_dark();
  let sapphire = cx.theme().primary;
  let cyan = cx.theme().cyan;
  let mauve = cx.theme().accent.divide(if dark { 0.16 } else { 0.05 });
  let sapphire_glow = sapphire.divide(if dark { 0.16 } else { 0.045 });
  let cyan_glow = cyan.divide(if dark { 0.10 } else { 0.03 });
  let sheen = hsla(0.0, 0.0, 1.0, if dark { 0.05 } else { 0.38 });

  div()
    .absolute()
    .size_full()
    .overflow_hidden()
    .child(div().absolute().size_full().bg(linear_gradient(
      148.,
      linear_color_stop(sapphire.divide(if dark { 0.12 } else { 0.025 }), 0.),
      linear_color_stop(mauve, 1.),
    )))
    .child(
      div()
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(220.))
        .bg(linear_gradient(
          180.,
          linear_color_stop(sheen, 0.),
          linear_color_stop(hsla(0.0, 0.0, 1.0, 0.0), 1.),
        )),
    )
    .child(
      div()
        .absolute()
        .top(px(-180.))
        .right(px(-80.))
        .w(px(420.))
        .h(px(420.))
        .rounded_full()
        .bg(sapphire_glow)
        .shadow(vec![box_shadow(
          px(0.),
          px(0.),
          px(160.),
          px(60.),
          sapphire_glow,
        )]),
    )
    .child(
      div()
        .absolute()
        .bottom(px(-200.))
        .left(px(-120.))
        .w(px(380.))
        .h(px(380.))
        .rounded_full()
        .bg(mauve)
        .shadow(vec![box_shadow(px(0.), px(0.), px(140.), px(48.), mauve)]),
    )
    .child(
      div()
        .absolute()
        .top(px(180.))
        .left(px(-40.))
        .w(px(180.))
        .h(px(180.))
        .rounded_full()
        .bg(cyan_glow)
        .shadow(vec![box_shadow(
          px(0.),
          px(0.),
          px(80.),
          px(20.),
          cyan_glow,
        )]),
    )
}
