//! In-memory search index. Content is decrypted once at startup and kept as a
//! folded (lower-case, accent-free) haystack per entry, so typing stays instant.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::Deserialize;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::model::{now_ms, ClipContent, EntrySummary, Kind};

/// Max characters of an entry that are searchable (keeps RAM bounded for huge texts).
const HAY_LIMIT: usize = 8_000;
const SHORT_LIMIT: usize = 240;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    All,
    Pinned,
    Text,
    Images,
}

struct Item {
    summary: EntrySummary,
    /// folded full haystack (title, text, files, source app, OCR)
    hay: String,
    /// folded short text (title + preview) for fuzzy matching
    short: String,
}

#[derive(Default)]
pub struct Index {
    items: Vec<Item>,
}

pub fn fold(s: &str) -> String {
    s.nfd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_lowercase).collect()
}

fn capped(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

fn build_item(summary: EntrySummary, content: &ClipContent, ocr: Option<&str>) -> Item {
    let mut raw = String::new();
    for part in [
        summary.title.as_deref(),
        content.text.as_deref().map(|t| capped(t, HAY_LIMIT)),
        Some(&content.files.join(" ")[..]),
        summary.source_app.as_deref(),
        ocr.map(|o| capped(o, HAY_LIMIT / 2)),
    ]
    .into_iter()
    .flatten()
    {
        raw.push_str(part);
        raw.push('\n');
    }
    let short_raw = format!("{} {}", summary.title.as_deref().unwrap_or(""), capped(&summary.preview, SHORT_LIMIT));
    Item { hay: fold(&raw), short: fold(&short_raw), summary }
}

impl Index {
    pub fn upsert(&mut self, summary: EntrySummary, content: &ClipContent, ocr: Option<&str>) {
        let item = build_item(summary, content, ocr);
        match self.items.iter_mut().find(|i| i.summary.id == item.summary.id) {
            Some(slot) => *slot = item,
            None => self.items.push(item),
        }
    }

    pub fn remove(&mut self, id: i64) {
        self.items.retain(|i| i.summary.id != id);
    }

    pub fn remove_many(&mut self, ids: &[i64]) {
        self.items.retain(|i| !ids.contains(&i.summary.id));
    }

    pub fn get(&self, id: i64) -> Option<&EntrySummary> {
        self.items.iter().find(|i| i.summary.id == id).map(|i| &i.summary)
    }

    pub fn update<F: FnOnce(&mut EntrySummary)>(&mut self, id: i64, f: F) {
        if let Some(i) = self.items.iter_mut().find(|i| i.summary.id == id) {
            f(&mut i.summary);
        }
    }

    /// Re-folds the title part after a rename (cheap, keeps the rest).
    pub fn set_title(&mut self, id: i64, title: Option<String>) {
        if let Some(i) = self.items.iter_mut().find(|i| i.summary.id == id) {
            if let Some(old) = &i.summary.title {
                let old_f = fold(old);
                if let Some(rest) = i.hay.strip_prefix(&format!("{old_f}\n")) {
                    i.hay = rest.to_string();
                }
            }
            if let Some(t) = &title {
                i.hay = format!("{}\n{}", fold(t), i.hay);
            }
            i.summary.title = title;
            i.short = fold(&format!("{} {}", i.summary.title.as_deref().unwrap_or(""), capped(&i.summary.preview, SHORT_LIMIT)));
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// When the newest entry was copied (ms).
    pub fn newest_created(&self) -> Option<i64> {
        self.items.iter().map(|i| i.summary.created_at).max()
    }

    /// Ids ordered by most recent use (newest first).
    pub fn recent_ids(&self) -> Vec<i64> {
        let mut v: Vec<&Item> = self.items.iter().collect();
        v.sort_by_key(|i| std::cmp::Reverse(i.summary.last_used_at));
        v.into_iter().map(|i| i.summary.id).collect()
    }

    pub fn search(&self, query: &str, tab: Tab, limit: usize) -> Vec<EntrySummary> {
        let atoms: Vec<String> = query.split_whitespace().map(fold).collect();
        let now = now_ms();
        let mut matcher = Matcher::new(Config::DEFAULT);
        let patterns: Vec<Option<Pattern>> = atoms
            .iter()
            .map(|a| (a.chars().count() >= 3).then(|| Pattern::new(a, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy)))
            .collect();
        let mut buf = Vec::new();

        let mut hits: Vec<(f64, &Item)> = self
            .items
            .iter()
            .filter(|i| match tab {
                Tab::All => true,
                Tab::Pinned => i.summary.pinned,
                Tab::Text => matches!(i.summary.kind, Kind::Text | Kind::Link),
                Tab::Images => i.summary.kind == Kind::Image,
            })
            .filter_map(|i| {
                let mut score = 0.0;
                for (atom, pat) in atoms.iter().zip(&patterns) {
                    if let Some(pos) = i.hay.find(atom.as_str()) {
                        score += 100.0;
                        let word_start = pos == 0 || !i.hay[..pos].chars().next_back().is_some_and(char::is_alphanumeric);
                        if word_start {
                            score += 30.0;
                        }
                        if i.short.contains(atom.as_str()) {
                            score += 40.0;
                        }
                    } else if let Some(p) = pat {
                        let s = p.score(Utf32Str::new(&i.short, &mut buf), &mut matcher)?;
                        let needed = 12 * atom.chars().count() as u32;
                        if s < needed {
                            return None;
                        }
                        score += (s as f64 / 4.0).min(80.0);
                    } else {
                        return None;
                    }
                }
                let age_h = ((now - i.summary.last_used_at).max(0) as f64) / 3_600_000.0;
                score += 25.0 * (-age_h / 72.0).exp();
                Some((score, i))
            })
            .collect();

        if atoms.is_empty() {
            hits.sort_by_key(|h| std::cmp::Reverse(h.1.summary.last_used_at));
        } else {
            hits.sort_by(|a, b| {
                b.0.partial_cmp(&a.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.1.summary.last_used_at.cmp(&a.1.summary.last_used_at))
            });
        }
        hits.into_iter().take(limit).map(|(_, i)| i.summary.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::make_preview;

    fn add(ix: &mut Index, id: i64, text: &str, used: i64) {
        let c = ClipContent::text_only(text);
        let s = EntrySummary {
            id,
            kind: c.kind(),
            title: None,
            preview: make_preview(&c),
            source_app: Some("notepad.exe".into()),
            created_at: used,
            last_used_at: used,
            pinned: false,
            formats: c.formats(),
            image_w: None,
            image_h: None,
            file_count: 0,
            byte_size: 1,
            has_ocr: false,
        };
        ix.upsert(s, &c, None);
    }

    #[test]
    fn fold_removes_accents_and_case() {
        assert_eq!(fold("Größe ÉCOLE"), "große ecole");
    }

    #[test]
    fn empty_query_orders_by_recency() {
        let mut ix = Index::default();
        add(&mut ix, 1, "old", 1);
        add(&mut ix, 2, "new", 2);
        let r = ix.search("", Tab::All, 10);
        assert_eq!(r.iter().map(|s| s.id).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn multi_word_and_accent_insensitive() {
        let mut ix = Index::default();
        add(&mut ix, 1, "Rechnung für Müller GmbH", 1);
        add(&mut ix, 2, "Rechnung für Schmidt", 1);
        let r = ix.search("rechnung muller", Tab::All, 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, 1);
    }

    #[test]
    fn typo_tolerant_on_short_text() {
        let mut ix = Index::default();
        add(&mut ix, 1, "Kubernetes deployment yaml", 1);
        add(&mut ix, 2, "something else", 1);
        let r = ix.search("kubrnetes", Tab::All, 10);
        assert_eq!(r.first().map(|s| s.id), Some(1));
    }

    #[test]
    fn source_app_searchable_and_tabs() {
        let mut ix = Index::default();
        add(&mut ix, 1, "abc", 1);
        assert_eq!(ix.search("notepad", Tab::All, 10).len(), 1);
        assert!(ix.search("", Tab::Pinned, 10).is_empty());
        assert_eq!(ix.search("", Tab::Text, 10).len(), 1);
        assert!(ix.search("", Tab::Images, 10).is_empty());
        ix.update(1, |s| s.pinned = true);
        assert_eq!(ix.search("", Tab::Pinned, 10).len(), 1);
    }

    #[test]
    fn rename_is_searchable() {
        let mut ix = Index::default();
        add(&mut ix, 1, "xyz", 1);
        ix.set_title(1, Some("Meine IBAN".into()));
        assert_eq!(ix.search("iban", Tab::All, 10).len(), 1);
        ix.set_title(1, None);
        assert!(ix.search("iban", Tab::All, 10).is_empty());
    }
}
