//! Self-update from GitHub Releases. The release workflow publishes a signed
//! `latest.json`; the updater only installs packages signed with our key.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
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

/// The changelog part of the release notes as plain text: the dialog can't
/// render markdown, and the install instructions after "## Download" are for
/// people downloading from the release page.
fn changelog(notes: &str) -> String {
    let mut out = Vec::new();
    for line in notes.lines() {
        let line = line.trim_end();
        if line.trim_start().starts_with("## Download") {
            break;
        }
        let line = if let Some(heading) = line.trim_start().strip_prefix('#') {
            heading.trim_start_matches('#').trim().to_string()
        } else if let Some(item) = line.strip_prefix("- ").or(line.strip_prefix("* ")) {
            format!("• {item}")
        } else {
            line.to_string()
        };
        out.push(strip_inline(&line));
    }
    // Collapse runs of blank lines and trim the ends.
    let mut text = String::new();
    for line in out {
        if line.is_empty() && (text.is_empty() || text.ends_with("\n\n")) {
            continue;
        }
        text.push_str(&line);
        text.push('\n');
    }
    text.trim().to_string()
}

/// Drops `**`, `__` and backticks, and turns `[text](url)` into `text`.
fn strip_inline(line: &str) -> String {
    let line = line.replace("**", "").replace("__", "").replace('`', "");
    let mut out = String::new();
    let mut rest = line.as_str();
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|i| open + i) else {
            break;
        };
        let Some(end) = rest[close..].find(')').map(|i| close + i) else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
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

/// Checks for an update and offers it in a dialog attached to `parent`.
pub async fn check_and_offer(
    app: &AppHandle,
    parent: Option<&WebviewWindow>,
) -> Result<Outcome, String> {
    if CHECKING.swap(true, Ordering::SeqCst) {
        return Ok(Outcome::Busy);
    }
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let outcome = match result {
        Ok(Some(update)) => {
            offer(app, update, parent).await;
            Ok(Outcome::Offered)
        }
        Ok(None) => Ok(Outcome::UpToDate),
        Err(e) => Err(e.to_string()),
    };
    CHECKING.store(false, Ordering::SeqCst);
    outcome
}

async fn offer(app: &AppHandle, update: Update, parent: Option<&WebviewWindow>) {
    let notes = changelog(update.body.as_deref().unwrap_or(""));
    let notes = if notes.chars().count() > 900 {
        format!("{}…", notes.chars().take(900).collect::<String>())
    } else {
        notes
    };
    let message = format!(
        "{}\n\n{notes}",
        tr("KlikSnap {new} is available. You have {old}.")
            .replace("{new}", &update.version)
            .replace("{old}", &update.current_version)
    );
    let mut dialog = app
        .dialog()
        .message(message.trim_end())
        .title(tr("Update available"))
        .buttons(MessageDialogButtons::OkCancelCustom(
            tr("Install and Restart").into(),
            tr("Later").into(),
        ));
    if let Some(parent) = parent {
        dialog = dialog.parent(parent);
    }
    if dialog.blocking_show() {
        match update.download_and_install(|_, _| {}, || {}).await {
            Ok(()) => app.restart(),
            Err(e) => notify(
                app,
                MessageDialogKind::Error,
                format!("{}\n\n{e}", tr("The update could not be installed.")),
                parent,
            ),
        }
    }
}

/// `manual` checks (from the tray) also report "up to date" and errors;
/// background checks stay quiet.
pub async fn check(app: AppHandle, manual: bool) {
    match check_and_offer(&app, None).await {
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
    fn keeps_the_changelog_as_plain_text() {
        let notes = "\n### New\n\n- **QR** scanning in `Copy Text` mode\n* See [the docs](https://x.y)\n\n\n### Fixes\n\n- OCR on macOS 27\n\n## Download\n\n**macOS**: download `KlikSnap.dmg`";
        assert_eq!(
            changelog(notes),
            "New\n\n• QR scanning in Copy Text mode\n• See the docs\n\nFixes\n\n• OCR on macOS 27"
        );
    }

    #[test]
    fn is_empty_without_a_changelog() {
        assert_eq!(changelog("## Download\n\n**Windows**: download it"), "");
    }
}
