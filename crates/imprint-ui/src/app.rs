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
  ActiveTheme as _, Colorize as _, Icon, IconName, Root, Sizable as _, TitleBar, WindowExt as _,
  button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants as _},
  h_flex,
  menu::{DropdownMenu as _, PopupMenuItem},
  notification::Notification,
  progress::ProgressCircle,
  separator::Separator,
  spinner::Spinner,
  status_bar::StatusBar,
  switch::Switch,
  tab::{Tab, TabBar},
  tag::Tag,
  tooltip::Tooltip,
  v_flex,
};
use imprint_core::i18n::{self, t, tr};
use imprint_core::{
  FlashPhase, FlashProgress, FlashRequest, ImageRef, Language, LocalePref, Settings, TargetDisk,
  format_bytes,
};
use imprint_device::list_targets;
use imprint_flash::flash;
use imprint_image::inspect;

use crate::actions::{
  About, CheckForUpdates, OpenImage, Quit, SelectTarget, StartFlash, ToggleSettings,
};
use crate::theme::Appearance;
use crate::theme::glass;
use crate::updater;
use crate::widgets::{
  atmosphere, brand_mark, glass_panel, glass_surface, hover_fill, icon_badge, icon_well, muted,
  section_label, stage_connector, stage_kicker,
};

enum ProgressEvent {
  Update(FlashProgress),
  Finished(Result<(), String>),
}

enum UpdateEvent {
  None,
  Available(cargo_packager_updater::Update),
  Failed(String),
  DownloadProgress { received: u64, total: Option<u64> },
  Installed,
}

enum UpdateStatus {
  Idle,
  Checking,
  UpToDate,
  Available(cargo_packager_updater::Update),
  Downloading {
    update: cargo_packager_updater::Update,
    received: u64,
    total: Option<u64>,
  },
  Installed {
    version: String,
  },
  Failed(String),
}

struct UpdateToast;

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
  update: UpdateStatus,
  update_interactive: bool,
  update_dismissed: bool,
  update_events: Option<Receiver<UpdateEvent>>,
  _update_pump: Option<gpui::Task<()>>,
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

    let mut this = Self {
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
      update: UpdateStatus::Idle,
      update_interactive: false,
      update_dismissed: false,
      update_events: None,
      _update_pump: None,
      _appearance: Some(appearance_sub),
    };
    if let Some(version) = updater::take_update_notice() {
      if version == env!("CARGO_PKG_VERSION") {
        window.push_notification(
          Notification::success(tr("update.updated_to", &[("version", &version)]))
            .id::<UpdateToast>(),
          cx,
        );
      }
    }
    if updater::is_configured() {
      this.begin_update_check(false, window, cx);
    }
    this
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

  fn set_locale(&mut self, locale: LocalePref, cx: &mut Context<Self>) {
    self.settings.locale = locale;
    i18n::set_pref(locale);
    crate::install_menus(cx);
    cx.notify();
  }

  fn refresh_disks(&mut self, cx: &mut Context<Self>) {
    match list_targets(&self.settings) {
      Ok(disks) => {
        self.disks = disks;
        self.selected.retain(|i| *i < self.disks.len());
      }
      Err(err) => self.error = Some(err.localized()),
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
      Err(err) => self.error = Some(err.localized()),
    }
    cx.notify();
  }

  fn pick_image(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some(t("image.pick_prompt").into()),
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

  fn on_check_for_updates(
    &mut self,
    _: &CheckForUpdates,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.begin_update_check(true, window, cx);
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
      message: t("progress.starting"),
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
        let _ = tx.send(ProgressEvent::Finished(result.map_err(|e| e.localized())));
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
                          p.message = t("progress.complete");
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

  fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let view = cx.entity();
    // Defer so the sheet builder is not invoked while ImprintApp is updating.
    window.defer(cx, move |window, cx| {
      window.open_sheet(cx, move |sheet, _, cx| {
        let app = view.read(cx);
        glass_panel(sheet, cx)
          .title(t("settings.title"))
          .size(px(380.))
          .child(
            v_flex()
              .gap_5()
              .py_3()
              .child(
                v_flex()
                  .gap_2()
                  .child(section_label(cx, t("settings.appearance")))
                  .child(
                    glass_surface(v_flex().w_full().gap_3().px_4().py_4(), cx)
                      .child(
                        TabBar::new("appearance")
                          .segmented()
                          .small()
                          .w_full()
                          .selected_index(app.appearance.as_index())
                          .child(Tab::new().label(t("settings.appearance_system")))
                          .child(Tab::new().label(t("settings.appearance_light")))
                          .child(Tab::new().label(t("settings.appearance_dark")))
                          .on_click({
                            let view = view.clone();
                            move |ix, window, cx| {
                              let appearance = Appearance::from_index(*ix);
                              view
                                .update(cx, |this, cx| this.set_appearance(appearance, window, cx));
                            }
                          }),
                      )
                      .child(muted(cx, t("settings.appearance_hint"))),
                  ),
              )
              .child(
                v_flex()
                  .gap_2()
                  .child(section_label(cx, t("settings.language")))
                  .child(
                    glass_surface(v_flex().w_full().gap_3().px_4().py_4(), cx)
                      .child(locale_dropdown(i18n::active_language(), view.clone()))
                      .child(muted(cx, t("settings.language_hint"))),
                  ),
              )
              .child(
                v_flex()
                  .gap_2()
                  .child(section_label(cx, t("settings.writing")))
                  .child(
                    glass_surface(v_flex().w_full(), cx)
                      .child(setting_switch(
                        "verify",
                        t("settings.verify"),
                        t("settings.verify_hint"),
                        app.settings.verify,
                        view.clone(),
                        |s, on| s.verify = on,
                        cx,
                      ))
                      .child(Separator::horizontal())
                      .child(setting_switch(
                        "unmount",
                        t("settings.eject"),
                        t("settings.eject_hint"),
                        app.settings.unmount_on_success,
                        view.clone(),
                        |s, on| s.unmount_on_success = on,
                        cx,
                      ))
                      .child(Separator::horizontal())
                      .child(setting_switch(
                        "hide-system",
                        t("settings.hide_system"),
                        t("settings.hide_system_hint"),
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
    let view = cx.entity();
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

  fn begin_update_check(
    &mut self,
    interactive: bool,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match &self.update {
      UpdateStatus::Checking | UpdateStatus::Downloading { .. } => {
        self.update_interactive = interactive || self.update_interactive;
        self.update_dismissed = false;
        cx.notify();
        return;
      }
      UpdateStatus::Available(_) | UpdateStatus::Installed { .. } if interactive => {
        self.update_dismissed = false;
        cx.notify();
        return;
      }
      _ => {}
    }

    self.update_interactive = interactive;
    self.update_dismissed = false;
    self.update = UpdateStatus::Checking;
    let (tx, rx): (Sender<UpdateEvent>, Receiver<UpdateEvent>) = unbounded();
    self.update_events = Some(rx);
    std::thread::Builder::new()
      .name("imprint-update-check".into())
      .spawn(move || {
        let event = match updater::check_for_update() {
          Ok(Some(update)) => UpdateEvent::Available(update),
          Ok(None) => UpdateEvent::None,
          Err(err) => UpdateEvent::Failed(err),
        };
        let _ = tx.send(event);
      })
      .ok();
    self.pump_updates(cx);
    cx.notify();
  }

  fn begin_download(&mut self, update: cargo_packager_updater::Update, cx: &mut Context<Self>) {
    if !updater::is_packaged() {
      self.update = UpdateStatus::Failed(t("update.unpackaged"));
      cx.notify();
      return;
    }
    self.update_dismissed = false;
    self.update = UpdateStatus::Downloading {
      update: update.clone(),
      received: 0,
      total: None,
    };
    let (tx, rx): (Sender<UpdateEvent>, Receiver<UpdateEvent>) = unbounded();
    self.update_events = Some(rx);
    std::thread::Builder::new()
      .name("imprint-update-install".into())
      .spawn(move || {
        let received = std::sync::atomic::AtomicU64::new(0);
        let result = update.download_and_install_extended(
          |chunk, total| {
            let n = received.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
            let _ = tx.send(UpdateEvent::DownloadProgress { received: n, total });
          },
          || {},
        );
        match result {
          Ok(()) => {
            let _ = tx.send(UpdateEvent::Installed);
          }
          Err(err) => {
            let _ = tx.send(UpdateEvent::Failed(err.to_string()));
          }
        }
      })
      .ok();
    self.pump_updates(cx);
    cx.notify();
  }

  fn pump_updates(&mut self, cx: &mut Context<Self>) {
    self._update_pump = Some(cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(50))
          .await;
        let outcome = this.update(cx, |this, cx| {
          let mut keep = true;
          let mut toast = None;
          if let Some(rx) = this.update_events.clone() {
            while let Ok(event) = rx.try_recv() {
              match event {
                UpdateEvent::None => {
                  this.update = UpdateStatus::UpToDate;
                  if this.update_interactive {
                    toast = Some(UpdateToastKind::UpToDate);
                  }
                  keep = false;
                }
                UpdateEvent::Available(update) => {
                  this.update = UpdateStatus::Available(update);
                  this.update_dismissed = false;
                  keep = false;
                }
                UpdateEvent::Failed(err) => {
                  let auto_check =
                    matches!(this.update, UpdateStatus::Checking) && !this.update_interactive;
                  if auto_check {
                    tracing::info!("auto-update check failed: {err}");
                    this.update = UpdateStatus::Idle;
                  } else {
                    tracing::warn!("update failed: {err}");
                    this.update = UpdateStatus::Failed(err);
                  }
                  keep = false;
                }
                UpdateEvent::DownloadProgress { received, total } => {
                  if let UpdateStatus::Downloading { update, .. } = &this.update {
                    this.update = UpdateStatus::Downloading {
                      update: update.clone(),
                      received,
                      total,
                    };
                  }
                }
                UpdateEvent::Installed => {
                  let version = match &this.update {
                    UpdateStatus::Downloading { update, .. } => update.version.clone(),
                    UpdateStatus::Installed { version } => version.clone(),
                    _ => env!("CARGO_PKG_VERSION").to_string(),
                  };
                  updater::mark_update_installed(&version);
                  this.update = UpdateStatus::Installed { version };
                  this.update_dismissed = false;
                  keep = false;
                }
              }
            }
          }
          let keep = keep
            && matches!(
              this.update,
              UpdateStatus::Checking | UpdateStatus::Downloading { .. }
            );
          cx.notify();
          (keep, toast)
        });
        let (keep, toast) = outcome.unwrap_or((false, None));
        if let Some(kind) = toast {
          if let Some(handle) = cx.update(|cx| cx.active_window()) {
            let _ = handle.update(cx, |_, window, cx| {
              window.push_notification(kind.into_notification(), cx);
            });
          }
        }
        if !keep {
          break;
        }
      }
    }));
  }

  fn dismiss_update_chip(&mut self, cx: &mut Context<Self>) {
    self.update_dismissed = true;
    cx.notify();
  }

  fn restart_to_update(&mut self, cx: &mut Context<Self>) {
    if let Err(err) = updater::relaunch() {
      tracing::error!("failed to relaunch after update: {err}");
      self.update = UpdateStatus::Failed(err);
      self.update_dismissed = false;
      cx.notify();
      return;
    }
    cx.quit();
  }

  fn update_chip_visible(&self) -> bool {
    if self.update_dismissed {
      return false;
    }
    match &self.update {
      UpdateStatus::Checking if self.update_interactive => true,
      UpdateStatus::Available(_)
      | UpdateStatus::Downloading { .. }
      | UpdateStatus::Installed { .. }
      | UpdateStatus::Failed(_) => true,
      _ => false,
    }
  }
}

enum UpdateToastKind {
  UpToDate,
}

impl UpdateToastKind {
  fn into_notification(self) -> Notification {
    match self {
      Self::UpToDate => Notification::success(t("update.up_to_date")).id::<UpdateToast>(),
    }
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
    move |_: &CheckForUpdates, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| {
        this.begin_update_check(true, window, cx);
      });
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
      .on_action(cx.listener(Self::on_check_for_updates))
      .on_action(|_: &Quit, _, cx| cx.quit())
      .on_drop(cx.listener(Self::on_drop_paths))
      .drag_over::<ExternalPaths>(|style, _, _, cx| {
        style
          .bg(cx.theme().drop_target)
          .border_color(cx.theme().accent.divide(0.45))
      })
      .relative()
      .size_full()
      .bg(cx.theme().transparent)
      .text_color(cx.theme().foreground)
      .child(atmosphere(cx))
      .child(header(self, cx))
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

fn header(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
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

fn write_form(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  v_flex().size_full().items_center().justify_center().child(
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
    d.border_color(cx.theme().accent.divide(0.50)).shadow(vec![
      gpui_component::box_shadow(px(0.), px(-1.), px(1.), px(0.), g.highlight),
      gpui_component::box_shadow(px(0.), px(12.), px(32.), px(-6.), g.shadow),
      gpui_component::box_shadow(
        px(0.),
        px(0.),
        px(28.),
        px(2.),
        cx.theme().accent.divide(0.22),
      ),
    ])
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
      .px_1()
      .text_center()
      .font_weight(FontWeight::SEMIBOLD)
      .truncate()
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
  let g = glass(cx);
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
      .border_color(cx.theme().accent.divide(0.40))
      .shadow(vec![
        gpui_component::box_shadow(px(0.), px(-1.), px(1.), px(0.), g.highlight),
        gpui_component::box_shadow(
          px(0.),
          px(14.),
          px(36.),
          px(-4.),
          cx.theme().primary.divide(0.40),
        ),
        gpui_component::box_shadow(
          px(0.),
          px(0.),
          px(28.),
          px(2.),
          cx.theme().accent.divide(0.28),
        ),
      ])
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

fn progress_panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let progress = app.progress.clone();
  let fraction = progress.as_ref().map(|p| p.fraction()).unwrap_or(0.0);
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
  let preparing = progress
    .as_ref()
    .is_some_and(|p| p.phase == FlashPhase::Preparing);
  let pct_value = (fraction * 100.0).round();
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
        .value(if preparing { 0.0 } else { pct_value })
        .loading(preparing && !failed)
        .color(ring_color)
        .child(if preparing && !failed {
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

fn done_panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
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
    .left(tr("status.drives", &[("n", &app.disks.len().to_string())]))
    .right(if let Some(err) = app.error.clone() {
      err
    } else if app.flashing {
      t("status.writing")
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

fn setting_switch(
  id: &'static str,
  title: impl Into<String>,
  hint: impl Into<String>,
  on: bool,
  view: Entity<ImprintApp>,
  flip: fn(&mut Settings, bool),
  cx: &App,
) -> impl IntoElement {
  let title = title.into();
  let hint = hint.into();
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

fn locale_dropdown(current: Language, view: Entity<ImprintApp>) -> impl IntoElement {
  Button::new("locale")
    .w_full()
    .outline()
    .label(current.native_name())
    .dropdown_caret(true)
    .dropdown_menu(move |menu, _, _| {
      Language::ALL.into_iter().fold(menu, |menu, lang| {
        menu.item(
          PopupMenuItem::new(lang.native_name())
            .checked(current == lang)
            .on_click({
              let view = view.clone();
              move |_, _, cx| {
                view.update(cx, |this, cx| {
                  this.set_locale(LocalePref::Language(lang), cx);
                });
              }
            }),
        )
      })
    })
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
