use gpui::{App, Entity, FontWeight, ParentElement, Styled, Window, div, px};
use gpui_component::{
  WindowExt as _,
  button::{Button, ButtonVariants as _},
  h_flex, v_flex,
};
use imprint_core::i18n::{t, tr};

use crate::app::ImprintApp;
use crate::widgets::{brand_mark, muted};

pub(crate) fn open(view: Entity<ImprintApp>, window: &mut Window, cx: &mut App) {
  window.defer(cx, move |window, cx| {
    window.open_dialog(cx, move |dialog, _, cx| {
      dialog
        .title(t("about.title"))
        .w(px(400.))
        .child(
          v_flex()
            .gap_3()
            .items_center()
            .py_4()
            .child(brand_mark(cx, px(56.)))
            .child(
              div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(t("app.name")),
            )
            .child(muted(
              cx,
              tr("about.version", &[("version", env!("CARGO_PKG_VERSION"))]),
            ))
            .child(
              div()
                .max_w(px(280.))
                .text_center()
                .child(muted(cx, t("about.tagline"))),
            )
            .child(muted(cx, env!("CARGO_PKG_LICENSE"))),
        )
        .footer(
          h_flex()
            .w_full()
            .justify_between()
            .child(
              Button::new("about-check-updates")
                .ghost()
                .label(t("about.check_updates"))
                .on_click({
                  let view = view.clone();
                  move |_, window, cx| {
                    window.close_dialog(cx);
                    view.update(cx, |this, cx| this.begin_update_check(true, window, cx));
                  }
                }),
            )
            .child(
              Button::new("about-ok")
                .primary()
                .label(t("about.ok"))
                .on_click(|_, window, cx| window.close_dialog(cx)),
            ),
        )
    });
  });
}
