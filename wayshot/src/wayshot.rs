use std::{
    io::{stdout, BufWriter, Cursor, Write},
    process::Command,
};

use clap::Parser;
use config::Config;
use eyre::{bail, Result};
use libwayshot::{region::LogicalRegion, WayshotConnection};

mod cli;
mod config;
mod utils;

use dialoguer::{theme::ColorfulTheme, FuzzySelect};

fn select_ouput<T>(ouputs: &[T]) -> Option<usize>
where
    T: ToString,
{
    let Ok(selection) = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose Screen")
        .default(0)
        .items(ouputs)
        .interact()
    else {
        return None;
    };
    Some(selection)
}

fn main() -> Result<()> {
    // cli args
    let cli = cli::Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.log_level)
        .with_writer(std::io::stderr)
        .init();

    // config path
    let config_path = dirs::config_local_dir()
        .and_then(|path| Some(path.join("wayshot").join("config.toml")))
        .unwrap_or_default();
    let config_path = cli.config.unwrap_or(config_path);

    // configs
    let config = Config::load(&config_path).unwrap_or_default();
    let screenshot = config.screenshot.unwrap_or_default();
    let filesystem = config.filesystem.unwrap_or_default();

    // vars
    let cursor = cli.cursor.unwrap_or(screenshot.cursor.unwrap_or_default());
    let filename_format = cli.filename_format.unwrap_or(filesystem.format.unwrap());
    let encoding = cli
        .encoding
        .unwrap_or(filesystem.encoding.unwrap_or_default());
    let dir = filesystem.path.unwrap();
    let file = match cli.file {
        Some(path) => Some(path),
        _ => match filesystem.filesystem {
            Some(true) => Some(utils::get_full_file_name(&dir, &filename_format, encoding)),
            _ => None,
        },
    };

    // actual work
    let wayshot_conn = WayshotConnection::new()?;

    if cli.list_outputs {
        let valid_outputs = wayshot_conn.get_all_outputs();
        for output in valid_outputs {
            tracing::info!("{:#?}", output.name);
        }
        return Ok(());
    }

    let image_buffer = if let Some(slurp_region) = cli.slurp {
        let slurp_region = slurp_region.clone();
        wayshot_conn.screenshot_freeze(
            Box::new(move || {
                || -> Result<LogicalRegion> {
                    let slurp_output = Command::new("slurp")
                        .args(slurp_region.split(" "))
                        .output()?
                        .stdout;

                    utils::parse_geometry(&String::from_utf8(slurp_output)?)
                }()
                .map_err(|_| libwayshot::Error::FreezeCallbackError)
            }),
            cursor,
        )?
    } else if let Some(output_name) = cli.output {
        let outputs = wayshot_conn.get_all_outputs();
        if let Some(output) = outputs.iter().find(|output| output.name == output_name) {
            wayshot_conn.screenshot_single_output(output, cursor)?
        } else {
            bail!("No output found!");
        }
    } else if cli.choose_output {
        let outputs = wayshot_conn.get_all_outputs();
        let output_names: Vec<String> = outputs
            .iter()
            .map(|display| display.name.to_string())
            .collect();
        if let Some(index) = select_ouput(&output_names) {
            wayshot_conn.screenshot_single_output(&outputs[index], cursor)?
        } else {
            bail!("No output found!");
        }
    } else {
        wayshot_conn.screenshot_all(cursor)?
    };

    if let Some(file) = file {
        image_buffer.save(file)?;
    } else {
        let stdout = stdout();
        let mut buffer = Cursor::new(Vec::new());

        let mut writer = BufWriter::new(stdout.lock());
        image_buffer.write_to(&mut buffer, encoding)?;

        writer.write_all(buffer.get_ref())?;
    }

    Ok(())
}
