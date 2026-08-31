use std::sync::atomic::Ordering;

use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px};
use gpui_component::{
  ActiveTheme as _, Icon, IconName,
  button::{Button, ButtonRounded, ButtonVariants as _},
  progress::ProgressCircle,
  spinner::Spinner,
  v_flex,
};
use imprint_core::FlashPhase;
use imprint_core::format_bytes;
use imprint_core::i18n::{t, tr};

use crate::app::ImprintApp;
use crate::widgets::{glass_surface, muted, section_label};

pub(crate) fn panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let progress = app.progress.clone();
  let phase = progress
    .as_ref()
    .map(|p| phase_label(p.phase))
    .unwrap_or_else(|| t("progress.working"));
  let message = localized_progress_message(app);
  let speed = progress
    .as_ref()
    .map(|p| {
      if p.bytes_per_sec == 0 {
        String::new()
      } else {
        format!("{}/s", format_bytes(p.bytes_per_sec))
      }
    })
    .unwrap_or_default();
  let failed = progress
    .as_ref()
    .is_some_and(|p| p.phase == FlashPhase::Failed);
  let indeterminate = progress.as_ref().is_some_and(|p| p.is_indeterminate());
  let pct_value = progress.as_ref().map(|p| p.percent() as f32).unwrap_or(0.0);
  let pct = format!("{}%", pct_value as u32);
  let view = cx.entity();
  let ring_color = if failed {
    cx.theme().danger
  } else if cx.theme().is_dark() {
    cx.theme().accent
  } else {
    cx.theme().primary
  };

  v_flex().size_full().items_center().justify_center().child(
    glass_surface(
      v_flex()
        .w(px(420.))
        .max_w_full()
        .items_center()
        .gap_4()
        .px_8()
        .py_8(),
      cx,
    )
    .child(section_label(
      cx,
      if failed {
        t("progress.phase.failed")
      } else {
        phase
      },
    ))
    .child(
      ProgressCircle::new("write-progress")
        .size(px(148.))
        .value(if indeterminate { 0.0 } else { pct_value })
        .loading(indeterminate && !failed)
        .color(ring_color)
        .child(if indeterminate && !failed {
          Spinner::new()
            .icon(Icon::new(IconName::LoaderCircle))
            .color(ring_color)
            .into_any_element()
        } else {
          div()
            .text_3xl()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if failed {
              cx.theme().danger
            } else {
              cx.theme().foreground
            })
            .child(pct)
            .into_any_element()
        }),
    )
    .child(
      div()
        .w_full()
        .text_center()
        .text_lg()
        .font_weight(FontWeight::MEDIUM)
        .text_color(if failed {
          cx.theme().danger
        } else {
          cx.theme().foreground
        })
        .child(message),
    )
    .when(!speed.is_empty(), |d| {
      d.child(
        div()
          .px_3()
          .py_1()
          .rounded_full()
          .bg(cx.theme().muted)
          .child(muted(cx, speed)),
      )
    })
    .when(app.flashing, |d| {
      d.child(
        Button::new("cancel")
          .ghost()
          .rounded(ButtonRounded::Large)
          .label(t("progress.cancel"))
          .on_click(move |_, _, cx| {
            view.update(cx, |this, cx| {
              this.cancel.store(true, Ordering::Relaxed);
              cx.notify();
            });
          }),
      )
    })
    .when(failed, |d| {
      let view = cx.entity();
      d.child(
        Button::new("retry")
          .rounded(ButtonRounded::Large)
          .label(t("progress.back"))
          .on_click(move |_, _, cx| {
            view.update(cx, |this, cx| this.flash_another(cx));
          }),
      )
    }),
  )
}

fn phase_label(phase: FlashPhase) -> String {
  t(match phase {
    FlashPhase::Preparing => "progress.phase.preparing",
    FlashPhase::Writing => "progress.phase.writing",
    FlashPhase::Verifying => "progress.phase.verifying",
    FlashPhase::Finishing => "progress.phase.finishing",
    FlashPhase::Done => "progress.phase.done",
    FlashPhase::Failed => "progress.phase.failed",
  })
}

fn localized_progress_message(app: &ImprintApp) -> String {
  let Some(progress) = &app.progress else {
    return t("progress.working");
  };
  match progress.phase {
    FlashPhase::Preparing => tr("progress.unmounting", &[("disk", &progress.target_label)]),
    FlashPhase::Writing => {
      if progress.bytes_done == 0 {
        let image = app
          .image
          .as_ref()
          .map(|i| i.display_name.as_str())
          .unwrap_or("");
        tr(
          "progress.writing",
          &[("image", image), ("disk", &progress.target_label)],
        )
      } else {
        let bytes = format_bytes(progress.bytes_done);
        tr("progress.written", &[("bytes", &bytes)])
      }
    }
    FlashPhase::Verifying => {
      if progress.bytes_done == 0 {
        t("progress.validating")
      } else {
        let bytes = format_bytes(progress.bytes_done);
        tr("progress.checked", &[("bytes", &bytes)])
      }
    }
    FlashPhase::Finishing => t("progress.syncing"),
    FlashPhase::Done => t("progress.complete"),
    FlashPhase::Failed => progress.message.clone(),
  }
}
