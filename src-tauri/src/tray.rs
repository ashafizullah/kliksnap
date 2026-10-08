use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::capture::Mode;
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
    let s = app.state::<AppState>().settings();
    Menu::with_items(
        app,
        &[
            &item(app, "area", "Capture Area", &s.hotkey_area)?,
            &item(app, "window", "Capture Window", &s.hotkey_window)?,
            &item(app, "screen", "Capture Screen", &s.hotkey_screen)?,
            &item(app, "text", "Copy Text (OCR)", &s.hotkey_text)?,
            &item(app, "last_area", "Capture Last Area", &s.hotkey_last_area)?,
            &delay_menu(app)?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "settings", "Settings…", "")?,
            &item(app, "update", "Check for Updates…", "")?,
            &item(app, "quit", "Quit KlikSnap", "")?,
        ],
    )
}

const DELAYS: [u32; 3] = [3, 5, 10];

fn delay_menu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let menu = Submenu::new(app, "Capture After Delay", true)?;
    for (mode, name) in [("area", "Area"), ("screen", "Screen")] {
        if mode == "screen" {
            menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
        for secs in DELAYS {
            let text = format!("{name} in {secs} Seconds");
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
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("icons/64x64.png"))
        .tooltip("KlikSnap")
        .menu(&menu(app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "area" => crate::start_capture(app, Mode::Area),
            "window" => crate::start_capture(app, Mode::Window),
            "screen" => crate::start_capture(app, Mode::Screen),
            "text" => crate::start_capture(app, Mode::Text),
            "last_area" => crate::start_capture(app, Mode::LastArea),
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
