mod ai;
#[cfg(any(target_os = "windows", test))]
mod audio_mix;
mod capture;
mod commands;
mod gif_writer;
mod history;
mod hotkeys;
mod i18n;
mod ocr;
mod output;
mod platform;
mod record;
mod redact;
mod settings;
mod stitch;
mod tray;
mod ui;
mod updater;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::http::{header, Response};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewWindow, WindowEvent};
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
    /// The scrolling capture in progress.
    scroll: Mutex<Option<ScrollSession>>,
    /// Pins that let clicks through to the windows below, by window label.
    click_through: Mutex<std::collections::HashSet<String>>,
}

struct ScrollSession {
    /// 0 while capturing, then DONE or CANCEL.
    stop: Arc<std::sync::atomic::AtomicU8>,
    /// The controls' window number (macOS), so captures leave them out.
    controls: Arc<AtomicU32>,
}

const SCROLL_DONE: u8 = 1;
const SCROLL_CANCEL: u8 = 2;

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
            click_through: Mutex::default(),
            scroll: Mutex::default(),
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

/// `--capture <mode>` on the command line (`kliksnap --capture area`), so a
/// desktop shortcut can drive KlikSnap where global shortcuts don't work, as
/// on Wayland. Launching it again hands the arguments to the running app.
fn capture_arg(args: &[String]) -> Option<Mode> {
    let i = args.iter().position(|a| a == "--capture")?;
    serde_json::from_value(serde_json::Value::String(args.get(i + 1)?.clone())).ok()
}

pub fn start_capture(app: &AppHandle, mode: Mode) {
    let state = app.state::<AppState>();
    // The record shortcut and menu items stop a recording in progress.
    if matches!(mode, Mode::Record | Mode::RecordScreen | Mode::RecordGif)
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
        start_recording(app, bounds, [0.0, 0.0, 1.0, 1.0], false);
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
/// `text_only`: Copy Text skips QR codes and reads only the text.
fn end_selection(app: &AppHandle, selection: Option<(usize, [f64; 4])>, text_only: bool) {
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
    if mode == Mode::Scroll {
        state.busy.store(false, Ordering::SeqCst);
        reveal_previews(app);
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            if let Err(e) = scroll_capture(&app, f.bounds, rect) {
                let toast = Toast {
                    title: format!("{}: {e}", i18n::tr("Scrolling capture failed")),
                    ..Default::default()
                };
                let _ = show_toast(&app, toast, &f.bounds);
            }
        });
        return;
    }
    if matches!(mode, Mode::Record | Mode::RecordGif) {
        state.busy.store(false, Ordering::SeqCst);
        reveal_previews(app);
        let app = app.clone();
        std::thread::spawn(move || {
            // Give the overlays time to leave the screen.
            std::thread::sleep(Duration::from_millis(150));
            start_recording(&app, f.bounds, rect, mode == Mode::RecordGif);
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
            finish_text(&app, &img, &f.bounds, text_only)
        } else {
            finish_shot(&app, img, f.bounds, Some(capture::origin(&f.bounds, rect)))
        };
        if let Err(e) = result {
            eprintln!("{e}");
        }
    });
}

/// Copies the QR codes in the selection, or if there are none (or
/// `text_only`), its text.
fn finish_text(
    app: &AppHandle,
    img: &RgbaImage,
    bounds: &Bounds,
    text_only: bool,
) -> Result<(), String> {
    let codes = if text_only {
        Vec::new()
    } else {
        ocr::scan_codes(img).unwrap_or_else(|e| {
            eprintln!("QR scan failed: {e}");
            Vec::new()
        })
    };
    let (title, result) = if codes.is_empty() {
        read_text(app, img, bounds)
    } else {
        (i18n::tr("QR code copied").into(), Ok(codes.join("\n")))
    };
    let toast = match result {
        Ok(text) if text.trim().is_empty() => Toast {
            title: i18n::tr("No text found").into(),
            ..Default::default()
        },
        Ok(text) => {
            arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(text.clone()))
                .map_err(|e| e.to_string())?;
            if app.state::<AppState>().settings().capture_sound {
                let _ = app.run_on_main_thread(|| platform::play_sound(platform::Sound::Pop));
            }
            Toast {
                title,
                text,
                ..Default::default()
            }
        }
        Err(e) => Toast {
            title: format!("{}: {e}", i18n::tr("Text recognition failed")),
            ..Default::default()
        },
    };
    show_toast(app, toast, bounds)
}

/// The text in `img`, with the AI profile in use when Settings asks for it and
/// it is set up, else on-device; and the toast's title. A failed AI read falls
/// back to on-device OCR and says so.
fn read_text(
    app: &AppHandle,
    img: &RgbaImage,
    bounds: &Bounds,
) -> (String, Result<String, String>) {
    let s = app.state::<AppState>().settings();
    let ai = s.ai(None).filter(|p| s.ocr_mode == "ai" && p.is_set_up());
    let Some(ai) = ai else {
        return (i18n::tr("Text copied").into(), ocr::recognize(img));
    };
    // The model takes seconds: say it's working.
    let busy = Toast {
        title: i18n::tr("Reading text with AI…").into(),
        ..Default::default()
    };
    let _ = show_toast(app, busy, bounds);
    let read = ai::encode(img).and_then(|png| tauri::async_runtime::block_on(ai.read_text(&png)));
    match read {
        Ok(text) => (i18n::tr("Text copied by AI").into(), Ok(text)),
        Err(e) => (
            format!(
                "{} ({e})",
                i18n::tr("AI failed; text copied with on-device OCR")
            ),
            ocr::recognize(img),
        ),
    }
}

/// Closes the selection and copies the color the user picked from the loupe.
pub(crate) fn pick_color(app: &AppHandle, index: usize, hex: &str) -> Result<(), String> {
    let valid =
        hex.len() == 7 && hex.starts_with('#') && hex[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(format!("not a color: {hex}"));
    }
    let bounds = app
        .state::<AppState>()
        .frozen
        .lock()
        .unwrap()
        .get(index)
        .map(|f| f.bounds);
    end_selection(app, None, false);
    let hex = hex.to_ascii_uppercase();
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(hex.clone()))
        .map_err(|e| e.to_string())?;
    let toast = Toast {
        title: i18n::tr("Color copied").into(),
        text: hex,
        ..Default::default()
    };
    match bounds {
        Some(b) => show_toast(app, toast, &b),
        None => Ok(()),
    }
}

fn show_toast(app: &AppHandle, toast: Toast, bounds: &Bounds) -> Result<(), String> {
    // A saved file's notice has one more line: where to click to see it.
    let tall = !toast.path.is_empty();
    *app.state::<AppState>().toast.lock().unwrap() = toast;
    ui::show_toast(app, bounds, tall).map_err(|e| e.to_string())
}

/// Starts recording `rect` (fractions of the monitor at `bounds`) into the
/// save folder, as a GIF if `gif`, with a Stop control in the corner.
fn start_recording(app: &AppHandle, bounds: Bounds, rect: [f64; 4], gif: bool) {
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
    let ext = if gif { "gif" } else { "mp4" };
    let path = output::unique_path(&dir, &settings.recording_name(ext));
    let started = std::fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())
        .and_then(|_| {
            let audio = record::Audio {
                system: settings.record_system_audio,
                mic: settings.record_mic,
            };
            record::start(&bounds, rect, settings.record_scale, &path, audio)
        });
    match started {
        Ok((recorder, warning)) => {
            *state.recording.lock().unwrap() = Some(Recording {
                recorder,
                path,
                bounds,
            });
            if let Err(e) = ui::show_recording(app, &bounds) {
                eprintln!("recording controls: {e}");
            }
            tray::refresh(app);
            let warning = warning.map(|w| match w {
                record::SoundWarning::MicDenied => (
                    i18n::tr("Recording without the microphone"),
                    i18n::tr(MIC_SETTINGS),
                ),
                record::SoundWarning::NoSound => (
                    i18n::tr("Recording without sound"),
                    i18n::tr("The sound couldn't be captured; the video is still recorded."),
                ),
            });
            if let Some((title, text)) = warning {
                let toast = Toast {
                    title: title.into(),
                    text: text.into(),
                    ..Default::default()
                };
                let _ = show_toast(app, toast, &bounds);
            }
        }
        Err(e) if e == record::DECLINED => {
            let toast = Toast {
                title: i18n::tr("Screen Recording isn't allowed").into(),
                text: i18n::tr(SCREEN_SETTINGS).into(),
                ..Default::default()
            };
            let _ = show_toast(app, toast, &bounds);
            platform::request_screen_permission();
        }
        Err(e) => {
            let toast = Toast {
                title: format!("{}: {e}", i18n::tr("Recording failed")),
                ..Default::default()
            };
            let _ = show_toast(app, toast, &bounds);
        }
    }
}

#[derive(Clone, PartialEq, serde::Serialize)]
struct ScrollProgress {
    height: usize,
    /// The last frame didn't line up: scrolled too fast or upwards.
    lost: bool,
    full: bool,
    /// KlikSnap scrolls; otherwise the user does.
    auto: bool,
    /// macOS: auto-scroll waits for Accessibility, so this one is by hand.
    needs_permission: bool,
}

/// Captures `rect` (fractions of the monitor at `bounds`) as it scrolls,
/// stitching the frames, until the end of the content, Done or Cancel.
/// KlikSnap scrolls when the platform lets it; otherwise the user does.
/// Blocking: call off the main thread.
fn scroll_capture(app: &AppHandle, bounds: Bounds, rect: [f64; 4]) -> Result<(), String> {
    let state = app.state::<AppState>();
    let session = ScrollSession {
        stop: Arc::default(),
        controls: Arc::default(),
    };
    let (stop, controls) = (session.stop.clone(), session.controls.clone());
    *state.scroll.lock().unwrap() = Some(session);
    let result = (|| -> Result<stitch::Stitcher, String> {
        ui::show_scroll(app, &bounds, rect).map_err(|e| e.to_string())?;
        // macOS captures what is below the controls, so it needs their window number.
        let wait = std::time::Instant::now();
        while cfg!(target_os = "macos")
            && controls.load(Ordering::SeqCst) == 0
            && wait.elapsed() < Duration::from_secs(3)
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        let grab = || capture::under_overlay(&bounds, controls.load(Ordering::SeqCst), rect);
        let first = grab().ok_or("couldn't capture the area")?;
        let mut stitcher = stitch::Stitcher::new(&first);
        let auto = platform::can_scroll();
        let mut progress = ScrollProgress {
            height: stitcher.height(),
            lost: false,
            full: false,
            auto,
            needs_permission: !auto && cfg!(target_os = "macos"),
        };
        let _ = app.emit_to("scroll", "scroll:progress", progress.clone());
        let report = |progress: &mut ScrollProgress, step: &stitch::Step, height: usize| {
            let next = ScrollProgress {
                height,
                lost: *step == stitch::Step::Lost,
                full: *step == stitch::Step::Full,
                ..progress.clone()
            };
            if next != *progress {
                *progress = next;
                let _ = app.emit_to("scroll", "scroll:progress", progress.clone());
            }
        };
        if auto {
            let at = (
                bounds.x as f64 + (rect[0] + rect[2] / 2.0) * bounds.w as f64,
                bounds.y as f64 + (rect[1] + rect[3] / 2.0) * bounds.h as f64,
            );
            // macOS scrolls in points, Windows in wheel notches: start small,
            // then size the steps from how far the first one moved.
            let start = if cfg!(target_os = "macos") {
                (rect[3] * bounds.h as f64 * 0.3).max(1.0) as i32
            } else {
                1
            };
            let done = auto_scroll(
                &mut stitcher,
                &stop,
                start,
                |units| platform::scroll_at(at, units),
                || settle(&grab, &stop),
                |step, height| report(&mut progress, step, height),
            );
            if done {
                stop.store(SCROLL_DONE, Ordering::SeqCst);
            }
            // Lost its place: the user scrolls the rest.
            if !done && stop.load(Ordering::SeqCst) == 0 {
                progress.auto = false;
                let _ = app.emit_to("scroll", "scroll:progress", progress.clone());
            }
        }
        while stop.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(40));
            let Some(frame) = grab() else { continue };
            let step = stitcher.push(&frame);
            report(&mut progress, &step, stitcher.height());
        }
        Ok(stitcher)
    })();
    *state.scroll.lock().unwrap() = None;
    if let Some(win) = app.get_webview_window("scroll") {
        let _ = win.destroy();
    }
    let stitcher = result?;
    if stop.load(Ordering::SeqCst) == SCROLL_DONE {
        finish_shot(app, stitcher.finish(), bounds, None)?;
    }
    Ok(())
}

/// The next frame once the content stops moving: smooth scrolling and
/// lazy loading settle first. None when stopped or the capture fails.
fn settle(
    grab: &impl Fn() -> Option<RgbaImage>,
    stop: &std::sync::atomic::AtomicU8,
) -> Option<RgbaImage> {
    let started = std::time::Instant::now();
    let mut last = grab()?;
    while started.elapsed() < Duration::from_millis(1500) {
        std::thread::sleep(Duration::from_millis(70));
        if stop.load(Ordering::SeqCst) != 0 {
            return None;
        }
        let frame = grab()?;
        if frame.as_raw() == last.as_raw() {
            return Some(frame);
        }
        last = frame;
    }
    Some(last)
}

/// Scrolls and stitches until the content stops moving (true: the end was
/// reached), or until stopped, or until it can't keep its place (false: the
/// user takes over). `scroll` takes platform units, positive down; steps aim
/// at half the region, measured from how far each one moved.
fn auto_scroll(
    stitcher: &mut stitch::Stitcher,
    stop: &std::sync::atomic::AtomicU8,
    start: i32,
    scroll: impl Fn(i32),
    next: impl Fn() -> Option<RgbaImage>,
    mut report: impl FnMut(&stitch::Step, usize),
) -> bool {
    let target = stitcher.region_height() as f64 * 0.5;
    let first_height = stitcher.height();
    let (mut units, mut still, mut misses) = (start.max(1), 0, 0);
    // Flipped once if the first steps move nothing: the platform's sign for
    // "down" may differ, and the page may start at its top either way.
    let (mut dir, mut flipped) = (1, false);
    while stop.load(Ordering::SeqCst) == 0 {
        scroll(dir * units);
        let Some(frame) = next() else { return false };
        let step = stitcher.push(&frame);
        report(&step, stitcher.height());
        match step {
            stitch::Step::Added(dy) if dy > 0 => {
                (still, misses) = (0, 0);
                let per_unit = dy as f64 / units as f64;
                units = ((target / per_unit).round() as i32).clamp(1, units.max(1) * 4);
            }
            stitch::Step::Added(_) | stitch::Step::Full => return true,
            // Twice without moving: the end of the content.
            stitch::Step::Same => {
                if !flipped && stitcher.height() == first_height {
                    (dir, flipped) = (-dir, true);
                    continue;
                }
                still += 1;
                if still >= 2 {
                    return true;
                }
            }
            // Too far for the frames to overlap: back up, take smaller steps.
            stitch::Step::Lost => {
                misses += 1;
                scroll(-dir * units);
                if misses > 3 || next().is_none() {
                    return false;
                }
                units = (units / 2).max(1);
            }
        }
    }
    false
}

/// The Done and Cancel buttons of a scrolling capture.
pub fn end_scroll(app: &AppHandle, done: bool) {
    if let Some(s) = app.state::<AppState>().scroll.lock().unwrap().as_ref() {
        let code = if done { SCROLL_DONE } else { SCROLL_CANCEL };
        s.stop.store(code, Ordering::SeqCst);
    }
}

const MIC_SETTINGS: &str = "Allow KlikSnap in System Settings → Privacy & Security → Microphone.";
const SCREEN_SETTINGS: &str =
    "Turn KlikSnap on in Privacy & Security → Screen Recording, or remove it (−) and add it again.";

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
                title: if record::is_gif(&rec.path) {
                    i18n::tr("GIF saved").into()
                } else {
                    i18n::tr("Recording saved").into()
                },
                text: rec
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: rec.path.display().to_string(),
            },
            Err(e) => Toast {
                title: format!("{}: {e}", i18n::tr("Recording failed")),
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
    if s.capture_sound {
        let _ = app.run_on_main_thread(|| platform::play_sound(platform::Sound::Shutter));
    }
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
    if s.auto_save {
        let path = output::unique_path(&s.save_dir(app), &s.file_name());
        output::save(&img, &path, s.format())?;
    }
    if s.history_limit > 0 {
        let (app, img, limit) = (app.clone(), img.clone(), s.history_limit);
        std::thread::spawn(move || {
            if let Err(e) = history::add(&app, &img, bounds.scale, limit) {
                eprintln!("history: {e}");
            }
        });
    }
    if s.auto_copy {
        output::copy(&img)?;
    }
    Ok(())
}

/// Pins the image on the clipboard, centered on the screen under the cursor.
pub fn pin_clipboard(app: &AppHandle) {
    let bounds = match capture::bounds_at(platform::cursor_pos()) {
        Ok(b) => b,
        Err(e) => return eprintln!("pin clipboard: {e}"),
    };
    let img = arboard::Clipboard::new()
        .and_then(|mut c| c.get_image())
        .ok();
    let img = img
        .and_then(|i| RgbaImage::from_raw(i.width as u32, i.height as u32, i.bytes.into_owned()));
    let Some(img) = img else {
        let toast = Toast {
            title: i18n::tr("No image on the clipboard").into(),
            ..Default::default()
        };
        let _ = show_toast(app, toast, &bounds);
        return;
    };
    let id = app.state::<AppState>().insert_shot(Shot {
        img: Arc::new(img),
        bounds,
        origin: None,
        refs: 0,
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = commands::open_pin(&app, id) {
            eprintln!("pin clipboard: {e}");
        }
    });
}

/// Makes every click-through pin take clicks again.
pub fn release_click_through(app: &AppHandle) {
    let labels = std::mem::take(&mut *app.state::<AppState>().click_through.lock().unwrap());
    for label in labels {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.set_ignore_cursor_events(false);
            let _ = win.emit_to(&label, "pin:click-through", false);
        }
    }
    tray::refresh(app);
}

/// Serves captures to the webviews as `ks://localhost/<name>`, with their type.
fn serve_image(app: &AppHandle, name: &str) -> Option<(Vec<u8>, &'static str)> {
    if let Some(id) = name.strip_prefix("hist-") {
        return Some((history::thumbnail(app, id)?, "image/jpeg"));
    }
    let state = app.state::<AppState>();
    let mut parts = name.split('-');
    let kind = parts.next()?;
    let n: u32 = parts.next()?.parse().ok()?;
    match kind {
        "frozen" => {
            let frozen = state.frozen.lock().unwrap();
            Some((
                output::bmp(frozen.get(n as usize)?.img.as_ref()?),
                "image/bmp",
            ))
        }
        "shot" => Some((output::bmp(&state.shot(n)?.0), "image/bmp")),
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
    } else if ["editor-", "pin-", "explain-"]
        .iter()
        .any(|p| label.starts_with(p))
    {
        if let Some(id) = state.window_shots.lock().unwrap().remove(label) {
            state.release(id);
        }
        if state.click_through.lock().unwrap().remove(label) {
            tray::refresh(app);
        }
    } else if label.starts_with("overlay-")
        && ui::overlays(app).iter().all(|w| w.label() == label)
        && !state.frozen.lock().unwrap().is_empty()
    {
        // An overlay was closed some other way (e.g. ⌘W): treat it as a cancel.
        end_selection(app, None, false);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First, so a second launch exits before setting anything up. It opens
        // Settings instead, the way back in when the tray icon is hidden.
        .plugin(tauri_plugin_single_instance::init(
            |app, args, _| match capture_arg(&args) {
                Some(mode) => start_capture(app, mode),
                None => {
                    let _ = ui::open_settings(app);
                }
            },
        ))
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
                    Some((bytes, mime)) => Response::builder()
                        .header(header::CONTENT_TYPE, mime)
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
            i18n::set(&s.language);
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
            if let Some(mode) = capture_arg(&std::env::args().collect::<Vec<_>>()) {
                let handle = app.handle().clone();
                app.handle()
                    .run_on_main_thread(move || start_capture(&handle, mode))?;
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
            commands::overlay_focus,
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
            commands::overlay_pick_color,
            commands::file_name_example,
            commands::find_sensitive,
            commands::history_list,
            commands::ui_language,
            commands::end_scroll,
            commands::open_history,
            commands::history_action,
            commands::history_clear,
            commands::get_settings,
            commands::save_settings,
            commands::pick_folder,
            commands::capture,
            commands::toast_text,
            commands::check_updates,
            commands::test_ai,
            commands::explain_shot,
            commands::ai_chat,
            commands::ai_info,
            commands::open_ai_settings,
            commands::update_info,
            commands::update_install,
            commands::update_notes,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::atomic::AtomicU8;

    /// Auto-scrolls a simulated page whose scroll moves `px_per_unit` rows per
    /// unit, `sign` = -1 when the platform's "down" is negative; returns the
    /// stitched image and whether the end was found.
    fn auto(
        page: &RgbaImage,
        h: u32,
        px_per_unit: f64,
        sign: i32,
        start: i32,
    ) -> (RgbaImage, bool) {
        let max = (page.height() - h) as f64;
        let y = Cell::new(0.0f64);
        let view = || stitch::tests::view(page, y.get() as u32, h, 0, 0);
        let mut stitcher = stitch::Stitcher::new(&view());
        let stop = AtomicU8::new(0);
        let done = auto_scroll(
            &mut stitcher,
            &stop,
            start,
            |units| y.set((y.get() + (sign * units) as f64 * px_per_unit).clamp(0.0, max)),
            || Some(view()),
            |_, _| {},
        );
        (stitcher.finish(), done)
    }

    #[test]
    fn auto_scroll_captures_the_whole_page() {
        let page = stitch::tests::page(64, 2400);
        // macOS-like: pixels, two image rows per point.
        let (img, done) = auto(&page, 300, 2.0, 1, 45);
        assert!(done);
        assert_eq!(img.as_raw(), page.as_raw());
        // Windows-like: one notch moves an odd 37 rows.
        let (img, done) = auto(&page, 300, 37.0, 1, 1);
        assert!(done);
        assert_eq!(img.as_raw(), page.as_raw());
    }

    #[test]
    fn auto_scroll_finds_the_platforms_down() {
        let page = stitch::tests::page(64, 1500);
        let (img, done) = auto(&page, 300, 2.0, -1, 45);
        assert!(done);
        assert_eq!(img.as_raw(), page.as_raw());
    }

    #[test]
    fn auto_scroll_backs_up_after_a_step_too_far() {
        let page = stitch::tests::page(64, 2400);
        // The first step jumps a whole view: lost, then smaller steps.
        let (img, done) = auto(&page, 300, 2.0, 1, 200);
        assert!(done);
        assert_eq!(img.as_raw(), page.as_raw());
    }

    #[test]
    fn capture_arg_reads_the_mode() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            capture_arg(&args(&["kliksnap", "--capture", "area"])),
            Some(Mode::Area)
        );
        assert_eq!(
            capture_arg(&args(&["kliksnap", "--capture", "record_gif"])),
            Some(Mode::RecordGif)
        );
        assert_eq!(capture_arg(&args(&["kliksnap", "--capture"])), None);
        assert_eq!(capture_arg(&args(&["kliksnap", "--capture", "nope"])), None);
        assert_eq!(capture_arg(&args(&["kliksnap"])), None);
    }
}
