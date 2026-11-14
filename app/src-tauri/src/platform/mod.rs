//! OS integration. The real implementation is Windows-only; other targets get a stub so
//! the platform-independent core (storage, search, crypto, …) can be built and tested anywhere.

use crate::model::ClipContent;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use self::stub::*;

/// Result of reading the clipboard once.
#[derive(Debug, Default)]
pub struct Captured {
    pub content: ClipContent,
    /// Lower-case executable name of the clipboard owner (or foreground app as fallback).
    pub source_exe: Option<String>,
    /// Marked as sensitive/excluded by the source application.
    pub excluded: bool,
    /// Something was skipped because it exceeded the configured size limit.
    pub too_large: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_text_bytes: usize,
    pub max_image_bytes: usize,
}

/// Physical-pixel rectangle.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A key combination handled by our own low-level keyboard hook (used for combinations that
/// Windows or another program already owns, e.g. Win+V).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookCombo {
    pub vk: u32,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Hotkey id (`HK_*` in lib.rs)
    pub id: u8,
}

/// Parses "Ctrl+Super+V" style strings into a virtual-key combination.
pub fn parse_combo(text: &str, id: u8) -> Option<HookCombo> {
    let mut c = HookCombo { vk: 0, ctrl: false, alt: false, shift: false, win: false, id };
    for part in text.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "commandorcontrol" => c.ctrl = true,
            "alt" | "option" => c.alt = true,
            "shift" => c.shift = true,
            "super" | "win" | "meta" | "cmd" => c.win = true,
            key => {
                let key = key.strip_prefix("key").filter(|k| k.len() == 1).or_else(|| key.strip_prefix("digit")).unwrap_or(key);
                c.vk = match key {
                    k if k.len() == 1 && k.as_bytes()[0].is_ascii_alphanumeric() => k.to_ascii_uppercase().as_bytes()[0] as u32,
                    k if k.starts_with('f') && k[1..].parse::<u32>().is_ok_and(|n| (1..=24).contains(&n)) => 0x6F + k[1..].parse::<u32>().ok()?,
                    "space" => 0x20,
                    "insert" => 0x2D,
                    "home" => 0x24,
                    "end" => 0x23,
                    "pageup" => 0x21,
                    "pagedown" => 0x22,
                    "backquote" => 0xC0,
                    "period" => 0xBE,
                    "comma" => 0xBC,
                    _ => return None,
                };
            }
        }
    }
    (c.vk != 0 && (c.ctrl || c.alt || c.win || c.shift)).then_some(c)
}

/// PNG → BMP file bytes (used to provide CF_DIB for apps that don't understand PNG).
pub fn png_to_bmp(png: &[u8]) -> anyhow::Result<Vec<u8>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?;
    let mut out = std::io::Cursor::new(Vec::new());
    // 24-bit BI_RGB is the most widely understood DIB flavour; alpha travels in the PNG format.
    img.to_rgb8().write_to(&mut out, image::ImageFormat::Bmp)?;
    Ok(out.into_inner())
}

/// BMP/DIB file bytes → PNG.
pub fn bmp_to_png(bmp: &[u8]) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(bmp, image::ImageFormat::Bmp)?;
    let (w, h) = (img.width(), img.height());
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)?;
    Ok((out.into_inner(), w, h))
}

/// Returns (thumbnail png, width, height) for an image.
pub fn make_thumbnail(png: &[u8], max: u32) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?;
    let (w, h) = (img.width(), img.height());
    let thumb = if w > max || h > max { img.thumbnail(max, max) } else { img };
    let mut out = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut out, image::ImageFormat::Png)?;
    Ok((out.into_inner(), w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(64, 32, image::Rgba([10, 20, 30, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn combo_parsing() {
        let c = parse_combo("Super+V", 0).unwrap();
        assert!(c.win && !c.ctrl && c.vk == 0x56);
        let c = parse_combo("Ctrl+Super+V", 1).unwrap();
        assert!(c.win && c.ctrl && c.id == 1);
        assert_eq!(parse_combo("Ctrl+Shift+F5", 0).unwrap().vk, 0x74);
        assert_eq!(parse_combo("Alt+Space", 0).unwrap().vk, 0x20);
        assert_eq!(parse_combo("Ctrl+Digit1", 0).unwrap().vk, 0x31);
        assert!(parse_combo("V", 0).is_none());
        assert!(parse_combo("Ctrl+Nope", 0).is_none());
    }

    #[test]
    fn bmp_png_roundtrip() {
        let png = sample_png();
        let bmp = png_to_bmp(&png).unwrap();
        let (png2, w, h) = bmp_to_png(&bmp).unwrap();
        assert_eq!((w, h), (64, 32));
        assert!(!png2.is_empty());
    }

    #[test]
    fn thumbnail_downscales() {
        let (t, w, h) = make_thumbnail(&sample_png(), 16).unwrap();
        assert_eq!((w, h), (64, 32));
        let ti = image::load_from_memory(&t).unwrap();
        assert!(ti.width() <= 16 && ti.height() <= 16);
    }
}
