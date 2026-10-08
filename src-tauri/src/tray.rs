use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
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
            &PredefinedMenuItem::separator(app)?,
            &item(app, "settings", "Settings…", "")?,
            &item(app, "update", "Check for Updates…", "")?,
            &item(app, "quit", "Quit KlikSnap", "")?,
        ],
    )
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
            "settings" => {
                let _ = crate::ui::open_settings(app);
            }
            "update" => {
                tauri::async_runtime::spawn(crate::updater::check(app.clone(), true));
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the menu so it shows the current hotkeys.
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}
