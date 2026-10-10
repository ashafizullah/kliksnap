use std::borrow::Cow;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use xcap::image::codecs::jpeg::JpegEncoder;
use xcap::image::codecs::png::PngEncoder;
use xcap::image::{ExtendedColorType, ImageEncoder, RgbImage, RgbaImage};

pub fn copy(img: &RgbaImage) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_image(arboard::ImageData {
            width: img.width() as usize,
            height: img.height() as usize,
            bytes: Cow::Borrowed(img.as_raw()),
        })
        .map_err(|e| e.to_string())
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
    Png,
    /// JPEG at this quality, 1–100.
    Jpg(u8),
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpg(_) => "jpg",
        }
    }

    /// The format a file name asks for, if it names one.
    pub fn from_path(path: &Path, quality: u8) -> Option<Format> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" => Some(Format::Jpg(quality)),
            _ => None,
        }
    }
}

pub fn save(img: &RgbaImage, path: &Path, format: Format) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let out = BufWriter::new(file);
    let (w, h) = img.dimensions();
    match format {
        Format::Png => {
            PngEncoder::new(out).write_image(img.as_raw(), w, h, ExtendedColorType::Rgba8)
        }
        Format::Jpg(quality) => {
            let rgb = flatten(img);
            JpegEncoder::new_with_quality(out, quality.clamp(1, 100)).write_image(
                rgb.as_raw(),
                w,
                h,
                ExtendedColorType::Rgb8,
            )
        }
    }
    .map_err(|e| e.to_string())
}

/// JPEG has no alpha: transparent pixels (a clear backdrop) turn white.
fn flatten(img: &RgbaImage) -> RgbImage {
    RgbImage::from_fn(img.width(), img.height(), |x, y| {
        let [r, g, b, a] = img.get_pixel(x, y).0;
        let blend = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32)) / 255) as u8;
        [blend(r), blend(g), blend(b)].into()
    })
}

pub const DEFAULT_TEMPLATE: &str = "KlikSnap %Y-%m-%d at %H.%M.%S";
pub const DEFAULT_RECORDING_TEMPLATE: &str = "KlikSnap Recording %Y-%m-%d at %H.%M.%S";

/// The template's date fields filled in, with characters no file system
/// allows replaced. A broken or empty template falls back to the default.
pub fn file_name(template: &str, ext: &str) -> String {
    file_name_at(template, DEFAULT_TEMPLATE, ext, chrono::Local::now())
}

/// Like `file_name`, for a recording: `ext` is "mp4" or "gif".
pub fn recording_name(template: &str, ext: &str) -> String {
    file_name_at(
        template,
        DEFAULT_RECORDING_TEMPLATE,
        ext,
        chrono::Local::now(),
    )
}

fn file_name_at<Tz: chrono::TimeZone>(
    template: &str,
    fallback: &str,
    ext: &str,
    now: chrono::DateTime<Tz>,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    use std::fmt::Write;
    let fill = |t: &str| {
        let mut out = String::new();
        write!(out, "{}", now.format(t)).ok()?;
        let clean: String = out
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
                c if c.is_control() => '-',
                c => c,
            })
            .collect();
        let clean = clean.trim_matches(|c: char| c == '.' || c.is_whitespace());
        (!clean.is_empty()).then(|| clean.to_string())
    };
    let stem = fill(template)
        .or_else(|| fill(fallback))
        .unwrap_or_default();
    format!("{stem}.{ext}")
}

/// `dir/name`, with " (2)", " (3)"… appended when the file already exists.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if !path.exists() {
        return path;
    }
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| !p.exists())
        .unwrap()
}

/// Shows the file selected in Finder or Explorer.
pub fn reveal(path: &Path) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("explorer");
        c.arg(format!("/select,{}", path.display()));
        c
    } else {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(path);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Uncompressed 32-bit BMP. Webviews decode it natively and it costs almost
/// nothing to produce, which keeps the overlay fast even on 5K displays.
pub fn bmp(img: &RgbaImage) -> Vec<u8> {
    let (w, h) = (img.width(), img.height());
    let data_len = w * h * 4;
    let mut out = Vec::with_capacity(54 + data_len as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(54 + data_len).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // positive height: bottom-up rows
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    for row in img.as_raw().chunks_exact(w as usize * 4).rev() {
        for &[r, g, b, _] in row.as_chunks::<4>().0 {
            out.extend_from_slice(&[b, g, r, 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> chrono::DateTime<chrono::Utc> {
        use chrono::TimeZone;
        chrono::Utc.with_ymd_and_hms(2026, 10, 9, 7, 5, 3).unwrap()
    }

    #[test]
    fn file_name_fills_the_template() {
        assert_eq!(
            file_name_at(DEFAULT_TEMPLATE, DEFAULT_TEMPLATE, "png", at()),
            "KlikSnap 2026-10-09 at 07.05.03.png"
        );
        assert_eq!(
            file_name_at("shot-%H%M%S", DEFAULT_TEMPLATE, "jpg", at()),
            "shot-070503.jpg"
        );
    }

    #[test]
    fn file_name_replaces_bad_characters() {
        assert_eq!(
            file_name_at("a/b:%H:%M", DEFAULT_TEMPLATE, "png", at()),
            "a-b-07-05.png"
        );
        assert_eq!(
            file_name_at("  ..x.. ", DEFAULT_TEMPLATE, "png", at()),
            "x.png"
        );
    }

    #[test]
    fn file_name_falls_back_on_a_broken_template() {
        let fallback = "KlikSnap 2026-10-09 at 07.05.03.png";
        assert_eq!(file_name_at("", DEFAULT_TEMPLATE, "png", at()), fallback);
        assert_eq!(
            file_name_at("%Q bad", DEFAULT_TEMPLATE, "png", at()),
            fallback
        );
        assert_eq!(
            file_name_at("/:..", DEFAULT_TEMPLATE, "png", at()),
            "--.png"
        );
        assert_eq!(
            file_name_at("", DEFAULT_RECORDING_TEMPLATE, "mp4", at()),
            "KlikSnap Recording 2026-10-09 at 07.05.03.mp4"
        );
    }

    #[test]
    fn format_from_path() {
        assert_eq!(
            Format::from_path(Path::new("a.JPEG"), 80),
            Some(Format::Jpg(80))
        );
        assert_eq!(Format::from_path(Path::new("a.png"), 80), Some(Format::Png));
        assert_eq!(Format::from_path(Path::new("a.gif"), 80), None);
        assert_eq!(Format::from_path(Path::new("a"), 80), None);
    }

    #[test]
    fn saves_jpeg_with_transparency_flattened() {
        let dir = std::env::temp_dir().join(format!("kliksnap-jpg-{}", std::process::id()));
        let path = dir.join("a.jpg");
        let img = RgbaImage::from_pixel(16, 16, [0, 0, 0, 0].into());
        save(&img, &path, Format::Jpg(90)).unwrap();
        let back = xcap::image::open(&path).unwrap().to_rgb8();
        assert_eq!(back.dimensions(), (16, 16));
        assert!(back.pixels().all(|p| p.0.iter().all(|&c| c > 245)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unique_path_keeps_the_extension() {
        let dir = std::env::temp_dir().join(format!("kliksnap-unique-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.mp4"), b"").unwrap();
        assert_eq!(unique_path(&dir, "a.mp4"), dir.join("a (2).mp4"));
        assert_eq!(unique_path(&dir, "b.png"), dir.join("b.png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
