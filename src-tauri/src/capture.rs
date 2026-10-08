use std::sync::Arc;

use serde::{Deserialize, Serialize};
use xcap::image::{imageops, RgbaImage};
use xcap::{Monitor, Window};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Area,
    Window,
    Screen,
    /// Area selection that copies the recognized text instead of the image.
    Text,
    /// The last area or window selection again, without the overlay.
    #[serde(rename = "last_area")]
    LastArea,
}

/// Monitor bounds in xcap's coordinate space, which is also the space Tauri
/// uses for window placement: points on macOS, physical pixels elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub scale: f64,
}

impl Bounds {
    fn of(m: &Monitor) -> Result<Self, String> {
        let e = |e: xcap::XCapError| e.to_string();
        Ok(Self {
            x: m.x().map_err(e)?,
            y: m.y().map_err(e)?,
            w: m.width().map_err(e)?,
            h: m.height().map_err(e)?,
            scale: m.scale_factor().map_err(e)? as f64,
        })
    }

    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w as i32 && y < self.y + self.h as i32
    }
}

pub struct Frozen {
    pub bounds: Bounds,
    /// None in live selection: the screen is captured when the selection ends.
    pub img: Option<RgbaImage>,
}

pub struct Shot {
    pub img: Arc<RgbaImage>,
    pub bounds: Bounds,
    /// Where the shot's top-left corner was on screen, in xcap space; None
    /// when it doesn't match a spot on screen (an edited copy).
    pub origin: Option<(i32, i32)>,
    pub refs: u32,
}

/// Window rectangle in xcap space.
pub type Rect = (i32, i32, u32, u32);

pub fn freeze_all() -> Result<Vec<Frozen>, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(monitors.len());
    for m in &monitors {
        let img = m.capture_image().map_err(|e| e.to_string())?;
        out.push(Frozen {
            bounds: Bounds::of(m)?,
            img: Some(img),
        });
    }
    if out.is_empty() {
        return Err("no monitors found".into());
    }
    Ok(out)
}

fn monitor_at(point: (i32, i32)) -> Result<Monitor, String> {
    match Monitor::from_point(point.0, point.1) {
        Ok(m) => Ok(m),
        Err(_) => Monitor::all()
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or_else(|| "no monitors found".into()),
    }
}

/// Bounds of the monitor holding `point`, without capturing it.
pub fn bounds_at(point: (i32, i32)) -> Result<Bounds, String> {
    Bounds::of(&monitor_at(point)?)
}

pub fn freeze_at(point: (i32, i32)) -> Result<Frozen, String> {
    let m = monitor_at(point)?;
    let img = m.capture_image().map_err(|e| e.to_string())?;
    Ok(Frozen {
        bounds: Bounds::of(&m)?,
        img: Some(img),
    })
}

/// Every monitor's bounds, without capturing it.
pub fn monitors() -> Result<Vec<Frozen>, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let out = monitors
        .iter()
        .map(|m| {
            Ok(Frozen {
                bounds: Bounds::of(m)?,
                img: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if out.is_empty() {
        return Err("no monitors found".into());
    }
    Ok(out)
}

/// Captures the monitor at `b` now.
pub fn capture_monitor(b: &Bounds) -> Result<RgbaImage, String> {
    let m = Monitor::all()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|m| m.x().ok() == Some(b.x) && m.y().ok() == Some(b.y))
        .ok_or("monitor disconnected")?;
    m.capture_image().map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
const IGNORED_OWNERS: &[&str] = &[
    "Window Server",
    "Dock",
    "Control Center",
    "SystemUIServer",
    "Notification Center",
    "Spotlight",
    "TextInputMenuAgent",
    "screencaptureui",
];

/// Visible top-level windows, front to back.
pub fn window_rects() -> Vec<Rect> {
    let own_pid = std::process::id();
    let Ok(windows) = Window::all() else {
        return Vec::new();
    };
    windows
        .iter()
        .filter_map(|w| {
            if w.pid().ok()? == own_pid || w.is_minimized().unwrap_or(true) {
                return None;
            }
            #[cfg(target_os = "macos")]
            if IGNORED_OWNERS.contains(&w.app_name().unwrap_or_default().as_str()) {
                return None;
            }
            let rect = (w.x().ok()?, w.y().ok()?, w.width().ok()?, w.height().ok()?);
            (rect.2 >= 40 && rect.3 >= 40).then_some(rect)
        })
        .collect()
}

/// Window rectangles that touch the monitor, as fractions of the monitor size.
pub fn relative_rects(rects: &[Rect], b: &Bounds) -> Vec<[f64; 4]> {
    let (bw, bh) = (b.w as f64, b.h as f64);
    rects
        .iter()
        .filter(|&&(x, y, w, h)| {
            x < b.x + b.w as i32 && y < b.y + b.h as i32 && x + w as i32 > b.x && y + h as i32 > b.y
        })
        .map(|&(x, y, w, h)| {
            [
                (x - b.x) as f64 / bw,
                (y - b.y) as f64 / bh,
                w as f64 / bw,
                h as f64 / bh,
            ]
        })
        .collect()
}

/// The screen under a live-selection overlay in `rect` (fractions of monitor
/// `b`), without the overlay. `overlay` is its window number on macOS.
pub fn under_overlay(b: &Bounds, overlay: u32, [fx, fy, fw, fh]: [f64; 4]) -> Option<RgbaImage> {
    let (bw, bh) = (b.w as f64, b.h as f64);
    let (x, y, w, h) = (b.x as f64 + fx * bw, b.y as f64 + fy * bh, fw * bw, fh * bh);
    #[cfg(target_os = "macos")]
    {
        crate::platform::capture_below(overlay, (x, y, w, h))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = overlay;
        let (x, y) = (x.round() as i32, y.round() as i32);
        let (w, h) = (w.round().max(1.0) as i32, h.round().max(1.0) as i32);
        // Capture the part on the monitor, and leave the rest transparent.
        let (x0, y0) = (x.max(b.x), y.max(b.y));
        let x1 = (x + w).min(b.x + b.w as i32);
        let y1 = (y + h).min(b.y + b.h as i32);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let m = Monitor::from_point(b.x, b.y).ok()?;
        let part = m
            .capture_region(
                (x0 - b.x) as u32,
                (y0 - b.y) as u32,
                (x1 - x0) as u32,
                (y1 - y0) as u32,
            )
            .ok()?;
        let mut full = RgbaImage::new(w as u32, h as u32);
        imageops::replace(&mut full, &part, (x0 - x) as i64, (y0 - y) as i64);
        Some(full)
    }
}

/// The top-left corner of `rect` (fractions of `b`) in xcap space.
pub fn origin(b: &Bounds, [fx, fy, _, _]: [f64; 4]) -> (i32, i32) {
    (
        b.x + (fx * b.w as f64).round() as i32,
        b.y + (fy * b.h as f64).round() as i32,
    )
}

/// Scales `img` to `percent` of its size, never below 1×1; returns the factor
/// used, so the caller can adjust the shot's pixels-per-point.
pub fn downscale(img: RgbaImage, percent: u32) -> (RgbaImage, f64) {
    let k = percent.clamp(10, 100) as f64 / 100.0;
    if k >= 1.0 {
        return (img, 1.0);
    }
    let w = ((img.width() as f64 * k).round() as u32).max(1);
    let h = ((img.height() as f64 * k).round() as u32).max(1);
    let k = w as f64 / img.width() as f64;
    (
        imageops::resize(&img, w, h, imageops::FilterType::CatmullRom),
        k,
    )
}

/// Crops by a rectangle given as fractions of the image size.
pub fn crop(img: &RgbaImage, [fx, fy, fw, fh]: [f64; 4]) -> Option<RgbaImage> {
    let (iw, ih) = (img.width() as f64, img.height() as f64);
    let x0 = (fx * iw).round().clamp(0.0, iw);
    let y0 = (fy * ih).round().clamp(0.0, ih);
    let x1 = ((fx + fw) * iw).round().clamp(0.0, iw);
    let y1 = ((fy + fh) * ih).round().clamp(0.0, ih);
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    if w == 0 || h == 0 {
        return None;
    }
    Some(imageops::crop_imm(img, x0 as u32, y0 as u32, w, h).to_image())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_maps_fractions_into_monitor_space() {
        let b = Bounds {
            x: -1440,
            y: 100,
            w: 1440,
            h: 900,
            scale: 2.0,
        };
        assert_eq!(origin(&b, [0.5, 0.25, 0.1, 0.1]), (-720, 325));
    }

    #[test]
    fn downscale_keeps_full_size_and_scales_down() {
        let (same, k) = downscale(RgbaImage::new(200, 100), 100);
        assert_eq!((same.dimensions(), k), ((200, 100), 1.0));
        let (half, k) = downscale(RgbaImage::new(200, 101), 50);
        assert_eq!(half.dimensions(), (100, 51));
        assert_eq!(k, 0.5);
        let (tiny, _) = downscale(RgbaImage::new(1, 1), 50);
        assert_eq!(tiny.dimensions(), (1, 1));
    }

    #[test]
    fn crop_rejects_empty_rects() {
        let img = RgbaImage::new(100, 50);
        assert!(crop(&img, [0.5, 0.5, 0.0, 0.2]).is_none());
        let part = crop(&img, [0.1, 0.2, 0.5, 0.5]).unwrap();
        assert_eq!(part.dimensions(), (50, 25));
    }
}
