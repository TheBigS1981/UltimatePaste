//! Core data types shared between storage, search, platform and the UI.

use serde::{Deserialize, Serialize};

/// Everything we keep from one clipboard snapshot.
/// The image is stored separately (encrypted file), so it is skipped in the JSON payload.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Raw Windows "HTML Format" payload (CF_HTML with header).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtf: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(skip)]
    pub image_png: Option<Vec<u8>>,
    /// Marked as private by its source (password manager): when we put it back on the
    /// clipboard, the "don't record / don't sync" markers go with it.
    #[serde(skip)]
    pub private: bool,
}

impl ClipContent {
    pub fn text_only(text: impl Into<String>) -> Self {
        Self { text: Some(text.into()), ..Default::default() }
    }

    pub fn is_empty(&self) -> bool {
        self.text.as_deref().is_none_or(|t| t.trim().is_empty())
            && self.files.is_empty()
            && self.image_png.is_none()
    }

    pub fn kind(&self) -> Kind {
        if !self.files.is_empty() {
            return Kind::Files;
        }
        if let Some(t) = self.text.as_deref() {
            if !t.trim().is_empty() {
                return if is_link(t) { Kind::Link } else { Kind::Text };
            }
        }
        if self.image_png.is_some() {
            return Kind::Image;
        }
        Kind::Text
    }

    /// Text-ish entries drop accompanying bitmaps (e.g. Excel/Word copy a rendered picture too).
    pub fn normalize(mut self) -> Self {
        if self.text.as_deref().is_some_and(|t| t.trim().is_empty()) {
            self.text = None;
        }
        if !self.files.is_empty() || self.text.is_some() {
            self.image_png = None;
        }
        self
    }

    /// Approximate size in bytes (for limits and display).
    pub fn byte_size(&self) -> usize {
        self.text.as_ref().map_or(0, |s| s.len())
            + self.html.as_ref().map_or(0, |s| s.len())
            + self.rtf.as_ref().map_or(0, |s| s.len())
            + self.files.iter().map(|f| f.len()).sum::<usize>()
            + self.image_png.as_ref().map_or(0, |b| b.len())
    }

    /// Formats available for "paste as…".
    pub fn formats(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.text.is_some() {
            f.push("text".to_string());
        }
        if self.html.is_some() {
            f.push("html".to_string());
        }
        if self.rtf.is_some() {
            f.push("rtf".to_string());
        }
        if !self.files.is_empty() {
            f.push("files".to_string());
        }
        if self.image_png.is_some() {
            f.push("image".to_string());
        }
        f
    }

    /// Applies a per-program paste rule ("plain", "oneline", "trim"; "formatted" keeps all).
    /// Only text entries are changed; images and files stay as they are.
    pub fn with_rule(&self, rule: &str) -> ClipContent {
        if self.text.is_none() || self.image_png.is_some() || !self.files.is_empty() {
            return self.clone();
        }
        let text = self.text.clone().unwrap_or_default();
        match rule {
            "plain" => ClipContent::text_only(text),
            "oneline" => ClipContent::text_only(text.split_whitespace().collect::<Vec<_>>().join(" ")),
            "trim" => ClipContent::text_only(text.trim().to_string()),
            _ => self.clone(),
        }
    }

    /// Builds the variant that should be written to the clipboard for a paste mode.
    pub fn for_mode(&self, mode: PasteMode) -> ClipContent {
        match mode {
            PasteMode::Default | PasteMode::Copy => self.clone(),
            PasteMode::Plain => ClipContent {
                text: self.text.clone().or_else(|| {
                    (!self.files.is_empty()).then(|| self.files.join("\r\n"))
                }),
                ..Default::default()
            },
            PasteMode::Html => ClipContent {
                text: self.text.clone(),
                html: self.html.clone(),
                ..Default::default()
            },
            PasteMode::Rtf => ClipContent {
                text: self.text.clone(),
                rtf: self.rtf.clone(),
                ..Default::default()
            },
        }
    }
}

pub fn is_link(t: &str) -> bool {
    let t = t.trim();
    !t.contains(char::is_whitespace)
        && (t.starts_with("http://") || t.starts_with("https://") || t.starts_with("www."))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Link,
    Image,
    Files,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Link => "link",
            Kind::Image => "image",
            Kind::Files => "files",
        }
    }
    pub fn parse(s: &str) -> Kind {
        match s {
            "link" => Kind::Link,
            "image" => Kind::Image,
            "files" => Kind::Files,
            _ => Kind::Text,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PasteMode {
    /// All stored formats (rich if available).
    Default,
    Plain,
    Html,
    Rtf,
    /// Only put it on the clipboard, do not send Ctrl+V.
    Copy,
}

/// Lightweight row shown in the list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    pub id: i64,
    pub kind: Kind,
    pub title: Option<String>,
    pub preview: String,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub last_used_at: i64,
    pub pinned: bool,
    pub formats: Vec<String>,
    pub image_w: Option<u32>,
    pub image_h: Option<u32>,
    pub file_count: usize,
    pub byte_size: i64,
    pub has_ocr: bool,
}

/// Full detail for the preview pane.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDetail {
    pub summary: EntrySummary,
    pub text: Option<String>,
    pub text_truncated: bool,
    pub files: Vec<String>,
    /// Paths from `files` that no longer exist (network paths are not checked here).
    pub missing_files: Vec<String>,
    /// `data:image/png;base64,...` of a display-sized version.
    pub image: Option<String>,
    pub ocr_text: Option<String>,
}

/// Splits copied file paths into (present, missing). Network paths (`\\server\…`) are
/// treated as present unless `check_network`, because an offline share can block for seconds.
pub fn split_missing(files: &[String], check_network: bool) -> (Vec<String>, Vec<String>) {
    files.iter().cloned().partition(|f| (!check_network && f.starts_with("\\\\")) || std::path::Path::new(f).try_exists().unwrap_or(true))
}

/// File name part of a path, for messages.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).find(|s| !s.is_empty()).unwrap_or(path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub keyword: String,
    pub body: String,
    #[serde(default)]
    pub updated_at: i64,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// First meaningful line, shortened, for list display.
pub fn make_preview(c: &ClipContent) -> String {
    if !c.files.is_empty() {
        let names: Vec<&str> = c
            .files
            .iter()
            .map(|f| f.rsplit(['\\', '/']).next().unwrap_or(f))
            .collect();
        return truncate_chars(&names.join(", "), 200);
    }
    if let Some(t) = &c.text {
        let collapsed: String = t
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .take(3)
            .collect::<Vec<_>>()
            .join(" ⏎ ");
        return truncate_chars(&collapsed, 200);
    }
    String::new()
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_detection() {
        assert_eq!(ClipContent::text_only("hello").kind(), Kind::Text);
        assert_eq!(ClipContent::text_only(" https://x.io/a ").kind(), Kind::Link);
        let f = ClipContent { files: vec!["C:\\a.txt".into()], ..Default::default() };
        assert_eq!(f.kind(), Kind::Files);
        let i = ClipContent { image_png: Some(vec![1]), ..Default::default() };
        assert_eq!(i.kind(), Kind::Image);
    }

    #[test]
    fn normalize_drops_image_for_text() {
        let c = ClipContent {
            text: Some("cell".into()),
            image_png: Some(vec![1, 2]),
            ..Default::default()
        }
        .normalize();
        assert!(c.image_png.is_none());
    }

    #[test]
    fn plain_mode_strips_formats() {
        let c = ClipContent {
            text: Some("a".into()),
            html: Some("<b>a</b>".into()),
            rtf: Some("{\\rtf a}".into()),
            ..Default::default()
        };
        let p = c.for_mode(PasteMode::Plain);
        assert_eq!(p.text.as_deref(), Some("a"));
        assert!(p.html.is_none() && p.rtf.is_none());
    }

    #[test]
    fn preview_and_truncate() {
        let c = ClipContent::text_only("\n  first \n\nsecond\nthird\nfourth");
        assert_eq!(make_preview(&c), "first ⏎ second ⏎ third");
        assert_eq!(truncate_chars("äöü", 2), "äö…");
    }

    #[test]
    fn missing_files_are_split() {
        let here = std::env::current_dir().unwrap().to_string_lossy().into_owned();
        let gone = format!("{here}/definitely-not-here-{}.txt", std::process::id());
        let (ok, missing) = split_missing(&[here.clone(), gone.clone(), "\\\\nas\\share\\x".into()], false);
        assert_eq!(ok, vec![here, "\\\\nas\\share\\x".to_string()]);
        assert_eq!(missing, vec![gone]);
        assert_eq!(file_name("C:\\Users\\a\\Bericht.pdf"), "Bericht.pdf");
        assert_eq!(file_name("C:\\Users\\a\\Ordner\\"), "Ordner");
    }
}
