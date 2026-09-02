use sha_crypt::{Sha512Params, sha512_simple};

use imprint_core::{BootCustomization, BootFile, Error, Result};

use crate::catalog::InitFormat;

/// First-boot options the user can set in Raspberry Pi mode.
#[derive(Debug, Clone, Default)]
pub struct PiCustomization {
  pub hostname: Option<String>,
  pub username: Option<String>,
  pub password: Option<String>,
  pub ssh_enabled: bool,
  pub ssh_public_key: Option<String>,
  pub wifi_ssid: Option<String>,
  pub wifi_password: Option<String>,
  pub wifi_country: Option<String>,
  pub timezone: Option<String>,
  pub keyboard: Option<String>,
}

impl PiCustomization {
  pub fn is_empty(&self) -> bool {
    self.hostname.is_none()
      && self.username.is_none()
      && self.password.is_none()
      && !self.ssh_enabled
      && self.ssh_public_key.is_none()
      && self.wifi_ssid.is_none()
      && self.timezone.is_none()
      && self.keyboard.is_none()
  }
}

pub fn generate_boot(init: InitFormat, cfg: &PiCustomization) -> Result<Option<BootCustomization>> {
  if matches!(init, InitFormat::None) || cfg.is_empty() {
    return Ok(None);
  }
  match init {
    InitFormat::CloudInit | InitFormat::CloudInitRpi => Ok(Some(cloud_init(init, cfg)?)),
    InitFormat::Systemd => Ok(Some(systemd(cfg)?)),
    InitFormat::None => Ok(None),
  }
}

fn cloud_init(init: InitFormat, cfg: &PiCustomization) -> Result<BootCustomization> {
  let mut files = vec![
    BootFile {
      name: "user-data".into(),
      contents: user_data(init, cfg)?,
    },
    BootFile {
      name: "meta-data".into(),
      contents: meta_data(cfg),
    },
  ];
  if cfg.wifi_ssid.is_some() {
    files.push(BootFile {
      name: "network-config".into(),
      contents: network_config(cfg),
    });
  }
  Ok(BootCustomization {
    files,
    cmdline_append: None,
  })
}

fn user_data(init: InitFormat, cfg: &PiCustomization) -> Result<String> {
  let mut out = String::from("#cloud-config\n");
  if let Some(hostname) = cfg.hostname.as_deref().filter(|s| !s.is_empty()) {
    out.push_str(&format!("hostname: {}\n", yaml_string(hostname)));
    out.push_str("manage_etc_hosts: true\n");
  }
  if let Some(tz) = cfg.timezone.as_deref().filter(|s| !s.is_empty()) {
    out.push_str(&format!("timezone: {}\n", yaml_string(tz)));
  }
  if let Some(layout) = cfg.keyboard.as_deref().filter(|s| !s.is_empty()) {
    out.push_str("keyboard:\n");
    out.push_str("  model: pc105\n");
    out.push_str(&format!("  layout: {}\n", yaml_string(layout)));
  }

  let username = cfg.username.as_deref().filter(|s| !s.is_empty());
  let password = cfg.password.as_deref().filter(|s| !s.is_empty());
  let ssh_key = cfg.ssh_public_key.as_deref().filter(|s| !s.is_empty());
  if username.is_some() || password.is_some() || ssh_key.is_some() {
    let name = username.unwrap_or("pi");
    out.push_str("user:\n");
    out.push_str(&format!("  name: {}\n", yaml_string(name)));
    out.push_str("  shell: /bin/bash\n");
    if let Some(password) = password {
      out.push_str("  lock_passwd: false\n");
      out.push_str(&format!(
        "  passwd: {}\n",
        yaml_string(&hash_password(password)?)
      ));
    }
    if let Some(key) = ssh_key {
      out.push_str("  ssh_authorized_keys:\n");
      for line in key.lines().map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str(&format!("    - {}\n", yaml_string(line)));
      }
    }
  }

  if cfg.ssh_enabled {
    out.push_str("ssh_pwauth: true\n");
    if matches!(init, InitFormat::CloudInitRpi) {
      out.push_str("enable_ssh: true\n");
      out.push_str("rpi:\n");
      out.push_str("  enable_ssh: true\n");
    }
  }
  Ok(out)
}

fn meta_data(cfg: &PiCustomization) -> String {
  let hostname = cfg
    .hostname
    .as_deref()
    .filter(|s| !s.is_empty())
    .unwrap_or("raspberrypi");
  let id = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0);
  format!("instance-id: imprint-{id}\nlocal-hostname: {hostname}\n")
}

fn network_config(cfg: &PiCustomization) -> String {
  let mut out = String::from(
    "network:\n  version: 2\n  ethernets:\n    eth0:\n      dhcp4: true\n      optional: true\n",
  );
  let Some(ssid) = cfg.wifi_ssid.as_deref().filter(|s| !s.is_empty()) else {
    return out;
  };
  out.push_str("  wifis:\n    wlan0:\n      dhcp4: true\n      optional: true\n");
  if let Some(country) = cfg.wifi_country.as_deref().filter(|s| !s.is_empty()) {
    out.push_str(&format!(
      "      regulatory-domain: {}\n",
      yaml_string(country)
    ));
  }
  out.push_str("      access-points:\n");
  out.push_str(&format!("        {}: \n", yaml_string(ssid)));
  if let Some(password) = cfg.wifi_password.as_deref().filter(|s| !s.is_empty()) {
    out.push_str(&format!("          password: {}\n", yaml_string(password)));
  }
  out
}

fn systemd(cfg: &PiCustomization) -> Result<BootCustomization> {
  Ok(BootCustomization {
    files: vec![BootFile {
      name: "firstrun.sh".into(),
      contents: firstrun(cfg)?,
    }],
    cmdline_append: Some(
      "systemd.run=/boot/firstrun.sh systemd.run_success_action=reboot systemd.run_failure_action=reboot"
        .into(),
    ),
  })
}

fn firstrun(cfg: &PiCustomization) -> Result<String> {
  let mut body = String::from("#!/bin/bash\nset +e\n");
  body.push_str("FIRSTUSER=$(getent passwd 1000 | cut -d: -f1)\n");
  body.push_str("FIRSTUSERHOME=$(getent passwd 1000 | cut -d: -f6)\n");

  if let Some(hostname) = cfg.hostname.as_deref().filter(|s| !s.is_empty()) {
    let host = sh_single(hostname);
    body.push_str("if [ -f /usr/lib/raspberrypi-sys-mods/imager_custom ]; then\n");
    body.push_str(&format!(
      "   /usr/lib/raspberrypi-sys-mods/imager_custom set_hostname {host}\n"
    ));
    body.push_str("else\n");
    body.push_str(&format!("   echo {host} >/etc/hostname\n"));
    body.push_str(&format!(
      "   sed -i \"s/127.0.1.1.*$/127.0.1.1\\t{}\"/g /etc/hosts\n",
      hostname.replace('\\', "\\\\").replace('"', "\\\"")
    ));
    body.push_str("fi\n");
  }

  let username = cfg.username.as_deref().filter(|s| !s.is_empty());
  let password = cfg.password.as_deref().filter(|s| !s.is_empty());
  if username.is_some() || password.is_some() {
    let name = username.unwrap_or("pi");
    let hash = hash_password(password.unwrap_or("raspberry"))?;
    body.push_str("if [ -f /usr/lib/userconf-pi/userconf ]; then\n");
    body.push_str(&format!(
      "   /usr/lib/userconf-pi/userconf {} {}\n",
      sh_single(name),
      sh_single(&hash)
    ));
    body.push_str("else\n");
    body.push_str(&format!(
      "   echo \"$FIRSTUSER\":{} | chpasswd -e\n",
      sh_single(&hash)
    ));
    body.push_str(&format!(
      "   if [ \"$FIRSTUSER\" != {} ]; then\n",
      sh_single(name)
    ));
    body.push_str(&format!(
      "      usermod -l {} \"$FIRSTUSER\"\n",
      sh_single(name)
    ));
    body.push_str(&format!(
      "      usermod -m -d /home/{} {}\n",
      sh_single(name),
      sh_single(name)
    ));
    body.push_str(&format!(
      "      groupmod -n {} \"$FIRSTUSER\"\n",
      sh_single(name)
    ));
    body.push_str("   fi\nfi\n");
  }

  if cfg.ssh_enabled {
    body.push_str("systemctl enable ssh\n");
    if let Some(key) = cfg.ssh_public_key.as_deref().filter(|s| !s.is_empty()) {
      body.push_str("install -o 1000 -g 1000 -m 700 -d \"$FIRSTUSERHOME/.ssh\"\n");
      body.push_str(&format!(
        "install -o 1000 -g 1000 -m 600 /dev/null \"$FIRSTUSERHOME/.ssh/authorized_keys\"\n"
      ));
      for line in key.lines().map(str::trim).filter(|s| !s.is_empty()) {
        body.push_str(&format!(
          "echo {} >> \"$FIRSTUSERHOME/.ssh/authorized_keys\"\n",
          sh_single(line)
        ));
      }
    }
  }

  if let Some(ssid) = cfg.wifi_ssid.as_deref().filter(|s| !s.is_empty()) {
    let country = cfg
      .wifi_country
      .as_deref()
      .filter(|s| !s.is_empty())
      .unwrap_or("GB");
    let psk = cfg.wifi_password.as_deref().unwrap_or("");
    body.push_str("if [ -f /usr/lib/raspberrypi-sys-mods/imager_custom ]; then\n");
    body.push_str(&format!(
      "   /usr/lib/raspberrypi-sys-mods/imager_custom set_wlan {} {} {}\n",
      sh_single(ssid),
      sh_single(psk),
      sh_single(country)
    ));
    body.push_str("else\ncat >/etc/wpa_supplicant/wpa_supplicant.conf <<'WPAEOF'\n");
    body.push_str(&format!("country={country}\n"));
    body.push_str("ctrl_interface=DIR=/var/run/wpa_supplicant GROUP=netdev\n");
    body.push_str("update_config=1\nnetwork={\n");
    body.push_str(&format!("\tssid={}\n", sh_single(ssid)));
    if !psk.is_empty() {
      body.push_str(&format!("\tpsk={}\n", sh_single(psk)));
    } else {
      body.push_str("\tkey_mgmt=NONE\n");
    }
    body.push_str(
      "}\nWPAEOF\nchmod 600 /etc/wpa_supplicant/wpa_supplicant.conf\nrfkill unblock wifi\nfi\n",
    );
  }

  if let Some(tz) = cfg.timezone.as_deref().filter(|s| !s.is_empty()) {
    body.push_str(&format!(
      "raspi-config nonint do_change_timezone {}\n",
      sh_single(tz)
    ));
  }
  if let Some(layout) = cfg.keyboard.as_deref().filter(|s| !s.is_empty()) {
    body.push_str(&format!(
      "raspi-config nonint do_configure_keyboard {}\n",
      sh_single(layout)
    ));
  }

  body.push_str("rm -f /boot/firstrun.sh /boot/firmware/firstrun.sh\n");
  body.push_str("sed -i 's| systemd.run.*||g' /boot/cmdline.txt /boot/firmware/cmdline.txt 2>/dev/null || true\n");
  body.push_str("exit 0\n");
  Ok(body)
}

fn hash_password(password: &str) -> Result<String> {
  let params = Sha512Params::new(5_000).map_err(|err| Error::msg(format!("{err:?}")))?;
  sha512_simple(password, &params).map_err(|err| Error::msg(format!("{err:?}")))
}

fn yaml_string(value: &str) -> String {
  if !value.is_empty()
    && value
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '$'))
  {
    value.to_string()
  } else {
    format!("'{}'", value.replace('\'', "''"))
  }
}

fn sh_single(value: &str) -> String {
  format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn empty_config_is_none() {
    assert!(
      generate_boot(InitFormat::CloudInitRpi, &PiCustomization::default())
        .unwrap()
        .is_none()
    );
    assert!(
      generate_boot(
        InitFormat::None,
        &PiCustomization {
          hostname: Some("pi".into()),
          ..Default::default()
        }
      )
      .unwrap()
      .is_none()
    );
  }

  #[test]
  fn cloud_init_user_data() {
    let boot = generate_boot(
      InitFormat::CloudInitRpi,
      &PiCustomization {
        hostname: Some("lab-pi".into()),
        username: Some("momo".into()),
        password: Some("secret".into()),
        ssh_enabled: true,
        ssh_public_key: Some("ssh-ed25519 AAAA".into()),
        wifi_ssid: Some("Cafe WiFi".into()),
        wifi_password: Some("pass word".into()),
        wifi_country: Some("DE".into()),
        timezone: Some("Europe/Berlin".into()),
        keyboard: Some("de".into()),
      },
    )
    .unwrap()
    .unwrap();
    let names: Vec<_> = boot.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["user-data", "meta-data", "network-config"]);
    let user = &boot.files[0].contents;
    assert!(user.starts_with("#cloud-config\n"));
    assert!(user.contains("hostname: lab-pi"));
    assert!(user.contains("name: momo"));
    assert!(user.contains("passwd: $6$"));
    assert!(user.contains("enable_ssh: true"));
    assert!(user.contains("rpi:"));
    let net = &boot.files[2].contents;
    assert!(net.contains("regulatory-domain: DE"));
    assert!(net.contains("'Cafe WiFi'"));
    assert!(boot.cmdline_append.is_none());
  }

  #[test]
  fn systemd_writes_firstrun_and_cmdline() {
    let boot = generate_boot(
      InitFormat::Systemd,
      &PiCustomization {
        hostname: Some("old-pi".into()),
        ssh_enabled: true,
        ..Default::default()
      },
    )
    .unwrap()
    .unwrap();
    assert_eq!(boot.files[0].name, "firstrun.sh");
    assert!(boot.files[0].contents.contains("set_hostname 'old-pi'"));
    assert!(boot.files[0].contents.contains("systemctl enable ssh"));
    assert!(
      boot
        .cmdline_append
        .as_deref()
        .unwrap()
        .contains("systemd.run=/boot/firstrun.sh")
    );
  }

  #[test]
  fn yaml_quotes_special_ssid() {
    assert_eq!(yaml_string("simple"), "simple");
    assert_eq!(yaml_string("Cafe WiFi"), "'Cafe WiFi'");
    assert_eq!(yaml_string("it's"), "'it''s'");
  }
}
