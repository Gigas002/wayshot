//! Desktop notifications via `notify-rust` (D-Bus / libnotify).

use eyre::Error;
use notify_rust::{Hint, Notification, Urgency};
use std::path::Path;
use std::process::Command;

use crate::config::NotificationConfig;
use crate::screenshot::ShotResult;

const DEFAULT_TIMEOUT_MS: i32 = 5000;

/// Where the screenshot went, for the notification's text.
pub(crate) struct Delivery<'a> {
    pub(crate) saved_to: Option<&'a Path>,
    pub(crate) clipboard: bool,
}

pub fn send_success(result: &ShotResult, delivery: Delivery, config: &NotificationConfig) {
    let mut notification = build_base_notification(config, true);
    notification.urgency(Urgency::Normal);
    notification.body(&success_body(result, &delivery));

    if let Some(path) = delivery.saved_to {
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();

        notification.action("open_location", "Open Folder");
        notification.action("default", "Open Folder");

        match unsafe { libc::fork() } {
            0 => {
                if let Ok(handle) = notification.show() {
                    handle.wait_for_action(|action| {
                        if action == "open_location" || action == "default" {
                            let _ = open_folder_command(config, &dir)
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                        }
                    });
                }
                std::process::exit(0);
            }
            -1 => {
                let e = std::io::Error::last_os_error();
                tracing::error!("Fork failed for notification action: {}", e);
                let _ = notification.show();
            }
            _ => {}
        }
    } else {
        let _ = notification.show();
    }
}

/// "Screenshot of output 'DP-1' saved to wayshot-….png": says "saved" only
/// when a file was written (it said so for `--clipboard` or `-` too).
pub(crate) fn success_body(result: &ShotResult, delivery: &Delivery) -> String {
    let what = match result {
        ShotResult::Output { name } => format!("Screenshot of output '{name}'"),
        ShotResult::Toplevel { identifier } => format!("Screenshot of toplevel '{identifier}'"),
        ShotResult::Area => "Screenshot of selected area".to_string(),
        ShotResult::All => "Screenshot of all outputs".to_string(),
    };
    match (delivery.saved_to, delivery.clipboard) {
        (Some(path), clipboard) => {
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy();
            let copied = if clipboard { " and copied" } else { "" };
            format!("{what} saved to {name}{copied}")
        }
        (None, true) => format!("{what} copied to the clipboard"),
        (None, false) => format!("{what} taken"),
    }
}

/// The "Open Folder" action. The default opens the folder with `xdg-open`
/// as one argument; it used to go through `sh -c "xdg-open {dir}"`, which
/// split a folder name with spaces and ran `$(…)` in one. A configured
/// `action` is a shell command, as documented.
pub(crate) fn open_folder_command(config: &NotificationConfig, dir: &Path) -> Command {
    match &config.action {
        Some(custom) => {
            let mut command = Command::new("sh");
            command.args(["-c", custom]);
            command
        }
        None => {
            let mut command = Command::new("xdg-open");
            command.arg(dir);
            command
        }
    }
}

pub fn send_failure(error: &Error, config: &NotificationConfig) {
    let _ = build_base_notification(config, false)
        .urgency(Urgency::Critical)
        .body(&error.to_string())
        .show();
}

pub(crate) fn build_base_notification(
    config: &NotificationConfig,
    is_success: bool,
) -> Notification {
    let default_summary = if is_success {
        "Screenshot Taken"
    } else {
        "Screenshot Failed"
    };
    let summary = if is_success {
        config.success_summary.as_deref().unwrap_or(default_summary)
    } else {
        config.failure_summary.as_deref().unwrap_or(default_summary)
    };

    let mut n = Notification::new();
    n.summary(summary)
        .appname(config.app_name.as_deref().unwrap_or("wayshot"))
        .timeout(config.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));

    if let Some(icon) = &config.icon {
        n.icon(icon);
    }

    if let Some(sound) = &config.sound_name {
        n.sound_name(sound);
    }

    if let Some(transient) = config.transient {
        n.hint(Hint::Transient(transient));
    }

    if let Some(category) = &config.category {
        n.hint(Hint::Category(category.clone()));
    }

    n
}
