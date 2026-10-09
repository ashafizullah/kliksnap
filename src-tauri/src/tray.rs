use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::capture::Mode;
use crate::i18n::tr;
use crate::AppState;

const TRAY_ID: &str = "kliksnap";

fn item(
    app: &AppHandle,
    id: &str,
    text: &str,
    accelerator: &str,
) -> tauri::Result<MenuItem<tauri::Wry>> {
    let accelerator = Some(accelerator).filter(|a| !a.trim().is_empty());
    // An accelerator the menu can't display shouldn't take the whole item down.
    MenuItem::with_id(app, id, text, true, accelerator)
        .or_else(|_| MenuItem::with_id(app, id, text, true, None::<&str>))
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let s = state.settings();
    if state.recording.lock().unwrap().is_some() {
        return Menu::with_items(
            app,
            &[
                &item(app, "stop_record", tr("Stop Recording"), &s.hotkey_record)?,
                &PredefinedMenuItem::separator(app)?,
                &item(app, "settings", tr("Settings…"), "")?,
                &item(app, "quit", tr("Quit KlikSnap"), "")?,
            ],
        );
    }
    let pins_through = !state.click_through.lock().unwrap().is_empty();
    let menu = Menu::with_items(
        app,
        &[
            &item(app, "area", tr("Capture Area"), &s.hotkey_area)?,
            &item(app, "window", tr("Capture Window"), &s.hotkey_window)?,
            &item(app, "screen", tr("Capture Screen"), &s.hotkey_screen)?,
            &item(app, "text", tr("Copy Text (OCR)"), &s.hotkey_text)?,
            &item(
                app,
                "last_area",
                tr("Capture Last Area"),
                &s.hotkey_last_area,
            )?,
            &item(app, "scroll", tr("Scrolling Capture"), &s.hotkey_scroll)?,
            &delay_menu(app)?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "record", tr("Record Area"), &s.hotkey_record)?,
            &item(app, "record_screen", tr("Record Screen"), "")?,
            &item(app, "record_gif", tr("Record GIF"), "")?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "pin_clipboard", tr("Pin Clipboard Image"), "")?,
            &item(app, "history", tr("History…"), "")?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "settings", tr("Settings…"), "")?,
            &item(app, "update", tr("Check for Updates…"), "")?,
            &item(app, "quit", tr("Quit KlikSnap"), "")?,
        ],
    )?;
    if pins_through {
        // Click-through pins take no clicks, so this is the way to reach them again.
        let after = menu
            .items()?
            .iter()
            .position(|i| i.id() == "pin_clipboard")
            .map_or(0, |i| i + 1);
        menu.insert(
            &item(app, "release_pins", tr("Make Pins Clickable Again"), "")?,
            after,
        )?;
    }
    Ok(menu)
}

const DELAYS: [u32; 3] = [3, 5, 10];

fn delay_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let menu = Submenu::new(app, tr("Capture After Delay"), true)?;
    for (mode, name) in [("area", tr("Area")), ("screen", tr("Screen"))] {
        if mode == "screen" {
            menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
        for secs in DELAYS {
            let text = tr("{name} in {secs} Seconds")
                .replace("{name}", name)
                .replace("{secs}", &secs.to_string());
            menu.append(&item(app, &format!("delay:{mode}:{secs}"), &text, "")?)?;
        }
    }
    Ok(menu)
}

fn on_delay(app: &AppHandle, id: &str) {
    let Some((mode, secs)) = id.strip_prefix("delay:").and_then(|r| r.split_once(':')) else {
        return;
    };
    let mode = if mode == "screen" {
        Mode::Screen
    } else {
        Mode::Area
    };
    if let Ok(secs) = secs.parse() {
        crate::start_capture_after(app, mode, secs);
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    // macOS: a template image, drawn white or black like the other menu bar
    // icons. Windows: the app icon, which shows on light and dark taskbars.
    let icon = if cfg!(target_os = "macos") {
        tauri::include_image!("icons/tray.png")
    } else {
        tauri::include_image!("icons/64x64.png")
    };
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("KlikSnap")
        .menu(&menu(app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "area" => crate::start_capture(app, Mode::Area),
            "window" => crate::start_capture(app, Mode::Window),
            "screen" => crate::start_capture(app, Mode::Screen),
            "text" => crate::start_capture(app, Mode::Text),
            "last_area" => crate::start_capture(app, Mode::LastArea),
            "scroll" => crate::start_capture(app, Mode::Scroll),
            "record" => crate::start_capture(app, Mode::Record),
            "record_screen" => crate::start_capture(app, Mode::RecordScreen),
            "record_gif" => crate::start_capture(app, Mode::RecordGif),
            "stop_record" => crate::stop_recording(app),
            "pin_clipboard" => crate::pin_clipboard(app),
            "history" => {
                let _ = crate::ui::open_history(app);
            }
            "release_pins" => crate::release_click_through(app),
            "settings" => {
                let _ = crate::ui::open_settings(app);
            }
            "update" => {
                tauri::async_runtime::spawn(crate::updater::check(app.clone(), true));
            }
            "quit" => app.exit(0),
            id => on_delay(app, id),
        })
        .build(app)?;
    Ok(())
}

/// Shows or hides the tray icon. Only Windows can hide it: the macOS menu
/// bar icon is the only way into the app there.
pub fn set_visible(app: &AppHandle, visible: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(visible || !cfg!(target_os = "windows"));
    }
}

/// Rebuilds the menu so it shows the current hotkeys.
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}
