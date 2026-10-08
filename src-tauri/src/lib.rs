mod capture;
mod commands;
mod hotkeys;
mod ocr;
mod output;
mod platform;
mod settings;
mod tray;
mod ui;
mod updater;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::http::{header, Response};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
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
    preview_shot: Mutex<Option<u32>>,
    editor_shots: Mutex<HashMap<String, u32>>,
    busy: AtomicBool,
    settings: Mutex<settings::Settings>,
    hotkeys: Mutex<Vec<(Shortcut, Mode)>>,
    toast: Mutex<Toast>,
}

/// The OCR result notice. An empty `text` means nothing was copied.
#[derive(Clone, Default, serde::Serialize)]
pub struct Toast {
    title: String,
    text: String,
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
            preview_shot: Mutex::default(),
            editor_shots: Mutex::default(),
            busy: AtomicBool::new(false),
            settings: Mutex::new(settings),
            hotkeys: Mutex::default(),
            toast: Mutex::default(),
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

    fn retain(&self, id: u32) {
        if let Some(s) = self.shots.lock().unwrap().get_mut(&id) {
            s.refs += 1;
        }
    }

    /// Drops the shot's pixels once neither the preview nor an editor shows it.
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
    if state.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    if !platform::has_screen_permission() {
        state.busy.store(false, Ordering::SeqCst);
        platform::request_screen_permission();
        return;
    }
    // Keep the previous preview out of the new screenshot.
    let had_preview = match app.get_webview_window("preview") {
        Some(win) => win.destroy().is_ok(),
        None => false,
    };
    let app = app.clone();
    std::thread::spawn(move || {
        if had_preview {
            std::thread::sleep(Duration::from_millis(150));
        }
        if let Err(e) = run_capture(&app, mode) {
            eprintln!("capture failed: {e}");
            app.state::<AppState>().busy.store(false, Ordering::SeqCst);
        }
    });
}

fn run_capture(app: &AppHandle, mode: Mode) -> Result<(), String> {
    let state = app.state::<AppState>();
    if mode == Mode::Screen {
        let frozen = capture::freeze_at(platform::cursor_pos())?;
        state.busy.store(false, Ordering::SeqCst);
        return finish_shot(app, frozen.img, frozen.bounds);
    }
    let frozen = capture::freeze_all()?;
    let bounds: Vec<Bounds> = frozen.iter().map(|f| f.bounds).collect();
    let generation = state.frozen_gen.fetch_add(1, Ordering::SeqCst) + 1;
    *state.frozen.lock().unwrap() = frozen;
    *state.window_rects.lock().unwrap() = None;
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
    state.busy.store(false, Ordering::SeqCst);
    let Some((index, rect)) = selection else {
        return;
    };
    let Some(f) = frozen.into_iter().nth(index) else {
        return;
    };
    let mode = *state.mode.lock().unwrap();
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(img) = capture::crop(&f.img, rect) else {
            return;
        };
        let result = if mode == Mode::Text {
            finish_text(&app, &img, &f.bounds)
        } else {
            finish_shot(&app, img, f.bounds)
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
            text: String::new(),
        },
        Ok(text) => {
            arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(text.clone()))
                .map_err(|e| e.to_string())?;
            Toast {
                title: title.into(),
                text,
            }
        }
        Err(e) => Toast {
            title: format!("Text recognition failed: {e}"),
            text: String::new(),
        },
    };
    *app.state::<AppState>().toast.lock().unwrap() = toast;
    ui::show_toast(app, bounds).map_err(|e| e.to_string())
}

fn finish_shot(app: &AppHandle, img: RgbaImage, bounds: Bounds) -> Result<(), String> {
    let state = app.state::<AppState>();
    let s = state.settings();
    let img = Arc::new(img);
    let id = state.next_shot.fetch_add(1, Ordering::SeqCst);
    state.shots.lock().unwrap().insert(
        id,
        Shot {
            img: img.clone(),
            bounds,
            refs: 1,
        },
    );
    if let Some(old) = state.preview_shot.lock().unwrap().replace(id) {
        state.release(old);
    }
    ui::show_preview(app, id, &bounds, s.preview_secs).map_err(|e| e.to_string())?;
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
            Some(output::bmp(&frozen.get(n as usize)?.img))
        }
        "shot" => Some(output::bmp(&state.shot(n)?.0)),
        _ => None,
    }
}

fn on_window_destroyed(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();
    if label == "preview" {
        if let Some(id) = state.preview_shot.lock().unwrap().take() {
            state.release(id);
        }
    } else if label.starts_with("editor-") {
        if let Some(id) = state.editor_shots.lock().unwrap().remove(label) {
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
            commands::shot_info,
            commands::copy_shot,
            commands::save_shot,
            commands::edit_shot,
            commands::export_image,
            commands::get_settings,
            commands::save_settings,
            commands::pick_folder,
            commands::capture,
            commands::toast_text,
            commands::check_updates,
            commands::app_version,
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
