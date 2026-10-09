use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use xcap::image::RgbaImage;

use crate::capture::{self, Mode, Shot};
use crate::i18n::tr;
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
        if frozen.get(index).is_some_and(|f| f.img.is_none()) {
            let id = platform::prepare_live_overlay(&window);
            state.overlay_ids.lock().unwrap().insert(index, id);
        }
        let under_cursor = frozen
            .get(index)
            .is_some_and(|f| f.bounds.contains(platform::cursor_pos()));
        if under_cursor || frozen.len() == 1 {
            let _ = window.set_focus();
            let mode = *state.mode.lock().unwrap();
            if mode != Mode::Window {
                let _ = window.run_on_main_thread(platform::activate_with_crosshair);
                keep_crosshair(window.clone());
            }
            if mode == Mode::Text {
                // The OS loads its OCR model on first use, which can take many
                // seconds; start that now while the user is still selecting.
                // Not earlier: it competes with capturing and painting the overlay.
                std::thread::spawn(|| crate::ocr::recognize(&RgbaImage::new(64, 32)));
            }
        }
    } else if label == "scroll" {
        // Keep the controls out of the frames being stitched.
        let id = platform::prepare_live_overlay(&window);
        if let Some(s) = state.scroll.lock().unwrap().as_ref() {
            s.controls.store(id, Ordering::SeqCst);
        }
        platform::show_inactive(&window);
    } else if label == "recording" {
        // Keep the controls out of the recording (Windows; macOS leaves all
        // of KlikSnap's windows out).
        platform::prepare_live_overlay(&window);
        platform::show_inactive(&window);
    } else if label.starts_with("preview-") || label == "toast" || label == "countdown" {
        platform::show_inactive(&window);
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Sets the crosshair again until KlikSnap has become the active app, which
/// macOS does asynchronously, then once more for good measure.
fn keep_crosshair(window: WebviewWindow) {
    std::thread::spawn(move || {
        let mut after_active = 0;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(25));
            if !window.is_visible().unwrap_or(false) {
                return;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let sent = window.run_on_main_thread(move || {
                let _ = tx.send(platform::set_crosshair_if_active());
            });
            if sent.is_err() {
                return;
            }
            if rx.recv().unwrap_or(false) {
                after_active += 1;
                if after_active >= 4 {
                    return;
                }
            }
        }
    });
}

/// The Check Now button in Settings: the result shows in Settings itself,
/// and an update is offered in the update window.
#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Result<String, String> {
    use crate::updater::Outcome;
    match crate::updater::check_and_offer(&app).await {
        Ok(Outcome::UpToDate) => Ok(tr("You're up to date: {version} is the latest version.")
            .replace("{version}", &app.package_info().version.to_string())),
        Ok(Outcome::Offered) => Ok(String::new()),
        Ok(Outcome::Busy) => Ok(tr("Already checking…").into()),
        Err(e) => Err(format!("{} {e}", tr("Couldn't check for updates."))),
    }
}

#[tauri::command]
pub fn update_info() -> Option<crate::updater::Info> {
    crate::updater::info()
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    crate::updater::install(&app).await
}

#[tauri::command]
pub fn update_notes() -> Result<(), String> {
    crate::updater::open_notes()
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn toast_text(state: State<AppState>) -> crate::Toast {
    state.toast.lock().unwrap().clone()
}

/// "en" or "id": the language the windows show.
#[tauri::command]
pub fn ui_language() -> &'static str {
    crate::i18n::code()
}

#[tauri::command]
pub fn end_scroll(app: AppHandle, done: bool) {
    crate::end_scroll(&app, done);
}

#[tauri::command]
pub fn stop_recording(app: AppHandle) {
    crate::stop_recording(&app);
}

/// Shows the file named by the current toast (a saved recording).
#[tauri::command]
pub fn reveal_toast_file(state: State<AppState>) -> Result<(), String> {
    let path = state.toast.lock().unwrap().path.clone();
    if path.is_empty() {
        return Ok(());
    }
    output::reveal(std::path::Path::new(&path))
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
    /// The still's size; 0 in live selection, which has none.
    width: u32,
    height: u32,
}

#[tauri::command]
pub fn overlay_info(index: usize, state: State<AppState>) -> Option<OverlayInfo> {
    let frozen = state.frozen.lock().unwrap();
    let f = frozen.get(index)?;
    Some(OverlayInfo {
        mode: *state.mode.lock().unwrap(),
        width: f.img.as_ref().map_or(0, |i| i.width()),
        height: f.img.as_ref().map_or(0, |i| i.height()),
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

/// The cursor on monitor `index` as fractions of it, or None when it's elsewhere.
#[tauri::command]
pub fn overlay_cursor(index: usize, state: State<AppState>) -> Option<[f64; 2]> {
    let b = state.frozen.lock().unwrap().get(index)?.bounds;
    let p = platform::cursor_pos();
    b.contains(p).then(|| {
        [
            (p.0 - b.x) as f64 / b.w as f64,
            (p.1 - b.y) as f64 / b.h as f64,
        ]
    })
}

/// Live selection: the screen under the loupe, `rect` in fractions of monitor
/// `index`, as width and height (u32 LE) followed by RGBA pixels; 0×0 if
/// it couldn't be captured.
#[tauri::command]
pub async fn overlay_loupe(
    index: usize,
    rect: [f64; 4],
    state: State<'_, AppState>,
) -> Result<Response, ()> {
    let bounds = state.frozen.lock().unwrap().get(index).map(|f| f.bounds);
    let overlay = state.overlay_ids.lock().unwrap().get(&index).copied();
    let img = bounds
        .zip(overlay)
        .and_then(|(b, overlay)| capture::under_overlay(&b, overlay, rect));
    let (w, h) = img.as_ref().map_or((0, 0), |i| i.dimensions());
    let mut out = Vec::with_capacity(8 + (w * h * 4) as usize);
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    if let Some(img) = img {
        out.extend_from_slice(img.as_raw());
    }
    Ok(Response::new(out))
}

/// Live selection: lets the overlays through to the apps below while the
/// user scrolls, so a page can be scrolled into place before selecting.
#[tauri::command]
pub fn overlay_pass_scroll(app: AppHandle, state: State<AppState>) {
    if state.passing_scroll.swap(true, Ordering::SeqCst) {
        return;
    }
    for win in ui::overlays(&app) {
        let _ = win.set_ignore_cursor_events(true);
    }
    std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(30));
            // Without a scroll clock, the overlays catch the next wheel tick and come back here.
            let idle = platform::secs_since_scroll().unwrap_or(start.elapsed().as_secs_f64());
            if idle > 0.2 {
                break;
            }
        }
        for win in ui::overlays(&app) {
            let _ = win.set_ignore_cursor_events(false);
        }
        app.state::<AppState>()
            .passing_scroll
            .store(false, Ordering::SeqCst);
    });
}

#[tauri::command]
pub fn overlay_finish(app: AppHandle, index: usize, rect: Option<[f64; 4]>) {
    crate::end_selection(&app, rect.map(|r| (index, r)));
}

#[tauri::command]
pub fn overlay_pick_color(app: AppHandle, index: usize, hex: String) -> Result<(), String> {
    crate::pick_color(&app, index, &hex)
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
    save_to_folder(&app, &img)
}

/// Saves into the folder from Settings; returns the file's path.
fn save_to_folder(app: &AppHandle, img: &RgbaImage) -> Result<String, String> {
    let s = app.state::<AppState>().settings();
    let path = output::unique_path(&s.save_dir(app), &s.file_name());
    output::save(img, &path, s.format())?;
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
    state.window_shots.lock().unwrap().insert(label, id);
    if let Some(preview) = app.get_webview_window(&ui::preview_label(id)) {
        let _ = preview.destroy();
    }
    Ok(())
}

/// Async for the same reason as `edit_shot`.
#[tauri::command]
pub async fn pin_shot(app: AppHandle, id: u32) -> Result<(), String> {
    open_pin(&app, id)?;
    if let Some(preview) = app.get_webview_window(&ui::preview_label(id)) {
        let _ = preview.destroy();
    }
    Ok(())
}

pub(crate) fn open_pin(app: &AppHandle, id: u32) -> Result<(), String> {
    let state = app.state::<AppState>();
    let origin = state.shots.lock().unwrap().get(&id).and_then(|s| s.origin);
    let (img, bounds) = state.shot(id).ok_or("screenshot expired")?;
    let label = ui::open_pin(app, id, &bounds, origin, img.width(), img.height())
        .map_err(|e| e.to_string())?;
    state.retain(id);
    state.window_shots.lock().unwrap().insert(label, id);
    Ok(())
}

/// Shows a pin's context menu; `on_pin_menu` handles the choice.
#[tauri::command]
pub fn pin_menu(window: WebviewWindow) -> Result<(), String> {
    let label = window.label();
    let e = |e: tauri::Error| e.to_string();
    let item = |action: &str, text: &str| {
        MenuItem::with_id(
            &window,
            format!("pin:{action}:{label}"),
            text,
            true,
            None::<&str>,
        )
    };
    let opacity = Submenu::new(&window, tr("Opacity"), true).map_err(e)?;
    for pct in [100, 80, 60, 40, 20] {
        opacity
            .append(&item(&format!("opacity{pct}"), &format!("{pct}%")).map_err(e)?)
            .map_err(e)?;
    }
    let menu = Menu::with_items(
        &window,
        &[
            &item("copy", tr("Copy")).map_err(e)?,
            &item("save", tr("Save")).map_err(e)?,
            &item("edit", tr("Annotate")).map_err(e)?,
            &PredefinedMenuItem::separator(&window).map_err(e)?,
            &opacity,
            &item("through", tr("Click Through")).map_err(e)?,
            &PredefinedMenuItem::separator(&window).map_err(e)?,
            &item("close", tr("Close")).map_err(e)?,
        ],
    )
    .map_err(e)?;
    window.popup_menu(&menu).map_err(e)
}

pub fn on_pin_menu(app: &AppHandle, event: MenuEvent) {
    let Some((action, label)) = event
        .id()
        .as_ref()
        .strip_prefix("pin:")
        .and_then(|rest| rest.split_once(':'))
    else {
        return;
    };
    let Some(win) = app.get_webview_window(label) else {
        return;
    };
    let state = app.state::<AppState>();
    let id = state.window_shots.lock().unwrap().get(label).copied();
    let img = id.and_then(|id| state.shot(id).map(|s| (id, s.0)));
    let result = match (action, img) {
        ("close", _) => win.destroy().map_err(|e| e.to_string()),
        ("through", _) => {
            state
                .click_through
                .lock()
                .unwrap()
                .insert(label.to_string());
            tray::refresh(app);
            let _ = win.emit_to(label, "pin:click-through", true);
            win.set_ignore_cursor_events(true)
                .map_err(|e| e.to_string())
        }
        (a, _) if a.starts_with("opacity") => {
            let pct: u32 = a["opacity".len()..].parse().unwrap_or(100);
            win.emit_to(label, "pin:opacity", pct as f64 / 100.0)
                .map_err(|e| e.to_string())
        }
        ("copy", Some((_, img))) => output::copy(&img),
        ("save", Some((_, img))) => save_to_folder(app, &img).map(|_| ()),
        ("edit", Some((id, _))) => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                if let Err(e) = edit_shot(app.clone(), id, state).await {
                    eprintln!("pin: {e}");
                }
            });
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("pin {action}: {e}");
    }
}

#[derive(Deserialize)]
struct ExportMeta {
    action: String,
    width: u32,
    height: u32,
    /// Share: the button to open the share menu under, in CSS pixels.
    anchor: Option<[f64; 4]>,
}

/// Receives the editor's rendered RGBA pixels as a raw body; metadata rides in
/// the `x-ks` header so the pixels never pass through JSON.
#[tauri::command]
pub async fn export_image(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Option<String>, String> {
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
    let s = state.settings();
    let dir = s.save_dir(&app);
    let mut format = s.format();
    let path = match meta.action.as_str() {
        "copy" => return output::copy(&img).map(|_| None),
        "share" => {
            let path = share_file(&app)?;
            output::save(&img, &path, format)?;
            let [x, y, w, h] = meta.anchor.unwrap_or_default();
            return platform::share(&window, &path, (x, y, w, h)).map(|_| None);
        }
        "pin" => {
            let edited = state
                .window_shots
                .lock()
                .unwrap()
                .get(window.label())
                .copied();
            let bounds = edited
                .and_then(|id| state.shot(id))
                .ok_or("screenshot expired")?
                .1;
            let id = state.insert_shot(Shot {
                img: std::sync::Arc::new(img),
                bounds,
                origin: None,
                refs: 0,
            });
            return open_pin(&app, id).map(|_| None);
        }
        "save" => output::unique_path(&dir, &s.file_name()),
        "savecopy" => {
            output::copy(&img)?;
            output::unique_path(&dir, &s.file_name())
        }
        "saveas" => {
            let (png, jpg) = ((tr("PNG image"), ["png"]), (tr("JPEG image"), ["jpg"]));
            let filters = if format == output::Format::Png {
                [png, jpg]
            } else {
                [jpg, png]
            };
            let Some(picked) = filters
                .iter()
                .fold(app.dialog().file(), |d, (name, ext)| {
                    d.add_filter(*name, ext)
                })
                .set_directory(&dir)
                .set_file_name(s.file_name())
                .blocking_save_file()
            else {
                return Ok(None);
            };
            let mut path = picked.into_path().map_err(|e| e.to_string())?;
            // The typed extension picks the format; without one, Settings does.
            match output::Format::from_path(&path, s.jpg_quality) {
                Some(f) => format = f,
                None => {
                    path.set_extension(format.ext());
                }
            }
            path
        }
        other => return Err(format!("unknown action {other}")),
    };
    output::save(&img, &path, format)?;
    Ok(Some(path.display().to_string()))
}

/// A fresh path for a file to share. Shared files stay in the cache for an
/// hour, since AirDrop and the like read them after the menu has closed.
fn share_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("share");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(3600));
            if old {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let s = app.state::<AppState>().settings();
    Ok(output::unique_path(&dir, &s.file_name()))
}

/// Async: creating a window from a synchronous command deadlocks on Windows.
#[tauri::command]
pub async fn open_history(app: AppHandle) -> Result<(), String> {
    ui::open_history(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn history_list(app: AppHandle) -> Vec<crate::history::Entry> {
    crate::history::list(&app)
}

/// Copy, Edit, Pin, Save or Delete a capture from History.
#[tauri::command]
pub async fn history_action(app: AppHandle, id: String, action: String) -> Result<(), String> {
    if action == "delete" {
        return crate::history::delete(&app, &id);
    }
    let (entry, img) = crate::history::get(&app, &id).ok_or("capture not found")?;
    match action.as_str() {
        "copy" => output::copy(&img),
        "save" => save_to_folder(&app, &img).map(|_| ()),
        "edit" | "pin" => {
            let bounds = capture::Bounds {
                scale: entry.scale,
                ..capture::bounds_at(platform::cursor_pos())?
            };
            let state = app.state::<AppState>();
            let id = state.insert_shot(Shot {
                img: std::sync::Arc::new(img),
                bounds,
                origin: None,
                refs: 0,
            });
            if action == "pin" {
                open_pin(&app, id)
            } else {
                edit_shot(app.clone(), id, state).await
            }
        }
        other => Err(format!("unknown action {other}")),
    }
}

#[tauri::command]
pub async fn history_clear(app: AppHandle) -> Result<(), String> {
    crate::history::trim(&app, 0)
}

/// Boxes (`x, y, w, h` in image pixels) around the sensitive text in a shot.
#[tauri::command]
pub async fn find_sensitive(id: u32, state: State<'_, AppState>) -> Result<Vec<[f64; 4]>, String> {
    let (img, _) = state.shot(id).ok_or("screenshot expired")?;
    let words = crate::ocr::words(&img)?;
    Ok(crate::redact::find(&words)
        .into_iter()
        .map(|i| words[i].rect)
        .collect())
}

#[tauri::command]
pub fn file_name_example(template: String, format: String) -> String {
    let ext = if format == "jpg" { "jpg" } else { "png" };
    output::file_name(&template, ext)
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
    let print_screen_changed = settings.print_screen != old.print_screen;
    if print_screen_changed {
        platform::set_print_screen_opens_snipping(!settings.print_screen)?;
    }
    if let Err(e) = hotkeys::register(&app, &settings) {
        let _ = hotkeys::register(&app, &old);
        if print_screen_changed {
            let _ = platform::set_print_screen_opens_snipping(!old.print_screen);
        }
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
    if settings.show_tray != old.show_tray {
        tray::set_visible(&app, settings.show_tray);
    }
    if settings.history_limit < old.history_limit {
        crate::history::trim(&app, settings.history_limit)?;
    }
    crate::settings::store(&app, &settings)?;
    crate::i18n::set(&settings.language);
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
