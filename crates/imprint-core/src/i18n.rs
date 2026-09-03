//! UI catalogs loaded from `locales/*.json` (one file per language).
//!
//! Nested JSON objects are flattened to dotted keys (`menu.about`). `{name}`
//! placeholders are filled by [`tr`]. Missing keys fall back to English, then
//! the key itself.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const EN: &str = include_str!("../../../locales/en.json");
const ZH_HANS: &str = include_str!("../../../locales/zh-Hans.json");
const ZH_HANT: &str = include_str!("../../../locales/zh-Hant.json");
const JA: &str = include_str!("../../../locales/ja.json");
const KO: &str = include_str!("../../../locales/ko.json");
const DE: &str = include_str!("../../../locales/de.json");
const ES: &str = include_str!("../../../locales/es.json");
const FR: &str = include_str!("../../../locales/fr.json");
const PT: &str = include_str!("../../../locales/pt.json");

static CATALOGS: OnceLock<HashMap<Language, HashMap<String, String>>> = OnceLock::new();
static PREF: RwLock<LocalePref> = RwLock::new(LocalePref::System);

/// Languages with a catalog file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
  #[serde(rename = "en")]
  En,
  #[serde(rename = "zh-Hans")]
  ZhHans,
  #[serde(rename = "zh-Hant")]
  ZhHant,
  #[serde(rename = "ja")]
  Ja,
  #[serde(rename = "ko")]
  Ko,
  #[serde(rename = "de")]
  De,
  #[serde(rename = "es")]
  Es,
  #[serde(rename = "fr")]
  Fr,
  #[serde(rename = "pt")]
  Pt,
}

impl Language {
  pub const ALL: [Language; 9] = [
    Language::En,
    Language::ZhHans,
    Language::ZhHant,
    Language::Ja,
    Language::Ko,
    Language::De,
    Language::Es,
    Language::Fr,
    Language::Pt,
  ];

  pub fn id(self) -> &'static str {
    match self {
      Self::En => "en",
      Self::ZhHans => "zh-Hans",
      Self::ZhHant => "zh-Hant",
      Self::Ja => "ja",
      Self::Ko => "ko",
      Self::De => "de",
      Self::Es => "es",
      Self::Fr => "fr",
      Self::Pt => "pt",
    }
  }

  pub fn native_name(self) -> &'static str {
    match self {
      Self::En => "English",
      Self::ZhHans => "简体中文",
      Self::ZhHant => "繁體中文",
      Self::Ja => "日本語",
      Self::Ko => "한국어",
      Self::De => "Deutsch",
      Self::Es => "Español",
      Self::Fr => "Français",
      Self::Pt => "Português",
    }
  }

  /// Map a BCP 47 / POSIX locale tag onto a catalog language.
  pub fn from_tag(tag: &str) -> Self {
    let tag = normalize_tag(tag);
    if tag.starts_with("zh-hant")
      || tag.starts_with("zh-tw")
      || tag.starts_with("zh-hk")
      || tag.starts_with("zh-mo")
    {
      return Self::ZhHant;
    }
    if tag.starts_with("zh") {
      return Self::ZhHans;
    }
    if tag.starts_with("ja") {
      return Self::Ja;
    }
    if tag.starts_with("ko") {
      return Self::Ko;
    }
    if tag.starts_with("de") {
      return Self::De;
    }
    if tag.starts_with("es") {
      return Self::Es;
    }
    if tag.starts_with("fr") {
      return Self::Fr;
    }
    if tag.starts_with("pt") {
      return Self::Pt;
    }
    Self::En
  }
}

/// User preference: follow the OS, or pin a catalog language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LocalePref {
  #[default]
  System,
  Language(Language),
}

/// Parse embedded catalogs. Invalid JSON is a programming error.
pub fn init() {
  let _ = catalogs();
}

pub fn pref() -> LocalePref {
  *PREF.read().unwrap_or_else(|e| e.into_inner())
}

pub fn set_pref(pref: LocalePref) {
  *PREF.write().unwrap_or_else(|e| e.into_inner()) = pref;
}

pub fn active_language() -> Language {
  match pref() {
    LocalePref::Language(lang) => lang,
    LocalePref::System => detect_system(),
  }
}

/// Look up `key` in the active catalog.
pub fn t(key: &str) -> String {
  lookup(key)
}

/// Look up `key` and replace `{name}` placeholders from `args`.
pub fn tr(key: &str, args: &[(&str, &str)]) -> String {
  interpolate(&lookup(key), args)
}

fn detect_system() -> Language {
  sys_locale::get_locale()
    .or_else(|| std::env::var("LC_ALL").ok())
    .or_else(|| std::env::var("LC_MESSAGES").ok())
    .or_else(|| std::env::var("LANG").ok())
    .map(|tag| Language::from_tag(&tag))
    .unwrap_or(Language::En)
}

fn catalogs() -> &'static HashMap<Language, HashMap<String, String>> {
  CATALOGS.get_or_init(|| {
    let mut map = HashMap::new();
    for (lang, raw) in [
      (Language::En, EN),
      (Language::ZhHans, ZH_HANS),
      (Language::ZhHant, ZH_HANT),
      (Language::Ja, JA),
      (Language::Ko, KO),
      (Language::De, DE),
      (Language::Es, ES),
      (Language::Fr, FR),
      (Language::Pt, PT),
    ] {
      map.insert(lang, parse_catalog(raw, lang.id()));
    }
    map
  })
}

fn parse_catalog(raw: &str, id: &str) -> HashMap<String, String> {
  let value: Value =
    serde_json::from_str(raw).unwrap_or_else(|err| panic!("invalid locale file {id}.json: {err}"));
  let mut out = HashMap::new();
  flatten(&value, "", &mut out);
  out
}

fn flatten(value: &Value, prefix: &str, out: &mut HashMap<String, String>) {
  match value {
    Value::Object(map) => {
      for (key, child) in map {
        let next = if prefix.is_empty() {
          key.clone()
        } else {
          format!("{prefix}.{key}")
        };
        flatten(child, &next, out);
      }
    }
    Value::String(text) => {
      if !prefix.is_empty() {
        out.insert(prefix.to_string(), text.clone());
      }
    }
    _ => {}
  }
}

fn lookup(key: &str) -> String {
  lookup_in(active_language(), key)
}

fn lookup_in(lang: Language, key: &str) -> String {
  let catalogs = catalogs();
  if let Some(text) = catalogs.get(&lang).and_then(|c| c.get(key)) {
    return text.clone();
  }
  if lang != Language::En {
    if let Some(text) = catalogs.get(&Language::En).and_then(|c| c.get(key)) {
      return text.clone();
    }
  }
  key.to_string()
}

fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
  let mut out = template.to_string();
  for (name, value) in args {
    out = out.replace(&format!("{{{name}}}"), value);
  }
  out
}

fn normalize_tag(tag: &str) -> String {
  let tag = tag.trim();
  let tag = tag.split('.').next().unwrap_or(tag);
  let tag = tag.split('@').next().unwrap_or(tag);
  tag.replace('_', "-").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn keys(lang: Language) -> Vec<String> {
    let mut keys: Vec<String> = catalogs()
      .get(&lang)
      .expect("catalog")
      .keys()
      .cloned()
      .collect();
    keys.sort();
    keys
  }

  #[test]
  fn english_lookup() {
    assert_eq!(lookup_in(Language::En, "write.action"), "Write");
    assert_eq!(lookup_in(Language::En, "missing.key"), "missing.key");
  }

  #[test]
  fn interpolates_placeholders() {
    assert_eq!(
      interpolate(
        &lookup_in(Language::En, "about.version"),
        &[("version", "0.1.0")]
      ),
      "Version 0.1.0"
    );
    assert_eq!(
      interpolate(&lookup_in(Language::En, "target.count"), &[("n", "2")]),
      "2 drives"
    );
  }

  #[test]
  fn simplified_chinese() {
    assert_eq!(lookup_in(Language::ZhHans, "write.action"), "写入");
    assert_eq!(
      interpolate(&lookup_in(Language::ZhHans, "target.count"), &[("n", "2")]),
      "2 个磁盘"
    );
  }

  #[test]
  fn falls_back_to_english() {
    assert_eq!(lookup_in(Language::De, "does.not.exist"), "does.not.exist");
    assert_eq!(lookup_in(Language::De, "write.action"), "Schreiben");
  }

  #[test]
  fn catalogs_share_english_keys() {
    let english = keys(Language::En);
    assert!(english.contains(&"menu.about".into()));
    for lang in Language::ALL {
      if lang == Language::En {
        continue;
      }
      assert_eq!(keys(lang), english, "key mismatch in {}", lang.id());
    }
  }

  #[test]
  fn from_tag_aliases() {
    assert_eq!(Language::from_tag("zh_TW.UTF-8"), Language::ZhHant);
    assert_eq!(Language::from_tag("zh-HK"), Language::ZhHant);
    assert_eq!(Language::from_tag("zh-Hans-CN"), Language::ZhHans);
    assert_eq!(Language::from_tag("zh_CN"), Language::ZhHans);
    assert_eq!(Language::from_tag("ja_JP.UTF-8"), Language::Ja);
    assert_eq!(Language::from_tag("ko-KR"), Language::Ko);
    assert_eq!(Language::from_tag("de_DE.UTF-8"), Language::De);
    assert_eq!(Language::from_tag("es-ES"), Language::Es);
    assert_eq!(Language::from_tag("es_MX"), Language::Es);
    assert_eq!(Language::from_tag("fr-FR"), Language::Fr);
    assert_eq!(Language::from_tag("fr_CA.UTF-8"), Language::Fr);
    assert_eq!(Language::from_tag("pt-BR"), Language::Pt);
    assert_eq!(Language::from_tag("pt_PT.UTF-8"), Language::Pt);
    assert_eq!(Language::from_tag("en-US"), Language::En);
    assert_eq!(Language::from_tag("it-IT"), Language::En);
  }

  #[test]
  fn romance_catalogs() {
    assert_eq!(lookup_in(Language::Fr, "write.action"), "Écrire");
    assert_eq!(lookup_in(Language::Es, "write.action"), "Escribir");
    assert_eq!(lookup_in(Language::Pt, "write.action"), "Gravar");
  }
}
