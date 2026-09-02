use gpui::{
  App, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
  StatefulInteractiveElement, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
  ActiveTheme as _, Icon, IconName, Sizable as _, WindowExt as _,
  button::{Button, ButtonVariants as _},
  h_flex,
  tag::Tag,
  v_flex,
};
use imprint_core::format_bytes;
use imprint_core::i18n::t;

use crate::app::ImprintApp;
use crate::theme::glass;
use crate::widgets::{icon_well, muted};

pub(crate) fn open(view: Entity<ImprintApp>, window: &mut Window, cx: &mut App) {
  // Defer so the dialog builder is not invoked while ImprintApp is updating.
  window.defer(cx, move |window, cx| {
    window.open_dialog(cx, move |dialog, _, cx| {
      let app = view.read(cx);
      dialog
        .title(t("drives.title"))
        .w(px(520.))
        .child(muted(cx, t("drives.hint")))
        .child(drive_list(&app, view.clone(), cx))
        .footer(
          h_flex()
            .w_full()
            .justify_end()
            .gap_2()
            .child(Button::new("refresh").label(t("drives.refresh")).on_click({
              let view = view.clone();
              move |_, _, cx| {
                view.update(cx, |this, cx| this.refresh_disks(cx));
              }
            }))
            .child(
              Button::new("confirm-drives")
                .primary()
                .label(t("drives.done"))
                .on_click(|_, window, cx| window.close_dialog(cx)),
            ),
        )
    });
  });
}

fn drive_list(app: &ImprintApp, view: Entity<ImprintApp>, cx: &App) -> impl IntoElement {
  v_flex()
    .id("drive-list")
    .gap_2()
    .max_h(px(320.))
    .overflow_y_scroll()
    .when(app.disks.is_empty(), |d| {
      d.child(
        v_flex()
          .items_center()
          .gap_2()
          .py_8()
          .child(icon_well(cx, IconName::HardDrive, false))
          .child(muted(cx, t("drives.empty"))),
      )
    })
    .children({
      let mut rows = Vec::new();
      for (ix, disk) in app.disks.iter().enumerate() {
        let selected = app.selected.contains(&ix);
        let need = app.needed_write_size();
        let too_small = need > 0 && disk.size < need;
        rows.push(drive_row(
          ix,
          disk.label(),
          format!(
            "{} · {} · {}",
            disk.bus.as_str(),
            format_bytes(disk.size),
            disk.path.display()
          ),
          selected,
          too_small,
          view.clone(),
          cx,
        ));
      }
      rows
    })
}

fn drive_row(
  ix: usize,
  label: String,
  detail: String,
  selected: bool,
  too_small: bool,
  view: Entity<ImprintApp>,
  cx: &App,
) -> impl IntoElement {
  let g = glass(cx);
  h_flex()
    .id(("drive", ix))
    .justify_between()
    .items_center()
    .gap_3()
    .px_3()
    .py_3()
    .rounded(cx.theme().radius)
    .border_1()
    .border_color(if selected {
      cx.theme().list_active_border
    } else {
      g.border
    })
    .bg(if selected {
      cx.theme().list_active
    } else {
      g.fill
    })
    .cursor_pointer()
    .hover(|s| {
      s.bg(if selected {
        cx.theme().list_active
      } else {
        g.fill_hover
      })
    })
    .on_click(move |_, _, cx| {
      view.update(cx, |this, cx| {
        if this.selected.contains(&ix) {
          this.selected.retain(|i| *i != ix);
        } else {
          this.selected.push(ix);
        }
        cx.notify();
      });
    })
    .child(icon_well(cx, IconName::HardDrive, selected))
    .child(
      v_flex()
        .flex_1()
        .gap_1()
        .min_w_0()
        .child(
          div()
            .font_weight(FontWeight::MEDIUM)
            .truncate()
            .child(label),
        )
        .child(muted(cx, detail)),
    )
    .child(if too_small {
      Tag::warning()
        .small()
        .child(t("drives.too_small"))
        .into_any_element()
    } else if selected {
      Icon::new(IconName::CircleCheck)
        .text_color(cx.theme().accent)
        .into_any_element()
    } else {
      div().into_any_element()
    })
}
