use gpui::{
  App, ClickEvent, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
  StatefulInteractiveElement, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
  ActiveTheme as _, Colorize as _, Icon, IconName, Sizable as _,
  button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants as _},
  h_flex, v_flex,
};
use imprint_core::format_bytes;
use imprint_core::i18n::{t, tr};

use crate::app::ImprintApp;
use crate::theme::{glass, raspberry_pi};
use crate::widgets::{
  glass_primary_shadows, glass_ready_shadows, glass_surface, hover_fill, icon_badge, muted,
  stage_connector, stage_kicker,
};

pub(crate) fn form(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  v_flex()
    .size_full()
    .items_center()
    .justify_center()
    .gap_4()
    .child(
      h_flex()
        .w_full()
        .h(px(348.))
        .items_stretch()
        .gap_3()
        .child(stage_card(
          cx,
          "image",
          "01",
          t("image.title"),
          IconName::FolderOpen,
          app
            .image
            .as_ref()
            .map(|i| i.display_name.clone())
            .unwrap_or_else(|| t("image.none")),
          image_subtitle(app),
          app.image.is_some(),
          t("image.select"),
          {
            let view = view.clone();
            move |_, window, cx| {
              view.update(cx, |this, cx| {
                if !this.flashing {
                  this.pick_image(window, cx);
                }
              });
            }
          },
        ))
        .child(stage_connector(cx))
        .child(stage_card(
          cx,
          "target",
          "02",
          t("target.title"),
          IconName::HardDrive,
          target_title(app),
          target_subtitle(app),
          !app.selected.is_empty(),
          t("target.select"),
          {
            let view = view.clone();
            move |_, window, cx| {
              view.update(cx, |this, cx| {
                if !this.flashing {
                  this.open_drives(window, cx);
                }
              });
            }
          },
        ))
        .child(stage_connector(cx))
        .child(write_stage(app, cx)),
    )
    .child(raspberry_pi_entry(view, cx))
}

fn raspberry_pi_entry(view: Entity<ImprintApp>, cx: &App) -> impl IntoElement {
  let raspberry = raspberry_pi(cx);
  let dark = cx.theme().is_dark();
  let hover = raspberry.divide(if dark { 0.34 } else { 0.14 });
  glass_surface(h_flex().w_full().items_center().gap_3().px_4().py_2(), cx)
    .id("open-raspberry-pi")
    .w_full()
    .border_color(raspberry.divide(if dark { 0.70 } else { 0.48 }))
    .bg(raspberry.divide(if dark { 0.22 } else { 0.08 }))
    .cursor_pointer()
    .hover(move |s| s.bg(hover))
    .on_click(move |_, _, cx| {
      view.update(cx, |this, cx| this.open_raspberry_pi(cx));
    })
    .child(
      div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded_full()
        .bg(raspberry.divide(if dark { 0.42 } else { 0.16 }))
        .child(Icon::new(IconName::Cpu).text_color(raspberry)),
    )
    .child(
      div()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(raspberry)
        .child(t("rpi.title")),
    )
    .child(
      div()
        .flex_1()
        .min_w_0()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.theme().muted_foreground)
        .truncate()
        .child(t("rpi.subtitle")),
    )
    .child(Icon::new(IconName::ChevronRight).text_color(raspberry))
}

fn image_subtitle(app: &ImprintApp) -> String {
  if let Some(image) = &app.image {
    let kind = image.kind.as_str();
    let file = format_bytes(image.file_size);
    if let Some(c) = image.compression {
      if image.payload_size > 0 && image.payload_size != image.file_size {
        format!(
          "{kind} · {file} → {} · {}",
          format_bytes(image.payload_size),
          c.as_str()
        )
      } else {
        format!("{kind} · {file} · {}", c.as_str())
      }
    } else {
      format!("{kind} · {file}")
    }
  } else {
    t("image.hint")
  }
}

fn target_title(app: &ImprintApp) -> String {
  if app.selected.len() == 1 {
    app.selected_disks()[0].label()
  } else if app.selected.len() > 1 {
    tr("target.count", &[("n", &app.selected.len().to_string())])
  } else {
    t("target.none")
  }
}

fn target_subtitle(app: &ImprintApp) -> String {
  if let Some(disk) = app.selected_disks().first() {
    format!("{} · {}", disk.bus.as_str(), format_bytes(disk.size))
  } else {
    t("target.hint")
  }
}

#[allow(clippy::too_many_arguments)]
fn stage_card(
  cx: &App,
  id: &'static str,
  step: &'static str,
  label: impl Into<String>,
  icon: IconName,
  title: String,
  subtitle: String,
  ready: bool,
  action: impl Into<String>,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
  let on_click = std::rc::Rc::new(on_click);
  let action = action.into();
  let g = glass(cx);
  let hover = hover_fill(cx);
  glass_surface(
    v_flex()
      .size_full()
      .items_center()
      .justify_center()
      .gap_3()
      .px_4()
      .py_5(),
    cx,
  )
  .flex_1()
  .min_w_0()
  .h_full()
  .id(id)
  .overflow_hidden()
  .cursor_pointer()
  .hover(move |s| s.bg(hover))
  .when(ready, |d| {
    d.border_color(cx.theme().accent.divide(0.55))
      .shadow(glass_ready_shadows(cx))
  })
  .on_click({
    let on_click = on_click.clone();
    move |ev, window, cx| on_click(ev, window, cx)
  })
  .child(stage_kicker(cx, step, label))
  .child(icon_badge(cx, icon, ready, px(56.)))
  .child(
    div()
      .w_full()
      .min_w_0()
      .px_1()
      .text_center()
      .font_weight(FontWeight::SEMIBOLD)
      .whitespace_normal()
      .child(title),
  )
  .child(
    div()
      .w_full()
      .px_2()
      .text_center()
      .child(muted(cx, subtitle)),
  )
  .child(
    Button::new(format!("{id}-select"))
      .small()
      .rounded(ButtonRounded::Large)
      .custom(
        ButtonCustomVariant::new(cx)
          .color(g.fill)
          .hover(g.fill_hover)
          .foreground(if cx.theme().is_dark() {
            cx.theme().accent
          } else {
            cx.theme().primary
          }),
      )
      .label(action)
      .on_click(move |ev, window, cx| {
        cx.stop_propagation();
        on_click(ev, window, cx);
      }),
  )
}

fn write_stage(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let can_write = app.can_flash();
  let primary_fill = cx.theme().tokens.button_primary.background;
  let primary_hover = cx.theme().tokens.button_primary_hover.background;
  glass_surface(
    v_flex()
      .size_full()
      .items_center()
      .justify_center()
      .gap_3()
      .px_4()
      .py_5(),
    cx,
  )
  .flex_1()
  .min_w_0()
  .h_full()
  .id("flash-stage")
  .overflow_hidden()
  .when(can_write, |d| {
    d.cursor_pointer()
      .bg(primary_fill)
      .border_color(cx.theme().accent.divide(0.55))
      .shadow(glass_primary_shadows(cx))
      .hover(move |s| s.bg(primary_hover))
      .on_click(cx.listener(ImprintApp::click_flash))
  })
  .child({
    let step_color = if can_write {
      cx.theme().primary_foreground
    } else if cx.theme().is_dark() {
      cx.theme().accent
    } else {
      cx.theme().primary
    };
    let label_color = if can_write {
      cx.theme().primary_foreground
    } else {
      cx.theme().muted_foreground
    };
    h_flex()
      .items_center()
      .gap_2()
      .child(
        div()
          .text_xs()
          .font_weight(FontWeight::SEMIBOLD)
          .text_color(step_color)
          .child("03"),
      )
      .child(
        div()
          .text_xs()
          .font_weight(FontWeight::SEMIBOLD)
          .text_color(label_color)
          .child(t("write.action")),
      )
  })
  .child(if can_write {
    div()
      .flex()
      .items_center()
      .justify_center()
      .size(px(56.))
      .rounded(cx.theme().radius)
      .bg(cx.theme().primary_foreground.divide(0.22))
      .child(
        Icon::new(IconName::Play)
          .large()
          .text_color(cx.theme().primary_foreground),
      )
      .into_any_element()
  } else {
    icon_badge(cx, IconName::Play, false, px(56.)).into_any_element()
  })
  .child(
    div()
      .w_full()
      .px_1()
      .text_center()
      .text_lg()
      .font_weight(FontWeight::SEMIBOLD)
      .text_color(if can_write {
        cx.theme().primary_foreground
      } else {
        cx.theme().foreground
      })
      .child(t("write.action")),
  )
  .child(
    div()
      .w_full()
      .px_2()
      .text_center()
      .text_sm()
      .font_weight(FontWeight::MEDIUM)
      .text_color(if can_write {
        cx.theme().primary_foreground
      } else {
        cx.theme().muted_foreground
      })
      .child(if can_write {
        t("write.erase_warning")
      } else {
        t("write.choose_hint")
      }),
  )
}
