use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use gpui::{
  App, ClickEvent, Context, Entity, ExternalPaths, FocusHandle, FontWeight, InteractiveElement,
  IntoElement, ParentElement, PathPromptOptions, Render, StatefulInteractiveElement, Styled,
  Subscription, Window, div, linear_color_stop, linear_gradient, prelude::*, px,
};
use gpui_component::{
  ActiveTheme as _, Colorize as _, Disableable as _, Icon, IconName, Root, Sizable as _, TitleBar,
  WindowExt as _,
  button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants as _},
  h_flex,
  progress::Progress,
  separator::Separator,
  status_bar::StatusBar,
  switch::Switch,
  tab::{Tab, TabBar},
  tag::Tag,
  v_flex,
};
use imprint_core::{
  FlashPhase, FlashProgress, FlashRequest, ImageRef, Settings, TargetDisk, format_bytes,
};
use imprint_device::list_targets;
use imprint_flash::flash;
use imprint_image::inspect;

use crate::actions::{About, OpenImage, Quit, SelectTarget, StartFlash, ToggleSettings};
use crate::theme::Appearance;
use crate::theme::glass;
use crate::widgets::{
  atmosphere, glass_panel, glass_surface, icon_well, muted, picker_row, section_label,
};

enum ProgressEvent {
  Update(FlashProgress),
  Finished(Result<(), String>),
}

pub struct ImprintApp {
  focus: FocusHandle,
  settings: Settings,
  appearance: Appearance,
  image: Option<ImageRef>,
  disks: Vec<TargetDisk>,
  selected: Vec<usize>,
  flashing: bool,
  progress: Option<FlashProgress>,
  error: Option<String>,
  drag_over: bool,
  cancel: Arc<AtomicBool>,
  events: Option<Receiver<ProgressEvent>>,
  _pump: Option<gpui::Task<()>>,
  _appearance: Option<Subscription>,
}

impl ImprintApp {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus = cx.focus_handle();
    focus.focus(window, cx);
    let disks = list_targets(&Settings::default()).unwrap_or_default();

    let weak = cx.weak_entity();
    bind_app_menu_actions(weak.clone(), cx);
    let appearance_sub = window.observe_window_appearance(move |window, cx| {
      let _ = weak.update(cx, |this, cx| {
        if this.appearance == Appearance::System {
          crate::theme::apply_appearance(Appearance::System, Some(window), cx);
          cx.notify();
        }
      });
    });

    Self {
      focus,
      settings: Settings::default(),
      appearance: Appearance::System,
      image: None,
      disks,
      selected: Vec::new(),
      flashing: false,
      progress: None,
      error: None,
      drag_over: false,
      cancel: Arc::new(AtomicBool::new(false)),
      events: None,
      _pump: None,
      _appearance: Some(appearance_sub),
    }
  }

  fn set_appearance(
    &mut self,
    appearance: Appearance,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.appearance = appearance;
    crate::theme::apply_appearance(appearance, Some(window), cx);
    cx.notify();
  }

  fn refresh_disks(&mut self, cx: &mut Context<Self>) {
    match list_targets(&self.settings) {
      Ok(disks) => {
        self.disks = disks;
        self.selected.retain(|i| *i < self.disks.len());
      }
      Err(err) => self.error = Some(err.to_string()),
    }
    cx.notify();
  }

  fn load_image(&mut self, path: PathBuf, cx: &mut Context<Self>) {
    match inspect(&path) {
      Ok(image) => {
        self.image = Some(image);
        self.error = None;
        self.progress = None;
      }
      Err(err) => self.error = Some(err.to_string()),
    }
    cx.notify();
  }

  fn pick_image(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some("Select a disk image".into()),
    });
    cx.spawn(async move |this, cx| match rx.await {
      Ok(Ok(Some(paths))) => {
        if let Some(path) = paths.into_iter().next() {
          this.update(cx, |this, cx| this.load_image(path, cx)).ok();
        }
      }
      Ok(Err(err)) => {
        this
          .update(cx, |this, cx| {
            this.error = Some(err.to_string());
            cx.notify();
          })
          .ok();
      }
      _ => {}
    })
    .detach();
  }

  fn on_open_image(&mut self, _: &OpenImage, window: &mut Window, cx: &mut Context<Self>) {
    self.pick_image(window, cx);
  }

  fn on_select_target(&mut self, _: &SelectTarget, window: &mut Window, cx: &mut Context<Self>) {
    if !self.flashing {
      self.open_drives(window, cx);
    }
  }

  fn on_toggle_settings(
    &mut self,
    _: &ToggleSettings,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if window.has_active_sheet(cx) {
      window.close_sheet(cx);
    } else {
      self.open_settings(window, cx);
    }
  }

  fn on_about(&mut self, _: &About, window: &mut Window, cx: &mut Context<Self>) {
    self.open_about(window, cx);
  }

  fn selected_disks(&self) -> Vec<TargetDisk> {
    self
      .selected
      .iter()
      .filter_map(|i| self.disks.get(*i).cloned())
      .collect()
  }

  fn can_flash(&self) -> bool {
    self.image.is_some() && !self.selected.is_empty() && !self.flashing
  }

  fn start_flash(&mut self, _: &StartFlash, _: &mut Window, cx: &mut Context<Self>) {
    self.begin_flash(cx);
  }

  fn begin_flash(&mut self, cx: &mut Context<Self>) {
    if !self.can_flash() {
      return;
    }
    let Some(image) = self.image.clone() else {
      return;
    };
    let targets = self.selected_disks();
    if targets.is_empty() {
      return;
    }
    let request = FlashRequest {
      image,
      targets,
      verify: self.settings.verify,
      unmount: self.settings.unmount_on_success,
    };
    self.flashing = true;
    self.error = None;
    self.progress = Some(FlashProgress {
      phase: FlashPhase::Preparing,
      bytes_done: 0,
      bytes_total: request.image.write_size().max(1),
      bytes_per_sec: 0,
      target_label: request.targets[0].label(),
      message: "Starting…".into(),
    });
    self.cancel.store(false, Ordering::Relaxed);

    let (tx, rx): (Sender<ProgressEvent>, Receiver<ProgressEvent>) = unbounded();
    self.events = Some(rx);
    let cancel = self.cancel.clone();
    std::thread::Builder::new()
      .name("imprint-flash".into())
      .spawn(move || {
        let result = flash(request, &cancel, |progress| {
          let _ = tx.send(ProgressEvent::Update(progress));
        });
        let _ = tx.send(ProgressEvent::Finished(result.map_err(|e| e.to_string())));
      })
      .ok();

    self.pump_progress(cx);
    cx.notify();
  }

  fn pump_progress(&mut self, cx: &mut Context<Self>) {
    self._pump = Some(cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(50))
          .await;
        let keep = this
          .update(cx, |this, cx| {
            let mut keep = true;
            if let Some(rx) = this.events.clone() {
              while let Ok(event) = rx.try_recv() {
                match event {
                  ProgressEvent::Update(progress) => this.progress = Some(progress),
                  ProgressEvent::Finished(result) => {
                    this.flashing = false;
                    match result {
                      Ok(()) => {
                        if let Some(p) = this.progress.as_mut() {
                          p.phase = FlashPhase::Done;
                          p.message = "Flash complete".into();
                          p.bytes_done = p.bytes_total;
                        }
                      }
                      Err(err) => {
                        this.error = Some(err);
                        if let Some(p) = this.progress.as_mut() {
                          p.phase = FlashPhase::Failed;
                        }
                      }
                    }
                    keep = false;
                  }
                }
              }
            }
            cx.notify();
            keep && this.flashing
          })
          .unwrap_or(false);
        if !keep {
          break;
        }
      }
    }));
  }

  fn flash_another(&mut self, cx: &mut Context<Self>) {
    self.progress = None;
    self.flashing = false;
    self.error = None;
    cx.notify();
  }

  fn click_flash(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.begin_flash(cx);
  }

  fn on_drop_paths(&mut self, paths: &ExternalPaths, _: &mut Window, cx: &mut Context<Self>) {
    self.drag_over = false;
    if let Some(path) = paths.paths().first() {
      self.load_image(path.clone(), cx);
    }
  }

  fn open_drives(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.refresh_disks(cx);
    let view = cx.entity();
    // Defer so the dialog builder is not invoked while ImprintApp is updating.
    window.defer(cx, move |window, cx| {
      window.open_dialog(cx, move |dialog, _, cx| {
        let app = view.read(cx);
        dialog
          .title("Select a drive")
          .w(px(520.))
          .child(muted(
            cx,
            "Internal disks are hidden. Writing erases the drive.",
          ))
          .child(drive_list(&app, view.clone(), cx))
          .footer(
            h_flex()
              .w_full()
              .justify_end()
              .gap_2()
              .child(Button::new("refresh").label("Refresh").on_click({
                let view = view.clone();
                move |_, _, cx| {
                  view.update(cx, |this, cx| this.refresh_disks(cx));
                }
              }))
              .child(
                Button::new("confirm-drives")
                  .primary()
                  .label("Done")
                  .on_click(|_, window, cx| window.close_dialog(cx)),
              ),
          )
      });
    });
  }

  fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let view = cx.entity();
    // Defer so the sheet builder is not invoked while ImprintApp is updating.
    window.defer(cx, move |window, cx| {
      window.open_sheet(cx, move |sheet, _, cx| {
        let app = view.read(cx);
        glass_panel(sheet, cx)
          .title("Settings")
          .size(px(380.))
          .child(
            v_flex()
              .gap_5()
              .py_3()
              .child(
                v_flex()
                  .gap_2()
                  .child(section_label(cx, "Appearance"))
                  .child(
                    glass_surface(v_flex().w_full().gap_3().px_4().py_4(), cx)
                      .child(
                        TabBar::new("appearance")
                          .segmented()
                          .small()
                          .w_full()
                          .selected_index(app.appearance.as_index())
                          .child(Tab::new().label("System"))
                          .child(Tab::new().label("Light"))
                          .child(Tab::new().label("Dark"))
                          .on_click({
                            let view = view.clone();
                            move |ix, window, cx| {
                              let appearance = Appearance::from_index(*ix);
                              view
                                .update(cx, |this, cx| this.set_appearance(appearance, window, cx));
                            }
                          }),
                      )
                      .child(muted(cx, "System follows the OS light or dark setting.")),
                  ),
              )
              .child(
                v_flex().gap_2().child(section_label(cx, "Writing")).child(
                  glass_surface(v_flex().w_full(), cx)
                    .child(setting_switch(
                      "verify",
                      "Validate write",
                      "Re-read the disk and compare every byte.",
                      app.settings.verify,
                      view.clone(),
                      |s, on| s.verify = on,
                      cx,
                    ))
                    .child(Separator::horizontal())
                    .child(setting_switch(
                      "unmount",
                      "Eject on success",
                      "Unmount the drive when writing finishes.",
                      app.settings.unmount_on_success,
                      view.clone(),
                      |s, on| s.unmount_on_success = on,
                      cx,
                    ))
                    .child(Separator::horizontal())
                    .child(setting_switch(
                      "hide-system",
                      "Hide system drives",
                      "Never list internal disks.",
                      app.settings.hide_system_drives,
                      view.clone(),
                      |s, on| s.hide_system_drives = on,
                      cx,
                    )),
                ),
              ),
          )
      });
    });
  }

  fn open_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    window.defer(cx, move |window, cx| {
      window.open_dialog(cx, move |dialog, _, cx| {
        dialog
          .title("About Imprint")
          .w(px(400.))
          .child(
            v_flex()
              .gap_3()
              .items_center()
              .py_3()
              .child(icon_well(cx, IconName::Info, true))
              .child(
                div()
                  .text_lg()
                  .font_weight(FontWeight::SEMIBOLD)
                  .child("Imprint"),
              )
              .child(muted(cx, format!("Version {}", env!("CARGO_PKG_VERSION"))))
              .child(muted(cx, "Flash OS images onto USB drives and SD cards."))
              .child(muted(cx, env!("CARGO_PKG_LICENSE"))),
          )
          .footer(
            h_flex().w_full().justify_end().child(
              Button::new("about-ok")
                .primary()
                .label("OK")
                .on_click(|_, window, cx| window.close_dialog(cx)),
            ),
          )
      });
    });
  }
}

/// Keep About / Settings enabled in the system menu even when a sheet or dialog
/// holds focus, so the action is not only on `ImprintApp`'s dispatch node.
fn bind_app_menu_actions(view: gpui::WeakEntity<ImprintApp>, cx: &mut App) {
  App::on_action(cx, {
    let view = view.clone();
    move |_: &About, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| this.open_about(window, cx));
    }
  });
  App::on_action(cx, {
    let view = view.clone();
    move |_: &ToggleSettings, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| {
        this.on_toggle_settings(&ToggleSettings, window, cx);
      });
    }
  });
}

fn dispatch_on_app(
  view: &gpui::WeakEntity<ImprintApp>,
  cx: &mut App,
  f: impl FnOnce(&mut ImprintApp, &mut Window, &mut Context<ImprintApp>),
) {
  let Some(handle) = cx.active_window() else {
    return;
  };
  let view = view.clone();
  let _ = handle.update(cx, |_, window, cx| {
    let _ = view.update(cx, |this, cx| f(this, window, cx));
  });
}

/// Window-level view under `Root`. Overlay layers live here so their builders
/// can read `ImprintApp` without re-entering it during `ImprintApp::render`.
pub struct ImprintShell {
  app: Entity<ImprintApp>,
}

impl ImprintShell {
  pub fn new(app: Entity<ImprintApp>) -> Self {
    Self { app }
  }
}

impl Render for ImprintShell {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .size_full()
      .child(self.app.clone())
      .children(Root::render_dialog_layer(window, cx))
      .children(Root::render_sheet_layer(window, cx))
      .children(Root::render_notification_layer(window, cx))
  }
}

impl Render for ImprintApp {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let done = self
      .progress
      .as_ref()
      .is_some_and(|p| p.phase == FlashPhase::Done);

    v_flex()
      .id("imprint-root")
      .track_focus(&self.focus)
      .on_action(cx.listener(Self::on_open_image))
      .on_action(cx.listener(Self::on_select_target))
      .on_action(cx.listener(Self::start_flash))
      .on_action(cx.listener(Self::on_toggle_settings))
      .on_action(cx.listener(Self::on_about))
      .on_action(|_: &Quit, _, cx| cx.quit())
      .on_drop(cx.listener(Self::on_drop_paths))
      .drag_over::<ExternalPaths>(|style, _, _, cx| style.bg(cx.theme().drop_target))
      .relative()
      .size_full()
      .bg(cx.theme().transparent)
      .text_color(cx.theme().foreground)
      .child(atmosphere(cx))
      .child(header(cx))
      .child(
        v_flex().flex_1().px_6().py_6().child(if done {
          done_panel(self, cx).into_any_element()
        } else if self.flashing
          || self
            .progress
            .as_ref()
            .is_some_and(|p| p.phase == FlashPhase::Failed)
        {
          progress_panel(self, cx).into_any_element()
        } else {
          write_form(self, cx).into_any_element()
        }),
      )
      .child(status_bar(self, cx))
  }
}

fn header(cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  TitleBar::new()
    .bg(linear_gradient(
      180.,
      linear_color_stop(cx.theme().title_bar, 0.),
      linear_color_stop(cx.theme().title_bar.divide(0.35), 1.),
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
            .child("Imprint"),
        )
        .child(
          Button::new("settings")
            .ghost()
            .small()
            .rounded(ButtonRounded::Large)
            .icon(IconName::Settings)
            .tooltip("Settings")
            .on_click(move |_, window, cx| {
              view.update(cx, |this, cx| this.open_settings(window, cx));
            }),
        ),
    )
}

fn write_form(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let can_write = app.can_flash();
  let view = cx.entity();
  v_flex()
    .size_full()
    .gap_4()
    .child(picker_block(
      cx,
      "Image",
      IconName::FolderOpen,
      app
        .image
        .as_ref()
        .map(|i| i.display_name.clone())
        .unwrap_or_else(|| "No image selected".into()),
      image_subtitle(app),
      app.image.is_some(),
      "Select",
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
    .child(picker_block(
      cx,
      "Target",
      IconName::HardDrive,
      target_title(app),
      target_subtitle(app),
      !app.selected.is_empty(),
      "Select",
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
    .child(div().flex_1())
    .child(
      glass_surface(
        h_flex()
          .w_full()
          .items_center()
          .justify_between()
          .px_4()
          .py_3(),
        cx,
      )
      .child(muted(
        cx,
        if can_write {
          "This will erase the selected drive."
        } else {
          "Choose an image and a drive to continue."
        },
      ))
      .child(
        Button::new("flash")
          .primary()
          .rounded(ButtonRounded::Large)
          .label("Write")
          .disabled(!can_write)
          .on_click(cx.listener(ImprintApp::click_flash)),
      ),
    )
}

fn image_subtitle(app: &ImprintApp) -> String {
  if let Some(image) = &app.image {
    let kind = image.kind.as_str();
    let size = format_bytes(image.file_size);
    if let Some(c) = image.compression {
      format!("{kind} · {size} · {}", c.as_str())
    } else {
      format!("{kind} · {size}")
    }
  } else {
    "ISO, IMG, DMG, or a compressed archive".into()
  }
}

fn target_title(app: &ImprintApp) -> String {
  if app.selected.len() == 1 {
    app.selected_disks()[0].label()
  } else if app.selected.len() > 1 {
    format!("{} drives", app.selected.len())
  } else {
    "No drive selected".into()
  }
}

fn target_subtitle(app: &ImprintApp) -> String {
  if let Some(disk) = app.selected_disks().first() {
    format!("{} · {}", disk.bus.as_str(), format_bytes(disk.size))
  } else {
    "Removable USB or SD card".into()
  }
}

fn picker_block(
  cx: &App,
  label: &'static str,
  icon: IconName,
  title: String,
  subtitle: String,
  ready: bool,
  action: &'static str,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
  let on_click = std::rc::Rc::new(on_click);
  let g = glass(cx);
  v_flex().gap_2().child(section_label(cx, label)).child(
    picker_row(cx)
      .id(label)
      .cursor_pointer()
      .hover(|s| s.bg(g.fill_hover))
      .when(ready, |d| d.border_color(cx.theme().accent.divide(0.70)))
      .on_click({
        let on_click = on_click.clone();
        move |ev, window, cx| on_click(ev, window, cx)
      })
      .child(icon_well(cx, icon, ready))
      .child(
        v_flex()
          .flex_1()
          .min_w_0()
          .gap_1()
          .child(
            div()
              .font_weight(FontWeight::MEDIUM)
              .truncate()
              .child(title),
          )
          .child(muted(cx, subtitle)),
      )
      .child(
        Button::new(format!("{label}-select"))
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
      ),
  )
}

fn progress_panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let progress = app.progress.clone();
  let fraction = progress.as_ref().map(|p| p.fraction()).unwrap_or(0.0);
  let phase = progress
    .as_ref()
    .map(|p| p.phase.as_str())
    .unwrap_or("Working");
  let message = progress
    .as_ref()
    .map(|p| p.message.clone())
    .unwrap_or_default();
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
  let pct = format!("{}%", (fraction * 100.0).round() as u32);
  let view = cx.entity();

  v_flex().size_full().items_center().justify_center().child(
    glass_surface(v_flex().w_full().gap_4().px_5().py_5(), cx)
      .child(section_label(cx, if failed { "Failed" } else { phase }))
      .child(
        div()
          .text_lg()
          .font_weight(FontWeight::MEDIUM)
          .text_color(if failed {
            cx.theme().danger
          } else {
            cx.theme().foreground
          })
          .child(message),
      )
      .child(
        Progress::new("write-progress")
          .value(fraction * 100.0)
          .color(if failed {
            cx.theme().danger
          } else {
            cx.theme().primary
          }),
      )
      .child(
        h_flex()
          .justify_between()
          .child(muted(cx, pct))
          .child(muted(cx, speed)),
      )
      .when(app.flashing, |d| {
        d.child(
          h_flex().child(
            Button::new("cancel")
              .ghost()
              .rounded(ButtonRounded::Large)
              .label("Cancel")
              .on_click(move |_, _, cx| {
                view.update(cx, |this, cx| {
                  this.cancel.store(true, Ordering::Relaxed);
                  cx.notify();
                });
              }),
          ),
        )
      })
      .when(failed, |d| {
        let view = cx.entity();
        d.child(
          Button::new("retry")
            .rounded(ButtonRounded::Large)
            .label("Back")
            .on_click(move |_, _, cx| {
              view.update(cx, |this, cx| this.flash_another(cx));
            }),
        )
      }),
  )
}

fn done_panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let name = app
    .image
    .as_ref()
    .map(|i| i.display_name.clone())
    .unwrap_or_default();
  let view = cx.entity();
  v_flex().size_full().items_center().justify_center().child(
    glass_surface(v_flex().w_full().gap_3().px_5().py_5(), cx)
      .child(
        h_flex()
          .gap_2()
          .items_center()
          .child(
            div()
              .flex()
              .items_center()
              .justify_center()
              .size(px(32.))
              .rounded_full()
              .bg(cx.theme().success.divide(0.18))
              .child(Icon::new(IconName::Check).text_color(cx.theme().success)),
          )
          .child(
            div()
              .text_lg()
              .font_weight(FontWeight::MEDIUM)
              .child("Write complete"),
          ),
      )
      .child(muted(cx, format!("{name} is ready to boot.")))
      .child(
        h_flex()
          .gap_2()
          .mt_3()
          .child(
            Button::new("again")
              .primary()
              .rounded(ButtonRounded::Large)
              .label("Write another")
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
              .label("Keep image")
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

fn status_bar(app: &ImprintApp, cx: &App) -> impl IntoElement {
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
    .left(format!("{} drive(s)", app.disks.len()))
    .right(if let Some(err) = app.error.clone() {
      err
    } else if app.flashing {
      "Writing".into()
    } else if ready {
      "Ready".into()
    } else {
      String::new()
    })
    .when(app.error.is_some(), |d| d.text_color(cx.theme().danger))
    .when(app.error.is_none() && ready, |d| {
      d.text_color(cx.theme().accent)
    })
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
          .child(muted(cx, "No removable drives found.")),
      )
    })
    .children({
      let mut rows = Vec::new();
      for (ix, disk) in app.disks.iter().enumerate() {
        let selected = app.selected.contains(&ix);
        let too_small = app
          .image
          .as_ref()
          .is_some_and(|img| img.write_size() > 0 && disk.size < img.write_size());
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
    .py_2()
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
    .child(
      v_flex()
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
      Tag::warning().small().child("Too small").into_any_element()
    } else if selected {
      Icon::new(IconName::Check)
        .text_color(cx.theme().primary)
        .into_any_element()
    } else {
      div().into_any_element()
    })
}

fn setting_switch(
  id: &'static str,
  title: &'static str,
  hint: &'static str,
  on: bool,
  view: Entity<ImprintApp>,
  flip: fn(&mut Settings, bool),
  cx: &App,
) -> impl IntoElement {
  let g = glass(cx);
  h_flex()
    .id(id)
    .justify_between()
    .items_start()
    .gap_4()
    .px_4()
    .py_3()
    .hover(|s| s.bg(g.fill_hover))
    .child(
      v_flex()
        .gap_1()
        .child(div().child(title))
        .child(muted(cx, hint)),
    )
    .child(Switch::new(id).checked(on).on_click(move |checked, _, cx| {
      let on = *checked;
      view.update(cx, |this, cx| {
        flip(&mut this.settings, on);
        if id == "hide-system" {
          this.refresh_disks(cx);
        }
        cx.notify();
      });
    }))
}
