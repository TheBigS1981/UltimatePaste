//! Export / import of settings, snippets, AI actions and per-program rules (Q35). API keys
//! only go along when the user chooses so, sealed with a password (PBKDF2-SHA256 + AES-GCM).

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::path::Path;
use tauri::{AppHandle, Manager};

use crate::crypto::Vault;
use crate::model::Snippet;
use crate::settings::Settings;
use crate::{ai, platform, AppState};

const FORMAT: &str = "ultimatepaste-settings";
const ITERATIONS: u32 = 600_000;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Keyring users of the built-in services (AI providers use their provider id).
const SERVICES: &[&str] = &["svc:deepl", "svc:giphy", "svc:klipy"];

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sealed {
    salt: String,
    iterations: u32,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Bundle {
    format: String,
    version: u32,
    app: String,
    exported: String,
    settings: Value,
    #[serde(default)]
    snippets: Vec<Snippet>,
    #[serde(default)]
    keys: Option<Sealed>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub app: String,
    pub exported: String,
    pub snippets: usize,
    pub has_keys: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub snippets: usize,
    pub keys: usize,
}

/// PBKDF2-HMAC-SHA256 with a 64-byte output (two blocks).
pub fn pbkdf2(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 64] {
    let mut out = [0u8; 64];
    for block in 0..2u32 {
        let mac = |data: &[u8]| -> [u8; 32] {
            let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(password).expect("any key size");
            m.update(data);
            m.finalize().into_bytes().into()
        };
        let mut first = salt.to_vec();
        first.extend_from_slice(&(block + 1).to_be_bytes());
        let mut u = mac(&first);
        let mut t = u;
        for _ in 1..iterations {
            u = mac(&u);
            for (a, b) in t.iter_mut().zip(u.iter()) {
                *a ^= b;
            }
        }
        out[block as usize * 32..block as usize * 32 + 32].copy_from_slice(&t);
    }
    out
}

fn seal(keys: &BTreeMap<String, String>, password: &str) -> Result<Sealed> {
    let salt: [u8; 16] = rand::random();
    let vault = Vault::from_secret(&pbkdf2(password.as_bytes(), &salt, ITERATIONS));
    let data = vault.encrypt(&serde_json::to_vec(keys)?);
    Ok(Sealed { salt: B64.encode(salt), iterations: ITERATIONS, data: B64.encode(data) })
}

fn unseal(s: &Sealed, password: &str) -> Result<BTreeMap<String, String>> {
    let salt = B64.decode(&s.salt)?;
    let vault = Vault::from_secret(&pbkdf2(password.as_bytes(), &salt, s.iterations.clamp(100_000, 2_000_000)));
    let plain = vault.decrypt(&B64.decode(&s.data)?).map_err(|_| anyhow!("wrong-password"))?;
    Ok(serde_json::from_slice(&plain)?)
}

fn key_users(s: &Settings) -> Vec<String> {
    s.ai.providers.iter().map(|p| p.id.clone()).chain(SERVICES.iter().map(|s| s.to_string())).collect()
}

/// Settings as exported: without window sizes and "don't ask again" notices (PC specific).
fn settings_value(s: &Settings) -> Result<Value> {
    let mut v = serde_json::to_value(s)?;
    if let Some(o) = v.as_object_mut() {
        o.remove("windowSizes");
        o.remove("notifiedConflicts");
    }
    Ok(v)
}

pub fn export(app: &AppHandle, path: &Path, password: Option<&str>) -> Result<Report> {
    let st = app.state::<AppState>();
    let settings = st.settings.read().clone();
    let snippets = st.store.lock().list_snippets()?;
    let mut keys = BTreeMap::new();
    if password.is_some() {
        for user in key_users(&settings) {
            if let Some(k) = platform::secret_get(ai::KEYRING_SERVICE, &user)?.filter(|k| !k.is_empty()) {
                keys.insert(user, k);
            }
        }
    }
    let bundle = Bundle {
        format: FORMAT.into(),
        version: 1,
        app: env!("CARGO_PKG_VERSION").into(),
        exported: chrono::Local::now().to_rfc3339(),
        settings: settings_value(&settings)?,
        snippets: snippets.clone(),
        keys: match password {
            Some(pw) if !keys.is_empty() => Some(seal(&keys, pw)?),
            _ => None,
        },
    };
    std::fs::write(path, serde_json::to_vec_pretty(&bundle)?)?;
    Ok(Report { snippets: snippets.len(), keys: keys.len() })
}

fn read(path: &Path) -> Result<Bundle> {
    let b: Bundle = serde_json::from_slice(&std::fs::read(path)?).map_err(|_| anyhow!("not-an-export"))?;
    if b.format != FORMAT {
        bail!("not-an-export");
    }
    Ok(b)
}

pub fn inspect(path: &Path) -> Result<Info> {
    let b = read(path)?;
    Ok(Info { app: b.app, exported: b.exported, snippets: b.snippets.len(), has_keys: b.keys.is_some() })
}

/// Merges `incoming` settings into `current`: everything is taken over, except that lists of
/// AI providers, AI actions and program rules are combined (by id) in "merge" mode.
pub fn merge_settings(current: &Settings, incoming: Value, replace: bool) -> Result<Settings> {
    let mut base = serde_json::to_value(current)?;
    if let (Some(b), Some(inc)) = (base.as_object_mut(), incoming.as_object()) {
        for (k, v) in inc {
            if k == "windowSizes" || k == "notifiedConflicts" {
                continue;
            }
            b.insert(k.clone(), v.clone());
        }
    }
    let mut s: Settings = serde_json::from_value(base)?;
    if !replace {
        for p in &current.ai.providers {
            if !s.ai.providers.iter().any(|x| x.id == p.id) {
                s.ai.providers.push(p.clone());
            }
        }
        for a in &current.ai.actions {
            if !s.ai.actions.iter().any(|x| x.id == a.id) {
                s.ai.actions.push(a.clone());
            }
        }
        for r in &current.app_rules {
            if !s.app_rules.iter().any(|x| x.app.eq_ignore_ascii_case(&r.app)) {
                s.app_rules.push(r.clone());
            }
        }
    }
    s.window_sizes = current.window_sizes.clone();
    s.notified_conflicts = current.notified_conflicts.clone();
    // A foreign file must not weaken privacy protection or change where files are written:
    // keep the local protection settings and only ever add blocked programs.
    s.sensitive = current.sensitive.clone();
    s.inbox_dir = current.inbox_dir.clone();
    s.snippet_expand = current.snippet_expand;
    s.retention_days = current.retention_days;
    for b in &current.blocked_apps {
        if !s.blocked_apps.iter().any(|x| x.eq_ignore_ascii_case(b)) {
            s.blocked_apps.push(b.clone());
        }
    }
    Ok(s)
}

pub fn import(app: &AppHandle, path: &Path, password: Option<&str>, replace: bool) -> Result<(Report, Settings)> {
    let b = read(path)?;
    let st = app.state::<AppState>();
    let keys = match (&b.keys, password) {
        (Some(sealed), Some(pw)) if !pw.is_empty() => unseal(sealed, pw)?,
        _ => BTreeMap::new(),
    };
    let current = st.settings.read().clone();
    let settings = merge_settings(&current, b.settings, replace)?;
    let mut n = 0;
    {
        let store = st.store.lock();
        let existing = store.list_snippets()?;
        if replace {
            for s in &existing {
                store.delete_snippet(s.id)?;
            }
        }
        for s in &b.snippets {
            let dup = !replace && existing.iter().any(|e| e.name == s.name && e.keyword == s.keyword);
            if !dup {
                // Keywords from a file are letters/digits only: a punctuation keyword like "@gmail.com"
                // would otherwise rewrite what the user types (snippets while typing).
                let keyword: String = s.keyword.chars().filter(|c| c.is_alphanumeric()).collect();
                store.save_snippet(&Snippet { id: 0, keyword, ..s.clone() })?;
                n += 1;
            }
        }
    }
    // A stored key must never go to a different server than before: if an imported provider
    // with the same id points elsewhere (or is of another kind), forget the old key.
    for p in &settings.ai.providers {
        if let Some(old) = current.ai.providers.iter().find(|o| o.id == p.id) {
            let moved = old.base_url.trim_end_matches('/') != p.base_url.trim_end_matches('/') || old.kind != p.kind;
            if moved && !keys.contains_key(&p.id) {
                platform::secret_set(ai::KEYRING_SERVICE, &p.id, None)?;
                log::info!("import: provider {} points elsewhere now; its stored key was removed", p.id);
            }
        }
    }
    for (user, key) in &keys {
        platform::secret_set(ai::KEYRING_SERVICE, user, Some(key))?;
    }
    Ok((Report { snippets: n, keys: keys.len() }, settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbkdf2_known_vector() {
        // RFC 7914 §11 test vector for PBKDF2-HMAC-SHA256 (P="passwd", S="salt", c=1).
        let out = pbkdf2(b"passwd", b"salt", 1);
        assert_eq!(&out[..8], &[0x55, 0xac, 0x04, 0x6e, 0x56, 0xe3, 0x08, 0x9f]);
    }

    #[test]
    fn seal_roundtrip_and_wrong_password() {
        let mut keys = BTreeMap::new();
        keys.insert("svc:deepl".to_string(), "abc:fx".to_string());
        let s = seal_with(&keys, "geheim", 100_000);
        assert_eq!(unseal(&s, "geheim").unwrap(), keys);
        assert_eq!(unseal(&s, "falsch").unwrap_err().to_string(), "wrong-password");
    }

    fn seal_with(keys: &BTreeMap<String, String>, pw: &str, it: u32) -> Sealed {
        let salt: [u8; 16] = [7; 16];
        let vault = Vault::from_secret(&pbkdf2(pw.as_bytes(), &salt, it));
        Sealed { salt: B64.encode(salt), iterations: it, data: B64.encode(vault.encrypt(&serde_json::to_vec(keys).unwrap())) }
    }

    #[test]
    fn merge_keeps_local_lists_in_merge_mode() {
        let current = Settings::default();
        let mut other = Settings { language: "fr".into(), ..Default::default() };
        other.ai.actions.retain(|a| a.id == "fix");
        let v = settings_value(&other).unwrap();
        let merged = merge_settings(&current, v.clone(), false).unwrap();
        assert_eq!(merged.language, "fr");
        assert_eq!(merged.ai.actions.len(), current.ai.actions.len());
        let replaced = merge_settings(&current, v, true).unwrap();
        assert_eq!(replaced.ai.actions.len(), 1);
        // privacy settings are never weakened by a foreign file
        let mut weak = Settings::default();
        weak.sensitive.otp = false;
        weak.blocked_apps.clear();
        weak.inbox_dir = Some("C:\\Windows".into());
        let m = merge_settings(&current, settings_value(&weak).unwrap(), true).unwrap();
        assert!(m.sensitive.otp);
        assert_eq!(m.blocked_apps.len(), current.blocked_apps.len());
        assert_eq!(m.inbox_dir, current.inbox_dir);
    }
}
