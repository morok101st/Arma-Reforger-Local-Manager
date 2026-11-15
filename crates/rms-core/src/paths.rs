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

/// Ordner „Downloads“ des Nutzers, auch wenn er umgeleitet ist. Dort landen Importdateien aus ARMM.
pub fn downloads_dir() -> Option<PathBuf> {
    dirs::download_dir()
}

/// Ordner „Dokumente“ des Nutzers, auch wenn er umgeleitet ist (z. B. nach OneDrive).
pub fn documents_dir() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// WebView2-Ordner der Vorschau (bis 0.4), heute heißt er `de.morok.reforgersynclaunch`.
pub const LEGACY_WEBVIEW_FOLDER: &str = "de.morok.reforgersynclaunch.tauri";

/// Entfernt den WebView2-Ordner der Vorschau. Darin liegt nur der Zwischenspeicher der Oberfläche.
pub fn remove_legacy_webview_data() {
    remove_legacy_webview_data_in(&local_app_data());
}

/// Wie [`remove_legacy_webview_data`], mit eigenem Basisordner für Tests. Fehler werden ignoriert,
/// etwa wenn die Vorschau gerade noch läuft. Dann klappt es beim nächsten Start.
pub fn remove_legacy_webview_data_in(local_app_data: &Path) {
    let folder = local_app_data.join(LEGACY_WEBVIEW_FOLDER);
    if folder.is_dir() {
        let _ = std::fs::remove_dir_all(folder);
    }
}

/// Löscht alle Daten der App: Einstellungen unter `%APPDATA%\ReforgerModSync` und Mod-Listen, Logs und
/// Profil des Clients unter `%LOCALAPPDATA%\ReforgerModSync`. Die geladenen Mods bleiben. Für die Deinstallation.
pub fn remove_user_data() -> Vec<String> {
    let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    remove_user_data_in(&config, &local_app_data())
}

/// Wie [`remove_user_data`], mit eigenen Basisordnern für Tests. Liefert, was nicht gelöscht werden konnte.
pub fn remove_user_data_in(config: &Path, local_app_data: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for folder in [config.join(DATA_FOLDER), local_app_data.join(DATA_FOLDER)] {
        if folder.is_dir()
            && let Err(error) = std::fs::remove_dir_all(&folder)
        {
            problems.push(format!("{}: {error}", folder.display()));
        }
    }
    problems
}

fn local_app_data() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Entspricht `Path.GetFullPath`: macht einen Pfad absolut, ohne dass er existieren muss.
pub fn full_path(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
