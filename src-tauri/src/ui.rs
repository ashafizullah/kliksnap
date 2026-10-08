//! Window creation and placement. Every window is created on demand and
//! destroyed when closed, so an idle KlikSnap is just a tray icon.

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::capture::Bounds;

/// A rectangle in native window units: points on macOS, physical pixels elsewhere.
#[derive(Clone, Copy)]
struct Area {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    scale: f64,
}

impl Area {
    /// Converts a logical length to native units.
    fn n(&self, logical: f64) -> f64 {
        if cfg!(target_os = "macos") {
            logical
        } else {
            logical * self.scale
        }
    }

    fn logical_size(&self) -> (f64, f64) {
        (self.w / self.n(1.0), self.h / self.n(1.0))
    }
}

/// The work area (screen minus menu bar, Dock or taskbar) of the monitor with these bounds.
fn work_area(app: &AppHandle, b: &Bounds) -> Area {
    let full = Area {
        x: b.x as f64,
        y: b.y as f64,
        w: b.w as f64,
        h: b.h as f64,
        scale: b.scale,
    };
    let Ok(monitors) = app.available_monitors() else {
        return full;
    };
    for m in monitors {
        let s = m.scale_factor();
        let wa = m.work_area();
        let (pos, area) = if cfg!(target_os = "macos") {
            let p = m.position().to_logical::<f64>(s);
            let wp = wa.position.to_logical::<f64>(s);
            let ws = wa.size.to_logical::<f64>(s);
            ((p.x, p.y), (wp.x, wp.y, ws.width, ws.height))
        } else {
            let p = m.position();
            (
                (p.x as f64, p.y as f64),
                (
                    wa.position.x as f64,
                    wa.position.y as f64,
                    wa.size.width as f64,
                    wa.size.height as f64,
                ),
            )
        };
        if (pos.0 - full.x).abs() < 2.0 && (pos.1 - full.y).abs() < 2.0 {
            return Area {
                x: area.0,
                y: area.1,
                w: area.2,
                h: area.3,
                scale: s,
            };
        }
    }
    full
}

/// Places a window at native coordinates with a logical size.
fn place(win: &WebviewWindow, area: &Area, x: f64, y: f64, w: f64, h: f64) {
    if cfg!(target_os = "macos") {
        let _ = win.set_size(LogicalSize::new(w, h));
        let _ = win.set_position(LogicalPosition::new(x, y));
    } else {
        let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
        let _ = win.set_size(PhysicalSize::new(area.n(w) as u32, area.n(h) as u32));
    }
}

pub fn open_overlay(
    app: &AppHandle,
    index: usize,
    b: &Bounds,
    generation: u32,
) -> tauri::Result<()> {
    let area = Area {
        x: b.x as f64,
        y: b.y as f64,
        w: b.w as f64,
        h: b.h as f64,
        scale: b.scale,
    };
    let (w, h) = area.logical_size();
    let url = format!("overlay.html?m={index}&g={generation}");
    let win =
        WebviewWindowBuilder::new(app, format!("overlay-{index}"), WebviewUrl::App(url.into()))
            .title("KlikSnap")
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .visible(false)
            .accept_first_mouse(true)
            .transparent(true)
            .inner_size(w, h)
            .build()?;
    place(&win, &area, area.x, area.y, w, h);
    Ok(())
}

pub fn overlays(app: &AppHandle) -> Vec<WebviewWindow> {
    app.webview_windows()
        .into_iter()
        .filter(|(label, _)| label.starts_with("overlay-"))
        .map(|(_, w)| w)
        .collect()
}

const PREVIEW_W: f64 = 248.0;
const PREVIEW_H: f64 = 168.0;
const PREVIEW_GAP: f64 = 10.0;
const MAX_PREVIEWS: usize = 5;
const MARGIN: f64 = 16.0;

pub fn preview_label(id: u32) -> String {
    format!("preview-{id}")
}

pub fn previews(app: &AppHandle) -> Vec<WebviewWindow> {
    app.webview_windows()
        .into_iter()
        .filter(|(label, _)| label.starts_with("preview-"))
        .map(|(_, w)| w)
        .collect()
}

/// How many previews fit stacked in the work area, up to `MAX_PREVIEWS`.
pub fn previews_that_fit(app: &AppHandle, b: &Bounds) -> usize {
    let area = work_area(app, b);
    let (_, h) = area.logical_size();
    let fit = ((h - 2.0 * MARGIN + PREVIEW_GAP) / (PREVIEW_H + PREVIEW_GAP)) as usize;
    fit.clamp(1, MAX_PREVIEWS)
}

/// Stacks the previews (oldest first) up from the bottom-right corner, newest at the bottom.
pub fn stack_previews(app: &AppHandle, ids: &[u32], b: &Bounds) {
    let area = work_area(app, b);
    let x = area.x + area.w - area.n(PREVIEW_W + MARGIN);
    for (slot, id) in ids.iter().rev().enumerate() {
        if let Some(win) = app.get_webview_window(&preview_label(*id)) {
            let up = PREVIEW_H + MARGIN + slot as f64 * (PREVIEW_H + PREVIEW_GAP);
            let y = area.y + area.h - area.n(up);
            place(&win, &area, x, y, PREVIEW_W, PREVIEW_H);
        }
    }
}

/// Opens a floating preview for a shot; `stack_previews` positions it.
pub fn show_preview(app: &AppHandle, id: u32, b: &Bounds, secs: u32) -> tauri::Result<()> {
    let area = work_area(app, b);
    let x = area.x + area.w - area.n(PREVIEW_W + MARGIN);
    let y = area.y + area.h - area.n(PREVIEW_H + MARGIN);
    let url = format!("preview.html?id={id}&t={secs}");
    let win = WebviewWindowBuilder::new(app, preview_label(id), WebviewUrl::App(url.into()))
        .title("KlikSnap")
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .accept_first_mouse(true)
        .visible_on_all_workspaces(true)
        .inner_size(PREVIEW_W, PREVIEW_H)
        .visible(false)
        .build()?;
    place(&win, &area, x, y, PREVIEW_W, PREVIEW_H);
    track_hover(win);
    Ok(())
}

/// Reports whether the cursor is over the window as `preview:hover` events.
/// The preview never takes focus, and an unfocused webview (always so on
/// macOS) gets no mouse-move events, so CSS `:hover` alone never fires.
fn track_hover(win: WebviewWindow) {
    std::thread::spawn(move || {
        let mut inside = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let Some(now) = cursor_inside(&win) else {
                return; // window destroyed
            };
            if now != inside {
                inside = now;
                let _ = win.emit_to(win.label(), "preview:hover", inside);
            }
        }
    });
}

/// `None` once the window is gone.
fn cursor_inside(win: &WebviewWindow) -> Option<bool> {
    let pos = win.outer_position().ok()?;
    let size = win.outer_size().ok()?;
    let visible = win.is_visible().ok()?;
    let Ok(cursor) = win.cursor_position() else {
        return Some(false);
    };
    let (cx, cy, x, y, w, h) = if cfg!(target_os = "macos") {
        // The cursor is scaled by the primary monitor, the window by its own.
        let primary = win
            .primary_monitor()
            .ok()
            .flatten()
            .map_or(1.0, |m| m.scale_factor());
        let s = win.scale_factor().ok()?;
        (
            cursor.x / primary,
            cursor.y / primary,
            pos.x as f64 / s,
            pos.y as f64 / s,
            size.width as f64 / s,
            size.height as f64 / s,
        )
    } else {
        (
            cursor.x,
            cursor.y,
            pos.x as f64,
            pos.y as f64,
            size.width as f64,
            size.height as f64,
        )
    };
    Some(visible && cx >= x && cx < x + w && cy >= y && cy < y + h)
}

const TOAST_W: f64 = 300.0;
const TOAST_H: f64 = 76.0;

/// Shows the OCR result notice in the bottom-right corner, replacing any open one.
pub fn show_toast(app: &AppHandle, b: &Bounds) -> tauri::Result<()> {
    if let Some(old) = app.get_webview_window("toast") {
        old.destroy()?;
    }
    let area = work_area(app, b);
    let win = WebviewWindowBuilder::new(app, "toast", WebviewUrl::App("toast.html".into()))
        .title("KlikSnap")
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible_on_all_workspaces(true)
        .inner_size(TOAST_W, TOAST_H)
        .visible(false)
        .build()?;
    let x = area.x + area.w - area.n(TOAST_W + MARGIN);
    let y = area.y + area.h - area.n(TOAST_H + MARGIN);
    place(&win, &area, x, y, TOAST_W, TOAST_H);
    Ok(())
}

/// Opens an editor for a shot. Returns the window label.
pub fn open_editor(
    app: &AppHandle,
    id: u32,
    b: &Bounds,
    img_w: u32,
    img_h: u32,
) -> tauri::Result<String> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let label = format!(
        "editor-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let area = work_area(app, b);
    let (max_w, max_h) = area.logical_size();
    const TOOLBAR: f64 = 52.0;
    // Wide enough for the toolbar on one row, never larger than the screen.
    let w = (img_w as f64 / b.scale + 48.0).max(920.0).min(max_w * 0.9);
    let h = (img_h as f64 / b.scale + TOOLBAR + 48.0)
        .max(400.0)
        .min(max_h * 0.9);
    let url = format!("editor.html?id={id}");
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("KlikSnap")
        .inner_size(w, h)
        .min_inner_size(640.0, 360.0)
        .visible(false)
        .build()?;
    let x = area.x + (area.w - area.n(w)) / 2.0;
    let y = area.y + (area.h - area.n(h)) / 2.0;
    place(&win, &area, x, y, w, h);
    Ok(label)
}

pub fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window("settings") {
        win.show()?;
        return win.set_focus();
    }
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("KlikSnap Settings")
        .inner_size(460.0, 720.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .center()
        .visible(false)
        .build()?;
    Ok(())
}
