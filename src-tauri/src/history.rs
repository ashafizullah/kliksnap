//! Recent captures, kept in the app's data folder so they can be found
//! again: the full image, a small JPEG thumbnail, and the text OCR found in
//! it for searching. Only the newest `history_limit` are kept.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use xcap::image::{imageops, RgbaImage};

use crate::output::{self, Format};

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    /// Milliseconds since the Unix epoch.
    pub created: i64,
    pub width: u32,
    pub height: u32,
    /// Pixels per point of the screen it came from.
    pub scale: f64,
    /// Recognized text, for search; filled in after the capture.
    #[serde(default)]
    pub text: String,
}

const THUMB_SIDE: u32 = 400;

/// Guards the index file; captures and the History window touch it from
/// different threads.
static LOCK: Mutex<()> = Mutex::new(());

fn dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("history"))
}

fn read_index(app: &AppHandle) -> Vec<Entry> {
    dir(app)
        .and_then(|d| std::fs::read_to_string(d.join("index.json")).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn write_index(app: &AppHandle, entries: &[Entry]) -> Result<(), String> {
    let d = dir(app).ok_or("no data folder")?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let json = serde_json::to_string(entries).map_err(|e| e.to_string())?;
    // Write then rename, so a crash never leaves half an index.
    let tmp = d.join("index.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, d.join("index.json")).map_err(|e| e.to_string())
}

/// Only digits and dashes: ids name files, so nothing can point outside the folder.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_digit() || c == '-')
}

fn remove_files(d: &std::path::Path, id: &str) {
    for ext in ["png", "jpg"] {
        let _ = std::fs::remove_file(d.join(format!("{id}.{ext}")));
    }
}

/// Saves a capture, drops the oldest beyond `limit`, then recognizes its
/// text. Blocking: call off the main thread.
pub fn add(app: &AppHandle, img: &RgbaImage, scale: f64, limit: u32) -> Result<(), String> {
    if limit == 0 {
        return Ok(());
    }
    let d = dir(app).ok_or("no data folder")?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let now = chrono::Local::now();
    let mut id = now.format("%Y%m%d-%H%M%S-%3f").to_string();
    while d.join(format!("{id}.png")).exists() {
        id.push('0');
    }
    output::save(img, &d.join(format!("{id}.png")), Format::Png)?;
    let k = (THUMB_SIDE as f64 / img.width().max(img.height()) as f64).min(1.0);
    let thumb = imageops::thumbnail(
        img,
        ((img.width() as f64 * k) as u32).max(1),
        ((img.height() as f64 * k) as u32).max(1),
    );
    output::save(&thumb, &d.join(format!("{id}.jpg")), Format::Jpg(80))?;

    {
        let _guard = LOCK.lock().unwrap();
        let mut entries = read_index(app);
        entries.insert(
            0,
            Entry {
                id: id.clone(),
                created: now.timestamp_millis(),
                width: img.width(),
                height: img.height(),
                scale,
                text: String::new(),
            },
        );
        for old in entries.drain((limit as usize).min(entries.len())..) {
            remove_files(&d, &old.id);
        }
        write_index(app, &entries)?;
    }
    let _ = app.emit_to("history", "history:changed", ());

    let text = crate::ocr::recognize(img).unwrap_or_default();
    if !text.trim().is_empty() {
        let _guard = LOCK.lock().unwrap();
        let mut entries = read_index(app);
        if let Some(e) = entries.iter_mut().find(|e| e.id == id) {
            e.text = text;
            write_index(app, &entries)?;
            let _ = app.emit_to("history", "history:changed", ());
        }
    }
    Ok(())
}

/// Newest first.
pub fn list(app: &AppHandle) -> Vec<Entry> {
    let _guard = LOCK.lock().unwrap();
    read_index(app)
}

pub fn get(app: &AppHandle, id: &str) -> Option<(Entry, RgbaImage)> {
    if !valid_id(id) {
        return None;
    }
    let entry = list(app).into_iter().find(|e| e.id == id)?;
    let img = xcap::image::open(dir(app)?.join(format!("{id}.png")))
        .ok()?
        .to_rgba8();
    Some((entry, img))
}

/// The thumbnail's JPEG bytes.
pub fn thumbnail(app: &AppHandle, id: &str) -> Option<Vec<u8>> {
    if !valid_id(id) {
        return None;
    }
    std::fs::read(dir(app)?.join(format!("{id}.jpg"))).ok()
}

pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("bad id".into());
    }
    let _guard = LOCK.lock().unwrap();
    let mut entries = read_index(app);
    entries.retain(|e| e.id != id);
    write_index(app, &entries)?;
    if let Some(d) = dir(app) {
        remove_files(&d, id);
    }
    Ok(())
}

/// Keeps only the newest `limit`; 0 deletes everything.
pub fn trim(app: &AppHandle, limit: u32) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap();
    let mut entries = read_index(app);
    if entries.len() <= limit as usize {
        return Ok(());
    }
    let d = dir(app).ok_or("no data folder")?;
    for old in entries.drain(limit as usize..) {
        remove_files(&d, &old.id);
    }
    write_index(app, &entries)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_cannot_leave_the_folder() {
        assert!(super::valid_id("20261009-220455-123"));
        assert!(!super::valid_id("../settings"));
        assert!(!super::valid_id(""));
        assert!(!super::valid_id("1/2"));
    }
}
