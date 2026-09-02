use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crossbeam_channel::{Receiver, unbounded};
use gpui::{
  App, AppContext as _, Context, Entity, PathPromptOptions, SharedString, Subscription, Window,
};
use gpui_component::{
  IndexPath,
  input::InputState,
  searchable_list::SearchableListItem,
  select::{SearchableVec, SelectEvent, SelectState},
};
use imprint_core::i18n::t;
use imprint_core::{BootCustomization, FlashPhase, FlashProgress};
use imprint_image::inspect;
use imprint_rpi::{
  Catalog, Device, InitFormat, OsItem, PiCustomization, cached_path, download_image, fetch_catalog,
  fetch_subitems, generate_boot,
};

use crate::app::ImprintApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppMode {
  Flash,
  RaspberryPi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RpiStep {
  Device,
  Os,
  Config,
  Storage,
}

impl RpiStep {
  pub(crate) fn index(self) -> usize {
    match self {
      Self::Device => 0,
      Self::Os => 1,
      Self::Config => 2,
      Self::Storage => 3,
    }
  }
}

pub(crate) enum CatalogStatus {
  Idle,
  Loading,
  Ready(Catalog),
  Failed(String),
}

pub(crate) enum DownloadStatus {
  Idle,
  Running {
    received: u64,
    total: Option<u64>,
    name: String,
  },
}

#[derive(Clone)]
pub(crate) struct Choice {
  value: SharedString,
  label: SharedString,
}

impl SearchableListItem for Choice {
  type Value = SharedString;

  fn title(&self) -> SharedString {
    self.label.clone()
  }

  fn value(&self) -> &Self::Value {
    &self.value
  }

  fn matches(&self, query: &str) -> bool {
    let q = query.to_lowercase();
    self.label.to_lowercase().contains(&q) || self.value.to_lowercase().contains(&q)
  }
}

pub(crate) type ChoiceSelect = SelectState<SearchableVec<Choice>>;

pub(crate) struct RpiFields {
  pub hostname: Entity<InputState>,
  pub username: Entity<InputState>,
  pub password: Entity<InputState>,
  pub wifi_ssid: Entity<InputState>,
  pub wifi_password: Entity<InputState>,
  pub ssh_key: Entity<InputState>,
  pub wifi_country: Entity<ChoiceSelect>,
  pub timezone: Entity<ChoiceSelect>,
  pub keyboard: Entity<ChoiceSelect>,
}

pub(crate) struct RpiState {
  pub catalog: CatalogStatus,
  pub step: RpiStep,
  pub os_stack: Vec<usize>,
  pub selected_device: Option<usize>,
  pub selected_os: Option<OsItem>,
  pub set_hostname: bool,
  pub set_user: bool,
  pub set_wifi: bool,
  pub set_ssh: bool,
  pub set_locale: bool,
  pub download: DownloadStatus,
  pub pending_boot: Option<BootCustomization>,
  pub fields: RpiFields,
  _choice_subs: Vec<Subscription>,
}

pub(crate) enum RpiEvent {
  Catalog(std::result::Result<Catalog, String>),
  Subitems {
    path: Vec<usize>,
    result: std::result::Result<Vec<OsItem>, String>,
  },
  DownloadProgress {
    received: u64,
    total: Option<u64>,
  },
  Downloaded(std::result::Result<PathBuf, String>),
}

impl RpiFields {
  fn new(window: &mut Window, cx: &mut Context<ImprintApp>, subs: &mut Vec<Subscription>) -> Self {
    Self {
      hostname: input(window, cx, "raspberrypi", Some("raspberrypi"), false),
      username: input(window, cx, "pi", Some("pi"), false),
      password: input(window, cx, "", None, true),
      wifi_ssid: input(window, cx, "SSID", None, false),
      wifi_password: input(window, cx, "", None, true),
      ssh_key: input(window, cx, "ssh-ed25519 …", None, false),
      wifi_country: choice_select(window, cx, WIFI_COUNTRIES, "GB", subs, |this, cx| {
        this.rpi.set_wifi = true;
        cx.notify();
      }),
      timezone: choice_select(window, cx, TIMEZONES, "Europe/London", subs, |this, cx| {
        this.rpi.set_locale = true;
        cx.notify();
      }),
      keyboard: choice_select(window, cx, KEYBOARDS, "us", subs, |this, cx| {
        this.rpi.set_locale = true;
        cx.notify();
      }),
    }
  }
}

fn input(
  window: &mut Window,
  cx: &mut Context<ImprintApp>,
  placeholder: &'static str,
  default: Option<&'static str>,
  masked: bool,
) -> Entity<InputState> {
  cx.new(|cx| {
    let mut state = InputState::new(window, cx).placeholder(placeholder);
    if let Some(value) = default {
      state = state.default_value(value);
    }
    if masked { state.masked(true) } else { state }
  })
}

impl RpiState {
  pub(crate) fn new(window: &mut Window, cx: &mut Context<ImprintApp>) -> Self {
    let mut choice_subs = Vec::new();
    let fields = RpiFields::new(window, cx, &mut choice_subs);
    Self {
      catalog: CatalogStatus::Idle,
      step: RpiStep::Device,
      os_stack: Vec::new(),
      selected_device: None,
      selected_os: None,
      set_hostname: false,
      set_user: false,
      set_wifi: false,
      set_ssh: false,
      set_locale: false,
      download: DownloadStatus::Idle,
      pending_boot: None,
      fields,
      _choice_subs: choice_subs,
    }
  }

  pub(crate) fn catalog(&self) -> Option<&Catalog> {
    match &self.catalog {
      CatalogStatus::Ready(catalog) => Some(catalog),
      _ => None,
    }
  }

  pub(crate) fn selected_device<'a>(&'a self) -> Option<&'a Device> {
    let catalog = self.catalog()?;
    self
      .selected_device
      .and_then(|i| catalog.imager.devices.get(i))
  }

  pub(crate) fn current_os_list(&self) -> &[OsItem] {
    let Some(catalog) = self.catalog() else {
      return &[];
    };
    let mut list = catalog.os_list.as_slice();
    for &ix in &self.os_stack {
      list = list
        .get(ix)
        .map(|item| item.subitems.as_slice())
        .unwrap_or(&[]);
    }
    list
  }

  pub(crate) fn downloading(&self) -> bool {
    matches!(self.download, DownloadStatus::Running { .. })
  }
}

impl ImprintApp {
  pub(crate) fn open_raspberry_pi(&mut self, cx: &mut Context<Self>) {
    if self.flashing || self.rpi.downloading() {
      return;
    }
    self.mode = AppMode::RaspberryPi;
    self.error = None;
    if matches!(
      self.rpi.catalog,
      CatalogStatus::Idle | CatalogStatus::Failed(_)
    ) {
      self.fetch_rpi_catalog(cx);
    }
    cx.notify();
  }

  pub(crate) fn leave_raspberry_pi(&mut self, cx: &mut Context<Self>) {
    if self.flashing || self.rpi.downloading() {
      return;
    }
    self.mode = AppMode::Flash;
    self.rpi.download = DownloadStatus::Idle;
    cx.notify();
  }

  pub(crate) fn fetch_rpi_catalog(&mut self, cx: &mut Context<Self>) {
    self.rpi.catalog = CatalogStatus::Loading;
    let (tx, rx) = unbounded();
    self.start_rpi_pump(rx, cx);
    std::thread::Builder::new()
      .name("imprint-rpi-catalog".into())
      .spawn(move || {
        let event = match fetch_catalog() {
          Ok(catalog) => RpiEvent::Catalog(Ok(catalog)),
          Err(err) => RpiEvent::Catalog(Err(err.localized())),
        };
        let _ = tx.send(event);
      })
      .ok();
    cx.notify();
  }

  pub(crate) fn select_rpi_device(&mut self, index: usize, cx: &mut Context<Self>) {
    self.rpi.selected_device = Some(index);
    if let Some(os) = &self.rpi.selected_os {
      if let Some(device) = self.rpi.selected_device() {
        if !imprint_rpi::os_matches_device(os, device) {
          self.rpi.selected_os = None;
        }
      }
    }
    cx.notify();
  }

  pub(crate) fn open_rpi_os_item(&mut self, index: usize, cx: &mut Context<Self>) {
    let Some(item) = self.rpi.current_os_list().get(index).cloned() else {
      return;
    };
    if item.is_category() {
      if item.subitems.is_empty() && item.subitems_url.is_some() {
        self.fetch_rpi_subitems(index, cx);
        return;
      }
      self.rpi.os_stack.push(index);
      cx.notify();
      return;
    }
    if item.is_image() {
      self.rpi.selected_os = Some(item);
      cx.notify();
    }
  }

  pub(crate) fn pick_custom_os(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some(t("rpi.os.custom_pick").into()),
    });
    cx.spawn(async move |this, cx| match rx.await {
      Ok(Ok(Some(paths))) => {
        if let Some(path) = paths.into_iter().next() {
          this
            .update(cx, |this, cx| this.set_custom_os(path, cx))
            .ok();
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

  fn set_custom_os(&mut self, path: PathBuf, cx: &mut Context<Self>) {
    let mut item = OsItem::from_local_path(&path);
    match inspect(&path) {
      Ok(image) => {
        let write = image.write_size();
        if write > 0 {
          item.extract_size = write;
        }
        item.image_download_size = image.file_size;
        self.image = Some(image);
        self.error = None;
      }
      Err(err) => {
        self.error = Some(err.localized());
        cx.notify();
        return;
      }
    }
    self.rpi.selected_os = Some(item);
    cx.notify();
  }

  pub(crate) fn set_rpi_init_format(&mut self, format: InitFormat, cx: &mut Context<Self>) {
    if let Some(os) = &mut self.rpi.selected_os {
      if os.is_local() {
        os.set_init_format(format);
      }
    }
    cx.notify();
  }

  pub(crate) fn rpi_os_back(&mut self, cx: &mut Context<Self>) {
    self.rpi.os_stack.pop();
    cx.notify();
  }

  pub(crate) fn set_rpi_step(&mut self, step: RpiStep, cx: &mut Context<Self>) {
    if self.rpi_can_open_step(step) {
      self.rpi.step = step;
      cx.notify();
    }
  }

  pub(crate) fn rpi_can_open_step(&self, step: RpiStep) -> bool {
    match step {
      RpiStep::Device => true,
      RpiStep::Os => self.rpi.selected_device.is_some(),
      RpiStep::Config | RpiStep::Storage => self.rpi.selected_os.is_some(),
    }
  }

  pub(crate) fn rpi_next_step(&mut self, cx: &mut Context<Self>) {
    let next = match self.rpi.step {
      RpiStep::Device => RpiStep::Os,
      RpiStep::Os => RpiStep::Config,
      RpiStep::Config => RpiStep::Storage,
      RpiStep::Storage => return,
    };
    self.set_rpi_step(next, cx);
  }

  pub(crate) fn rpi_prev_step(&mut self, cx: &mut Context<Self>) {
    let prev = match self.rpi.step {
      RpiStep::Device => return,
      RpiStep::Os => RpiStep::Device,
      RpiStep::Config => RpiStep::Os,
      RpiStep::Storage => RpiStep::Config,
    };
    self.set_rpi_step(prev, cx);
  }

  pub(crate) fn can_rpi_write(&self) -> bool {
    self.rpi.selected_os.is_some()
      && !self.selected.is_empty()
      && !self.flashing
      && !self.rpi.downloading()
  }

  pub(crate) fn needed_write_size(&self) -> u64 {
    if self.mode == AppMode::RaspberryPi {
      if let Some(os) = &self.rpi.selected_os {
        if os.extract_size > 0 {
          return os.extract_size;
        }
      }
    }
    self.image.as_ref().map(|i| i.write_size()).unwrap_or(0)
  }

  pub(crate) fn begin_rpi_write(&mut self, cx: &mut Context<Self>) {
    if !self.can_rpi_write() {
      return;
    }
    let Some(os) = self.rpi.selected_os.clone() else {
      return;
    };
    match self.rpi_boot_config(cx) {
      Ok(boot) => self.rpi.pending_boot = boot,
      Err(err) => {
        self.error = Some(err);
        cx.notify();
        return;
      }
    }
    self.error = None;
    if let Some(path) = cached_path(&os) {
      self.load_image(path, cx);
      self.begin_flash(cx);
      return;
    }
    self.rpi.download = DownloadStatus::Running {
      received: 0,
      total: (os.image_download_size > 0).then_some(os.image_download_size),
      name: os.name.clone(),
    };
    self.progress = Some(FlashProgress {
      phase: FlashPhase::Preparing,
      bytes_done: 0,
      bytes_total: os.image_download_size,
      bytes_per_sec: 0,
      target_label: os.name.clone(),
      message: t("rpi.downloading"),
    });
    let (tx, rx) = unbounded();
    self.start_rpi_pump(rx, cx);
    let cancel = self.cancel.clone();
    cancel.store(false, Ordering::Relaxed);
    std::thread::Builder::new()
      .name("imprint-rpi-download".into())
      .spawn(move || {
        let result = download_image(&os, &cancel, |received, total| {
          let _ = tx.send(RpiEvent::DownloadProgress { received, total });
        });
        let _ = tx.send(RpiEvent::Downloaded(result.map_err(|e| e.localized())));
      })
      .ok();
    cx.notify();
  }

  fn rpi_boot_config(&self, cx: &App) -> std::result::Result<Option<BootCustomization>, String> {
    let Some(os) = &self.rpi.selected_os else {
      return Ok(None);
    };
    let init = os.init_format();
    if !init.supports_customisation() {
      return Ok(None);
    }
    let cfg = self.collect_pi_config(cx);
    generate_boot(init, &cfg).map_err(|err| err.localized())
  }

  fn collect_pi_config(&self, cx: &App) -> PiCustomization {
    let value = |entity: &Entity<InputState>| nonempty(&entity.read(cx).value());
    PiCustomization {
      hostname: self
        .rpi
        .set_hostname
        .then(|| value(&self.rpi.fields.hostname))
        .flatten(),
      username: self
        .rpi
        .set_user
        .then(|| value(&self.rpi.fields.username))
        .flatten(),
      password: self
        .rpi
        .set_user
        .then(|| value(&self.rpi.fields.password))
        .flatten(),
      ssh_enabled: self.rpi.set_ssh,
      ssh_public_key: self
        .rpi
        .set_ssh
        .then(|| value(&self.rpi.fields.ssh_key))
        .flatten(),
      wifi_ssid: self
        .rpi
        .set_wifi
        .then(|| value(&self.rpi.fields.wifi_ssid))
        .flatten(),
      wifi_password: self
        .rpi
        .set_wifi
        .then(|| value(&self.rpi.fields.wifi_password))
        .flatten(),
      wifi_country: self
        .rpi
        .set_wifi
        .then(|| selected_choice(&self.rpi.fields.wifi_country, cx))
        .flatten(),
      timezone: self
        .rpi
        .set_locale
        .then(|| selected_choice(&self.rpi.fields.timezone, cx))
        .flatten(),
      keyboard: self
        .rpi
        .set_locale
        .then(|| selected_choice(&self.rpi.fields.keyboard, cx))
        .flatten(),
    }
  }

  fn fetch_rpi_subitems(&mut self, index: usize, cx: &mut Context<Self>) {
    let mut path = self.rpi.os_stack.clone();
    path.push(index);
    let Some(mut item) = self.rpi.current_os_list().get(index).cloned() else {
      return;
    };
    let (tx, rx) = unbounded();
    self.start_rpi_pump(rx, cx);
    std::thread::Builder::new()
      .name("imprint-rpi-subitems".into())
      .spawn(move || {
        let result = fetch_subitems(&mut item)
          .map(|()| item.subitems)
          .map_err(|err| err.localized());
        let _ = tx.send(RpiEvent::Subitems { path, result });
      })
      .ok();
  }

  fn start_rpi_pump(&mut self, rx: Receiver<RpiEvent>, cx: &mut Context<Self>) {
    self.rpi_events = Some(rx);
    self._rpi_pump = Some(cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(50))
          .await;
        let keep = this
          .update(cx, |this, cx| this.drain_rpi_events(cx))
          .unwrap_or(false);
        if !keep {
          break;
        }
      }
    }));
  }

  fn drain_rpi_events(&mut self, cx: &mut Context<Self>) -> bool {
    let Some(rx) = self.rpi_events.clone() else {
      return false;
    };
    let mut keep = true;
    while let Ok(event) = rx.try_recv() {
      match event {
        RpiEvent::Catalog(Ok(catalog)) => {
          if self.rpi.selected_device.is_none() {
            self.rpi.selected_device = imprint_rpi::default_device_index(&catalog.imager.devices);
          }
          self.rpi.catalog = CatalogStatus::Ready(catalog);
          keep = false;
        }
        RpiEvent::Catalog(Err(err)) => {
          self.rpi.catalog = CatalogStatus::Failed(err.clone());
          self.error = Some(err);
          keep = false;
        }
        RpiEvent::Subitems { path, result } => {
          match result {
            Ok(subitems) => {
              if let CatalogStatus::Ready(catalog) = &mut self.rpi.catalog {
                if let Some(item) = os_item_at_mut(&mut catalog.os_list, &path) {
                  item.subitems = subitems;
                }
              }
              if let Some(index) = path.last().copied() {
                let parent = &path[..path.len().saturating_sub(1)];
                if self.rpi.os_stack == parent {
                  self.rpi.os_stack.push(index);
                }
              }
            }
            Err(err) => self.error = Some(err),
          }
          keep = false;
        }
        RpiEvent::DownloadProgress { received, total } => {
          if let DownloadStatus::Running { name, .. } = &self.rpi.download {
            let name = name.clone();
            self.rpi.download = DownloadStatus::Running {
              received,
              total,
              name,
            };
          }
        }
        RpiEvent::Downloaded(Ok(path)) => {
          self.rpi.download = DownloadStatus::Idle;
          keep = false;
          self.load_image(path, cx);
          self.begin_flash(cx);
        }
        RpiEvent::Downloaded(Err(err)) => {
          self.rpi.download = DownloadStatus::Idle;
          self.error = Some(err);
          self.progress = None;
          keep = false;
        }
      }
    }
    cx.notify();
    keep
      && (matches!(self.rpi.catalog, CatalogStatus::Loading)
        || matches!(self.rpi.download, DownloadStatus::Running { .. }))
  }
}

fn nonempty(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_string())
  }
}

fn selected_choice(state: &Entity<ChoiceSelect>, cx: &App) -> Option<String> {
  state
    .read(cx)
    .selected_value()
    .and_then(|value| nonempty(value))
}

fn choice_select(
  window: &mut Window,
  cx: &mut Context<ImprintApp>,
  options: &'static [(&'static str, &'static str)],
  current: &'static str,
  subs: &mut Vec<Subscription>,
  on_pick: impl Fn(&mut ImprintApp, &mut Context<ImprintApp>) + 'static,
) -> Entity<ChoiceSelect> {
  let items = SearchableVec::new(
    options
      .iter()
      .map(|(value, label)| Choice {
        value: SharedString::from(*value),
        label: SharedString::from(*label),
      })
      .collect::<Vec<_>>(),
  );
  let index = options
    .iter()
    .position(|(value, _)| *value == current)
    .map(IndexPath::new);
  let state = cx.new(|cx| SelectState::new(items, index, window, cx).searchable(true));
  subs.push(cx.subscribe(
    &state,
    move |this, _, event: &SelectEvent<SearchableVec<Choice>>, cx| {
      if matches!(event, SelectEvent::Confirm(Some(_))) {
        on_pick(this, cx);
      }
    },
  ));
  state
}

const WIFI_COUNTRIES: &[(&str, &str)] = &[
  ("GB", "United Kingdom (GB)"),
  ("US", "United States (US)"),
  ("DE", "Germany (DE)"),
  ("FR", "France (FR)"),
  ("NL", "Netherlands (NL)"),
  ("BE", "Belgium (BE)"),
  ("AT", "Austria (AT)"),
  ("CH", "Switzerland (CH)"),
  ("SE", "Sweden (SE)"),
  ("NO", "Norway (NO)"),
  ("DK", "Denmark (DK)"),
  ("FI", "Finland (FI)"),
  ("IE", "Ireland (IE)"),
  ("IT", "Italy (IT)"),
  ("ES", "Spain (ES)"),
  ("PT", "Portugal (PT)"),
  ("PL", "Poland (PL)"),
  ("CZ", "Czechia (CZ)"),
  ("JP", "Japan (JP)"),
  ("KR", "South Korea (KR)"),
  ("CN", "China (CN)"),
  ("TW", "Taiwan (TW)"),
  ("HK", "Hong Kong (HK)"),
  ("SG", "Singapore (SG)"),
  ("AU", "Australia (AU)"),
  ("NZ", "New Zealand (NZ)"),
  ("CA", "Canada (CA)"),
  ("BR", "Brazil (BR)"),
  ("IN", "India (IN)"),
  ("MX", "Mexico (MX)"),
];

const TIMEZONES: &[(&str, &str)] = &[
  ("UTC", "UTC"),
  ("Europe/London", "London (Europe/London)"),
  ("Europe/Berlin", "Berlin (Europe/Berlin)"),
  ("Europe/Paris", "Paris (Europe/Paris)"),
  ("Europe/Amsterdam", "Amsterdam (Europe/Amsterdam)"),
  ("Europe/Stockholm", "Stockholm (Europe/Stockholm)"),
  ("America/New_York", "New York (America/New_York)"),
  ("America/Chicago", "Chicago (America/Chicago)"),
  ("America/Denver", "Denver (America/Denver)"),
  ("America/Los_Angeles", "Los Angeles (America/Los_Angeles)"),
  ("America/Toronto", "Toronto (America/Toronto)"),
  ("America/Sao_Paulo", "São Paulo (America/Sao_Paulo)"),
  ("Asia/Shanghai", "Shanghai (Asia/Shanghai)"),
  ("Asia/Hong_Kong", "Hong Kong (Asia/Hong_Kong)"),
  ("Asia/Tokyo", "Tokyo (Asia/Tokyo)"),
  ("Asia/Seoul", "Seoul (Asia/Seoul)"),
  ("Asia/Singapore", "Singapore (Asia/Singapore)"),
  ("Asia/Kolkata", "Kolkata (Asia/Kolkata)"),
  ("Asia/Dubai", "Dubai (Asia/Dubai)"),
  ("Australia/Sydney", "Sydney (Australia/Sydney)"),
  ("Australia/Melbourne", "Melbourne (Australia/Melbourne)"),
  ("Pacific/Auckland", "Auckland (Pacific/Auckland)"),
];

const KEYBOARDS: &[(&str, &str)] = &[
  ("us", "US (us)"),
  ("gb", "UK (gb)"),
  ("de", "German (de)"),
  ("fr", "French (fr)"),
  ("es", "Spanish (es)"),
  ("it", "Italian (it)"),
  ("nl", "Dutch (nl)"),
  ("se", "Swedish (se)"),
  ("no", "Norwegian (no)"),
  ("dk", "Danish (dk)"),
  ("pl", "Polish (pl)"),
  ("pt", "Portuguese (pt)"),
  ("jp", "Japanese (jp)"),
  ("kr", "Korean (kr)"),
  ("cn", "Chinese (cn)"),
  ("latam", "Latin American (latam)"),
];

fn os_item_at_mut<'a>(list: &'a mut [OsItem], path: &[usize]) -> Option<&'a mut OsItem> {
  let (first, rest) = path.split_first()?;
  let item = list.get_mut(*first)?;
  if rest.is_empty() {
    Some(item)
  } else {
    os_item_at_mut(&mut item.subitems, rest)
  }
}
