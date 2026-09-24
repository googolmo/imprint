use crate::app::ImprintApp;
use gpui::{App, AppContext as _, Context, Entity, PathPromptOptions, Window};
use gpui_kit::{component::input::InputState, gpui};
use imprint_core::BootCustomization;
use imprint_image::looks_like_openwrt;
use imprint_openwrt::{OpenWrtCustomization, generate_boot};
use std::path::PathBuf;

pub(crate) struct OpenWrtFields {
  pub hostname: Entity<InputState>,
  pub password: Entity<InputState>,
  pub keys: Entity<InputState>,
  pub ssid: Entity<InputState>,
  pub wifi_password: Entity<InputState>,
  pub country: Entity<InputState>,
  pub lan_ip: Entity<InputState>,
  pub prefix: Entity<InputState>,
  pub dhcp_start: Entity<InputState>,
  pub dhcp_limit: Entity<InputState>,
  pub leasetime: Entity<InputState>,
}
pub(crate) struct OpenWrtState {
  pub path: Option<PathBuf>,
  pub wifi: bool,
  pub dhcp: bool,
  pub fields: OpenWrtFields,
  pub pending_boot: Option<BootCustomization>,
}
fn field(
  window: &mut Window,
  cx: &mut Context<ImprintApp>,
  placeholder: &'static str,
  masked: bool,
) -> Entity<InputState> {
  cx.new(|cx| {
    let s = InputState::new(window, cx).placeholder(placeholder);
    if masked { s.masked(true) } else { s }
  })
}
impl OpenWrtState {
  pub(crate) fn new(window: &mut Window, cx: &mut Context<ImprintApp>) -> Self {
    Self {
      path: None,
      wifi: false,
      dhcp: true,
      pending_boot: None,
      fields: OpenWrtFields {
        hostname: field(window, cx, "openwrt", false),
        password: field(window, cx, "Root password", true),
        keys: field(window, cx, "ssh-ed25519 … (one per line)", false),
        ssid: field(window, cx, "Wi-Fi name", false),
        wifi_password: field(window, cx, "Wi-Fi password", true),
        country: field(window, cx, "Country code, e.g. JP", false),
        lan_ip: field(window, cx, "192.168.1.1", false),
        prefix: field(window, cx, "24", false),
        dhcp_start: field(window, cx, "100", false),
        dhcp_limit: field(window, cx, "150", false),
        leasetime: field(window, cx, "12h", false),
      },
    }
  }
}
impl ImprintApp {
  pub(crate) fn open_openwrt(&mut self, cx: &mut Context<Self>) {
    if !self.flashing {
      self.mode = crate::rpi::AppMode::OpenWrt;
      self.sync_disk_watch(cx);
      cx.notify();
    }
  }
  pub(crate) fn leave_openwrt(&mut self, cx: &mut Context<Self>) {
    if !self.flashing {
      self.mode = crate::rpi::AppMode::Flash;
      self.sync_disk_watch(cx);
      cx.notify();
    }
  }
  pub(crate) fn pick_openwrt_image(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some("Choose an OpenWrt disk image".into()),
    });
    cx.spawn(async move |this, cx| {
      if let Ok(Ok(Some(paths))) = rx.await {
        if let Some(path) = paths.into_iter().next() {
          this
            .update(cx, |this, cx| {
              if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(looks_like_openwrt)
              {
                this.error =
                  Some("OpenWrt mode requires an OpenWrt .img/.img.gz/.img.xz disk image".into());
                cx.notify();
                return;
              }
              this.load_image(path.clone(), cx);
              if this.image.is_some() {
                this.openwrt.path = Some(path);
              }
            })
            .ok();
        }
      }
    })
    .detach();
  }
  pub(crate) fn openwrt_boot(&self, cx: &App) -> Result<Option<BootCustomization>, String> {
    let v = |x: &Entity<InputState>| {
      let s = x.read(cx).value().trim().to_string();
      (!s.is_empty()).then_some(s)
    };
    let f = &self.openwrt.fields;
    generate_boot(&OpenWrtCustomization {
      hostname: v(&f.hostname),
      root_password: v(&f.password),
      ssh_public_keys: v(&f.keys),
      wifi_enabled: self.openwrt.wifi,
      wifi_ssid: v(&f.ssid),
      wifi_password: v(&f.wifi_password),
      wifi_country: v(&f.country),
      lan_ip: v(&f.lan_ip),
      lan_prefix: v(&f.prefix).and_then(|s| s.parse().ok()),
      dhcp_enabled: Some(self.openwrt.dhcp),
      dhcp_start: v(&f.dhcp_start).and_then(|s| s.parse().ok()),
      dhcp_limit: v(&f.dhcp_limit).and_then(|s| s.parse().ok()),
      dhcp_leasetime: v(&f.leasetime),
    })
    .map_err(|e| e.localized())
  }
  pub(crate) fn begin_openwrt_write(&mut self, cx: &mut Context<Self>) {
    if self.image.is_none() || self.selected.is_empty() {
      return;
    }
    match self.openwrt_boot(cx) {
      Ok(b) => self.openwrt.pending_boot = b,
      Err(e) => {
        self.error = Some(e);
        cx.notify();
        return;
      }
    }
    self.begin_flash(cx);
  }
}
