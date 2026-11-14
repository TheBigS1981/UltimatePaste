//! "Inbox" export: drops an entry as a file into a folder (e.g. an Obsidian vault inbox).
//! Text → Markdown with YAML front matter, images → PNG, files → copied.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};

use crate::model::ClipContent;

pub struct ExportMeta<'a> {
    pub title: Option<&'a str>,
    pub source_app: Option<&'a str>,
    pub created: DateTime<Local>,
    pub tags: &'a [String],
}

fn yaml_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn markdown(text: &str, meta: &ExportMeta) -> String {
    let mut fm = String::from("---\n");
    if let Some(t) = meta.title {
        fm.push_str(&format!("title: {}\n", yaml_str(t)));
    }
    fm.push_str("tags:\n");
    for t in meta.tags {
        fm.push_str(&format!("  - {}\n", yaml_str(t)));
    }
    fm.push_str(&format!("created: {}\n", meta.created.format("%Y-%m-%dT%H:%M:%S%:z")));
    if let Some(s) = meta.source_app {
        fm.push_str(&format!("source: {}\n", yaml_str(s)));
    }
    fm.push_str("---\n\n");
    fm.push_str(text.replace("\r\n", "\n").trim_end());
    fm.push('\n');
    fm
}

/// File-system safe base name from a title or the first words of the text.
pub fn base_name(meta: &ExportMeta, text: Option<&str>) -> String {
    let src = meta
        .title
        .map(str::to_string)
        .or_else(|| text.map(|t| t.split_whitespace().take(8).collect::<Vec<_>>().join(" ")))
        .unwrap_or_default();
    let cleaned: String = src
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned: String = cleaned.chars().take(60).collect();
    let stamp = meta.created.format("%Y-%m-%d %H%M%S");
    if cleaned.is_empty() {
        format!("{stamp} Clipboard")
    } else {
        format!("{stamp} {cleaned}")
    }
}

fn unique(dir: &Path, base: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{base}.{ext}"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{base} ({n}).{ext}"));
        n += 1;
    }
    p
}

/// Writes the export and returns the created paths.
pub fn export(dir: &Path, c: &ClipContent, meta: &ExportMeta) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let base = base_name(meta, c.text.as_deref());
    let mut written = Vec::new();
    if !c.files.is_empty() {
        for f in &c.files {
            let src = Path::new(f);
            if src.is_file() {
                let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| base.clone());
                let (stem, ext) = match name.rsplit_once('.') {
                    Some((s, e)) => (s.to_string(), e.to_string()),
                    None => (name.clone(), String::new()),
                };
                let target = if ext.is_empty() { unique(dir, &stem, "file") } else { unique(dir, &stem, &ext) };
                std::fs::copy(src, &target)?;
                written.push(target);
            }
        }
        if written.is_empty() {
            bail!("no regular files to copy");
        }
    } else if let Some(png) = &c.image_png {
        let p = unique(dir, &base, "png");
        std::fs::write(&p, png)?;
        written.push(p);
    } else if let Some(text) = &c.text {
        let p = unique(dir, &base, "md");
        std::fs::write(&p, markdown(text, meta))?;
        written.push(p);
    } else {
        bail!("nothing to export");
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn meta<'a>(tags: &'a [String], title: Option<&'a str>) -> ExportMeta<'a> {
        ExportMeta { title, source_app: Some("chrome.exe"), created: Local.with_ymd_and_hms(2026, 11, 13, 22, 1, 2).unwrap(), tags }
    }

    #[test]
    fn markdown_front_matter() {
        let tags = vec!["clipboard".to_string(), "in\"box".to_string()];
        let md = markdown("Hello\r\nWorld\n\n", &meta(&tags, Some("My \"note\"")));
        assert!(md.starts_with("---\ntitle: \"My \\\"note\\\"\"\ntags:\n  - \"clipboard\"\n  - \"in\\\"box\"\ncreated: 2026-11-13T22:01:02"));
        assert!(md.contains("source: \"chrome.exe\"\n---\n\nHello\nWorld\n"));
    }

    #[test]
    fn names_are_safe_and_unique() {
        let tags = vec![];
        assert_eq!(base_name(&meta(&tags, None), Some("a/b:c*d? e")), "2026-11-13 220102 a b c d e");
        let dir = std::env::temp_dir().join(format!("up-inbox-{}", std::process::id()));
        let c = ClipContent::text_only("same");
        let a = export(&dir, &c, &meta(&tags, None)).unwrap();
        let b = export(&dir, &c, &meta(&tags, None)).unwrap();
        assert_ne!(a[0], b[0]);
        assert!(b[0].to_string_lossy().ends_with("(2).md"));
        std::fs::remove_dir_all(dir).ok();
    }
}
