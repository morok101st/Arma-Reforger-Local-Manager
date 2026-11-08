//! Einstellungen pro Windows-Benutzer, im selben JSON-Format wie die WPF-App.
//!
//! Beide Ausgaben teilen sich die Datei. Felder, die diese Ausgabe nicht kennt, bleiben beim Speichern
//! erhalten. Die WPF-App verwirft unbekannte Felder, deshalb fügt diese Ausgabe keine eigenen hinzu.

mod basic;
mod profile_input;
mod sanitizer;
mod store;

pub use basic::{
    BasicSettingsField, BasicSettingsInput, BasicSettingsResult, MAX_IDLE_MINUTES, MAX_TOTAL_MINUTES, MIN_IDLE_MINUTES, MIN_TOTAL_MINUTES,
    apply_basic_settings,
};
pub use profile_input::validate_profile_input;
pub use sanitizer::{MAX_FPS, MAX_HEIGHT, MAX_WIDTH, MIN_FPS, MIN_HEIGHT, MIN_WIDTH, sanitize};
pub use store::{SettingsError, SettingsStore};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

/// Unbekannte Felder aus der Datei, damit sie beim Speichern nicht verloren gehen.
pub type ExtraFields = Map<String, Value>;

/// Einstellungen pro Windows-Benutzer. Enthält nie Geheimnisse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    #[serde(deserialize_with = "null_as_default")]
    pub profiles: Vec<ServerProfile>,
    pub selected_profile_id: Option<String>,

    /// `None`: Client automatisch über Steam erkennen.
    pub reforger_exe: Option<String>,

    #[serde(deserialize_with = "null_as_default")]
    pub client: ClientWindowSettings,
    #[serde(deserialize_with = "null_as_default")]
    pub timeouts: TimeoutSettings,

    /// Zielordner, in denen der Sync überzählige Mods löschen darf. Der Nutzer stimmt je Ordner einmal zu.
    #[serde(deserialize_with = "null_as_default")]
    pub cleanup_approved_folders: Vec<String>,

    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            selected_profile_id: None,
            reforger_exe: None,
            client: ClientWindowSettings::default(),
            timeouts: TimeoutSettings::default(),
            cleanup_approved_folders: Vec::new(),
            extra: ExtraFields::new(),
        }
    }
}

/// Ein Spielserver mit eigenem Zielordner. Sein Passwort liegt im Anmeldeinformationsspeicher.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerProfile {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,

    /// IP und Port, z. B. `203.0.113.10:2001`. Die Mod-Liste kommt von diesem Server.
    pub server_address: Option<String>,

    /// `None`: Standardordner des Clients nutzen.
    pub download_dir: Option<String>,

    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl ServerProfile {
    pub fn new(id: &str, name: &str) -> Self {
        Self { id: id.into(), name: name.into(), ..Self::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientWindowSettings {
    pub width: i32,
    pub height: i32,
    #[serde(rename = "maxFps")]
    pub max_fps: i32,

    /// Spielfenster beim Sync ausblenden. `false` hilft bei der Fehlersuche.
    pub hide_window: bool,

    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Default for ClientWindowSettings {
    fn default() -> Self {
        Self { width: 640, height: 360, max_fps: 15, hide_window: true, extra: ExtraFields::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TimeoutSettings {
    pub total_minutes: i32,
    pub idle_minutes: i32,

    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Default for TimeoutSettings {
    fn default() -> Self {
        Self { total_minutes: 120, idle_minutes: 10, extra: ExtraFields::new() }
    }
}

impl TimeoutSettings {
    pub fn new(total_minutes: i32, idle_minutes: i32) -> Self {
        Self { total_minutes, idle_minutes, ..Self::default() }
    }
}

/// `null` in der Datei gilt wie ein fehlender Wert, so wie bei der WPF-App nach der Bereinigung.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}
