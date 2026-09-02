use gpui::{
  App, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
  StatefulInteractiveElement, Styled, div, linear_color_stop, linear_gradient, prelude::*, px,
};
use gpui_component::{
  ActiveTheme as _, Colorize as _, Icon, IconName, Sizable as _, TitleBar,
  button::{Button, ButtonRounded, ButtonVariants as _},
  h_flex,
  progress::ProgressCircle,
  spinner::Spinner,
  status_bar::StatusBar,
  tooltip::Tooltip,
};
use imprint_core::i18n::{t, tr};

use crate::app::{ImprintApp, UpdateStatus};

pub(crate) fn header(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  TitleBar::new()
    .bg(linear_gradient(
      180.,
      linear_color_stop(cx.theme().title_bar, 0.),
      linear_color_stop(cx.theme().title_bar.divide(0.28), 1.),
    ))
    .border_color(cx.theme().title_bar_border)
    .child(
      h_flex()
        .w_full()
        .pr_2()
        .items_center()
        .justify_between()
        .child(
          div()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().foreground)
            .child(t("app.name")),
        )
        .child(
          h_flex()
            .items_center()
            .gap_2()
            .child(update_status_chip(app, view.clone(), cx))
            .child(
              Button::new("settings")
                .ghost()
                .small()
                .rounded(ButtonRounded::Large)
                .icon(IconName::Settings)
                .tooltip(t("header.settings_tooltip"))
                .on_click(move |_, window, cx| {
                  view.update(cx, |this, cx| this.open_settings(window, cx));
                }),
            ),
        ),
    )
}

fn update_status_chip(
  app: &ImprintApp,
  view: Entity<ImprintApp>,
  cx: &mut Context<ImprintApp>,
) -> impl IntoElement {
  if !app.update_chip_visible() {
    return div().into_any_element();
  }

  let clickable = matches!(
    app.update,
    UpdateStatus::Available(_) | UpdateStatus::Installed { .. } | UpdateStatus::Failed(_)
  );
  let dismissable = clickable;
  let border = if clickable {
    cx.theme().foreground.divide(0.18)
  } else {
    cx.theme().border
  };

  let (icon, label, tooltip) = match &app.update {
    UpdateStatus::Checking => (UpdateChipIcon::Spinner, t("update.checking_chip"), None),
    UpdateStatus::Available(update) => (
      UpdateChipIcon::ArrowDown,
      t("update.available_chip"),
      Some(tr(
        "update.version_tooltip",
        &[("version", update.version.as_str())],
      )),
    ),
    UpdateStatus::Downloading {
      update,
      received,
      total,
    } => {
      let progress = total
        .filter(|total| *total > 0)
        .map(|total| (*received as f32 / total as f32).clamp(0.0, 1.0));
      let tooltip = Some(match progress {
        Some(progress) => tr(
          "update.progress_tooltip",
          &[
            ("version", update.version.as_str()),
            ("percent", &format!("{:.0}", progress * 100.0)),
          ],
        ),
        None => tr(
          "update.version_tooltip",
          &[("version", update.version.as_str())],
        ),
      });
      (
        UpdateChipIcon::Download { progress },
        t("update.downloading_chip"),
        tooltip,
      )
    }
    UpdateStatus::Installed { version } => (
      UpdateChipIcon::ArrowDown,
      t("update.restart_chip"),
      Some(tr(
        "update.version_tooltip",
        &[("version", version.as_str())],
      )),
    ),
    UpdateStatus::Failed(err) => (
      UpdateChipIcon::Warning,
      t("update.failed_chip"),
      Some(err.clone()),
    ),
    _ => {
      return div().into_any_element();
    }
  };

  h_flex()
    .items_center()
    .rounded(cx.theme().radius)
    .border_1()
    .border_color(border)
    .overflow_hidden()
    .child(
      h_flex()
        .id("update-chip")
        .items_center()
        .gap_1()
        .h(px(22.))
        .px_2()
        .child(update_chip_icon(icon, cx))
        .child(
          div()
            .text_xs()
            .text_color(cx.theme().foreground)
            .child(label),
        )
        .when(clickable, {
          let view = view.clone();
          move |this| {
            this.cursor_pointer().on_click(move |_, window, cx| {
              view.update(cx, |this, cx| match &this.update {
                UpdateStatus::Available(update) => {
                  let update = update.clone();
                  this.begin_download(update, cx);
                }
                UpdateStatus::Installed { .. } => this.restart_to_update(cx),
                UpdateStatus::Failed(_) => this.begin_update_check(true, window, cx),
                _ => {}
              });
            })
          }
        })
        .when_some(tooltip, |this, tooltip| {
          this.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        }),
    )
    .when(dismissable, |this| {
      this
        .child(div().w(px(1.)).h_full().min_h(px(22.)).bg(border))
        .child(
          Button::new("update-dismiss")
            .ghost()
            .xsmall()
            .compact()
            .icon(IconName::Close)
            .tooltip(t("update.dismiss"))
            .on_click({
              let view = view.clone();
              move |_, _, cx| {
                view.update(cx, |this, cx| this.dismiss_update_chip(cx));
              }
            }),
        )
    })
    .into_any_element()
}

enum UpdateChipIcon {
  Spinner,
  Download { progress: Option<f32> },
  ArrowDown,
  Warning,
}

fn update_chip_icon(icon: UpdateChipIcon, cx: &App) -> impl IntoElement {
  let color = cx.theme().foreground;
  match icon {
    UpdateChipIcon::Spinner => Spinner::new()
      .xsmall()
      .icon(Icon::new(IconName::LoaderCircle))
      .color(color)
      .into_any_element(),
    UpdateChipIcon::Download {
      progress: Some(progress),
    } => ProgressCircle::new("update-download")
      .size(px(12.))
      .value(progress.clamp(0.0, 1.0) * 100.0)
      .color(color)
      .into_any_element(),
    UpdateChipIcon::Download { progress: None } | UpdateChipIcon::ArrowDown => {
      Icon::new(IconName::ArrowDown)
        .xsmall()
        .text_color(color)
        .into_any_element()
    }
    UpdateChipIcon::Warning => Icon::new(IconName::TriangleAlert)
      .xsmall()
      .text_color(cx.theme().warning)
      .into_any_element(),
  }
}

pub(crate) fn status_bar(app: &ImprintApp, cx: &App) -> impl IntoElement {
  let ready = app.can_flash();
  StatusBar::new()
    .px_4()
    .py_1p5()
    .text_sm()
    .bg(linear_gradient(
      180.,
      linear_color_stop(cx.theme().status_bar.divide(0.55), 0.),
      linear_color_stop(cx.theme().status_bar, 1.),
    ))
    .border_color(cx.theme().status_bar_border)
    .left(tr("status.drives", &[("n", &app.disks.len().to_string())]))
    .right(if let Some(err) = app.error.clone() {
      err
    } else if app.rpi.downloading() {
      t("status.downloading")
    } else if app.flashing {
      t("status.writing")
    } else if app.mode == crate::rpi::AppMode::RaspberryPi {
      t("status.raspberry_pi")
    } else if ready {
      t("status.ready")
    } else {
      String::new()
    })
    .when(app.error.is_some(), |d| d.text_color(cx.theme().danger))
    .when(app.error.is_none() && ready, |d| {
      d.text_color(cx.theme().accent)
    })
}
