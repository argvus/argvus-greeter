use serde::Deserialize;
use std::{fs, path::PathBuf};
use tracing::{info, warn};

const CONFIG_PATH: &str = "/etc/argvus/greeter.toml";
const DEFAULT_WALLPAPER: &str = "/usr/share/backgrounds/argvus/default.png";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub appearance: AppearanceConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub wallpaper: PathBuf,
    pub show_clock: bool,
    pub show_date: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub default: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            wallpaper: PathBuf::from(DEFAULT_WALLPAPER),
            show_clock: true,
            show_date: true,
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            default: "argvus".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        match fs::read_to_string(CONFIG_PATH) {
            Ok(contents) => {
                let config = toml::from_str(&contents)?;
                info!(path = CONFIG_PATH, "configuration loaded");
                Ok(config)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                info!(
                    path = CONFIG_PATH,
                    "configuration not found, using defaults"
                );
                Ok(Self::default())
            }
            Err(error) => {
                warn!(path = CONFIG_PATH, %error, "could not read configuration");
                Err(error.into())
            }
        }
    }
}
