use crate::utils::EncodingFormat;
use serde::{Deserialize, Serialize};
use std::{env, error::Error, io::Read, path::PathBuf};
use tracing::Level;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub base: Option<Base>,
    pub file: Option<File>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            base: Some(Base::default()),
            file: Some(File::default()),
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Config, Box<dyn Error>> {
        let mut config_file = std::fs::File::open(path)?;
        let mut config_str = String::new();
        config_file.read_to_string(&mut config_str)?;

        toml::from_str(&config_str).map_err(|err| err.into())
    }

    pub fn get_default_path() -> Result<PathBuf, Box<dyn Error>> {
        dirs::config_local_dir()
            .ok_or_else(|| "Couldn't get local config directory path".into())
            .map(|path| path.join("wayshot").join("config.toml"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Base {
    pub output: Option<String>,
    pub cursor: Option<bool>,
    pub clipboard: Option<bool>,
    pub file: Option<bool>,
    pub stdout: Option<bool>,
    pub log_level: Option<String>,
}

impl Default for Base {
    fn default() -> Self {
        Base {
            output: None,
            cursor: Some(false),
            clipboard: Some(false),
            file: Some(true),
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
pub struct File {
    pub path: Option<PathBuf>,
    pub format: Option<String>,
    pub encoding: Option<EncodingFormat>,
}

impl Default for File {
    fn default() -> Self {
        File {
            path: Some(env::current_dir().unwrap_or_default()),
            format: Some("wayshot-%Y_%m_%d-%H_%M_%S".to_string()),
            encoding: Some(EncodingFormat::Png),
        }
    }
}
