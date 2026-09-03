use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use gpui::{
  AnyWindowHandle, App, ClickEvent, Context, Entity, ExternalPaths, FocusHandle,
  InteractiveElement, IntoElement, ParentElement, PathPromptOptions, Render, Styled, Subscription,
  Window, div, px,
};
use gpui_component::{
  ActiveTheme as _, Colorize as _, Root, WindowExt as _, notification::Notification, v_flex,
};
use imprint_core::i18n::{self, t, tr};
use imprint_core::{
  FlashPhase, FlashProgress, FlashRequest, ImageRef, LocalePref, Settings, TargetDisk,
};
use imprint_device::list_targets;
use imprint_flash::flash;
use imprint_image::inspect;

use crate::actions::{
  About, AppearanceDark, AppearanceLight, AppearanceSystem, CheckForUpdates, OpenImage,
  OpenRaspberryPi, Quit, RefreshDrives, SelectTarget, StartFlash, ToggleSettings,
};
use crate::rpi::{AppMode, RpiEvent, RpiState};
use crate::theme::Appearance;
use crate::updater;
use crate::views;

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

pub(crate) enum UpdateStatus {
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
  pub(crate) settings: Settings,
  pub(crate) appearance: Appearance,
  pub(crate) mode: AppMode,
  pub(crate) rpi: RpiState,
  pub(crate) image: Option<ImageRef>,
  pub(crate) disks: Vec<TargetDisk>,
  pub(crate) selected: Vec<usize>,
  pub(crate) flashing: bool,
  pub(crate) progress: Option<FlashProgress>,
  pub(crate) error: Option<String>,
  drag_over: bool,
  pub(crate) cancel: Arc<AtomicBool>,
  events: Option<Receiver<ProgressEvent>>,
  _pump: Option<gpui::Task<()>>,
  pub(crate) rpi_events: Option<Receiver<RpiEvent>>,
  pub(crate) _rpi_pump: Option<gpui::Task<()>>,
  pub(crate) update: UpdateStatus,
  update_interactive: bool,
  update_dismissed: bool,
  update_events: Option<Receiver<UpdateEvent>>,
  _update_pump: Option<gpui::Task<()>>,
  _appearance: Option<Subscription>,
  pub(crate) main_window: AnyWindowHandle,
  pub(crate) about_window: Option<AnyWindowHandle>,
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

    let rpi = RpiState::new(window, cx);
    let mut this = Self {
      focus,
      settings: Settings::default(),
      appearance: Appearance::System,
      mode: AppMode::Flash,
      rpi,
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
      rpi_events: None,
      _rpi_pump: None,
      update: UpdateStatus::Idle,
      update_interactive: false,
      update_dismissed: false,
      update_events: None,
      _update_pump: None,
      _appearance: Some(appearance_sub),
      main_window: window.window_handle(),
      about_window: None,
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

  pub(crate) fn set_appearance(
    &mut self,
    appearance: Appearance,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.appearance = appearance;
    crate::theme::apply_appearance(appearance, Some(window), cx);
    crate::install_menus_with(appearance, cx);
    cx.notify();
  }

  pub(crate) fn set_locale(&mut self, locale: LocalePref, cx: &mut Context<Self>) {
    self.settings.locale = locale;
    i18n::set_pref(locale);
    crate::install_menus_with(self.appearance, cx);
    cx.notify();
  }

  pub(crate) fn refresh_disks(&mut self, cx: &mut Context<Self>) {
    match list_targets(&self.settings) {
      Ok(disks) => {
        self.disks = disks;
        self.selected.retain(|i| *i < self.disks.len());
      }
      Err(err) => self.error = Some(err.localized()),
    }
    cx.notify();
  }

  pub(crate) fn load_image(&mut self, path: PathBuf, cx: &mut Context<Self>) {
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

  pub(crate) fn pick_image(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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

  fn on_open_raspberry_pi(&mut self, _: &OpenRaspberryPi, _: &mut Window, cx: &mut Context<Self>) {
    self.open_raspberry_pi(cx);
  }

  fn on_refresh_drives(&mut self, _: &RefreshDrives, _: &mut Window, cx: &mut Context<Self>) {
    self.refresh_disks(cx);
  }

  fn on_appearance_system(
    &mut self,
    _: &AppearanceSystem,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.set_appearance(Appearance::System, window, cx);
  }

  fn on_appearance_light(
    &mut self,
    _: &AppearanceLight,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.set_appearance(Appearance::Light, window, cx);
  }

  fn on_appearance_dark(
    &mut self,
    _: &AppearanceDark,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.set_appearance(Appearance::Dark, window, cx);
  }

  fn on_check_for_updates(
    &mut self,
    _: &CheckForUpdates,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.begin_update_check(true, window, cx);
  }

  pub(crate) fn selected_disks(&self) -> Vec<TargetDisk> {
    self
      .selected
      .iter()
      .filter_map(|i| self.disks.get(*i).cloned())
      .collect()
  }

  pub(crate) fn can_flash(&self) -> bool {
    self.image.is_some() && !self.selected.is_empty() && !self.flashing
  }

  fn start_flash(&mut self, _: &StartFlash, _: &mut Window, cx: &mut Context<Self>) {
    self.begin_flash(cx);
  }

  pub(crate) fn begin_flash(&mut self, cx: &mut Context<Self>) {
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
    let boot = if self.mode == AppMode::RaspberryPi {
      self.rpi.pending_boot.clone()
    } else {
      None
    };
    let request = FlashRequest {
      image,
      targets,
      verify: self.settings.verify,
      unmount: self.settings.unmount_on_success,
      boot,
    };
    self.flashing = true;
    self.error = None;
    self.progress = Some(FlashProgress {
      phase: FlashPhase::Preparing,
      bytes_done: 0,
      bytes_total: request.image.write_size(),
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

  pub(crate) fn flash_another(&mut self, cx: &mut Context<Self>) {
    self.progress = None;
    self.flashing = false;
    self.error = None;
    cx.notify();
  }

  pub(crate) fn click_flash(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.begin_flash(cx);
  }

  fn on_drop_paths(&mut self, paths: &ExternalPaths, _: &mut Window, cx: &mut Context<Self>) {
    self.drag_over = false;
    if let Some(path) = paths.paths().first() {
      self.load_image(path.clone(), cx);
    }
  }

  pub(crate) fn open_drives(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.refresh_disks(cx);
    views::drives::open(cx.entity(), window, cx);
  }

  pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    views::settings::open(cx.entity(), window, cx);
  }

  pub(crate) fn open_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(handle) = self.about_window {
      if handle
        .update(cx, |_, window, _| window.activate_window())
        .is_ok()
      {
        return;
      }
      self.about_window = None;
    }
    views::about::open(cx.entity(), window, cx);
  }

  pub(crate) fn begin_update_check(
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

  pub(crate) fn begin_download(
    &mut self,
    update: cargo_packager_updater::Update,
    cx: &mut Context<Self>,
  ) {
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

  pub(crate) fn dismiss_update_chip(&mut self, cx: &mut Context<Self>) {
    self.update_dismissed = true;
    cx.notify();
  }

  pub(crate) fn restart_to_update(&mut self, cx: &mut Context<Self>) {
    if let Err(err) = updater::relaunch() {
      tracing::error!("failed to relaunch after update: {err}");
      self.update = UpdateStatus::Failed(err);
      self.update_dismissed = false;
      cx.notify();
      return;
    }
    cx.quit();
  }

  pub(crate) fn update_chip_visible(&self) -> bool {
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
  App::on_action(cx, {
    let view = view.clone();
    move |_: &OpenRaspberryPi, cx| {
      dispatch_on_app(&view, cx, |this, _, cx| this.open_raspberry_pi(cx));
    }
  });
  App::on_action(cx, {
    let view = view.clone();
    move |_: &OpenImage, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| this.pick_image(window, cx));
    }
  });
  App::on_action(cx, {
    let view = view.clone();
    move |_: &SelectTarget, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| {
        if !this.flashing {
          this.open_drives(window, cx);
        }
      });
    }
  });
  App::on_action(cx, {
    let view = view.clone();
    move |_: &RefreshDrives, cx| {
      dispatch_on_app(&view, cx, |this, _, cx| this.refresh_disks(cx));
    }
  });
  App::on_action(cx, {
    let view = view.clone();
    move |_: &AppearanceSystem, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| {
        this.set_appearance(Appearance::System, window, cx);
      });
    }
  });
  App::on_action(cx, {
    let view = view.clone();
    move |_: &AppearanceLight, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| {
        this.set_appearance(Appearance::Light, window, cx);
      });
    }
  });
  App::on_action(cx, {
    let view = view.clone();
    move |_: &AppearanceDark, cx| {
      dispatch_on_app(&view, cx, |this, window, cx| {
        this.set_appearance(Appearance::Dark, window, cx);
      });
    }
  });
}

fn dispatch_on_app(
  view: &gpui::WeakEntity<ImprintApp>,
  cx: &mut App,
  f: impl FnOnce(&mut ImprintApp, &mut Window, &mut Context<ImprintApp>),
) {
  let Some(entity) = view.upgrade() else {
    return;
  };
  let handle = entity.read(cx).main_window;
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
      .on_action(cx.listener(Self::on_open_raspberry_pi))
      .on_action(cx.listener(Self::on_refresh_drives))
      .on_action(cx.listener(Self::on_appearance_system))
      .on_action(cx.listener(Self::on_appearance_light))
      .on_action(cx.listener(Self::on_appearance_dark))
      .on_action(|_: &Quit, _, cx| cx.quit())
      .on_drop(cx.listener(Self::on_drop_paths))
      .drag_over::<ExternalPaths>(|style, _, _, cx| {
        style
          .bg(cx.theme().drop_target)
          .border_color(cx.theme().accent.divide(0.45))
      })
      .relative()
      .size_full()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(views::chrome::header(self, cx))
      .child(
        v_flex()
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .px_6()
          .py(
            if self.mode == AppMode::RaspberryPi
              && !self.rpi.downloading()
              && !self.flashing
              && !done
            {
              px(12.)
            } else {
              px(24.)
            },
          )
          .child(if done {
            views::done::panel(self, cx).into_any_element()
          } else if self.rpi.downloading() {
            views::rpi::download_panel(self, cx).into_any_element()
          } else if self.flashing
            || self
              .progress
              .as_ref()
              .is_some_and(|p| p.phase == FlashPhase::Failed)
          {
            views::progress::panel(self, cx).into_any_element()
          } else if self.mode == AppMode::RaspberryPi {
            views::rpi::page(self, cx).into_any_element()
          } else {
            views::write::form(self, cx).into_any_element()
          }),
      )
      .child(views::chrome::status_bar(self, cx))
  }
}
