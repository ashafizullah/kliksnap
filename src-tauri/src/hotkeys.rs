use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::capture::Mode;
use crate::settings::Settings;
use crate::AppState;

pub fn plugin() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            let mode = state
                .hotkeys
                .lock()
                .unwrap()
                .iter()
                .find(|(s, _)| s == shortcut)
                .map(|(_, m)| *m);
            if let Some(mode) = mode {
                crate::start_capture(app, mode);
            }
        })
        .build()
}

pub fn bindings(s: &Settings) -> [(&str, Mode); 4] {
    [
        (s.hotkey_area.as_str(), Mode::Area),
        (s.hotkey_window.as_str(), Mode::Window),
        (s.hotkey_screen.as_str(), Mode::Screen),
        (s.hotkey_text.as_str(), Mode::Text),
    ]
}

pub fn register(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;
    let mut registered = Vec::new();
    for (accelerator, mode) in bindings(s) {
        if accelerator.trim().is_empty() {
            continue;
        }
        let shortcut: Shortcut = accelerator
            .parse()
            .map_err(|e| format!("{accelerator}: {e}"))?;
        manager
            .register(shortcut)
            .map_err(|e| format!("{accelerator}: {e}"))?;
        registered.push((shortcut, mode));
    }
    *app.state::<AppState>().hotkeys.lock().unwrap() = registered;
    Ok(())
}
