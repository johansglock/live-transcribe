use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub transcription: TranscriptionConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HotkeyConfig {
    #[serde(default = "default_start_hotkey")]
    pub start_transcription: String,
    #[serde(default = "default_stop_hotkey")]
    pub stop_transcription: String,
}

fn default_start_hotkey() -> String {
    "Cmd+Shift+T".to_string()
}

fn default_stop_hotkey() -> String {
    "Cmd+Shift+S".to_string()
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            start_transcription: default_start_hotkey(),
            stop_transcription: default_stop_hotkey(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptionConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_use_gpu")]
    pub use_gpu: bool,
    #[serde(default = "default_streaming")]
    pub streaming: bool,
    #[serde(default = "default_chunk_duration")]
    pub chunk_duration_ms: u64,
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold: f32,
}

fn default_model() -> String {
    "small.en".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_use_gpu() -> bool {
    true
}

fn default_streaming() -> bool {
    true
}

fn default_chunk_duration() -> u64 {
    300 // 300ms for low latency (padded to 1s+ with sliding window)
}

fn default_silence_threshold() -> f32 {
    0.003 // RMS threshold for silence detection (more sensitive, picks up quieter speech)
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        TranscriptionConfig {
            model: default_model(),
            language: default_language(),
            use_gpu: default_use_gpu(),
            streaming: default_streaming(),
            chunk_duration_ms: default_chunk_duration(),
            silence_threshold: default_silence_threshold(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkeys: HotkeyConfig::default(),
            transcription: TranscriptionConfig::default(),
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

            // Validate configuration after loading
            config.validate()?;

            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            config.save()?;
            println!("Created default config at: {}", config_path.display());
            Ok(config)
        }
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate chunk duration
        if self.transcription.chunk_duration_ms == 0 {
            bail!("chunk_duration_ms must be greater than 0");
        }
        if self.transcription.chunk_duration_ms > 5000 {
            bail!("chunk_duration_ms must be <= 5000 (5 seconds)");
        }

        // Validate silence threshold
        if self.transcription.silence_threshold < 0.0 {
            bail!("silence_threshold must be >= 0.0");
        }
        if self.transcription.silence_threshold > 1.0 {
            bail!("silence_threshold must be <= 1.0");
        }

        // Validate model name (basic check)
        if self.transcription.model.is_empty() {
            bail!("model name cannot be empty");
        }

        // Validate language code (basic check)
        if self.transcription.language.is_empty() {
            bail!("language code cannot be empty");
        }

        // Validate hotkeys are not empty
        if self.hotkeys.start_transcription.is_empty() {
            bail!("start_transcription hotkey cannot be empty");
        }
        if self.hotkeys.stop_transcription.is_empty() {
            bail!("stop_transcription hotkey cannot be empty");
        }

        Ok(())
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
