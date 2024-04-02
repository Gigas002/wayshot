use crate::utils::EncodingFormat;
use serde::{Deserialize, Serialize};
use std::{env, fs::File, io::Read, path::PathBuf};
use tracing::Level;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub base: Option<Base>,
    pub fs: Option<Fs>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            base: Some(Base::default()),
            fs: Some(Fs::default()),
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Option<Config> {
        let mut config_file = File::open(path).ok()?;
        let mut config_str = String::new();
        config_file.read_to_string(&mut config_str).ok()?;

        toml::from_str(&config_str).ok()?
    }

    pub fn get_default_path() -> PathBuf {
        dirs::config_local_dir()
            .map(|path| path.join("wayshot").join("config.toml"))
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Base {
    pub output: Option<String>,
    pub cursor: Option<bool>,
    pub clipboard: Option<bool>,
    pub fs: Option<bool>,
    pub stdout: Option<bool>,
    pub log_level: Option<String>,
}

impl Default for Base {
    fn default() -> Self {
        Base {
            output: None,
            cursor: Some(false),
            clipboard: Some(false),
            fs: Some(true),
            stdout: Some(false),
            log_level: Some("info".to_string()),
        }
    }
}

impl Base {
    pub fn get_log_level(&self) -> Level {
        self.log_level
            .as_ref()
            .map_or(Level::INFO, |level| match level.as_str() {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fs {
    pub path: Option<PathBuf>,
    pub format: Option<String>,
    pub encoding: Option<EncodingFormat>,
}

impl Default for Fs {
    fn default() -> Self {
        Fs {
            path: Some(env::current_dir().unwrap_or_default()),
            format: Some("wayshot-%Y_%m_%d-%H_%M_%S".to_string()),
            encoding: Some(EncodingFormat::Png),
        }
    }
}
