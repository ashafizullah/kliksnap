//! Self-update from GitHub Releases. The release workflow publishes a signed
//! `latest.json`; the updater only installs packages signed with our key.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::i18n::tr;
use crate::AppState;

const STARTUP_DELAY: Duration = Duration::from_secs(10);
const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

static CHECKING: AtomicBool = AtomicBool::new(false);

/// Checks shortly after launch and then once a day, if enabled in settings.
pub fn start_background_checks(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(STARTUP_DELAY);
        loop {
            if app.state::<AppState>().settings().check_updates {
                tauri::async_runtime::block_on(check(app.clone(), false));
            }
            std::thread::sleep(INTERVAL);
        }
    });
}

/// The changelog part of the release notes, as markdown: everything before
/// "## Download", whose install instructions are for the release page.
fn changelog(notes: &str) -> String {
    notes
        .lines()
        .take_while(|l| !l.trim_start().starts_with("## Download"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// The update found by the last check, waiting in the update window.
static PENDING: Mutex<Option<Update>> = Mutex::new(None);

#[derive(Clone, serde::Serialize)]
pub struct Info {
    version: String,
    current: String,
    /// Markdown.
    notes: String,
    url: String,
}

/// What the update window shows.
pub fn info() -> Option<Info> {
    let update = PENDING.lock().unwrap().clone()?;
    Some(Info {
        notes: changelog(update.body.as_deref().unwrap_or("")),
        url: release_url(&update.version),
        version: update.version,
        current: update.current_version,
    })
}

/// Opens the pending update's release page in the browser.
pub fn open_notes() -> Result<(), String> {
    let version = PENDING.lock().unwrap().as_ref().map(|u| u.version.clone());
    let url = release_url(&version.ok_or("no update")?);
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn release_url(version: &str) -> String {
    format!("https://github.com/ashafizullah/kliksnap/releases/tag/v{version}")
}

#[derive(Clone, serde::Serialize)]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

/// Downloads and installs the pending update, reporting progress to the
/// update window, then restarts. Returns only on failure.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = PENDING.lock().unwrap().clone().ok_or("no update")?;
    let mut downloaded = 0u64;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ =
                    progress.emit_to("update", "update:progress", Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| format!("{} {e}", tr("The update could not be installed.")))?;
    app.restart()
}

/// A message box, attached to `parent` when given so it stays with that
/// window instead of sending KlikSnap (a menu bar app) to the background.
fn notify(
    app: &AppHandle,
    kind: MessageDialogKind,
    message: String,
    parent: Option<&WebviewWindow>,
) {
    let mut dialog = app.dialog().message(message).title("KlikSnap").kind(kind);
    if let Some(parent) = parent {
        dialog = dialog.parent(parent);
    }
    dialog.blocking_show();
}

/// What a check found.
pub enum Outcome {
    UpToDate,
    /// An update was found and offered in a dialog.
    Offered,
    /// Another check is still running.
    Busy,
}

/// Checks for an update and offers it in the update window.
pub async fn check_and_offer(app: &AppHandle) -> Result<Outcome, String> {
    if CHECKING.swap(true, Ordering::SeqCst) {
        return Ok(Outcome::Busy);
    }
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let outcome = match result {
        Ok(Some(update)) => {
            offer(app, update).await;
            Ok(Outcome::Offered)
        }
        Ok(None) => Ok(Outcome::UpToDate),
        Err(e) => Err(e.to_string()),
    };
    CHECKING.store(false, Ordering::SeqCst);
    outcome
}

/// Shows the update in its own window: release notes of any length, and
/// the download's progress once the user installs it.
async fn offer(app: &AppHandle, update: Update) {
    *PENDING.lock().unwrap() = Some(update);
    if let Err(e) = crate::ui::open_update(app) {
        eprintln!("update window: {e}");
    }
}

/// `manual` checks (from the tray) also report "up to date" and errors;
/// background checks stay quiet.
pub async fn check(app: AppHandle, manual: bool) {
    match check_and_offer(&app).await {
        Ok(Outcome::UpToDate) if manual => notify(
            &app,
            MessageDialogKind::Info,
            tr("You're up to date. KlikSnap {version} is the latest version.")
                .replace("{version}", &app.package_info().version.to_string()),
            None,
        ),
        Err(e) if manual => notify(
            &app,
            MessageDialogKind::Error,
            format!("{}\n\n{e}", tr("Couldn't check for updates.")),
            None,
        ),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::changelog;

    #[test]
    fn keeps_the_changelog_before_the_download_section() {
        let notes =
            "\n### New\n\n- **QR** scanning\n\n## Download\n\n**macOS**: download `KlikSnap.dmg`";
        assert_eq!(changelog(notes), "### New\n\n- **QR** scanning");
    }

    #[test]
    fn is_empty_without_a_changelog() {
        assert_eq!(changelog("## Download\n\n**Windows**: download it"), "");
    }
}
