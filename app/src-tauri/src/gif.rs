//! GIF and sticker search via GIPHY or KLIPY, using the user's own API keys.
//!
//! Rules of both services that shape this module:
//! - requests go directly from this app (no proxy), with the user's key;
//! - media and media URLs are not stored: only provider + id go into "recently used",
//!   and a GIF downloaded for pasting is deleted again shortly afterwards;
//! - attribution is shown by the UI ("Powered by GIPHY", "Search KLIPY").

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::picker::GifRef;

pub const KEY_GIPHY: &str = "svc:giphy";
pub const KEY_KLIPY: &str = "svc:klipy";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GifItem {
    pub provider: String,
    /// GIPHY id or KLIPY slug
    pub id: String,
    pub title: String,
    pub sticker: bool,
    /// Small animated preview for the grid (WebP)
    pub preview: String,
    pub width: u32,
    pub height: u32,
    /// GIF to paste (size depends on the "original size" setting)
    pub gif_url: String,
    /// GIPHY: analytics "onsent" pingback URL (empty for KLIPY)
    #[serde(default)]
    pub ping: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub provider: String,
    /// Empty = trending
    pub q: String,
    pub sticker: bool,
    pub page: u32,
}

pub struct Ctx<'a> {
    pub key: &'a str,
    /// "g" | "pg" | "pg-13" | "r"
    pub rating: &'a str,
    /// UI language, "de" | "en"
    pub lang: &'a str,
    pub customer_id: &'a str,
    pub original: bool,
}

pub const PAGE_SIZE: u32 = 30;

/// Maps the shared age rating onto KLIPY's content filter levels.
pub fn klipy_filter(rating: &str) -> &'static str {
    match rating {
        "g" => "high",
        "pg" => "medium",
        "pg-13" => "low",
        _ => "off",
    }
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn search_url(q: &Query, c: &Ctx) -> Result<String> {
    let kind = if q.sticker { "stickers" } else { "gifs" };
    let trending = q.q.trim().is_empty();
    Ok(match q.provider.as_str() {
        "giphy" => {
            let mut u = format!(
                "https://api.giphy.com/v1/{kind}/{}?api_key={}&limit={PAGE_SIZE}&offset={}&rating={}&random_id={}",
                if trending { "trending" } else { "search" },
                enc(c.key),
                q.page * PAGE_SIZE,
                enc(c.rating),
                enc(c.customer_id)
            );
            if !trending {
                u.push_str(&format!("&q={}&lang={}", enc(&q.q.chars().take(50).collect::<String>()), enc(c.lang)));
            }
            u
        }
        "klipy" => {
            let mut u = format!(
                "https://api.klipy.com/api/v1/{}/{kind}/{}?page={}&per_page={PAGE_SIZE}&customer_id={}&locale={}&content_filter={}",
                enc(c.key),
                if trending { "trending" } else { "search" },
                q.page + 1,
                enc(c.customer_id),
                match c.lang {
                    "en" => "us",
                    "pt" => "br",
                    "vi" => "vn",
                    l => l,
                },
                klipy_filter(c.rating)
            );
            if !trending {
                u.push_str(&format!("&q={}", enc(&q.q)));
            }
            u
        }
        other => bail!("unknown GIF provider {other}"),
    })
}

/// URL that loads specific items again (for "recently used").
pub fn items_url(provider: &str, sticker: bool, ids: &[String], c: &Ctx) -> Result<String> {
    let list = ids.iter().map(|i| enc(i)).collect::<Vec<_>>().join(",");
    Ok(match provider {
        "giphy" => format!("https://api.giphy.com/v1/gifs?api_key={}&ids={list}&random_id={}", enc(c.key), enc(c.customer_id)),
        "klipy" => format!(
            "https://api.klipy.com/api/v1/{}/{}/items?slugs={list}&customer_id={}",
            enc(c.key),
            if sticker { "stickers" } else { "gifs" },
            enc(c.customer_id)
        ),
        other => bail!("unknown GIF provider {other}"),
    })
}

fn s(v: &Value, ptr: &str) -> String {
    v.pointer(ptr).and_then(Value::as_str).unwrap_or_default().to_string()
}
fn n(v: &Value, ptr: &str) -> u32 {
    v.pointer(ptr).and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0) as u32
}

/// Parses a GIPHY or KLIPY list response. Ads and unknown item types are skipped.
pub fn parse_items(provider: &str, sticker_hint: bool, body: &Value, original: bool) -> Vec<GifItem> {
    match provider {
        "giphy" => body
            .get("data")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| {
                        let id = s(g, "/id");
                        let preview = s(g, "/images/fixed_height_small/webp");
                        let preview = if preview.is_empty() { s(g, "/images/fixed_height_small/url") } else { preview };
                        let gif_url = if original { s(g, "/images/original/url") } else { s(g, "/images/downsized/url") };
                        if id.is_empty() || preview.is_empty() || gif_url.is_empty() {
                            return None;
                        }
                        Some(GifItem {
                            provider: "giphy".into(),
                            id,
                            title: s(g, "/title"),
                            sticker: s(g, "/type") == "sticker" || sticker_hint,
                            preview,
                            width: n(g, "/images/fixed_height_small/width"),
                            height: n(g, "/images/fixed_height_small/height"),
                            gif_url,
                            ping: s(g, "/analytics/onsent/url"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => body
            .pointer("/data/data")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| {
                        let kind = s(g, "/type");
                        if kind != "gif" && kind != "sticker" {
                            return None; // ads ("ad") and anything unknown
                        }
                        let slug = s(g, "/slug");
                        let preview = s(g, "/file/sm/webp/url");
                        let gif_url = if original { s(g, "/file/hd/gif/url") } else { s(g, "/file/md/gif/url") };
                        if slug.is_empty() || preview.is_empty() || gif_url.is_empty() {
                            return None;
                        }
                        Some(GifItem {
                            provider: "klipy".into(),
                            id: slug,
                            title: s(g, "/title"),
                            sticker: kind == "sticker",
                            preview,
                            width: n(g, "/file/sm/webp/width"),
                            height: n(g, "/file/sm/webp/height"),
                            gif_url,
                            ping: String::new(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Error text for non-2xx answers; rate limits are reported as "rate-limited".
pub fn check_status(status: u16, body: &Value) -> Result<()> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    if status == 429 {
        bail!("rate-limited");
    }
    if status == 401 || status == 403 {
        bail!("bad-key");
    }
    let msg = body.pointer("/meta/msg").or_else(|| body.pointer("/message")).and_then(Value::as_str).unwrap_or("error");
    Err(anyhow!("HTTP {status}: {msg}"))
}

#[cfg(windows)]
pub async fn get_json(url: &str) -> Result<Value> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).redirect(reqwest::redirect::Policy::none()).build()?;
    let resp = client.get(url).send().await.map_err(|e| anyhow!("request failed: {}", e.without_url()))?;
    let status = resp.status().as_u16();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    check_status(status, &body)?;
    Ok(body)
}

#[cfg(windows)]
pub async fn download(url: &str) -> Result<Vec<u8>> {
    const MAX: u64 = 30 << 20;
    // Redirects only within the GIF services' own hosts.
    let policy = reqwest::redirect::Policy::custom(|a| if a.previous().len() < 5 && trusted_url(a.url().as_str()) { a.follow() } else { a.stop() });
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).redirect(policy).build()?;
    let resp = client.get(url).send().await.map_err(|e| anyhow!("download failed: {}", e.without_url()))?;
    if !resp.status().is_success() {
        bail!("download failed: HTTP {}", resp.status().as_u16());
    }
    if resp.content_length().is_some_and(|n| n > MAX) {
        bail!("download too large");
    }
    let bytes = resp.bytes().await?;
    if bytes.len() as u64 > MAX {
        bail!("download too large");
    }
    Ok(bytes.to_vec())
}

/// Tells the service which GIF was sent (GIPHY pingback / KLIPY share trigger).
#[cfg(windows)]
pub async fn report_sent(item: &GifItem, key: &str, customer_id: &str, query: &str) {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).redirect(reqwest::redirect::Policy::none()).build() {
        Ok(c) => c,
        Err(_) => return,
    };
    let r = match item.provider.as_str() {
        "giphy" if !item.ping.is_empty() => {
            let sep = if item.ping.contains('?') { '&' } else { '?' };
            let url = format!("{}{sep}ts={}&random_id={}", item.ping, crate::model::now_ms(), enc(customer_id));
            client.get(url).send().await.map(|_| ())
        }
        "klipy" => {
            let kind = if item.sticker { "stickers" } else { "gifs" };
            let url = format!("https://api.klipy.com/api/v1/{}/{kind}/share/{}", enc(key), enc(&item.id));
            client.post(url).json(&serde_json::json!({ "customer_id": customer_id, "q": query })).send().await.map(|_| ())
        }
        _ => Ok(()),
    };
    if let Err(e) = r {
        log::info!("gif feedback failed: {e}");
    }
}

#[cfg(windows)]
pub async fn giphy_random_id(key: &str) -> Result<String> {
    let body = get_json(&format!("https://api.giphy.com/v1/randomid?api_key={}", enc(key))).await?;
    let id = s(&body, "/data/random_id");
    if id.is_empty() {
        bail!("no random id");
    }
    Ok(id)
}

#[cfg(not(windows))]
pub async fn get_json(_url: &str) -> Result<Value> {
    bail!("HTTP client is only built for Windows")
}
#[cfg(not(windows))]
pub async fn download(_url: &str) -> Result<Vec<u8>> {
    bail!("HTTP client is only built for Windows")
}
#[cfg(not(windows))]
pub async fn report_sent(_item: &GifItem, _key: &str, _customer_id: &str, _query: &str) {}
#[cfg(not(windows))]
pub async fn giphy_random_id(_key: &str) -> Result<String> {
    bail!("HTTP client is only built for Windows")
}

/// Groups "recently used" refs per provider and kind (GIF/sticker), keeping the order.
pub fn by_provider(refs: &[GifRef]) -> Vec<(String, bool, Vec<String>)> {
    let mut out: Vec<(String, bool, Vec<String>)> = Vec::new();
    for r in refs {
        match out.iter_mut().find(|(p, st, _)| *p == r.provider && *st == r.sticker) {
            Some((_, _, ids)) => ids.push(r.id.clone()),
            None => out.push((r.provider.clone(), r.sticker, vec![r.id.clone()])),
        }
    }
    out
}

/// HTML fragment for pasting a GIF into mail/web editors.
pub fn html_for(item: &GifItem) -> String {
    let alt = item.title.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
    format!("<img src=\"{}\" alt=\"{alt}\">", item.gif_url.replace('"', "%22"))
}

/// True for https URLs on GIPHY's or KLIPY's own hosts.
pub fn trusted_url(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase();
    if host.contains('@') || host.contains(':') {
        return false;
    }
    ["giphy.com", "klipy.com"].iter().any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_hosts() {
        assert!(trusted_url("https://media2.giphy.com/media/x/giphy.gif"));
        assert!(trusted_url("https://static.klipy.com/a.gif"));
        assert!(!trusted_url("http://media.giphy.com/a.gif"));
        assert!(!trusted_url("https://giphy.com.evil.example/a.gif"));
        assert!(!trusted_url("https://evil.example/?x=giphy.com"));
        assert!(!trusted_url("https://user@evil.example/a.gif"));
        assert!(!trusted_url("https://localhost:8080/a.gif"));
    }
    use serde_json::json;

    fn ctx() -> Ctx<'static> {
        Ctx { key: "K y", rating: "pg", lang: "de", customer_id: "abc", original: false }
    }

    #[test]
    fn giphy_urls() {
        let q = Query { provider: "giphy".into(), q: "glückwunsch".into(), sticker: false, page: 1 };
        let u = search_url(&q, &ctx()).unwrap();
        assert!(u.starts_with("https://api.giphy.com/v1/gifs/search?api_key=K%20y&limit=30&offset=30&rating=pg"));
        assert!(u.contains("&q=gl%C3%BCckwunsch&lang=de"));
        let t = search_url(&Query { q: " ".into(), sticker: true, page: 0, ..q }, &ctx()).unwrap();
        assert!(t.starts_with("https://api.giphy.com/v1/stickers/trending?"));
        assert!(!t.contains("&q="));
    }

    #[test]
    fn klipy_urls_map_rating() {
        let q = Query { provider: "klipy".into(), q: "cat".into(), sticker: true, page: 0 };
        let u = search_url(&q, &ctx()).unwrap();
        assert!(u.starts_with("https://api.klipy.com/api/v1/K%20y/stickers/search?page=1&per_page=30&customer_id=abc&locale=de&content_filter=medium"));
        assert!(u.ends_with("&q=cat"));
        assert_eq!(klipy_filter("g"), "high");
        assert_eq!(klipy_filter("r"), "off");
    }

    #[test]
    fn parse_giphy() {
        let body = json!({"data":[{"id":"x1","title":"Yay","type":"gif","images":{
            "fixed_height_small":{"webp":"https://media.giphy.com/s.webp","url":"https://media.giphy.com/s.gif","width":"120","height":"100"},
            "downsized":{"url":"https://media.giphy.com/d.gif"},"original":{"url":"https://media.giphy.com/o.gif"}},
            "analytics":{"onsent":{"url":"https://pingback.giphy.com/x"}}}]});
        let items = parse_items("giphy", false, &body, false);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview, "https://media.giphy.com/s.webp");
        assert_eq!(items[0].gif_url, "https://media.giphy.com/d.gif");
        assert_eq!((items[0].width, items[0].height), (120, 100));
        assert_eq!(items[0].ping, "https://pingback.giphy.com/x");
        assert_eq!(parse_items("giphy", false, &body, true)[0].gif_url, "https://media.giphy.com/o.gif");
    }

    #[test]
    fn parse_klipy_skips_ads() {
        let file = json!({"sm":{"webp":{"url":"https://static.klipy.com/s.webp","width":220,"height":180}},
                          "md":{"gif":{"url":"https://static.klipy.com/m.gif"}},"hd":{"gif":{"url":"https://static.klipy.com/h.gif"}}});
        let body = json!({"data":{"data":[
            {"type":"ad","content":"<html>"},
            {"type":"sticker","slug":"happy-cat","title":"Cat","file":file}
        ],"has_next":true}});
        let items = parse_items("klipy", false, &body, false);
        assert_eq!(items.len(), 1);
        assert!(items[0].sticker);
        assert_eq!(items[0].id, "happy-cat");
        assert_eq!(items[0].gif_url, "https://static.klipy.com/m.gif");
    }

    #[test]
    fn status_errors() {
        assert_eq!(check_status(429, &Value::Null).unwrap_err().to_string(), "rate-limited");
        assert_eq!(check_status(401, &Value::Null).unwrap_err().to_string(), "bad-key");
        assert!(check_status(200, &Value::Null).is_ok());
    }

    #[test]
    fn groups_refs() {
        let r = |p: &str, id: &str| GifRef { provider: p.into(), id: id.into(), sticker: false };
        let g = by_provider(&[r("giphy", "a"), r("klipy", "b"), r("giphy", "c")]);
        assert_eq!(g, vec![("giphy".to_string(), false, vec!["a".to_string(), "c".into()]), ("klipy".to_string(), false, vec!["b".to_string()])]);
    }
}
