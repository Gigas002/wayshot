use crate::utils::EncodingFormat;
use serde::{Deserialize, Serialize};
use std::{env, io::Read, path::PathBuf};
#[cfg(feature = "logger")]
use tracing::Level;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub base: Option<Base>,
    pub file: Option<File>,
    pub geometry: Option<Geometry>,
    pub encoding: Option<Encoding>,
    pub notification: Option<NotificationConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            base: Some(Base::default()),
            file: Some(File::default()),
            geometry: Some(Geometry::default()),
            encoding: Some(Encoding::default()),
            notification: Some(NotificationConfig::default()),
        }
    }
}

impl Config {
    /// Read and check a config file. Errors name the file and, for a bad
    /// value or an unknown key, the setting. `Ok(None)` when the file
    /// doesn't exist.
    pub fn load(path: &PathBuf) -> Result<Option<Config>, String> {
        let mut config_file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let mut config_str = String::new();
        config_file
            .read_to_string(&mut config_str)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let config: Config =
            toml::from_str(&config_str).map_err(|e| format!("{}: {e}", path.display()))?;
        config
            .check()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Some(config))
    }

    /// Values that parse but mean nothing: misspelt names used to fall
    /// back to a default without a word.
    fn check(&self) -> Result<(), String> {
        if let Some(level) = self.base.as_ref().and_then(|b| b.log_level.as_deref())
            && !["trace", "debug", "info", "warn", "error"].contains(&level)
        {
            return Err(format!(
                "[base] log_level: unknown level '{level}' (trace, debug, info, warn, error)"
            ));
        }
        let png = self.encoding.as_ref().and_then(|e| e.png.as_ref());
        match png.and_then(|p| p.compression.as_ref()) {
            Some(PngCompression::Level(level)) if *level > 9 => {
                return Err(format!(
                    "[encoding.png] compression: level {level} is out of range (0-9)"
                ));
            }
            Some(PngCompression::Named(name))
                if !["default", "best", "fast", "none", "uncompressed"]
                    .contains(&name.as_str()) =>
            {
                return Err(format!(
                    "[encoding.png] compression: unknown value '{name}' \
                     (default, fast, best, none, uncompressed or 0-9)"
                ));
            }
            _ => {}
        }
        if let Some(filter) = png.and_then(|p| p.filter.as_deref())
            && !["none", "sub", "up", "avg", "paeth", "adaptive"].contains(&filter)
        {
            return Err(format!(
                "[encoding.png] filter: unknown value '{filter}' \
                 (none, sub, up, avg, paeth, adaptive)"
            ));
        }
        Ok(())
    }

    pub fn get_default_path() -> PathBuf {
        dirs::config_local_dir()
            .map(|path| path.join("wayshot").join("config.toml"))
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Base {
    pub output: Option<String>,
    pub cursor: Option<bool>,
    pub freeze: Option<bool>,
    pub delay: Option<u32>,
    pub clipboard: Option<bool>,
    pub file: Option<bool>,
    pub stdout: Option<bool>,
    pub log_level: Option<String>,
    pub notifications: Option<bool>,
    pub geometry_foreground_color: Option<String>,
    pub geometry_background_color: Option<String>,
}

impl Default for Base {
    fn default() -> Self {
        Base {
            output: None,
            cursor: Some(false),
            freeze: Some(true),
            delay: None,
            clipboard: Some(false),
            file: Some(true),
            stdout: Some(false),
            log_level: Some("info".to_string()),
            notifications: Some(true),
            geometry_foreground_color: None,
            geometry_background_color: None,
        }
    }
}

#[cfg(feature = "logger")]
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
#[serde(deny_unknown_fields)]
pub struct File {
    pub path: Option<PathBuf>,
    pub name_format: Option<String>,
    pub encoding: Option<EncodingFormat>,
}

impl Default for File {
    fn default() -> Self {
        File {
            path: Some(env::current_dir().unwrap_or_default()),
            name_format: Some("wayshot-%Y_%m_%d-%H_%M_%S".to_string()),
            encoding: Some(EncodingFormat::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub foreground_color: Option<String>,
    pub background_color: Option<String>,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            foreground_color: Some("#000000ff".to_string()),
            background_color: Some("#66666680".to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Encoding {
    pub jxl: Option<Jxl>,
    pub png: Option<Png>,
}

impl Default for Encoding {
    fn default() -> Self {
        Encoding {
            jxl: Some(Jxl::default()),
            png: Some(Png::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Jxl {
    pub lossless: Option<bool>,
    pub distance: Option<f32>,
    pub effort: Option<u8>,
}

impl Default for Jxl {
    fn default() -> Self {
        Jxl {
            lossless: Some(false),
            distance: Some(1.0),
            effort: Some(7),
        }
    }
}

#[cfg(feature = "jxl")]
impl Jxl {
    pub fn get_lossless(&self) -> bool {
        self.lossless.unwrap_or(false)
    }

    pub fn get_distance(&self) -> f32 {
        self.distance.unwrap_or(1.0)
    }

    pub fn get_effort(&self) -> u8 {
        self.effort.unwrap_or(7).clamp(1, 10)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PngCompression {
    Level(u8),
    Named(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Png {
    pub compression: Option<PngCompression>,
    pub filter: Option<String>,
}

impl Default for Png {
    fn default() -> Self {
        Png {
            compression: Some(PngCompression::Named("default".to_string())),
            filter: Some("adaptive".to_string()),
        }
    }
}

impl Png {
    pub fn get_compression(&self) -> image::codecs::png::CompressionType {
        match self
            .compression
            .as_ref()
            .unwrap_or(&PngCompression::Named("default".to_string()))
        {
            PngCompression::Level(level) => {
                if *level <= 9 {
                    image::codecs::png::CompressionType::Level(*level)
                } else {
                    image::codecs::png::CompressionType::Default
                }
            }
            PngCompression::Named(name) => match name.as_str() {
                "default" => image::codecs::png::CompressionType::Default,
                "best" => image::codecs::png::CompressionType::Best,
                "fast" => image::codecs::png::CompressionType::Fast,
                "none" | "uncompressed" => image::codecs::png::CompressionType::Uncompressed,
                _ => image::codecs::png::CompressionType::Default,
            },
        }
    }

    pub fn get_filter(&self) -> image::codecs::png::FilterType {
        match self.filter.as_deref().unwrap_or("default") {
            "none" => image::codecs::png::FilterType::NoFilter,
            "sub" => image::codecs::png::FilterType::Sub,
            "up" => image::codecs::png::FilterType::Up,
            "avg" => image::codecs::png::FilterType::Avg,
            "paeth" => image::codecs::png::FilterType::Paeth,
            "adaptive" => image::codecs::png::FilterType::Adaptive,
            _ => image::codecs::png::FilterType::Adaptive,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationConfig {
    /// Shell command to run when the notification is clicked.
    pub action: Option<String>,
    /// Notification icon name (e.g. "camera-photo").
    pub icon: Option<String>,
    /// Duration in milliseconds. -1 = never expire, 0 = server default.
    pub timeout_ms: Option<i32>,
    /// Application name shown in the notification.
    pub app_name: Option<String>,
    /// Title for the success notification.
    pub success_summary: Option<String>,
    /// Title for the failure notification.
    pub failure_summary: Option<String>,
    /// XDG sound name to play (e.g. "message-new-instant").
    pub sound_name: Option<String>,
    /// Transient hint: notification won't persist in notification center.
    pub transient: Option<bool>,
    /// Notification category hint (e.g. "transfer.complete", "im.received").
    pub category: Option<String>,
}
