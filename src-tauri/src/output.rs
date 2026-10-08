use std::borrow::Cow;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use xcap::image::codecs::png::PngEncoder;
use xcap::image::{ExtendedColorType, ImageEncoder, RgbaImage};

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

pub fn save_png(img: &RgbaImage, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    PngEncoder::new(BufWriter::new(file))
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
}

pub fn file_name() -> String {
    chrono::Local::now()
        .format("KlikSnap %Y-%m-%d at %H.%M.%S.png")
        .to_string()
}

/// `dir/name`, with " (2)", " (3)"… appended when the file already exists.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if !path.exists() {
        return path;
    }
    let stem = name.trim_end_matches(".png");
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).png")))
        .find(|p| !p.exists())
        .unwrap()
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
