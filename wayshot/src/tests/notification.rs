use crate::config::NotificationConfig;
use crate::notification::build_base_notification;

#[test]
fn success_notification_uses_default_summary_and_appname() {
    let config = NotificationConfig::default();
    let n = build_base_notification(&config, true);
    assert_eq!(n.summary, "Screenshot Taken");
    assert_eq!(n.appname, "wayshot");
    assert_eq!(n.timeout, notify_rust::Timeout::Milliseconds(5000));
}

#[test]
fn failure_notification_uses_default_summary() {
    let config = NotificationConfig::default();
    let n = build_base_notification(&config, false);
    assert_eq!(n.summary, "Screenshot Failed");
}

#[test]
fn custom_summaries_override_defaults() {
    let config = NotificationConfig {
        success_summary: Some("Custom Success".to_string()),
        failure_summary: Some("Custom Failure".to_string()),
        ..NotificationConfig::default()
    };
    assert_eq!(
        build_base_notification(&config, true).summary,
        "Custom Success"
    );
    assert_eq!(
        build_base_notification(&config, false).summary,
        "Custom Failure"
    );
}

#[test]
fn custom_app_name_and_timeout() {
    let config = NotificationConfig {
        app_name: Some("my-wayshot".to_string()),
        timeout_ms: Some(1234),
        ..NotificationConfig::default()
    };
    let n = build_base_notification(&config, true);
    assert_eq!(n.appname, "my-wayshot");
    assert_eq!(n.timeout, notify_rust::Timeout::Milliseconds(1234));
}

#[test]
fn icon_is_set_when_configured() {
    let config = NotificationConfig {
        icon: Some("camera-photo".to_string()),
        ..NotificationConfig::default()
    };
    let n = build_base_notification(&config, true);
    assert_eq!(n.icon, "camera-photo");
}

#[test]
fn icon_is_empty_when_not_configured() {
    let config = NotificationConfig::default();
    let n = build_base_notification(&config, true);
    assert!(n.icon.is_empty());
}

#[test]
fn sound_name_hint_is_set_when_configured() {
    let config = NotificationConfig {
        sound_name: Some("message-new-instant".to_string()),
        ..NotificationConfig::default()
    };
    let n = build_base_notification(&config, true);
    assert!(n.hints.contains(&notify_rust::Hint::SoundName(
        "message-new-instant".to_string()
    )));
}

#[test]
fn transient_hint_is_set_when_configured() {
    let config = NotificationConfig {
        transient: Some(true),
        ..NotificationConfig::default()
    };
    let n = build_base_notification(&config, true);
    assert!(n.hints.contains(&notify_rust::Hint::Transient(true)));
}

#[test]
fn category_hint_is_set_when_configured() {
    let config = NotificationConfig {
        category: Some("transfer.complete".to_string()),
        ..NotificationConfig::default()
    };
    let n = build_base_notification(&config, true);
    assert!(n.hints.contains(&notify_rust::Hint::Category(
        "transfer.complete".to_string()
    )));
}

#[test]
fn no_hints_are_set_when_nothing_configured() {
    let config = NotificationConfig::default();
    let n = build_base_notification(&config, true);
    assert!(n.hints.is_empty());
}

#[test]
fn success_body_says_where_the_screenshot_went() {
    use crate::notification::{Delivery, success_body};
    use crate::screenshot::ShotResult;
    use std::path::Path;
    let output = ShotResult::Output {
        name: "DP-1".to_string(),
    };
    let saved = Path::new("/home/me/Pictures/shot.png");
    let body = |saved_to, clipboard| {
        success_body(
            &output,
            &Delivery {
                saved_to,
                clipboard,
            },
        )
    };
    assert_eq!(
        body(Some(saved), false),
        "Screenshot of output 'DP-1' saved to shot.png"
    );
    assert_eq!(
        body(Some(saved), true),
        "Screenshot of output 'DP-1' saved to shot.png and copied"
    );
    // Nothing was saved: it used to say "saved" anyway.
    assert_eq!(
        body(None, true),
        "Screenshot of output 'DP-1' copied to the clipboard"
    );
    assert_eq!(body(None, false), "Screenshot of output 'DP-1' taken");
}

/// The folder is one argument to xdg-open, not shell text: a name with
/// spaces or `$(...)` used to be split or run by `sh -c`.
#[test]
fn open_folder_runs_xdg_open_without_a_shell() {
    use crate::notification::open_folder_command;
    use std::ffi::OsStr;
    use std::path::Path;
    let dir = Path::new("/tmp/My Shots/$(touch pwned)");
    let command = open_folder_command(&NotificationConfig::default(), dir);
    assert_eq!(command.get_program(), "xdg-open");
    let args: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(args, [dir.as_os_str()]);

    let config = NotificationConfig {
        action: Some("thunar ~/Pictures".to_string()),
        ..NotificationConfig::default()
    };
    let command = open_folder_command(&config, dir);
    assert_eq!(command.get_program(), "sh");
    let args: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(args, ["-c", "thunar ~/Pictures"]);
}
