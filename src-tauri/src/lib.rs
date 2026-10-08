mod capture;
mod commands;
mod hotkeys;
mod ocr;
mod output;
mod platform;
mod record;
mod settings;
mod tray;
mod ui;
mod updater;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::http::{header, Response};
use tauri::{AppHandle, Manager, RunEvent, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::Shortcut;
use xcap::image::RgbaImage;

use capture::{Bounds, Frozen, Mode, Shot};

pub struct AppState {
    /// Screens captured for the current area/window selection.
    frozen: Mutex<Vec<Frozen>>,
    frozen_gen: AtomicU32,
    mode: Mutex<Mode>,
    /// Window rects for the current selection, computed on first request.
    window_rects: Mutex<Option<Vec<capture::Rect>>>,
    shots: Mutex<HashMap<u32, Shot>>,
    next_shot: AtomicU32,
    /// Shots with an open preview, oldest first.
    previews: Mutex<Vec<u32>>,
    /// Previews hidden to keep them out of the capture in progress.
    hidden_previews: Mutex<Vec<WebviewWindow>>,
    /// Shots shown by an editor or a pin, by window label.
    window_shots: Mutex<HashMap<String, u32>>,
    /// The last area or window selection: its monitor and rect in fractions of it.
    last_area: Mutex<Option<(Bounds, [f64; 4])>>,
    busy: AtomicBool,
    /// Live selection: overlay window numbers by monitor, for the loupe.
    overlay_ids: Mutex<HashMap<usize, u32>>,
    /// Live selection: the overlays let the wheel through to the apps below.
    passing_scroll: AtomicBool,
    settings: Mutex<settings::Settings>,
    hotkeys: Mutex<Vec<(Shortcut, Mode)>>,
    toast: Mutex<Toast>,
    recording: Mutex<Option<Recording>>,
}

struct Recording {
    recorder: record::Recorder,
    path: std::path::PathBuf,
    bounds: Bounds,
}

/// The OCR result notice. An empty `text` means nothing was copied.
#[derive(Clone, Default, serde::Serialize)]
pub struct Toast {
    title: String,
    text: String,
    /// A file the toast shows in Finder or Explorer when clicked.
    path: String,
}

impl AppState {
    fn new(settings: settings::Settings) -> Self {
        Self {
            frozen: Mutex::default(),
            frozen_gen: AtomicU32::new(0),
            mode: Mutex::new(Mode::Area),
            window_rects: Mutex::default(),
            shots: Mutex::default(),
            next_shot: AtomicU32::new(1),
            previews: Mutex::default(),
            hidden_previews: Mutex::default(),
            window_shots: Mutex::default(),
            last_area: Mutex::default(),
            busy: AtomicBool::new(false),
            passing_scroll: AtomicBool::new(false),
            overlay_ids: Mutex::default(),
            settings: Mutex::new(settings),
            hotkeys: Mutex::default(),
            toast: Mutex::default(),
            recording: Mutex::default(),
        }
    }

    fn settings(&self) -> settings::Settings {
        self.settings.lock().unwrap().clone()
    }

    fn shot(&self, id: u32) -> Option<(Arc<RgbaImage>, Bounds)> {
        self.shots
            .lock()
            .unwrap()
            .get(&id)
            .map(|s| (s.img.clone(), s.bounds))
    }

    fn insert_shot(&self, shot: Shot) -> u32 {
        let id = self.next_shot.fetch_add(1, Ordering::SeqCst);
        self.shots.lock().unwrap().insert(id, shot);
        id
    }

    fn retain(&self, id: u32) {
        if let Some(s) = self.shots.lock().unwrap().get_mut(&id) {
            s.refs += 1;
        }
    }

    /// Drops the shot's pixels once neither a preview nor an editor shows it.
    fn release(&self, id: u32) {
        let mut shots = self.shots.lock().unwrap();
        if let Some(s) = shots.get_mut(&id) {
            s.refs = s.refs.saturating_sub(1);
            if s.refs == 0 {
                shots.remove(&id);
            }
        }
    }
}

pub fn start_capture(app: &AppHandle, mode: Mode) {
    let state = app.state::<AppState>();
    // The record shortcut and menu items stop a recording in progress.
    if matches!(mode, Mode::Record | Mode::RecordScreen)
        && state.recording.lock().unwrap().is_some()
    {
        stop_recording(app);
        return;
    }
    if state.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    if !platform::has_screen_permission() {
        state.busy.store(false, Ordering::SeqCst);
        platform::request_screen_permission();
        return;
    }
    // Keep the open previews out of the new screenshot.
    let hidden: Vec<WebviewWindow> = ui::previews(app)
        .into_iter()
        .filter(|w| w.is_visible().unwrap_or(false) && w.hide().is_ok())
        .collect();
    let had_preview = !hidden.is_empty();
    *state.hidden_previews.lock().unwrap() = hidden;
    let app = app.clone();
    std::thread::spawn(move || {
        if had_preview {
            std::thread::sleep(Duration::from_millis(150));
        }
        if let Err(e) = run_capture(&app, mode) {
            eprintln!("capture failed: {e}");
            app.state::<AppState>().busy.store(false, Ordering::SeqCst);
            reveal_previews(&app);
        }
    });
}

fn run_capture(app: &AppHandle, mode: Mode) -> Result<(), String> {
    let state = app.state::<AppState>();
    if mode == Mode::RecordScreen {
        let bounds = capture::bounds_at(platform::cursor_pos())?;
        state.busy.store(false, Ordering::SeqCst);
        reveal_previews(app);
        start_recording(app, bounds, [0.0, 0.0, 1.0, 1.0]);
        return Ok(());
    }
    if mode == Mode::Screen {
        let frozen = capture::freeze_at(platform::cursor_pos())?;
        state.busy.store(false, Ordering::SeqCst);
        let origin = (frozen.bounds.x, frozen.bounds.y);
        return finish_shot(
            app,
            frozen.img.unwrap_or_default(),
            frozen.bounds,
            Some(origin),
        );
    }
    let mode = if mode == Mode::LastArea {
        let last = *state.last_area.lock().unwrap();
        // With nothing to repeat, or its monitor gone, select a new area.
        match last.map(|(b, rect)| (b, rect, capture::capture_monitor(&b))) {
            Some((b, rect, Ok(screen))) => {
                state.busy.store(false, Ordering::SeqCst);
                let img = capture::crop(&screen, rect).ok_or("empty selection")?;
                return finish_shot(app, img, b, Some(capture::origin(&b, rect)));
            }
            _ => Mode::Area,
        }
    } else {
        mode
    };
    let frozen = if state.settings().live_selection {
        capture::monitors()?
    } else {
        capture::freeze_all()?
    };
    let bounds: Vec<Bounds> = frozen.iter().map(|f| f.bounds).collect();
    let generation = state.frozen_gen.fetch_add(1, Ordering::SeqCst) + 1;
    *state.frozen.lock().unwrap() = frozen;
    *state.window_rects.lock().unwrap() = None;
    state.overlay_ids.lock().unwrap().clear();
    *state.mode.lock().unwrap() = mode;
    platform::remember_frontmost();
    for (i, b) in bounds.iter().enumerate() {
        ui::open_overlay(app, i, b, generation).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Ends an area/window selection; `rect` is in fractions of monitor `index`.
fn end_selection(app: &AppHandle, selection: Option<(usize, [f64; 4])>) {
    let state = app.state::<AppState>();
    let frozen = std::mem::take(&mut *state.frozen.lock().unwrap());
    for win in ui::overlays(app) {
        let _ = win.destroy();
    }
    platform::restore_frontmost();
    let f = selection.and_then(|(index, rect)| Some((frozen.into_iter().nth(index)?, rect)));
    let Some((f, rect)) = f else {
        state.busy.store(false, Ordering::SeqCst);
        reveal_previews(app);
        return;
    };
    let mode = *state.mode.lock().unwrap();
    if mode == Mode::Record {
        state.busy.store(false, Ordering::SeqCst);
        reveal_previews(app);
        let app = app.clone();
        std::thread::spawn(move || {
            // Give the overlays time to leave the screen.
            std::thread::sleep(Duration::from_millis(150));
            start_recording(&app, f.bounds, rect);
        });
        return;
    }
    let live = f.img.is_none();
    if !live {
        state.busy.store(false, Ordering::SeqCst);
        reveal_previews(app);
    }
    if mode != Mode::Text {
        *state.last_area.lock().unwrap() = Some((f.bounds, rect));
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let screen = match f.img {
            Some(img) => img,
            None => {
                // Give the overlays time to leave the screen.
                std::thread::sleep(Duration::from_millis(150));
                let shot = capture::capture_monitor(&f.bounds);
                app.state::<AppState>().busy.store(false, Ordering::SeqCst);
                reveal_previews(&app);
                match shot {
                    Ok(img) => img,
                    Err(e) => return eprintln!("capture failed: {e}"),
                }
            }
        };
        let Some(img) = capture::crop(&screen, rect) else {
            return;
        };
        let result = if mode == Mode::Text {
            finish_text(&app, &img, &f.bounds)
        } else {
            finish_shot(&app, img, f.bounds, Some(capture::origin(&f.bounds, rect)))
        };
        if let Err(e) = result {
            eprintln!("{e}");
        }
    });
}

/// Copies the QR codes in the selection, or if there are none, its text.
fn finish_text(app: &AppHandle, img: &RgbaImage, bounds: &Bounds) -> Result<(), String> {
    let codes = ocr::scan_codes(img).unwrap_or_else(|e| {
        eprintln!("QR scan failed: {e}");
        Vec::new()
    });
    let (title, result) = if codes.is_empty() {
        ("Text copied", ocr::recognize(img))
    } else {
        ("QR code copied", Ok(codes.join("\n")))
    };
    let toast = match result {
        Ok(text) if text.trim().is_empty() => Toast {
            title: "No text found".into(),
            ..Default::default()
        },
        Ok(text) => {
            arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(text.clone()))
                .map_err(|e| e.to_string())?;
            Toast {
                title: title.into(),
                text,
                ..Default::default()
            }
        }
        Err(e) => Toast {
            title: format!("Text recognition failed: {e}"),
            ..Default::default()
        },
    };
    show_toast(app, toast, bounds)
}

fn show_toast(app: &AppHandle, toast: Toast, bounds: &Bounds) -> Result<(), String> {
    // A saved file's notice has one more line: where to click to see it.
    let tall = !toast.path.is_empty();
    *app.state::<AppState>().toast.lock().unwrap() = toast;
    ui::show_toast(app, bounds, tall).map_err(|e| e.to_string())
}

/// Starts recording `rect` (fractions of the monitor at `bounds`) into the
/// save folder, with a Stop control in the corner.
fn start_recording(app: &AppHandle, bounds: Bounds, rect: [f64; 4]) {
    let state = app.state::<AppState>();
    let secs = state.settings().record_countdown;
    if secs > 0 {
        let [fx, fy, fw, fh] = rect;
        let center = (
            bounds.x as f64 + (fx + fw / 2.0) * bounds.w as f64,
            bounds.y as f64 + (fy + fh / 2.0) * bounds.h as f64,
        );
        if !countdown(app, &bounds, secs, Some(center)) {
            return;
        }
    }
    let settings = state.settings();
    let dir = settings.save_dir(app);
    let path = output::unique_path(&dir, &output::recording_name());
    let started = std::fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())
        .and_then(|_| record::start(&bounds, rect, settings.record_scale, &path));
    match started {
        Ok(recorder) => {
            *state.recording.lock().unwrap() = Some(Recording {
                recorder,
                path,
                bounds,
            });
            if let Err(e) = ui::show_recording(app, &bounds) {
                eprintln!("recording controls: {e}");
            }
            tray::refresh(app);
        }
        Err(e) => {
            let toast = Toast {
                title: format!("Recording failed: {e}"),
                ..Default::default()
            };
            let _ = show_toast(app, toast, &bounds);
        }
    }
}

/// Stops the recording, finishes the file and says where it went.
pub fn stop_recording(app: &AppHandle) {
    let Some(rec) = app.state::<AppState>().recording.lock().unwrap().take() else {
        return;
    };
    if let Some(win) = app.get_webview_window("recording") {
        let _ = win.destroy();
    }
    tray::refresh(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let toast = match rec.recorder.stop() {
            Ok(()) => Toast {
                title: "Recording saved".into(),
                text: rec
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: rec.path.display().to_string(),
            },
            Err(e) => Toast {
                title: format!("Recording failed: {e}"),
                ..Default::default()
            },
        };
        let _ = show_toast(&app, toast, &rec.bounds);
    });
}

/// Counts down in the corner, then starts the capture; clicking the countdown
/// cancels it. The countdown never takes focus, so menus and hover states
/// opened in the meantime stay open for the capture.
pub fn start_capture_after(app: &AppHandle, mode: Mode, secs: u32) {
    let bounds = match capture::bounds_at(platform::cursor_pos()) {
        Ok(b) => b,
        Err(e) => return eprintln!("timed capture: {e}"),
    };
    let app = app.clone();
    std::thread::spawn(move || {
        if countdown(&app, &bounds, secs, None) {
            start_capture(&app, mode);
        }
    });
}

/// Counts down `secs` on screen, blocking; false when the user cancelled
/// it by clicking it. Call off the main thread.
fn countdown(app: &AppHandle, bounds: &Bounds, secs: u32, center: Option<(f64, f64)>) -> bool {
    let win = match ui::show_countdown(app, bounds, secs, center) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("countdown: {e}");
            return true;
        }
    };
    let end = std::time::Instant::now() + Duration::from_secs(secs.into());
    while std::time::Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
        if app.get_webview_window(win.label()).is_none() {
            return false;
        }
    }
    let _ = win.destroy();
    // Give the countdown time to leave the screen.
    std::thread::sleep(Duration::from_millis(150));
    true
}

/// Shows the previews hidden by `start_capture` again.
fn reveal_previews(app: &AppHandle) {
    let hidden = std::mem::take(&mut *app.state::<AppState>().hidden_previews.lock().unwrap());
    for win in hidden {
        platform::show_inactive(&win);
    }
}

fn finish_shot(
    app: &AppHandle,
    img: RgbaImage,
    bounds: Bounds,
    origin: Option<(i32, i32)>,
) -> Result<(), String> {
    reveal_previews(app);
    let state = app.state::<AppState>();
    let s = state.settings();
    let (img, k) = capture::downscale(img, s.capture_scale);
    // Annotation sizes, the editor and pins go by pixels per point.
    let bounds = Bounds {
        scale: bounds.scale * k,
        ..bounds
    };
    let img = Arc::new(img);
    let id = state.insert_shot(Shot {
        img: img.clone(),
        bounds,
        origin,
        refs: 1,
    });
    let previews = {
        let mut previews = state.previews.lock().unwrap();
        previews.push(id);
        previews.clone()
    };
    ui::show_preview(app, id, &bounds, s.preview_secs).map_err(|e| e.to_string())?;
    // Drop the oldest previews that no longer fit; closing them restacks the rest.
    let fit = ui::previews_that_fit(app, &bounds);
    if previews.len() > fit {
        for old in &previews[..previews.len() - fit] {
            if let Some(win) = app.get_webview_window(&ui::preview_label(*old)) {
                let _ = win.destroy();
            }
        }
    }
    // Not under the lock: placing a window waits on the main thread, which
    // takes the lock in `on_window_destroyed`.
    let previews = state.previews.lock().unwrap().clone();
    ui::stack_previews(app, &previews, &bounds);
    if s.auto_copy {
        output::copy(&img)?;
    }
    if s.auto_save {
        let path = output::unique_path(&s.save_dir(app), &output::file_name());
        output::save_png(&img, &path)?;
    }
    Ok(())
}

/// Serves captures to the webviews as `ks://localhost/<name>`.
fn serve_image(app: &AppHandle, name: &str) -> Option<Vec<u8>> {
    let state = app.state::<AppState>();
    let mut parts = name.split('-');
    let kind = parts.next()?;
    let n: u32 = parts.next()?.parse().ok()?;
    match kind {
        "frozen" => {
            let frozen = state.frozen.lock().unwrap();
            Some(output::bmp(frozen.get(n as usize)?.img.as_ref()?))
        }
        "shot" => Some(output::bmp(&state.shot(n)?.0)),
        _ => None,
    }
}

fn on_window_destroyed(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();
    if let Some(id) = label
        .strip_prefix("preview-")
        .and_then(|id| id.parse().ok())
    {
        let previews = {
            let mut previews = state.previews.lock().unwrap();
            previews.retain(|&p| p != id);
            previews.clone()
        };
        state
            .hidden_previews
            .lock()
            .unwrap()
            .retain(|w| w.label() != label);
        state.release(id);
        // Close the gap, on the monitor of the newest preview.
        if let Some((_, bounds)) = previews.last().and_then(|&newest| state.shot(newest)) {
            ui::stack_previews(app, &previews, &bounds);
        }
    } else if label.starts_with("editor-") || label.starts_with("pin-") {
        if let Some(id) = state.window_shots.lock().unwrap().remove(label) {
            state.release(id);
        }
    } else if label.starts_with("overlay-")
        && ui::overlays(app).iter().all(|w| w.label() == label)
        && !state.frozen.lock().unwrap().is_empty()
    {
        // An overlay was closed some other way (e.g. ⌘W): treat it as a cancel.
        end_selection(app, None);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First, so a second launch exits before setting anything up. It opens
        // Settings instead, the way back in when the tray icon is hidden.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = ui::open_settings(app);
        }))
        .plugin(hotkeys::plugin())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_asynchronous_uri_scheme_protocol("ks", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let name = request.uri().path().trim_start_matches('/').to_string();
            std::thread::spawn(move || {
                let response = match serve_image(&app, &name) {
                    Some(bytes) => Response::builder()
                        .header(header::CONTENT_TYPE, "image/bmp")
                        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                        .header(header::CACHE_CONTROL, "no-store")
                        .body(bytes),
                    None => Response::builder().status(404).body(Vec::new()),
                };
                responder.respond(response.unwrap());
            });
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let (s, first_run) = settings::load(app.handle());
            app.manage(AppState::new(s.clone()));
            tray::create(app.handle())?;
            app.on_menu_event(commands::on_pin_menu);
            tray::set_visible(app.handle(), s.show_tray);
            if let Err(e) = hotkeys::register(app.handle(), &s) {
                eprintln!("hotkeys: {e}");
            }
            updater::start_background_checks(app.handle());
            if first_run {
                let _ = settings::store(app.handle(), &s);
                ui::open_settings(app.handle())?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                on_window_destroyed(window.app_handle(), window.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::window_ready,
            commands::close_window,
            commands::overlay_info,
            commands::overlay_windows,
            commands::overlay_finish,
            commands::overlay_pass_scroll,
            commands::overlay_cursor,
            commands::overlay_loupe,
            commands::shot_info,
            commands::copy_shot,
            commands::save_shot,
            commands::edit_shot,
            commands::pin_shot,
            commands::pin_menu,
            commands::stop_recording,
            commands::reveal_toast_file,
            commands::export_image,
            commands::get_settings,
            commands::save_settings,
            commands::pick_folder,
            commands::capture,
            commands::toast_text,
            commands::check_updates,
            commands::app_version,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building KlikSnap")
        .run(|_, event| {
            // Closing the last window must not quit a tray app.
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
