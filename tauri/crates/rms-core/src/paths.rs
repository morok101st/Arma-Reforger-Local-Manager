//! Ablageorte der App. Dieselben Ordner wie die C#-Vorgängerversion, damit beide dieselben Daten nutzen.

use std::path::{Path, PathBuf};

/// Name des Datenordners, gemeinsam mit der C#-Vorgängerversion.
pub const DATA_FOLDER: &str = "ReforgerModSync";

/// Ablageorte unter `%LOCALAPPDATA%\ReforgerModSync`. Tests übergeben einen eigenen Basisordner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub local_data: PathBuf,
}

impl AppPaths {
    pub fn new(local_data: impl Into<PathBuf>) -> Self {
        Self { local_data: local_data.into() }
    }

    /// `%LOCALAPPDATA%\ReforgerModSync`.
    pub fn default_for_user() -> Self {
        Self::new(local_app_data().join(DATA_FOLDER))
    }

    /// Je Lauf ein Ordner für die Logs des Spielclients.
    pub fn client_logs(&self) -> PathBuf {
        self.local_data.join("clientlogs")
    }

    /// Eigenes Profil des Spielclients für Liste und Download, die Einstellungen des Spielers bleiben unberührt.
    pub fn client_profile(&self) -> PathBuf {
        self.local_data.join("clientprofile")
    }

    pub fn mod_lists(&self) -> PathBuf {
        self.local_data.join("modlists")
    }

    pub fn logs(&self) -> PathBuf {
        self.local_data.join("logs")
    }
}

/// `%APPDATA%\ReforgerModSync\settings.json`, dieselbe Datei wie bei der C#-Vorgängerversion.
pub fn default_settings_file() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join(DATA_FOLDER).join("settings.json")
}

/// Ordner „Dokumente“ des Nutzers, auch wenn er umgeleitet ist (z. B. nach OneDrive).
pub fn documents_dir() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn local_app_data() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Entspricht `Path.GetFullPath`: macht einen Pfad absolut, ohne dass er existieren muss.
pub fn full_path(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
