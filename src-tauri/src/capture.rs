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
    pub img: RgbaImage,
}

pub struct Shot {
    pub img: Arc<RgbaImage>,
    pub bounds: Bounds,
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
            img,
        });
    }
    if out.is_empty() {
        return Err("no monitors found".into());
    }
    Ok(out)
}

pub fn freeze_at(point: (i32, i32)) -> Result<Frozen, String> {
    let m = match Monitor::from_point(point.0, point.1) {
        Ok(m) => m,
        Err(_) => Monitor::all()
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or("no monitors found")?,
    };
    let img = m.capture_image().map_err(|e| e.to_string())?;
    Ok(Frozen {
        bounds: Bounds::of(&m)?,
        img,
    })
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
