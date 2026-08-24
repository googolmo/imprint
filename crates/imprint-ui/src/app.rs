use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use gpui::{
  ClickEvent, Context, ExternalPaths, FocusHandle, InteractiveElement, IntoElement, ParentElement,
  PathPromptOptions, Render, StatefulInteractiveElement, Styled, Window, div, prelude::*, px,
  relative,
};
use imprint_core::{
  FlashPhase, FlashProgress, FlashRequest, ImageRef, Settings, TargetDisk, format_bytes,
};
use imprint_device::list_targets;
use imprint_flash::flash;
use imprint_image::inspect;

use crate::actions::{OpenImage, Quit, SelectTarget, StartFlash, ToggleSettings};
use crate::theme::THEME;
use crate::widgets::{
  card, ghost_button, kicker, muted, primary_button, progress_track, step_badge,
};

enum Overlay {
  None,
  Drives,
  Settings,
}

enum ProgressEvent {
  Update(FlashProgress),
  Finished(Result<(), String>),
}

pub struct ImprintApp {
  focus: FocusHandle,
  settings: Settings,
  image: Option<ImageRef>,
  disks: Vec<TargetDisk>,
  selected: Vec<usize>,
  overlay: Overlay,
  flashing: bool,
  progress: Option<FlashProgress>,
  error: Option<String>,
  drag_over: bool,
  cancel: Arc<AtomicBool>,
  events: Option<Receiver<ProgressEvent>>,
  _pump: Option<gpui::Task<()>>,
}

impl ImprintApp {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus = cx.focus_handle();
    focus.focus(window, cx);
    let disks = list_targets(&Settings::default()).unwrap_or_default();
    Self {
      focus,
      settings: Settings::default(),
      image: None,
      disks,
      selected: Vec::new(),
      overlay: Overlay::None,
      flashing: false,
      progress: None,
      error: None,
      drag_over: false,
      cancel: Arc::new(AtomicBool::new(false)),
      events: None,
      _pump: None,
    }
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

  fn on_select_target(&mut self, _: &SelectTarget, _: &mut Window, cx: &mut Context<Self>) {
    self.refresh_disks(cx);
    self.overlay = Overlay::Drives;
    cx.notify();
  }

  fn on_toggle_settings(&mut self, _: &ToggleSettings, _: &mut Window, cx: &mut Context<Self>) {
    self.overlay = match self.overlay {
      Overlay::Settings => Overlay::None,
      _ => Overlay::Settings,
    };
    cx.notify();
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

  fn click_source(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
    if !self.flashing {
      self.pick_image(window, cx);
    }
  }

  fn click_target(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    if !self.flashing {
      self.refresh_disks(cx);
      self.overlay = Overlay::Drives;
      cx.notify();
    }
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
}

impl Render for ImprintApp {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let image_ready = self.image.is_some();
    let target_ready = !self.selected.is_empty();
    let done = self
      .progress
      .as_ref()
      .is_some_and(|p| p.phase == FlashPhase::Done);

    div()
      .id("imprint-root")
      .track_focus(&self.focus)
      .on_action(cx.listener(Self::on_open_image))
      .on_action(cx.listener(Self::on_select_target))
      .on_action(cx.listener(Self::start_flash))
      .on_action(cx.listener(Self::on_toggle_settings))
      .on_action(|_: &Quit, _, cx| cx.quit())
      .on_drop(cx.listener(Self::on_drop_paths))
      .drag_over::<ExternalPaths>(|style, _, _, _| style.border_color(THEME.accent))
      .size_full()
      .flex()
      .flex_col()
      .bg(THEME.bg)
      .text_color(THEME.text)
      .child(header(cx))
      .child(
        div()
          .flex()
          .flex_1()
          .flex_col()
          .px_8()
          .pb_8()
          .gap_6()
          .child(hero())
          .child(if done {
            done_panel(self, cx).into_any_element()
          } else if self.flashing
            || self
              .progress
              .as_ref()
              .is_some_and(|p| p.phase == FlashPhase::Failed)
          {
            progress_panel(self, cx).into_any_element()
          } else {
            steps_panel(self, image_ready, target_ready, cx).into_any_element()
          })
          .child(status_bar(self)),
      )
      .children(match self.overlay {
        Overlay::Drives => Some(drive_overlay(self, cx).into_any_element()),
        Overlay::Settings => Some(settings_overlay(self, cx).into_any_element()),
        Overlay::None => None,
      })
  }
}

fn header(cx: &mut Context<ImprintApp>) -> impl IntoElement {
  div()
    .flex()
    .items_center()
    .justify_between()
    .px_8()
    .py_4()
    .bg(THEME.bg_elevated)
    .child(
      div()
        .flex()
        .items_center()
        .gap_3()
        .child(
          div()
            .size(px(36.))
            .rounded_xl()
            .bg(THEME.accent)
            .flex()
            .items_center()
            .justify_center()
            .text_color(THEME.bg)
            .child("◆"),
        )
        .child(
          div()
            .flex()
            .flex_col()
            .child(div().text_lg().child("Imprint"))
            .child(muted("Flash OS images, safely.")),
        ),
    )
    .child(
      ghost_button("settings", "Settings").on_click(cx.listener(|this, _, _, cx| {
        this.overlay = Overlay::Settings;
        cx.notify();
      })),
    )
}

fn hero() -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_1()
    .child(kicker("SELECT  ·  TARGET  ·  FLASH"))
    .child(
      div()
        .text_3xl()
        .child("Write an image to a USB drive or SD card."),
    )
}

fn steps_panel(
  app: &ImprintApp,
  image_ready: bool,
  target_ready: bool,
  cx: &mut Context<ImprintApp>,
) -> impl IntoElement {
  let flash_enabled = app.can_flash();
  div().flex().flex_col().gap_5().flex_1().child(
    div()
      .flex()
      .gap_5()
      .flex_1()
      .child(source_card(app, image_ready, cx))
      .child(target_card(app, target_ready, cx))
      .child(flash_card(flash_enabled, cx)),
  )
}

fn source_card(app: &ImprintApp, ready: bool, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let title = app
    .image
    .as_ref()
    .map(|i| i.display_name.clone())
    .unwrap_or_else(|| "Flash from file".into());
  let subtitle = if let Some(image) = &app.image {
    let kind = image.kind.as_str();
    let size = format_bytes(image.file_size);
    if let Some(c) = image.compression {
      format!("{kind} · {size} · {}", c.as_str())
    } else {
      format!("{kind} · {size}")
    }
  } else {
    "ISO, IMG, DMG — or a compressed archive".into()
  };
  card()
    .id("source-card")
    .cursor_pointer()
    .hover(|s| s.bg(THEME.card_hover))
    .on_click(cx.listener(ImprintApp::click_source))
    .gap_4()
    .child(step_badge(1, !ready, ready))
    .child(div().text_xl().child(title))
    .child(muted(subtitle))
    .when(app.drag_over, |d| d.border_color(THEME.accent))
    .child(
      div()
        .mt_auto()
        .text_sm()
        .text_color(THEME.accent)
        .child(if ready {
          "Change image"
        } else {
          "Drop a file or browse"
        }),
    )
}

fn target_card(app: &ImprintApp, ready: bool, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let title = if app.selected.len() == 1 {
    app.selected_disks()[0].label()
  } else if app.selected.len() > 1 {
    format!("{} drives", app.selected.len())
  } else {
    "Select target".into()
  };
  let subtitle = if let Some(disk) = app.selected_disks().first() {
    format!("{} · {}", disk.bus.as_str(), format_bytes(disk.size))
  } else {
    "Removable USB / SD only — system disks stay hidden".into()
  };
  card()
    .id("target-card")
    .cursor_pointer()
    .hover(|s| s.bg(THEME.card_hover))
    .on_click(cx.listener(ImprintApp::click_target))
    .gap_4()
    .child(step_badge(2, ready || app.image.is_some(), ready))
    .child(div().text_xl().child(title))
    .child(muted(subtitle))
    .child(
      div()
        .mt_auto()
        .text_sm()
        .text_color(THEME.accent)
        .child(if ready {
          "Change target"
        } else {
          "Choose a drive"
        }),
    )
}

fn flash_card(enabled: bool, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  card()
    .id("flash-card")
    .gap_4()
    .child(step_badge(3, enabled, false))
    .child(div().text_xl().child("Flash!"))
    .child(muted("Writes every byte, then verifies the disk."))
    .child(div().mt_auto().child(
      primary_button("flash", "Flash!", enabled).on_click(cx.listener(ImprintApp::click_flash)),
    ))
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

  card()
    .max_w(px(720.))
    .mx_auto()
    .my_auto()
    .gap_4()
    .child(kicker(if failed { "FAILED" } else { phase }))
    .child(div().text_2xl().child(message))
    .child(progress_track(fraction))
    .child(
      div()
        .flex()
        .justify_between()
        .child(muted(format!("{}%", (fraction * 100.0).round() as u32)))
        .child(muted(speed)),
    )
    .when(app.flashing, |d| {
      d.child(
        ghost_button("cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
          this.cancel.store(true, Ordering::Relaxed);
          cx.notify();
        })),
      )
    })
    .when(failed, |d| {
      d.child(
        ghost_button("retry", "Back")
          .on_click(cx.listener(|this, _, _, cx| this.flash_another(cx))),
      )
    })
}

fn done_panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let name = app
    .image
    .as_ref()
    .map(|i| i.display_name.clone())
    .unwrap_or_default();
  card()
    .max_w(px(640.))
    .mx_auto()
    .my_auto()
    .items_center()
    .gap_4()
    .child(
      div()
        .size(px(72.))
        .rounded_full()
        .bg(THEME.ok)
        .flex()
        .items_center()
        .justify_center()
        .text_color(THEME.bg)
        .text_2xl()
        .child("✓"),
    )
    .child(div().text_2xl().child("Flash complete"))
    .child(muted(format!("{name} is ready to boot.")))
    .child(
      div()
        .flex()
        .gap_3()
        .mt_4()
        .child(
          primary_button("again", "Flash another", true)
            .on_click(cx.listener(|this, _, _, cx| this.flash_another(cx))),
        )
        .child(
          ghost_button("same", "Use same image").on_click(cx.listener(|this, _, _, cx| {
            this.progress = None;
            this.selected.clear();
            cx.notify();
          })),
        ),
    )
}

fn status_bar(app: &ImprintApp) -> impl IntoElement {
  let drives = format!("{} drive(s) detected", app.disks.len());
  div()
    .flex()
    .justify_between()
    .items_center()
    .child(muted(drives))
    .children(
      app
        .error
        .clone()
        .map(|e| div().text_sm().text_color(THEME.danger).child(e)),
    )
}

fn drive_overlay(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  div()
    .absolute()
    .inset_0()
    .flex()
    .items_center()
    .justify_center()
    .bg(gpui::rgba(0x000000aa))
    .on_mouse_down(
      gpui::MouseButton::Left,
      cx.listener(|this, _, _, cx| {
        this.overlay = Overlay::None;
        cx.notify();
      }),
    )
    .child(
      card()
        .id("drive-picker")
        .w(px(560.))
        .max_h(relative(0.8))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .gap_3()
        .child(div().text_xl().child("Select target"))
        .child(muted(
          "System disks are hidden. Writing will erase the drive.",
        ))
        .child(
          div()
            .id("drive-list")
            .flex()
            .flex_col()
            .gap_2()
            .overflow_y_scroll()
            .children({
              let view = cx.entity();
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
                ));
              }
              rows
            }),
        )
        .when(app.disks.is_empty(), |d| {
          d.child(muted(
            "No removable drives found. Plug in a USB stick or SD card.",
          ))
        })
        .child(
          div()
            .flex()
            .justify_end()
            .gap_2()
            .mt_2()
            .child(
              ghost_button("refresh", "Refresh")
                .on_click(cx.listener(|this, _, _, cx| this.refresh_disks(cx))),
            )
            .child(
              primary_button("confirm-drives", "Select", true).on_click(cx.listener(
                |this, _, _, cx| {
                  this.overlay = Overlay::None;
                  cx.notify();
                },
              )),
            ),
        ),
    )
}

fn drive_row(
  ix: usize,
  label: String,
  detail: String,
  selected: bool,
  too_small: bool,
  view: gpui::Entity<ImprintApp>,
) -> impl IntoElement {
  let border = if selected { THEME.accent } else { THEME.line };
  div()
    .id(("drive", ix))
    .flex()
    .items_center()
    .justify_between()
    .p_3()
    .rounded_lg()
    .border_1()
    .border_color(border)
    .cursor_pointer()
    .hover(|s| s.bg(THEME.card_hover))
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
      div()
        .flex()
        .flex_col()
        .gap_1()
        .child(label)
        .child(muted(detail)),
    )
    .child(if too_small {
      div()
        .text_xs()
        .text_color(THEME.warn)
        .child("Too small")
        .into_any_element()
    } else if selected {
      div()
        .text_xs()
        .text_color(THEME.accent)
        .child("Selected")
        .into_any_element()
    } else {
      div()
        .text_xs()
        .text_color(THEME.muted)
        .child(" ")
        .into_any_element()
    })
}

fn settings_overlay(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  div()
    .absolute()
    .inset_0()
    .flex()
    .items_center()
    .justify_center()
    .bg(gpui::rgba(0x000000aa))
    .on_mouse_down(
      gpui::MouseButton::Left,
      cx.listener(|this, _, _, cx| {
        this.overlay = Overlay::None;
        cx.notify();
      }),
    )
    .child(
      card()
        .id("settings")
        .w(px(420.))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .gap_4()
        .child(div().text_xl().child("Settings"))
        .child(toggle_row(
          "verify",
          "Validate write",
          "Re-read the disk and compare every byte.",
          app.settings.verify,
          cx,
          |s| s.verify = !s.verify,
        ))
        .child(toggle_row(
          "unmount",
          "Eject on success",
          "Unmount / eject the drive when flashing finishes.",
          app.settings.unmount_on_success,
          cx,
          |s| s.unmount_on_success = !s.unmount_on_success,
        ))
        .child(toggle_row(
          "hide-system",
          "Hide system drives",
          "Never list internal disks. Keep this on.",
          app.settings.hide_system_drives,
          cx,
          |s| s.hide_system_drives = !s.hide_system_drives,
        ))
        .child(
          ghost_button("close-settings", "Done").on_click(cx.listener(|this, _, _, cx| {
            this.overlay = Overlay::None;
            this.refresh_disks(cx);
          })),
        ),
    )
}

fn toggle_row(
  id: &'static str,
  title: &'static str,
  hint: &'static str,
  on: bool,
  cx: &mut Context<ImprintApp>,
  flip: fn(&mut Settings),
) -> impl IntoElement {
  div()
    .id(id)
    .flex()
    .items_center()
    .justify_between()
    .gap_4()
    .cursor_pointer()
    .on_click(cx.listener(move |this, _, _, cx| {
      flip(&mut this.settings);
      cx.notify();
    }))
    .child(div().flex().flex_col().child(title).child(muted(hint)))
    .child(
      div()
        .w(px(44.))
        .h(px(24.))
        .rounded_full()
        .bg(if on { THEME.accent } else { THEME.accent_dim })
        .flex()
        .items_center()
        .px_1()
        .child(
          div()
            .size(px(18.))
            .rounded_full()
            .bg(THEME.text)
            .when(on, |d| d.ml_auto()),
        ),
    )
}
