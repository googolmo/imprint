use std::sync::{Arc, OnceLock};

use gpui::{
  App, BoxShadow, Div, FontFeatures, FontWeight, Image, ImageFormat, Pixels, Styled, div, img,
  linear_color_stop, linear_gradient, prelude::*, px,
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

fn app_icon_image(dark: bool) -> Arc<Image> {
  static LIGHT: OnceLock<Arc<Image>> = OnceLock::new();
  static DARK: OnceLock<Arc<Image>> = OnceLock::new();
  const LIGHT_PNG: &[u8] = include_bytes!("../../../assets/icon/AppIcon-macos.png");
  const DARK_PNG: &[u8] = include_bytes!("../../../assets/icon/AppIcon-macos-dark.png");
  if dark {
    DARK
      .get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Png, DARK_PNG.to_vec())))
      .clone()
  } else {
    LIGHT
      .get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Png, LIGHT_PNG.to_vec())))
      .clone()
  }
}

/// Squircle app icon used in About. Light/dark artwork follows the theme.
pub fn app_icon(cx: &App, size: Pixels) -> impl gpui::IntoElement {
  img(app_icon_image(cx.theme().is_dark())).size(size)
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

/// Wide enough for `1023.9 MiB` at `text_sm`.
const BYTE_SLOT: Pixels = px(112.);

/// `done / total` with fixed-width slots so the slash does not drift as digits change.
pub fn bytes_progress(
  cx: &App,
  done: impl Into<String>,
  total: impl Into<String>,
) -> impl gpui::IntoElement {
  let tnum = FontFeatures(Arc::new(vec![("tnum".into(), 1)]));
  h_flex()
    .w_full()
    .h(px(20.))
    .flex_shrink_0()
    .items_center()
    .justify_center()
    .font_features(tnum)
    .text_sm()
    .font_weight(FontWeight::MEDIUM)
    .text_color(cx.theme().muted_foreground)
    .child(
      div()
        .w(BYTE_SLOT)
        .flex_shrink_0()
        .flex()
        .justify_end()
        .whitespace_nowrap()
        .child(done.into()),
    )
    .child(div().px_1().flex_shrink_0().child("/"))
    .child(
      div()
        .w(BYTE_SLOT)
        .flex_shrink_0()
        .flex()
        .justify_start()
        .whitespace_nowrap()
        .child(total.into()),
    )
}

pub fn hover_fill(cx: &App) -> gpui::Background {
  glass_hover_fill(cx)
}
