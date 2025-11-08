use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub hotkeys: HotkeyConfig,
    pub transcription: TranscriptionConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HotkeyConfig {
    pub start_transcription: String,
    pub stop_transcription: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptionConfig {
    pub model: String,
    pub language: String,
    pub use_gpu: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkeys: HotkeyConfig {
                start_transcription: "Cmd+Shift+T".to_string(),
                stop_transcription: "Cmd+Shift+S".to_string(),
            },
            transcription: TranscriptionConfig {
                model: "base".to_string(),
                language: "en".to_string(),
                use_gpu: true,
            },
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        Ok(home.join(".live-transcribe"))
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("settings.yaml"))
    }

    pub fn load_or_create() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = fs::read_to_string(&config_path)
                .context("Failed to read config file")?;
            let config: Config = serde_yaml::from_str(&contents)
                .context("Failed to parse config file")?;
            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            config.save()?;
            println!("Created default config at: {}", config_path.display());
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir()?;
        fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;

        let config_path = Self::config_path()?;
        let yaml = serde_yaml::to_string(self)
            .context("Failed to serialize config")?;

        fs::write(&config_path, yaml)
            .context("Failed to write config file")?;

        Ok(())
    }
}
