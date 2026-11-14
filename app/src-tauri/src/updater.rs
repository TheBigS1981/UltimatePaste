//! Update check and self-update from GitHub Releases (Q49–Q51).
//!
//! Only the public "latest release" of the project is queried – no data about the user is
//! sent. Updating always needs the user's click; the download is checked against the
//! SHA-256 sums published with the release.

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::AppState;

pub const REPO: &str = "TheBigS1981/UltimatePaste";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_DATE: &str = env!("BUILD_DATE");

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    /// Release notes (Markdown as written on GitHub)
    pub notes: String,
    pub published: String,
    pub page: String,
    /// Asset that fits this installation (installer or portable exe)
    pub asset: String,
    pub asset_url: String,
    pub sums_url: String,
    pub installed: bool,
}

/// "v1.2.3" / "1.2.3-beta" → (1, 2, 3)
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(l), Some(c)) if l > c)
}

/// SHA-256 for `file` from a "SHA256SUMS.txt" ("<hex>  <name>" per line).
pub fn sum_for(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut p = l.split_whitespace();
        let (h, n) = (p.next()?, p.next()?);
        (n.trim_start_matches('*') == file && h.len() == 64).then(|| h.to_lowercase())
    })
}

/// Installed via the setup (uninstaller next to the exe) or portable.
pub fn is_installed(exe: &Path) -> bool {
    exe.parent().is_some_and(|d| d.join("uninstall.exe").exists())
}

/// Picks the asset name for this kind of installation from a release's asset names.
pub fn pick_asset<'a>(names: &[&'a str], installed: bool) -> Option<&'a str> {
    names.iter().copied().find(|n| {
        let n = n.to_lowercase();
        n.ends_with(".exe") && if installed { n.contains("setup") } else { n.contains("portable") }
    })
}

#[cfg(windows)]
fn client() -> Result<reqwest::Client> {
    // A generic user agent (GitHub requires one); nothing about the user.
    Ok(reqwest::Client::builder().user_agent(format!("UltimatePaste/{VERSION}")).timeout(std::time::Duration::from_secs(60)).build()?)
}

#[cfg(windows)]
pub async fn check() -> Result<UpdateInfo> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = client()?.get(&url).header("Accept", "application/vnd.github+json").send().await.map_err(|e| anyhow!("request failed: {}", e.without_url()))?;
    if resp.status().as_u16() == 404 {
        bail!("no-release");
    }
    if !resp.status().is_success() {
        bail!("GitHub HTTP {}", resp.status().as_u16());
    }
    let v: serde_json::Value = resp.json().await?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let latest = s("tag_name").trim_start_matches('v').to_string();
    let assets: Vec<(String, String)> = v
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|x| Some((x.get("name")?.as_str()?.to_string(), x.get("browser_download_url")?.as_str()?.to_string()))).collect())
        .unwrap_or_default();
    let exe = std::env::current_exe()?;
    let installed = is_installed(&exe);
    let names: Vec<&str> = assets.iter().map(|(n, _)| n.as_str()).collect();
    let asset = pick_asset(&names, installed).unwrap_or_default().to_string();
    let url_of = |name: &str| assets.iter().find(|(n, _)| n == name).map(|(_, u)| u.clone()).unwrap_or_default();
    Ok(UpdateInfo {
        current: VERSION.into(),
        newer: is_newer(&latest, VERSION),
        latest,
        notes: s("body"),
        published: s("published_at"),
        page: s("html_url"),
        asset_url: url_of(&asset),
        sums_url: url_of("SHA256SUMS.txt"),
        asset,
        installed,
    })
}

#[cfg(not(windows))]
pub async fn check() -> Result<UpdateInfo> {
    bail!("updates are only available on Windows")
}

#[cfg(windows)]
async fn fetch(url: &str) -> Result<Vec<u8>> {
    let allowed = url.starts_with("https://github.com/") || url.starts_with("https://objects.githubusercontent.com/") || url.starts_with("https://release-assets.githubusercontent.com/");
    if !allowed {
        bail!("unexpected download host");
    }
    let resp = client()?.get(url).send().await.map_err(|e| anyhow!("download failed: {}", e.without_url()))?;
    if !resp.status().is_success() {
        bail!("download HTTP {}", resp.status().as_u16());
    }
    Ok(resp.bytes().await?.to_vec())
}

/// Downloads, verifies and starts the update; the app quits afterwards.
#[cfg(windows)]
pub async fn install(app: &AppHandle, info: &UpdateInfo) -> Result<()> {
    if info.asset_url.is_empty() || info.sums_url.is_empty() {
        bail!("no-asset");
    }
    let sums = String::from_utf8(fetch(&info.sums_url).await?)?;
    let expected = sum_for(&sums, &info.asset).ok_or_else(|| anyhow!("no checksum for {}", info.asset))?;
    let bytes = fetch(&info.asset_url).await?;
    let actual = hex(&Sha256::digest(&bytes));
    if actual != expected {
        bail!("checksum-mismatch");
    }
    log::info!("update {}: downloaded and verified {}", info.latest, info.asset);
    if info.installed {
        let dir = std::env::temp_dir().join("UltimatePaste");
        std::fs::create_dir_all(&dir)?;
        let setup = dir.join(&info.asset);
        std::fs::write(&setup, &bytes)?;
        // Passive install with progress bar, restarts the app when done.
        std::process::Command::new(&setup).args(["/P", "/R"]).spawn()?;
    } else {
        let exe = std::env::current_exe()?;
        let old = old_path(&exe);
        let _ = std::fs::remove_file(&old);
        // A running exe can be renamed; the new one takes its place.
        std::fs::rename(&exe, &old)?;
        if let Err(e) = std::fs::write(&exe, &bytes) {
            let _ = std::fs::rename(&old, &exe);
            return Err(e.into());
        }
        std::process::Command::new(&exe).spawn()?;
    }
    app.exit(0);
    Ok(())
}

#[cfg(not(windows))]
pub async fn install(_app: &AppHandle, _info: &UpdateInfo) -> Result<()> {
    bail!("updates are only available on Windows")
}

fn old_path(exe: &Path) -> PathBuf {
    exe.with_extension("exe.old")
}

/// Removes the previous exe left by a portable update.
pub fn cleanup_old() {
    if let Ok(exe) = std::env::current_exe() {
        let old = old_path(&exe);
        if old.exists() {
            // The old process may still be closing; try a few times.
            for _ in 0..10 {
                if std::fs::remove_file(&old).is_ok() {
                    log::info!("removed the previous version after updating");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Background check after start and every 24 h; opens the notice window when there is a
/// newer version the user hasn't postponed.
pub fn spawn_checker(app: AppHandle) {
    std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(15));
            loop {
                if app.state::<AppState>().settings.read().update_check {
                    match tauri::async_runtime::block_on(check()) {
                        Ok(info) => {
                            let newer = info.newer;
                            *app.state::<AppState>().update.lock() = Some(info.clone());
                            if newer {
                                log::info!("update available: {}", info.latest);
                                let _ = crate::build_tray(&app);
                                let h = app.clone();
                                let _ = app.run_on_main_thread(move || open_window(&h));
                            }
                        }
                        Err(e) => log::info!("update check: {e}"),
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(24 * 3600));
            }
        })
        .expect("spawn update-check");
}

pub const LABEL: &str = "update";

pub fn open_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let title = crate::i18n::tr(app, "UltimatePaste – Update", "UltimatePaste – Update");
    let r = tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(520.0, 420.0)
        .min_inner_size(420.0, 320.0)
        .center()
        .build();
    match r {
        Ok(_) => crate::apply_theme(app),
        Err(e) => log::error!("open update window: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.0"), Some((1, 0, 0)));
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.13.6", "1.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
    }

    #[test]
    fn sums_and_assets() {
        let sums = "AB12cd34ab12cd34ab12cd34ab12cd34ab12cd34ab12cd34ab12cd34ab12cd34  UltimatePaste-1.0.1-portable.exe\n0000000000000000000000000000000000000000000000000000000000000000 *UltimatePaste_1.0.1_x64-setup.exe\n";
        assert_eq!(sum_for(sums, "UltimatePaste-1.0.1-portable.exe").unwrap().len(), 64);
        assert!(sum_for(sums, "UltimatePaste_1.0.1_x64-setup.exe").is_some());
        assert!(sum_for(sums, "other.exe").is_none());
        let names = ["SHA256SUMS.txt", "UltimatePaste_1.0.1_x64-setup.exe", "UltimatePaste-1.0.1-portable.exe"];
        assert_eq!(pick_asset(&names, true), Some("UltimatePaste_1.0.1_x64-setup.exe"));
        assert_eq!(pick_asset(&names, false), Some("UltimatePaste-1.0.1-portable.exe"));
    }
}
