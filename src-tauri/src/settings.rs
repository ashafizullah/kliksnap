use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey_area: String,
    pub hotkey_window: String,
    pub hotkey_screen: String,
    pub hotkey_text: String,
    /// Captures the last area or window selection again. Off by default, so
    /// a shortcut another app already holds can't break the others.
    pub hotkey_last_area: String,
    /// Starts recording an area, and stops the recording. Off by default.
    pub hotkey_record: String,
    /// Windows: Print Screen also captures an area, instead of opening the
    /// Snipping Tool.
    pub print_screen: bool,
    /// Empty means the Desktop folder.
    pub save_dir: String,
    pub auto_copy: bool,
    pub auto_save: bool,
    /// Captures are scaled to this percentage of the screen's pixels (100 =
    /// full resolution; 50 on a Retina display gives its point size).
    pub capture_scale: u32,
    /// Seconds counted down before a recording starts; 0 starts at once.
    pub record_countdown: u32,
    /// Recordings are scaled to this percentage of the screen's pixels.
    pub record_scale: u32,
    /// Seconds before the floating preview hides itself; 0 keeps it open.
    pub preview_secs: u32,
    /// Area/window selection over the live screen, captured once the
    /// selection ends, instead of over a still taken when the shortcut fires.
    pub live_selection: bool,
    pub launch_at_login: bool,
    pub check_updates: bool,
    /// Windows: hiding the tray icon keeps KlikSnap running on its shortcuts;
    /// launching it again opens Settings.
    pub show_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey_area: "Alt+Shift+4".into(),
            hotkey_window: "Alt+Shift+5".into(),
            hotkey_screen: "Alt+Shift+3".into(),
            hotkey_text: "Alt+Shift+2".into(),
            hotkey_last_area: String::new(),
            hotkey_record: String::new(),
            print_screen: false,
            save_dir: String::new(),
            auto_copy: true,
            auto_save: false,
            capture_scale: 100,
            record_countdown: 3,
            record_scale: 100,
            preview_secs: 6,
            live_selection: true,
            launch_at_login: false,
            check_updates: true,
            show_tray: true,
        }
    }
}

impl Settings {
    pub fn save_dir(&self, app: &AppHandle) -> PathBuf {
        if !self.save_dir.trim().is_empty() {
            return PathBuf::from(&self.save_dir);
        }
        app.path()
            .desktop_dir()
            .or_else(|_| app.path().home_dir())
            .unwrap_or_default()
    }
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

/// Returns the settings and whether this is the first launch.
pub fn load(app: &AppHandle) -> (Settings, bool) {
    match file(app).and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(json) => (serde_json::from_str(&json).unwrap_or_default(), false),
        None => (Settings::default(), true),
    }
}

pub fn store(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let path = file(app).ok_or("no config directory")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
