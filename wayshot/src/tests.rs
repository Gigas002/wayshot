//! Unit tests for the wayshot CLI and supporting logic.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::cli::Cli;
use crate::config::Config;
use crate::settings::{AppSettings, Command};
use crate::utils::{self, EncodingFormat};
use clap::Parser;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    let args: Vec<OsString> = std::iter::once(OsString::from("wayshot"))
        .chain(args.iter().map(OsString::from))
        .collect();
    Cli::try_parse_from(args)
}

#[test]
fn list_outputs_long() {
    let cli = parse(&["--list-outputs"]).unwrap();
    assert!(cli.list_outputs);
}

#[test]
fn list_outputs_short() {
    let cli = parse(&["-l"]).unwrap();
    assert!(cli.list_outputs);
}

#[test]
fn list_outputs_info() {
    let cli = parse(&["--list-outputs-info"]).unwrap();
    assert!(cli.list_outputs_info);
}

#[test]
fn list_toplevels() {
    let cli = parse(&["--list-toplevels"]).unwrap();
    assert!(cli.list_toplevels);
}

#[test]
fn list_windows_alias() {
    let cli = parse(&["--list-windows"]).unwrap();
    assert!(cli.list_toplevels);
}

#[test]
fn geometry_and_output_conflict() {
    assert!(parse(&["--geometry", "0,0 100x100", "--output", "HDMI-1"]).is_err());
}

#[test]
fn output_and_geometry_conflict() {
    assert!(parse(&["--output", "HDMI-1", "-g", "0,0 200x200"]).is_err());
}

#[test]
fn geometry_with_value() {
    let cli = parse(&["--geometry", "10,20 30x40"]).unwrap();
    assert_eq!(cli.geometry, Some(Some("10,20 30x40".into())));
}

#[test]
fn encoding_png() {
    let cli = parse(&["--list-outputs", "--encoding", "png"]).unwrap();
    assert_eq!(cli.encoding, Some(EncodingFormat::Png));
}

#[test]
fn encoding_jpeg() {
    // CLI ValueEnum accepts "jpg" (suggested value); "jpeg" is accepted by FromStr in utils.
    let cli = parse(&["--list-outputs", "--encoding", "jpg"]).unwrap();
    assert_eq!(cli.encoding, Some(EncodingFormat::Jpg));
}

#[test]
fn encoding_invalid() {
    assert!(parse(&["--list-outputs", "--encoding", "invalid"]).is_err());
}

#[test]
fn delay_value() {
    let cli = parse(&["--list-outputs", "--delay", "500"]).unwrap();
    assert_eq!(cli.delay, Some(500));
}

#[test]
fn cursor_flag() {
    let cli = parse(&["--list-outputs", "--cursor"]).unwrap();
    assert!(cli.cursor);
}

#[test]
fn no_freeze_flag() {
    let cli = parse(&["--list-outputs", "--no-freeze"]).unwrap();
    assert!(cli.no_freeze);
}

#[test]
fn file_positional() {
    let cli = parse(&["--list-outputs", "/tmp/out.png"]).unwrap();
    assert_eq!(
        cli.file.as_ref().map(|p| p.as_path()),
        Some(std::path::Path::new("/tmp/out.png"))
    );
}

#[test]
fn config_flag() {
    let cli = parse(&["--list-outputs", "--config", "/etc/wayshot.toml"]).unwrap();
    assert_eq!(
        cli.config.as_ref().map(|p| p.as_path()),
        Some(std::path::Path::new("/etc/wayshot.toml"))
    );
}

// ─── utils: parse_slurp_geometry ─────────────────────────────────────────────

#[test]
fn parse_slurp_geometry_valid() {
    let r = utils::parse_slurp_geometry("10,20 30x40").unwrap();
    assert_eq!(r.inner.position.x, 10);
    assert_eq!(r.inner.position.y, 20);
    assert_eq!(r.inner.size.width, 30);
    assert_eq!(r.inner.size.height, 40);
}

#[test]
fn parse_slurp_geometry_with_whitespace() {
    // Parser splits on first space: position must be "x,y" (no space), size "WxH". Only leading/trailing whitespace is trimmed.
    let r = utils::parse_slurp_geometry("  100,200 300x400  ").unwrap();
    assert_eq!(r.inner.position.x, 100);
    assert_eq!(r.inner.position.y, 200);
    assert_eq!(r.inner.size.width, 300);
    assert_eq!(r.inner.size.height, 400);
}

#[test]
fn parse_slurp_geometry_empty() {
    assert!(utils::parse_slurp_geometry("").is_err());
    assert!(utils::parse_slurp_geometry("   ").is_err());
}

#[test]
fn parse_slurp_geometry_zero_size() {
    assert!(utils::parse_slurp_geometry("0,0 0x100").is_err());
    assert!(utils::parse_slurp_geometry("0,0 100x0").is_err());
}

#[test]
fn parse_slurp_geometry_invalid_format() {
    assert!(utils::parse_slurp_geometry("10,20").is_err());
    assert!(utils::parse_slurp_geometry("10 20 30x40").is_err());
    assert!(utils::parse_slurp_geometry("10,20 30,40").is_err());
    assert!(utils::parse_slurp_geometry("a,b 30x40").is_err());
}

// ─── utils: EncodingFormat ────────────────────────────────────────────────────

#[test]
fn encoding_format_from_str() {
    assert_eq!(
        "png".parse::<EncodingFormat>().unwrap(),
        EncodingFormat::Png
    );
    assert_eq!(
        "jpg".parse::<EncodingFormat>().unwrap(),
        EncodingFormat::Jpg
    );
    assert_eq!(
        "jpeg".parse::<EncodingFormat>().unwrap(),
        EncodingFormat::Jpg
    );
    assert_eq!(
        "webp".parse::<EncodingFormat>().unwrap(),
        EncodingFormat::Webp
    );
    assert_eq!(
        "ppm".parse::<EncodingFormat>().unwrap(),
        EncodingFormat::Ppm
    );
    assert_eq!(
        "qoi".parse::<EncodingFormat>().unwrap(),
        EncodingFormat::Qoi
    );
    assert!("invalid".parse::<EncodingFormat>().is_err());
}

#[test]
fn encoding_format_from_path() {
    assert_eq!(
        EncodingFormat::try_from(&PathBuf::from("out.png")).unwrap(),
        EncodingFormat::Png
    );
    assert_eq!(
        EncodingFormat::try_from(&PathBuf::from("x/y.jpeg")).unwrap(),
        EncodingFormat::Jpg
    );
    assert!(EncodingFormat::try_from(&PathBuf::from("noext")).is_err());
}

// ─── utils: path helpers ─────────────────────────────────────────────────────

#[test]
fn get_absolute_path_absolute() {
    let p = PathBuf::from("/tmp/foo");
    assert_eq!(utils::get_absolute_path(&p), PathBuf::from("/tmp/foo"));
}

#[test]
fn get_full_file_name_directory() {
    let dir = std::env::temp_dir();
    let out = utils::get_full_file_name(&dir, "wayshot-%Y_%m_%d-%H_%M_%S", EncodingFormat::Png);
    assert!(out.is_absolute());
    assert!(out.file_name().unwrap().to_string_lossy().ends_with(".png"));
}

#[test]
fn get_full_file_name_file_path() {
    let path = PathBuf::from("/tmp/screenshot.png");
    let out = utils::get_full_file_name(&path, "wayshot-%Y", EncodingFormat::Jpg);
    assert_eq!(out.file_name().unwrap(), "screenshot.jpg");
}

#[test]
fn get_default_file_name_format() {
    let out = utils::get_default_file_name("test-%Y", EncodingFormat::Png);
    assert!(out.to_string_lossy().contains("test-"));
    assert!(out.to_string_lossy().ends_with(".png"));
}

// ─── config ──────────────────────────────────────────────────────────────────

#[test]
fn config_load_from_toml() {
    let toml = r#"
[base]
cursor = true
freeze = false
delay = 100
output = "HDMI-1"

[file]
encoding = "webp"
name_format = "shot-%Y%m%d"
"#;
    let dir = std::env::temp_dir();
    let path = dir.join("wayshot_test_config.toml");
    std::fs::write(&path, toml).unwrap();
    let config = Config::load(&path).unwrap();
    assert_eq!(config.base.as_ref().unwrap().cursor, Some(true));
    assert_eq!(config.base.as_ref().unwrap().freeze, Some(false));
    assert_eq!(config.base.as_ref().unwrap().delay, Some(100));
    assert_eq!(
        config.base.as_ref().unwrap().output.as_deref(),
        Some("HDMI-1")
    );
    assert_eq!(
        config.file.as_ref().unwrap().encoding,
        Some(EncodingFormat::Webp)
    );
    assert_eq!(
        config.file.as_ref().unwrap().name_format.as_deref(),
        Some("shot-%Y%m%d")
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn config_load_nonexistent_returns_none() {
    let path = PathBuf::from("/nonexistent/wayshot_config_does_not_exist.toml");
    assert!(Config::load(&path).is_none());
}

#[test]
fn config_default() {
    let config = Config::default();
    assert!(config.base.is_some());
    assert!(config.file.is_some());
    assert!(config.encoding.is_some());
}

#[test]
fn png_compression_and_filter() {
    use crate::config::Png;
    use image::codecs::png::{CompressionType, FilterType};

    let png = Png::default();
    assert_eq!(png.get_compression(), CompressionType::Default);
    assert_eq!(png.get_filter(), FilterType::Adaptive);

    let png_fast = Png {
        compression: Some(crate::config::PngCompression::Named("fast".to_string())),
        filter: Some("none".to_string()),
    };
    assert_eq!(png_fast.get_compression(), CompressionType::Fast);
    assert_eq!(png_fast.get_filter(), FilterType::NoFilter);
}

// ─── settings: AppSettings::resolve ───────────────────────────────────────────

#[test]
fn resolve_command_list_outputs() {
    let cli = parse(&["--list-outputs"]).unwrap();
    let config = Config::default();
    let settings = AppSettings::resolve(&cli, &config).unwrap();
    assert!(matches!(settings.command, Command::ListOutputs));
}

#[test]
fn resolve_command_list_outputs_info() {
    let cli = parse(&["--list-outputs-info"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert!(matches!(settings.command, Command::ListOutputsInfo));
}

#[test]
fn resolve_command_list_toplevels() {
    let cli = parse(&["--list-toplevels"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert!(matches!(settings.command, Command::ListToplevels));
}

#[test]
fn resolve_command_screenshot_geometry_region() {
    let cli = parse(&["--geometry", "5,10 20x30"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    match &settings.command {
        Command::Screenshot(crate::screenshot::CaptureMode::GeometryRegion(r)) => {
            assert_eq!(r.inner.position.x, 5);
            assert_eq!(r.inner.position.y, 10);
            assert_eq!(r.inner.size.width, 20);
            assert_eq!(r.inner.size.height, 30);
        }
        _ => panic!("expected Screenshot(GeometryRegion(_))"),
    }
}

#[test]
fn resolve_command_screenshot_output() {
    let cli = parse(&["--output", "HDMI-1"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    match &settings.command {
        Command::Screenshot(crate::screenshot::CaptureMode::Output(name)) => {
            assert_eq!(name, "HDMI-1");
        }
        _ => panic!("expected Screenshot(Output(_))"),
    }
}

#[test]
fn resolve_command_screenshot_choose_output() {
    let cli = parse(&["--choose-output"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert!(matches!(
        settings.command,
        Command::Screenshot(crate::screenshot::CaptureMode::ChooseOutput)
    ));
}

#[test]
fn resolve_command_screenshot_toplevel() {
    let cli = parse(&["--toplevel", "firefox Firefox"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    match &settings.command {
        Command::Screenshot(crate::screenshot::CaptureMode::Toplevel(name)) => {
            assert_eq!(name, "firefox Firefox");
        }
        _ => panic!("expected Screenshot(Toplevel(_))"),
    }
}

#[test]
fn resolve_command_screenshot_all_default() {
    let cli = parse(&[]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert!(matches!(
        settings.command,
        Command::Screenshot(crate::screenshot::CaptureMode::All)
    ));
}

#[test]
fn resolve_cursor_cli_overrides() {
    let cli = parse(&["--list-outputs", "--cursor"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert!(settings.cursor);
}

#[test]
fn resolve_freeze_no_freeze_flag() {
    let cli = parse(&["--list-outputs", "--no-freeze"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert!(!settings.freeze);
}

#[test]
fn resolve_delay_from_cli() {
    let cli = parse(&["--list-outputs", "--delay", "250"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert_eq!(settings.delay, Some(250));
}

#[test]
fn resolve_encoding_from_cli() {
    let cli = parse(&["--list-outputs", "--encoding", "webp"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert_eq!(settings.encoding, EncodingFormat::Webp);
}

#[test]
fn resolve_encoding_from_file_extension() {
    let cli = parse(&["--list-outputs", "/tmp/out.jpeg"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert_eq!(settings.encoding, EncodingFormat::Jpg);
}

#[test]
fn resolve_encoding_from_config() {
    let cli = parse(&["--list-outputs"]).unwrap();
    let mut config = Config::default();
    config.file.as_mut().unwrap().encoding = Some(EncodingFormat::Qoi);
    let settings = AppSettings::resolve(&cli, &config).unwrap();
    assert_eq!(settings.encoding, EncodingFormat::Qoi);
}

#[test]
fn resolve_file_stdout_when_dash() {
    let cli = parse(&["--list-outputs", "-"]).unwrap();
    let config = Config::default();
    let settings = AppSettings::resolve(&cli, &config).unwrap();
    assert!(settings.stdout_print);
    assert!(settings.file.is_none());
}

#[test]
fn resolve_file_path_from_cli() {
    let cli = parse(&["--list-outputs", "/tmp/my.png"]).unwrap();
    let settings = AppSettings::resolve(&cli, &Config::default()).unwrap();
    assert!(settings.file.is_some());
    assert!(
        settings
            .file
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .contains("my")
    );
    assert!(
        settings
            .file
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .ends_with(".png")
    );
}
