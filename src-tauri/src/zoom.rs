//! Zooms a recording in on the mouse while a key is held. A thread follows
//! the key and the mouse and eases the view toward them; the recorder crops
//! each frame to the view and scales it back up to the video's size. The
//! view is anchored at the mouse: what is under the pointer stays under it,
//! so the zoom can be shown live on screen and clicks still land right.
#![cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::capture::Bounds;
use crate::platform;

/// The zoom levels Settings offers, in percent.
pub const MIN_LEVEL: u32 = 125;
pub const MAX_LEVEL: u32 = 400;

/// Seconds the zoom takes to get most of the way to where it's going.
const EASE: f64 = 0.12;

/// The modifier key held to zoom: "ctrl", "alt" (Option), "shift" or
/// "meta" (Command, or the Windows key). Held with another modifier, as in
/// a shortcut, it doesn't zoom.
pub fn key_mask(key: &str) -> Option<u8> {
    match key {
        "ctrl" => Some(platform::MOD_CTRL),
        "alt" => Some(platform::MOD_ALT),
        "shift" => Some(platform::MOD_SHIFT),
        "meta" => Some(platform::MOD_META),
        _ => None,
    }
}

/// What a frame shows: magnified `scale` times with `(x, y)` held in place,
/// fractions of the region recorded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}

impl View {
    pub const FULL: View = View {
        scale: 1.0,
        x: 0.5,
        y: 0.5,
    };

    pub fn is_full(&self) -> bool {
        self.scale <= 1.0
    }

    /// The part of the region shown: x, y, width and height as fractions,
    /// y from the top.
    pub fn rect(&self) -> [f64; 4] {
        let size = 1.0 / self.scale.max(1.0);
        let x = self.x.clamp(0.0, 1.0) * (1.0 - size);
        let y = self.y.clamp(0.0, 1.0) * (1.0 - size);
        [x, y, size, size]
    }

    /// The part of a `w`×`h` frame shown: x, y, width and height in pixels,
    /// kept inside the frame.
    pub fn crop(&self, w: usize, h: usize) -> (usize, usize, usize, usize) {
        let [fx, fy, fw, fh] = self.rect();
        let cw = ((w as f64 * fw).round() as usize).clamp(1, w.max(1));
        let ch = ((h as f64 * fh).round() as usize).clamp(1, h.max(1));
        let x = (fx * w as f64).round().min(w.saturating_sub(cw) as f64) as usize;
        let y = (fy * h as f64).round().min(h.saturating_sub(ch) as f64) as usize;
        (x, y, cw, ch)
    }

    /// The view `dt` seconds on, easing toward `level` while `held` and back
    /// out when not, at the mouse `(mx, my)`.
    fn step(self, held: bool, level: f64, (mx, my): (f64, f64), dt: f64) -> View {
        let target = if held { level } else { 1.0 };
        let mut scale = self.scale + (target - self.scale) * (1.0 - (-dt / EASE).exp());
        if (scale - target).abs() < 0.002 {
            scale = target;
        }
        View {
            scale,
            x: mx,
            y: my,
        }
    }
}

pub struct Zoom {
    view: Mutex<View>,
    stop: AtomicBool,
}

impl Zoom {
    pub fn view(&self) -> View {
        *self.view.lock().unwrap()
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// A zoom that stays at `view`, for tests.
    #[cfg(test)]
    pub fn fixed(view: View) -> Arc<Zoom> {
        Arc::new(Zoom {
            view: Mutex::new(view),
            stop: AtomicBool::new(false),
        })
    }
}

/// Starts following the modifiers in `mask` and the mouse over `rect`
/// (fractions of monitor `b`), zooming in `level` percent while held, until
/// stopped.
pub fn start(b: Bounds, rect: [f64; 4], mask: u8, level: u32) -> Arc<Zoom> {
    let zoom = Arc::new(Zoom {
        view: Mutex::new(View::FULL),
        stop: AtomicBool::new(false),
    });
    let z = zoom.clone();
    let level = level.clamp(MIN_LEVEL, MAX_LEVEL) as f64 / 100.0;
    let [fx, fy, fw, fh] = rect;
    std::thread::spawn(move || {
        let mut last = Instant::now();
        while !z.stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(10));
            let now = Instant::now();
            let dt = (now - last).as_secs_f64();
            last = now;
            let held = platform::modifiers() == mask;
            // The cursor and the bounds share a coordinate space.
            let (x, y) = platform::cursor_pos();
            let mouse = (
                (((x - b.x) as f64 / b.w as f64 - fx) / fw).clamp(0.0, 1.0),
                (((y - b.y) as f64 / b.h as f64 - fy) / fh).clamp(0.0, 1.0),
            );
            let mut view = z.view.lock().unwrap();
            *view = view.step(held, level, mouse, dt);
        }
    });
    zoom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mouse_stays_on_what_it_points_at() {
        for (x, y) in [(0.0, 0.0), (0.3, 0.8), (1.0, 0.5)] {
            let v = View { scale: 3.0, x, y };
            let [fx, fy, fw, fh] = v.rect();
            // The point under the mouse shows at the mouse.
            assert!(((x - fx) / fw - x).abs() < 1e-9);
            assert!(((y - fy) / fh - y).abs() < 1e-9);
        }
    }

    #[test]
    fn crop_stays_inside_the_frame() {
        let v = View {
            scale: 2.0,
            x: 0.5,
            y: 0.5,
        };
        assert_eq!(v.crop(1000, 600), (250, 150, 500, 300));
        let corner = View {
            scale: 2.0,
            x: 0.0,
            y: 1.0,
        };
        assert_eq!(corner.crop(1000, 600), (0, 300, 500, 300));
        assert_eq!(View::FULL.crop(1000, 600), (0, 0, 1000, 600));
    }

    #[test]
    fn step_eases_in_on_the_mouse_and_back_out() {
        let mut v = View::FULL;
        for _ in 0..200 {
            v = v.step(true, 2.0, (0.2, 0.7), 0.01);
        }
        assert_eq!(v.scale, 2.0);
        assert!((v.x - 0.2).abs() < 1e-3 && (v.y - 0.7).abs() < 1e-3);
        let half = v.step(false, 2.0, (0.9, 0.9), 0.05);
        assert!(half.scale > 1.0 && half.scale < 2.0);
        assert_eq!((half.x, half.y), (0.9, 0.9));
        for _ in 0..200 {
            v = v.step(false, 2.0, (0.9, 0.9), 0.01);
        }
        assert!(v.is_full());
    }

    #[test]
    fn keys_map_to_modifiers() {
        assert_eq!(key_mask("ctrl"), Some(platform::MOD_CTRL));
        assert_eq!(key_mask("meta"), Some(platform::MOD_META));
        assert_eq!(key_mask(""), None);
    }
}
