//! Stderr logging via `tracing-subscriber`.

use std::io;

#[cfg(feature = "cli")]
use crate::cli::Cli;
use crate::config::Config;

/// Initialise the tracing subscriber, writing formatted logs to stderr.
///
/// The log level is taken from `--log-level` if supplied, then from the config
/// file, falling back to `INFO`.
pub fn setup(#[cfg(feature = "cli")] cli: &Cli, config: &Config) {
    let level = {
        #[cfg(feature = "cli")]
        {
            cli.log_level
        }
        #[cfg(not(feature = "cli"))]
        {
            None
        }
    }
    .unwrap_or_else(|| {
        config
            .base
            .as_ref()
            .map_or(tracing::Level::INFO, |b| b.get_log_level())
    });
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(io::stderr)
        .init();
}
