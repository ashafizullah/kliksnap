use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use xcap::image::RgbaImage;

use crate::capture::{self, Mode};
use crate::settings::Settings;
use crate::{hotkeys, output, platform, tray, ui, AppState};

/// Called by every page once it has painted, so windows never flash empty.
#[tauri::command]
pub fn window_ready(window: WebviewWindow, state: State<AppState>) {
    let label = window.label();
    if let Some(index) = label.strip_prefix("overlay-") {
        platform::raise_overlay(&window);
        let _ = window.show();
        let index: usize = index.parse().unwrap_or(0);
        let frozen = state.frozen.lock().unwrap();
        let under_cursor = frozen
            .get(index)
            .is_some_and(|f| f.bounds.contains(platform::cursor_pos()));
        if under_cursor || frozen.len() == 1 {
            let _ = window.set_focus();
            let mode = *state.mode.lock().unwrap();
            if mode != Mode::Window {
                let _ = window.run_on_main_thread(platform::activate_with_crosshair);
            }
            if mode == Mode::Text {
                // The OS loads its OCR model on first use, which can take many
                // seconds; start that now while the user is still selecting.
                // Not earlier: it competes with capturing and painting the overlay.
                std::thread::spawn(|| crate::ocr::recognize(&RgbaImage::new(64, 32)));
            }
        }
    } else if label.starts_with("preview-") || label == "toast" {
        platform::show_inactive(&window);
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub async fn check_updates(app: AppHandle) {
    crate::updater::check(app, true).await;
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn toast_text(state: State<AppState>) -> crate::Toast {
    state.toast.lock().unwrap().clone()
}

#[tauri::command]
pub fn close_window(window: WebviewWindow) {
    let _ = window.destroy();
}

#[tauri::command]
pub fn capture(app: AppHandle, mode: Mode) {
    crate::start_capture(&app, mode);
}

#[derive(Serialize)]
pub struct OverlayInfo {
    mode: Mode,
    width: u32,
    height: u32,
}

#[tauri::command]
pub fn overlay_info(index: usize, state: State<AppState>) -> Option<OverlayInfo> {
    let frozen = state.frozen.lock().unwrap();
    let f = frozen.get(index)?;
    Some(OverlayInfo {
        mode: *state.mode.lock().unwrap(),
        width: f.img.width(),
        height: f.img.height(),
    })
}

/// Window rectangles on monitor `index`, front to back, as fractions of the monitor.
#[tauri::command]
pub async fn overlay_windows(
    index: usize,
    state: State<'_, AppState>,
) -> Result<Vec<[f64; 4]>, ()> {
    let Some(bounds) = state.frozen.lock().unwrap().get(index).map(|f| f.bounds) else {
        return Ok(Vec::new());
    };
    let mut cached = state.window_rects.lock().unwrap();
    let rects = cached.get_or_insert_with(capture::window_rects);
    Ok(capture::relative_rects(rects, &bounds))
}

#[tauri::command]
pub fn overlay_finish(app: AppHandle, index: usize, rect: Option<[f64; 4]>) {
    crate::end_selection(&app, rect.map(|r| (index, r)));
}

#[derive(Serialize)]
pub struct ShotInfo {
    width: u32,
    height: u32,
    scale: f64,
}

#[tauri::command]
pub fn shot_info(id: u32, state: State<AppState>) -> Option<ShotInfo> {
    let (img, bounds) = state.shot(id)?;
    Some(ShotInfo {
        width: img.width(),
        height: img.height(),
        scale: bounds.scale,
    })
}

fn shot_image(state: &AppState, id: u32) -> Result<std::sync::Arc<RgbaImage>, String> {
    state
        .shot(id)
        .map(|s| s.0)
        .ok_or_else(|| "screenshot expired".into())
}

#[tauri::command]
pub async fn copy_shot(id: u32, state: State<'_, AppState>) -> Result<(), String> {
    let img = shot_image(&state, id)?;
    output::copy(&img)
}

#[tauri::command]
pub async fn save_shot(
    app: AppHandle,
    id: u32,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let img = shot_image(&state, id)?;
    let dir = state.settings().save_dir(&app);
    let path = output::unique_path(&dir, &output::file_name());
    output::save_png(&img, &path)?;
    Ok(path.display().to_string())
}

/// Async because creating a window from a synchronous command deadlocks on
/// Windows (the command runs on the main thread the window needs).
#[tauri::command]
pub async fn edit_shot(app: AppHandle, id: u32, state: State<'_, AppState>) -> Result<(), String> {
    let (img, bounds) = state.shot(id).ok_or("screenshot expired")?;
    let label =
        ui::open_editor(&app, id, &bounds, img.width(), img.height()).map_err(|e| e.to_string())?;
    state.retain(id);
    state.editor_shots.lock().unwrap().insert(label, id);
    if let Some(preview) = app.get_webview_window(&ui::preview_label(id)) {
        let _ = preview.destroy();
    }
    Ok(())
}

#[derive(Deserialize)]
struct ExportMeta {
    action: String,
    width: u32,
    height: u32,
}

/// Receives the editor's rendered RGBA pixels as a raw body; metadata rides in
/// the `x-ks` header so the pixels never pass through JSON.
#[tauri::command]
pub async fn export_image(app: AppHandle, request: Request<'_>) -> Result<Option<String>, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw image data".into());
    };
    let meta = request
        .headers()
        .get("x-ks")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing export metadata")?;
    let meta: ExportMeta = serde_json::from_str(meta).map_err(|e| e.to_string())?;
    let img =
        RgbaImage::from_raw(meta.width, meta.height, bytes.clone()).ok_or("image size mismatch")?;
    let state = app.state::<AppState>();
    let dir = state.settings().save_dir(&app);
    let path = match meta.action.as_str() {
        "copy" => return output::copy(&img).map(|_| None),
        "save" => output::unique_path(&dir, &output::file_name()),
        "savecopy" => {
            output::copy(&img)?;
            output::unique_path(&dir, &output::file_name())
        }
        "saveas" => {
            let Some(picked) = app
                .dialog()
                .file()
                .set_directory(&dir)
                .set_file_name(output::file_name())
                .add_filter("PNG image", &["png"])
                .blocking_save_file()
            else {
                return Ok(None);
            };
            let mut path = picked.into_path().map_err(|e| e.to_string())?;
            if path.extension().is_none() {
                path.set_extension("png");
            }
            path
        }
        other => return Err(format!("unknown action {other}")),
    };
    output::save_png(&img, &path)?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<AppState>) -> Settings {
    let mut s = state.settings();
    s.save_dir = s.save_dir(&app).display().to_string();
    s
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    settings: Settings,
    state: State<AppState>,
) -> Result<(), String> {
    let old = state.settings();
    if let Err(e) = hotkeys::register(&app, &settings) {
        let _ = hotkeys::register(&app, &old);
        return Err(e);
    }
    {
        use tauri_plugin_autostart::ManagerExt;
        let autostart = app.autolaunch();
        let enabled = autostart.is_enabled().unwrap_or(false);
        if settings.launch_at_login != enabled {
            let r = if settings.launch_at_login {
                autostart.enable()
            } else {
                autostart.disable()
            };
            r.map_err(|e| e.to_string())?;
        }
    }
    crate::settings::store(&app, &settings)?;
    *state.settings.lock().unwrap() = settings;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Option<String> {
    let picked: PathBuf = app
        .dialog()
        .file()
        .blocking_pick_folder()?
        .into_path()
        .ok()?;
    Some(picked.display().to_string())
}
