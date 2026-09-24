//! OpenWrt first-boot customisation via `sysupgrade.tgz`.

use flate2::{Compression, write::GzEncoder};
use imprint_core::{BootCustomization, BootFile, Error, Result};
use sha_crypt::{Sha512Params, sha512_simple};

#[derive(Debug, Clone, Default)]
pub struct OpenWrtCustomization {
  pub hostname: Option<String>,
  pub root_password: Option<String>,
  pub ssh_public_keys: Option<String>,
  pub wifi_enabled: bool,
  pub wifi_ssid: Option<String>,
  pub wifi_password: Option<String>,
  pub wifi_country: Option<String>,
  pub lan_ip: Option<String>,
  pub lan_prefix: Option<u8>,
  pub dhcp_enabled: Option<bool>,
  pub dhcp_start: Option<u16>,
  pub dhcp_limit: Option<u16>,
  pub dhcp_leasetime: Option<String>,
}

impl OpenWrtCustomization {
  pub fn is_empty(&self) -> bool {
    self.hostname.is_none()
      && self.root_password.is_none()
      && self.ssh_public_keys.is_none()
      && !self.wifi_enabled
      && self.wifi_country.is_none()
      && self.lan_ip.is_none()
      && self.dhcp_enabled.is_none()
  }
}

pub fn generate_boot(cfg: &OpenWrtCustomization) -> Result<Option<BootCustomization>> {
  if cfg.is_empty() {
    return Ok(None);
  }
  if cfg.wifi_enabled {
    let password = cfg.wifi_password.as_deref().unwrap_or("");
    if cfg
      .wifi_ssid
      .as_deref()
      .filter(|s| !s.trim().is_empty())
      .is_none()
      || !(8..=63).contains(&password.len())
    {
      return Err(Error::BootConfig(
        "Wi-Fi AP requires an SSID and an 8–63 character password".into(),
      ));
    }
    if cfg
      .wifi_country
      .as_deref()
      .filter(|s| s.len() == 2)
      .is_none()
    {
      return Err(Error::BootConfig(
        "Wi-Fi AP requires a two-letter country code".into(),
      ));
    }
  }
  let script = script(cfg)?;
  let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
  {
    let mut tar = tar::Builder::new(&mut encoder);
    let mut header = tar::Header::new_ustar();
    header.set_size(script.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    tar
      .append_data(
        &mut header,
        "etc/uci-defaults/99-imprint",
        script.as_bytes(),
      )
      .map_err(|e| Error::BootConfig(e.to_string()))?;
    tar.finish().map_err(|e| Error::BootConfig(e.to_string()))?;
  }
  let payload = encoder
    .finish()
    .map_err(|e| Error::BootConfig(e.to_string()))?;
  Ok(Some(BootCustomization {
    files: vec![BootFile {
      name: "sysupgrade.tgz".into(),
      contents: payload,
    }],
    cmdline_append: None,
  }))
}

fn script(cfg: &OpenWrtCustomization) -> Result<String> {
  let mut out = String::from("#!/bin/sh\n");
  if let Some(v) = cfg.hostname.as_deref().filter(|v| !v.trim().is_empty()) {
    out += &format!("uci set system.@system[0].hostname={}\n", q(v));
  }
  if let Some(password) = cfg.root_password.as_deref().filter(|v| !v.is_empty()) {
    let clean = password.replace(['\r', '\n'], "");
    let hash = sha512_simple(
      &clean,
      &Sha512Params::new(10_000).map_err(|e| Error::BootConfig(format!("{e:?}")))?,
    )
    .map_err(|e| Error::BootConfig(format!("{e:?}")))?;
    out += &format!(
      "sed -i 's|^root:[!*]*:|root:{}:|' /etc/shadow\n",
      hash.replace('|', "\\|")
    );
  }
  if let Some(keys) = cfg.ssh_public_keys.as_deref() {
    let keys: Vec<_> = keys
      .lines()
      .map(str::trim)
      .filter(|x| !x.is_empty())
      .collect();
    if !keys.is_empty() {
      out += "mkdir -p /etc/dropbear\ncat > /etc/dropbear/authorized_keys <<'IMPRINT_KEYS'\n";
      for key in keys {
        out += key;
        out += "\n";
      }
      out += "IMPRINT_KEYS\nchmod 600 /etc/dropbear/authorized_keys\n";
    }
  }
  if let Some(ip) = cfg.lan_ip.as_deref().filter(|v| !v.trim().is_empty()) {
    out += &format!("uci set network.lan.ipaddr={}\n", q(ip));
    if let Some(prefix) = cfg.lan_prefix {
      out += &format!(
        "uci set network.lan.netmask={}\n",
        q(&prefix_to_mask(prefix)?)
      );
    }
  }
  if let Some(enabled) = cfg.dhcp_enabled {
    out += &format!(
      "uci set dhcp.lan.ignore={}\n",
      if enabled { "'0'" } else { "'1'" }
    );
  }
  if let Some(v) = cfg.dhcp_start {
    out += &format!("uci set dhcp.lan.start={}\n", q(&v.to_string()));
  }
  if let Some(v) = cfg.dhcp_limit {
    out += &format!("uci set dhcp.lan.limit={}\n", q(&v.to_string()));
  }
  if let Some(v) = cfg
    .dhcp_leasetime
    .as_deref()
    .filter(|v| !v.trim().is_empty())
  {
    out += &format!("uci set dhcp.lan.leasetime={}\n", q(v));
  }
  if let Some(country) = cfg.wifi_country.as_deref().filter(|v| !v.trim().is_empty()) {
    out += &format!(
      "if uci -q get wireless.@wifi-device[0] >/dev/null; then uci set wireless.@wifi-device[0].country={}; fi\n",
      q(country)
    );
  }
  if cfg.wifi_enabled {
    out += &format!(
      "if uci -q get wireless.@wifi-iface[0] >/dev/null; then uci set wireless.@wifi-iface[0].mode='ap'; uci set wireless.@wifi-iface[0].ssid={}; uci set wireless.@wifi-iface[0].encryption='psk2'; uci set wireless.@wifi-iface[0].key={}; uci set wireless.@wifi-iface[0].disabled='0'; fi\n",
      q(cfg.wifi_ssid.as_deref().unwrap()),
      q(cfg.wifi_password.as_deref().unwrap())
    );
  }
  out += "uci commit\nexit 0\n";
  Ok(out)
}
fn q(value: &str) -> String {
  format!("'{}'", value.replace('\'', "'\\''"))
}
fn prefix_to_mask(prefix: u8) -> Result<String> {
  if prefix > 32 {
    return Err(Error::BootConfig("LAN prefix must be 0–32".into()));
  }
  let n = if prefix == 0 {
    0
  } else {
    u32::MAX << (32 - prefix)
  };
  Ok(std::net::Ipv4Addr::from(n).to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Read;
  #[test]
  fn archive_is_safe() {
    let cfg = OpenWrtCustomization {
      hostname: Some("o'reilly".into()),
      root_password: Some("secret".into()),
      ..Default::default()
    };
    let boot = generate_boot(&cfg).unwrap().unwrap();
    let mut gz = flate2::read::GzDecoder::new(&boot.files[0].contents[..]);
    let mut bytes = Vec::new();
    gz.read_to_end(&mut bytes).unwrap();
    let mut ar = tar::Archive::new(&bytes[..]);
    let mut e = ar.entries().unwrap().next().unwrap().unwrap();
    assert_eq!(
      e.path().unwrap().to_str().unwrap(),
      "etc/uci-defaults/99-imprint"
    );
    let mut s = String::new();
    e.read_to_string(&mut s).unwrap();
    assert!(!s.contains("secret"));
    assert!(s.contains("$6$"));
    assert!(s.contains("o'\\''reilly"));
  }
}
