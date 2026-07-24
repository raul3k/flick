//! User settings, persisted as TOML under the platform config directory
//! (`~/.config/flick/config.toml` on Linux).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Either [`AUTO`] to follow the system, or one of the codes in
    /// [`crate::i18n::LANGUAGES`].
    pub language: String,
    /// One of the codes in [`crate::palette::PALETTES`].
    pub palette: String,
}

/// Language setting meaning "whatever the system is set to".
pub const AUTO: &str = "auto";

impl Default for Config {
    fn default() -> Self {
        Self {
            language: AUTO.into(),
            palette: crate::palette::DEFAULT.into(),
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("flick").join("config.toml"))
}

impl Config {
    /// Reads the config, falling back to defaults when it is missing or
    /// unreadable. Settings are a convenience, so a broken file must never
    /// stop the app from starting.
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_err()
        {
            return;
        }
        if let Ok(text) = toml::to_string_pretty(self) {
            std::fs::write(path, text).ok();
        }
    }
}
