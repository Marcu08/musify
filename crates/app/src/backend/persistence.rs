use std::path::{Path, PathBuf};

use musify_core::RepeatMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) music_folders: Vec<PathBuf>,
    pub(crate) volume: u16,
    pub(crate) shuffle: bool,
    pub(crate) repeat: RepeatMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_folders: Vec::new(),
            volume: 80,
            shuffle: false,
            repeat: RepeatMode::Off,
        }
    }
}

pub(crate) fn app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MUSIFY_DATA_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Musify")
}

pub(crate) fn env_folders() -> Option<Vec<PathBuf>> {
    let value = std::env::var("MUSIFY_MUSIC_DIRS").ok()?;
    let folders: Vec<PathBuf> = value
        .split(';')
        .map(str::trim)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .collect();
    (!folders.is_empty()).then_some(folders)
}

pub(crate) fn load_config(path: &Path) -> Config {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Config::default();
    };
    match toml::from_str(&text) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "invalid config, using defaults");
            Config::default()
        }
    }
}

pub(crate) fn save_config(path: &Path, config: &Config) -> anyhow::Result<()> {
    let text = toml::to_string_pretty(config)?;
    let temporary = path.with_extension("toml.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}
