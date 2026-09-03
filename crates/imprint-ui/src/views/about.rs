use gpui::{
  App, Bounds, Context, Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement,
  ParentElement, Render, Styled, Subscription, Window, WindowBounds, WindowKind, div, prelude::*,
  px, size,
};
use gpui_component::{
  ActiveTheme as _, Root, TitleBar,
  button::{Button, ButtonVariants as _},
  h_flex, v_flex,
};
use imprint_core::i18n::{t, tr};

use crate::CloseAbout;
use crate::app::ImprintApp;
use crate::widgets::{app_icon, muted};

pub(crate) fn open(view: Entity<ImprintApp>, window: &mut Window, cx: &mut App) {
  window.defer(cx, move |_, cx| open_now(view, cx));
}

fn open_now(view: Entity<ImprintApp>, cx: &mut App) {
  if focus_existing(&view, cx) {
    return;
  }

  let bounds = Bounds::centered(None, size(px(400.), px(380.)), cx);
  let mut options = TitleBar::window_options();
  options.window_bounds = Some(WindowBounds::Windowed(bounds));
  options.window_min_size = Some(size(px(400.), px(380.)));
  options.app_id = Some(crate::APP_IDENTIFIER.into());
  options.kind = WindowKind::Normal;
  options.is_resizable = false;

  let app = view.clone();
  let Ok(handle) = cx.open_window(options, |window, cx| {
    let about = cx.new(|cx| AboutWindow::new(app, window, cx));
    cx.new(|cx| Root::new(about, window, cx))
  }) else {
    return;
  };

  view.update(cx, |this, _| this.about_window = Some(handle.into()));
  let _ = handle.update(cx, |_, window, _| {
    window.activate_window();
  });
}

fn focus_existing(view: &Entity<ImprintApp>, cx: &mut App) -> bool {
  let Some(handle) = view.read(cx).about_window else {
    return false;
  };
  if handle
    .update(cx, |_, window, _| window.activate_window())
    .is_ok()
  {
    true
  } else {
    view.update(cx, |this, _| this.about_window = None);
    false
  }
}

struct AboutWindow {
  app: Entity<ImprintApp>,
  focus: FocusHandle,
  _observe: Subscription,
}

impl AboutWindow {
  fn new(app: Entity<ImprintApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus = cx.focus_handle();
    focus.focus(window, cx);
    window.set_window_title(&t("about.title"));
    let _observe = cx.observe_in(&app, window, |_, _, window, cx| {
      window.set_window_title(&t("about.title"));
      cx.notify();
    });
    Self {
      app,
      focus,
      _observe,
    }
  }

  fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.app.update(cx, |this, _| this.about_window = None);
    window.remove_window();
  }

  fn check_for_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.app.update(cx, |this, cx| {
      this.begin_update_check(true, window, cx);
    });
  }
}

impl Render for AboutWindow {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .id("about-window")
      .key_context("About")
      .track_focus(&self.focus)
      .on_action(cx.listener(|this, _: &CloseAbout, window, cx| {
        this.close(window, cx);
      }))
      .size_full()
      .relative()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(
        TitleBar::new()
          .bg(cx.theme().title_bar)
          .border_color(cx.theme().title_bar_border)
          .child(
            div()
              .text_sm()
              .font_weight(FontWeight::SEMIBOLD)
              .text_color(cx.theme().foreground)
              .child(t("about.title")),
          ),
      )
      .child(
        v_flex()
          .flex_1()
          .w_full()
          .items_center()
          .justify_center()
          .gap_3()
          .px_6()
          .child(app_icon(cx, px(96.)))
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
      .child(
        h_flex()
          .w_full()
          .px_5()
          .pb_5()
          .justify_between()
          .child(
            Button::new("about-check-updates")
              .ghost()
              .label(t("about.check_updates"))
              .on_click(cx.listener(|this, _, window, cx| {
                this.check_for_updates(window, cx);
              })),
          )
          .child(
            Button::new("about-ok")
              .primary()
              .label(t("about.ok"))
              .on_click(cx.listener(|this, _, window, cx| {
                this.close(window, cx);
              })),
          ),
      )
      .children(Root::render_dialog_layer(window, cx))
      .children(Root::render_sheet_layer(window, cx))
      .children(Root::render_notification_layer(window, cx))
  }
}
