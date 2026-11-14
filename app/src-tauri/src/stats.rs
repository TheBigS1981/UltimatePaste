//! Local usage statistics: only counters (no texts), stored in stats.json in the data folder.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tauri::{AppHandle, Manager};

use crate::AppState;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Day {
    pub paste: u32,
    pub sequential: u32,
    pub translate: u32,
    pub correct: u32,
    pub emoji: u32,
    pub symbol: u32,
    pub gif: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Stats {
    /// First day with data ("YYYY-MM-DD")
    pub since: String,
    /// Per day ("YYYY-MM-DD"), the last 400 days
    pub days: BTreeMap<String, Day>,
    pub emoji: BTreeMap<String, u64>,
    pub symbols: BTreeMap<String, u64>,
    /// Programs pasted into (exe name)
    pub apps: BTreeMap<String, u64>,
    /// Translated characters per language pair, e.g. "DE → EN-US"
    pub pairs: BTreeMap<String, u64>,
    /// AI costs (USD) per month ("YYYY-MM")
    pub ai_cost: BTreeMap<String, f64>,
}

pub enum Event<'a> {
    Paste,
    Sequential,
    Translate { pair: String, chars: u64 },
    Correct,
    Emoji(&'a str),
    Symbol(&'a str),
    Gif,
    App(Option<String>),
    AiCost(f64),
}

const KEEP_DAYS: usize = 400;

impl Stats {
    pub fn apply(&mut self, ev: Event, today: &str) {
        if self.since.is_empty() {
            self.since = today.to_string();
        }
        let day = self.days.entry(today.to_string()).or_default();
        match ev {
            Event::Paste => day.paste += 1,
            Event::Sequential => day.sequential += 1,
            Event::Translate { pair, chars } => {
                day.translate += 1;
                *self.pairs.entry(pair).or_default() += chars;
            }
            Event::Correct => day.correct += 1,
            Event::Emoji(e) => {
                day.emoji += 1;
                *self.emoji.entry(e.to_string()).or_default() += 1;
            }
            Event::Symbol(s) => {
                day.symbol += 1;
                *self.symbols.entry(s.to_string()).or_default() += 1;
            }
            Event::Gif => day.gif += 1,
            Event::App(Some(a)) => *self.apps.entry(a.to_lowercase()).or_default() += 1,
            Event::App(None) => {}
            Event::AiCost(c) => *self.ai_cost.entry(today[..7].to_string()).or_default() += c,
        }
        while self.days.len() > KEEP_DAYS {
            let first = self.days.keys().next().cloned().unwrap_or_default();
            self.days.remove(&first);
        }
    }

    fn path(dir: &Path) -> std::path::PathBuf {
        dir.join("stats.json")
    }

    pub fn load(dir: &Path) -> Stats {
        std::fs::read(Self::path(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save(&self, dir: &Path) {
        if let Ok(b) = serde_json::to_vec(self) {
            let _ = std::fs::write(Self::path(dir), b);
        }
    }
}

static LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

pub fn record(app: &AppHandle, ev: Event) {
    let st = app.state::<AppState>();
    let _g = LOCK.lock();
    let mut s = Stats::load(&st.data_dir);
    s.apply(ev, &chrono::Local::now().format("%Y-%m-%d").to_string());
    s.save(&st.data_dir);
}

pub fn reset(app: &AppHandle) {
    let st = app.state::<AppState>();
    let _g = LOCK.lock();
    let _ = std::fs::remove_file(Stats::path(&st.data_dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_events() {
        let mut s = Stats::default();
        s.apply(Event::Paste, "2026-11-14");
        s.apply(Event::Emoji("😀"), "2026-11-14");
        s.apply(Event::Emoji("😀"), "2026-11-15");
        s.apply(Event::Translate { pair: "DE → EN-US".into(), chars: 120 }, "2026-11-15");
        s.apply(Event::App(Some("OUTLOOK.EXE".into())), "2026-11-15");
        s.apply(Event::AiCost(0.25), "2026-11-15");
        assert_eq!(s.since, "2026-11-14");
        assert_eq!(s.days["2026-11-14"].paste, 1);
        assert_eq!(s.days["2026-11-15"].emoji, 1);
        assert_eq!(s.emoji["😀"], 2);
        assert_eq!(s.pairs["DE → EN-US"], 120);
        assert_eq!(s.apps["outlook.exe"], 1);
        assert!((s.ai_cost["2026-11"] - 0.25).abs() < 1e-9);
    }

    #[test]
    fn keeps_limited_days() {
        let mut s = Stats::default();
        for i in 0..(KEEP_DAYS + 5) {
            s.apply(Event::Paste, &format!("2025-{:04}", i));
        }
        assert_eq!(s.days.len(), KEEP_DAYS);
    }
}
