//! Removes tracking parameters from links (Q29).

const EXACT: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "dclid", "gbraid", "wbraid", "msclkid", "mc_eid", "mc_cid", "igshid", "igsh", "yclid", "twclid", "ttclid", "li_fsh",
    "_hsenc", "_hsmi", "mkt_tok", "oly_anon_id", "oly_enc_id", "vero_id", "rb_clickid", "s_cid", "ref_src", "ref_url", "spm", "scm", "_ga", "_gl",
];
const PREFIX: &[&str] = &["utm_", "pk_", "mtm_", "hsa_"];

/// Hosts where "si" is a share tracker (YouTube, Spotify).
fn si_is_tracking(host: &str) -> bool {
    ["youtube.com", "youtu.be", "spotify.com", "music.youtube.com"].iter().any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

/// Cleaned link, or None if `text` is not a single http(s) link or there was nothing to remove.
pub fn clean(text: &str) -> Option<String> {
    let t = text.trim();
    if !(t.starts_with("http://") || t.starts_with("https://")) || t.chars().any(char::is_whitespace) {
        return None;
    }
    let (before_hash, hash) = match t.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (t, None),
    };
    let (base, query) = before_hash.split_once('?')?;
    let host = base.split("://").nth(1).unwrap_or("").split('/').next().unwrap_or("").to_lowercase();
    let keep: Vec<&str> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .filter(|p| {
            let name = p.split('=').next().unwrap_or("").to_lowercase();
            !(EXACT.contains(&name.as_str()) || PREFIX.iter().any(|x| name.starts_with(x)) || (name == "si" && si_is_tracking(&host)) || (name == "feature" && host.contains("youtu")))
        })
        .collect();
    let total = query.split('&').filter(|p| !p.is_empty()).count();
    if keep.len() == total {
        return None;
    }
    let mut out = base.to_string();
    if !keep.is_empty() {
        out.push('?');
        out.push_str(&keep.join("&"));
    }
    if let Some(h) = hash {
        out.push('#');
        out.push_str(h);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_trackers() {
        assert_eq!(clean("https://example.com/a?utm_source=x&utm_medium=y").as_deref(), Some("https://example.com/a"));
        assert_eq!(clean("https://example.com/a?id=5&fbclid=abc#top").as_deref(), Some("https://example.com/a?id=5#top"));
        assert_eq!(clean("https://youtu.be/xyz?si=abc123").as_deref(), Some("https://youtu.be/xyz"));
        assert_eq!(clean("https://www.youtube.com/watch?v=abc&si=zz&feature=shared").as_deref(), Some("https://www.youtube.com/watch?v=abc"));
        assert_eq!(clean("https://shop.example/?si=keep").as_deref(), None);
    }

    #[test]
    fn leaves_other_text() {
        assert_eq!(clean("https://example.com/a?id=5"), None);
        assert_eq!(clean("https://example.com/a"), None);
        assert_eq!(clean("siehe https://example.com/?utm_source=x"), None);
        assert_eq!(clean("hello"), None);
    }
}
