use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::output::{self, Format};

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
    /// Starts a scrolling capture. Off by default.
    pub hotkey_scroll: String,
    /// Windows: Print Screen also captures an area, instead of opening the
    /// Snipping Tool.
    pub print_screen: bool,
    /// Empty means the Desktop folder.
    pub save_dir: String,
    pub auto_copy: bool,
    pub auto_save: bool,
    /// A camera shutter sounds when a screenshot is taken, a pop when text
    /// or a QR code is copied.
    pub capture_sound: bool,
    /// Captures are scaled to this percentage of the screen's pixels (100 =
    /// full resolution; 50 on a Retina display gives its point size).
    pub capture_scale: u32,
    /// "png" or "jpg".
    pub image_format: String,
    /// JPEG quality, 1–100.
    pub jpg_quality: u8,
    /// File names for screenshots: strftime fields such as %Y-%m-%d.
    pub file_template: String,
    /// File names for recordings, like `file_template`.
    pub record_template: String,
    /// Seconds counted down before a recording starts; 0 starts at once.
    pub record_countdown: u32,
    /// Recordings are scaled to this percentage of the screen's pixels.
    pub record_scale: u32,
    /// MP4 recordings carry what the computer plays (macOS 13+).
    pub record_system_audio: bool,
    /// MP4 recordings carry the microphone (macOS 15+).
    pub record_mic: bool,
    /// How many recent captures History keeps; 0 keeps none.
    pub history_limit: u32,
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
    /// "auto" (the system's language), "en" or "id".
    pub language: String,
    /// Copy Text reads with "system" (on-device) OCR or the "ai" profile in use.
    pub ocr_mode: String,
    pub ai_profiles: Vec<crate::ai::Profile>,
    /// Index of the profile in use.
    pub ai_profile: usize,
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
            hotkey_scroll: String::new(),
            print_screen: false,
            save_dir: String::new(),
            auto_copy: true,
            auto_save: false,
            capture_sound: true,
            capture_scale: 100,
            image_format: "png".into(),
            jpg_quality: 90,
            file_template: crate::output::DEFAULT_TEMPLATE.into(),
            record_template: crate::output::DEFAULT_RECORDING_TEMPLATE.into(),
            record_countdown: 3,
            record_scale: 100,
            record_system_audio: false,
            record_mic: false,
            preview_secs: 6,
            history_limit: 50,
            live_selection: true,
            launch_at_login: false,
            check_updates: true,
            show_tray: true,
            language: "auto".into(),
            ocr_mode: "system".into(),
            ai_profiles: vec![crate::ai::Profile::openai()],
            ai_profile: 0,
        }
    }
}

impl Settings {
    pub fn format(&self) -> Format {
        match self.image_format.as_str() {
            "jpg" => Format::Jpg(self.jpg_quality),
            _ => Format::Png,
        }
    }

    /// A screenshot's file name, from the template and the chosen format.
    pub fn file_name(&self) -> String {
        output::file_name(&self.file_template, self.format().ext())
    }

    /// A recording's file name; `ext` is "mp4" or "gif".
    pub fn recording_name(&self, ext: &str) -> String {
        output::recording_name(&self.record_template, ext)
    }

    /// Keeps at least one AI profile, with a valid one in use.
    pub fn normalize(mut self) -> Self {
        if self.ai_profiles.is_empty() {
            self.ai_profiles.push(crate::ai::Profile::openai());
        }
        self.ai_profile = self.ai_profile.min(self.ai_profiles.len() - 1);
        self
    }

    /// The AI profile in use, or `index` when given.
    pub fn ai(&self, index: Option<usize>) -> Option<&crate::ai::Profile> {
        self.ai_profiles.get(index.unwrap_or(self.ai_profile))
    }

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
        Some(json) => (
            serde_json::from_str::<Settings>(&json)
                .unwrap_or_default()
                .normalize(),
            false,
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_keeps_a_valid_ai_profile() {
        let s: Settings = serde_json::from_str(r#"{"ai_profiles": [], "ai_profile": 3}"#).unwrap();
        let s = s.normalize();
        assert_eq!(s.ai_profiles.len(), 1);
        assert_eq!(s.ai_profile, 0);
        assert!(s.ai(None).is_some());
    }
}
