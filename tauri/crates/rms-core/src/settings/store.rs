use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{AppSettings, sanitize};
use crate::paths::full_path;

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// Die Datei ist kein gültiges JSON. Sie bleibt unverändert.
    #[error("Die Einstellungsdatei „{}“ ist beschädigt.", .path.display())]
    Damaged { path: PathBuf, source: serde_json::Error },

    #[error("Die Einstellungsdatei „{}“ konnte nicht gelesen oder geschrieben werden: {source}", .path.display())]
    Io { path: PathBuf, source: io::Error },
}

pub struct SettingsStore {
    file_path: PathBuf,
}

impl SettingsStore {
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
        Self { file_path: file_path.into() }
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Lädt die Einstellungen. Fehlt die Datei, gelten die Standardwerte, ungültige Werte werden ersetzt.
    pub fn load(&self) -> Result<AppSettings, SettingsError> {
        let text = match fs::read_to_string(&self.file_path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(AppSettings::default()),
            Err(source) => return Err(self.io_error(source)),
        };

        // Die C#-Vorgängerversion schreibt eine BOM nicht, ein Editor vielleicht schon.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let settings: Option<AppSettings> =
            serde_json::from_str(text).map_err(|source| SettingsError::Damaged { path: self.file_path.clone(), source })?;
        Ok(settings.map(sanitize).unwrap_or_default())
    }

    /// Schreibt erst in eine Temp-Datei, damit ein Absturz die Einstellungen nicht zerstört.
    pub fn save(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        if let Some(folder) = full_path(&self.file_path).parent() {
            fs::create_dir_all(folder).map_err(|source| self.io_error(source))?;
        }

        let json = serde_json::to_string_pretty(settings).expect("Einstellungen lassen sich immer als JSON schreiben");
        let mut temp_path = self.file_path.clone().into_os_string();
        temp_path.push(".tmp");
        fs::write(&temp_path, json).map_err(|source| self.io_error(source))?;
        fs::rename(&temp_path, &self.file_path).map_err(|source| self.io_error(source))
    }

    fn io_error(&self, source: io::Error) -> SettingsError {
        SettingsError::Io { path: self.file_path.clone(), source }
    }
}
