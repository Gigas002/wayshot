use std::io::{self, BufWriter, Write};
use std::time::Duration;

use eyre::Result;
use libwayshot::WayshotConnection;

#[cfg(feature = "cli")]
use clap::Parser;

#[cfg(feature = "cli")]
mod cli;
#[cfg(feature = "clipboard")]
mod clipboard;
#[cfg(feature = "color_picker")]
mod color_picker;
mod config;
#[cfg(feature = "logger")]
mod logger;
#[cfg(feature = "notifications")]
mod notification;
mod screenshot;
mod settings;
mod utils;

use config::Config;
use settings::{AppSettings, Command};

fn main() -> Result<()> {
    #[cfg(feature = "cli")]
    let cli = cli::Cli::parse();

    #[cfg(feature = "completions")]
    if let Some(shell) = cli.completions {
        utils::print_completions(shell);
        return Ok(());
    }

    #[cfg(feature = "config")]
    let config = {
        #[cfg(feature = "cli")]
        let config_path = cli.config.clone().unwrap_or(Config::get_default_path());
        #[cfg(not(feature = "cli"))]
        let config_path = Config::get_default_path();
        Config::load(&config_path).unwrap_or_default()
    };
    #[cfg(not(feature = "config"))]
    let config = Config::default();

    #[cfg(feature = "logger")]
    logger::setup(
        #[cfg(feature = "cli")]
        &cli,
        &config,
    );

    let settings = AppSettings::resolve(
        #[cfg(feature = "cli")]
        &cli,
        &config,
    );

    let connection = WayshotConnection::new()?;
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    match settings.command {
        #[cfg(feature = "cli")]
        Command::ListOutputs => {
            for output in connection.get_all_outputs() {
                writeln!(writer, "{}", output.name)?;
            }
            writer.flush()?;
            Ok(())
        }
        #[cfg(feature = "cli")]
        Command::ListOutputsInfo => {
            connection.print_displays_info();
            Ok(())
        }
        #[cfg(feature = "cli")]
        Command::ListToplevels => {
            for tl in connection.get_all_toplevels().iter().filter(|t| t.active) {
                writeln!(writer, "{}", tl.id_and_title())?;
            }
            writer.flush()?;
            Ok(())
        }
        #[cfg(feature = "color_picker")]
        Command::ColorPicker => color_picker::pick(&connection, settings.freeze),
        Command::Screenshot(mode) => {
            if let Some(ms) = settings.delay {
                std::thread::sleep(Duration::from_millis(ms as u64));
            }
            let result = screenshot::capture(&connection, &mode, settings.cursor, settings.freeze);
            match result {
                Ok((image_buffer, shot_result)) => {
                    let encoded = utils::encode_image(
                        &image_buffer,
                        settings.encoding,
                        &settings.jxl,
                        &settings.png,
                    )
                    .map_err(|e| eyre::eyre!("Failed to encode image: {e}"))?;

                    if let Some(ref f) = settings.file {
                        std::fs::write(f, &encoded)?;
                    }

                    if settings.stdout_print {
                        writer.write_all(&encoded)?;
                    }

                    #[cfg(feature = "clipboard")]
                    if settings.clipboard {
                        clipboard::copy_to_clipboard(encoded)?;
                    }

                    #[cfg(feature = "notifications")]
                    if settings.notifications {
                        notification::send_success(&shot_result);
                    }
                    // Silence unused warning when the notifications feature is disabled.
                    #[cfg(not(feature = "notifications"))]
                    drop(shot_result);

                    Ok(())
                }
                Err(e) => {
                    #[cfg(feature = "notifications")]
                    if settings.notifications {
                        notification::send_failure(&e);
                    }
                    Err(e)
                }
            }
        }
    }
}
