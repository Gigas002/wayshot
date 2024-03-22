use std::path::PathBuf;

use clap::arg;

use clap::Parser;
use eyre::WrapErr;

use crate::utils::EncodingFormat;
use clap::builder::TypedValueParser;

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Where to save the screenshot, "-" for stdout. Defaults to "$UNIX_TIMESTAMP-wayshot.$EXTENSION".
    #[arg(value_name = "OUTPUT")]
    pub file: Option<PathBuf>,

    /// Log level to be used for printing to stderr
    #[arg(long, default_value = "info", value_parser = clap::builder::PossibleValuesParser::new(["trace", "debug", "info", "warn", "error"]).map(|s| -> tracing::Level{ s.parse().wrap_err_with(|| format!("Failed to parse log level: {}", s)).unwrap()}))]
    pub log_level: tracing::Level,

    /// Arguments to call slurp with for selecting a region
    #[arg(short, long, value_name = "SLURP_ARGS")]
    pub slurp: Option<String>,

    /// Enable cursor in screenshots
    /// defaults to false
    #[arg(long)]
    pub cursor: Option<bool>,

    /// Filename format
    /// defaults to wayshot-%Y_%m_%d-%H_%M_%S
    #[arg(long)]
    pub filename_format: Option<String>,

    /// Path to config.toml file
    /// defaults to $HOME/.config/wayshot
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Set image encoder, by default uses the file extension from the OUTPUT
    /// positional argument. Otherwise defaults to png.
    #[arg(long, visible_aliases = ["extension", "format", "output-format"], value_name = "FILE_EXTENSION")]
    pub encoding: Option<EncodingFormat>,

    /// List all valid outputs
    #[arg(short, long, alias = "listoutputs")]
    pub list_outputs: bool,

    /// Choose a particular output/display to screenshot
    #[arg(short, long, conflicts_with = "slurp")]
    pub output: Option<String>,

    /// Present a fuzzy selector for output/display selection
    #[arg(long, alias = "chooseoutput", conflicts_with_all = ["slurp", "output"])]
    pub choose_output: bool,
}
