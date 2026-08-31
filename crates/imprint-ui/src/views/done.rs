use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{
  ActiveTheme as _, Colorize as _, Icon, IconName, Sizable as _,
  button::{Button, ButtonRounded, ButtonVariants as _},
  h_flex, v_flex,
};
use imprint_core::i18n::{t, tr};

use crate::app::ImprintApp;
use crate::widgets::{glass_surface, muted};

pub(crate) fn panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let name = app
    .image
    .as_ref()
    .map(|i| i.display_name.clone())
    .unwrap_or_default();
  let view = cx.entity();
  v_flex().size_full().items_center().justify_center().child(
    glass_surface(
      v_flex()
        .w(px(420.))
        .max_w_full()
        .items_center()
        .gap_3()
        .px_8()
        .py_8(),
      cx,
    )
    .child(
      div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(64.))
        .rounded_full()
        .bg(cx.theme().success.divide(0.16))
        .shadow(vec![gpui_component::box_shadow(
          px(0.),
          px(0.),
          px(24.),
          px(2.),
          cx.theme().success.divide(0.28),
        )])
        .child(
          Icon::new(IconName::CircleCheck)
            .large()
            .text_color(cx.theme().success),
        ),
    )
    .child(
      div()
        .text_xl()
        .font_weight(FontWeight::SEMIBOLD)
        .child(t("done.title")),
    )
    .child(
      div()
        .w_full()
        .text_center()
        .child(muted(cx, tr("done.ready", &[("name", &name)]))),
    )
    .child(
      h_flex()
        .gap_2()
        .mt_4()
        .child(
          Button::new("again")
            .primary()
            .rounded(ButtonRounded::Large)
            .label(t("done.another"))
            .on_click({
              let view = view.clone();
              move |_, _, cx| {
                view.update(cx, |this, cx| this.flash_another(cx));
              }
            }),
        )
        .child(
          Button::new("same")
            .ghost()
            .rounded(ButtonRounded::Large)
            .label(t("done.keep"))
            .on_click(move |_, _, cx| {
              view.update(cx, |this, cx| {
                this.progress = None;
                this.selected.clear();
                cx.notify();
              });
            }),
        ),
    ),
  )
}
