//! Portable settings with explicit, journaled changes and no silent rewrites.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub editor: EditorSettings,
    pub appearance: AppearanceSettings,
    pub ai: AiSettings,
    pub keymap: Keymap,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct EditorSettings {
    pub font_family: String,
    pub font_size: u16,
    pub tab_size: u8,
    pub format_on_save: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AiSettings {
    pub local_only: bool,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Keymap {
    #[default]
    Apex,
    VsCode,
    JetBrains,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            editor: EditorSettings::default(),
            appearance: AppearanceSettings::default(),
            ai: AiSettings::default(),
            keymap: Keymap::Apex,
        }
    }
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_family: "monospace".into(),
            font_size: 17,
            tab_size: 4,
            format_on_save: false,
        }
    }
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "Apex Dark".into(),
        }
    }
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            local_only: true,
            provider: "local".into(),
            model: String::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("settings TOML is invalid: {0}")]
    Decode(#[from] toml::de::Error),
    #[error("settings could not be encoded: {0}")]
    Encode(#[from] toml::ser::Error),
}

#[derive(Clone, Debug)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Missing settings resolve to defaults in memory without creating a file.
    pub fn load(&self) -> Result<Settings, SettingsError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Persist a user-approved settings snapshot and journal why it changed.
    pub fn apply(&self, settings: &Settings, reason: &str) -> Result<(), SettingsError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encoded = toml::to_string_pretty(settings)?;
        let temporary = self.path.with_extension("toml.apex-new");
        fs::write(&temporary, encoded)?;
        fs::rename(&temporary, &self.path)?;
        self.journal(reason)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn journal(&self, reason: &str) -> Result<(), SettingsError> {
        let journal = self.path.with_extension("changes.log");
        let mut file = OpenOptions::new().create(true).append(true).open(journal)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        writeln!(file, "{timestamp}\t{reason}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "apex-settings-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn defaults_do_not_silently_create_configuration() {
        let directory = test_directory();
        let store = SettingsStore::new(directory.join("settings.toml"));
        assert_eq!(store.load().unwrap(), Settings::default());
        assert!(!store.path().exists());
    }

    #[test]
    fn explicit_change_round_trips_and_is_journaled() {
        let directory = test_directory();
        let store = SettingsStore::new(directory.join("settings.toml"));
        let settings = Settings {
            keymap: Keymap::JetBrains,
            ..Settings::default()
        };
        store
            .apply(&settings, "user selected JetBrains keymap")
            .unwrap();
        assert_eq!(store.load().unwrap(), settings);
        let journal = fs::read_to_string(directory.join("settings.changes.log")).unwrap();
        assert!(journal.contains("user selected JetBrains keymap"));
        fs::remove_dir_all(directory).unwrap();
    }
}
