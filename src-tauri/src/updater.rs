//! Self-update from GitHub Releases. The release workflow publishes a signed
//! `latest.json`; the updater only installs packages signed with our key.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

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

fn notify(app: &AppHandle, kind: MessageDialogKind, message: String) {
    app.dialog()
        .message(message)
        .title("KlikSnap")
        .kind(kind)
        .blocking_show();
}

/// `manual` checks also report "up to date" and errors; background checks stay quiet.
pub async fn check(app: AppHandle, manual: bool) {
    if CHECKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    match result {
        Ok(Some(update)) => {
            let notes = update.body.as_deref().unwrap_or("").trim();
            let notes = if notes.chars().count() > 600 {
                format!("{}…", notes.chars().take(600).collect::<String>())
            } else {
                notes.to_string()
            };
            let message = format!(
                "KlikSnap {} is available. You have {}.\n\n{notes}",
                update.version, update.current_version
            );
            let install = app
                .dialog()
                .message(message.trim_end())
                .title("Update available")
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Install and Restart".into(),
                    "Later".into(),
                ))
                .blocking_show();
            if install {
                match update.download_and_install(|_, _| {}, || {}).await {
                    Ok(()) => app.restart(),
                    Err(e) => notify(
                        &app,
                        MessageDialogKind::Error,
                        format!("The update could not be installed.\n\n{e}"),
                    ),
                }
            }
        }
        Ok(None) if manual => notify(
            &app,
            MessageDialogKind::Info,
            format!(
                "You're up to date. KlikSnap {} is the latest version.",
                app.package_info().version
            ),
        ),
        Err(e) if manual => notify(
            &app,
            MessageDialogKind::Error,
            format!("Couldn't check for updates.\n\n{e}"),
        ),
        _ => {}
    }
    CHECKING.store(false, Ordering::SeqCst);
}
