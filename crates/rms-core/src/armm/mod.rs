//! Verbindung zu ARMM, dem Mod-Manager der Betreiber. Ein importierter Server holt seine Mod-Liste
//! per HTTPS aus einem freigegebenen Modset statt vom Server-Eintrag bei Bohemia.

mod fetch;
mod import;
mod modset;

pub use fetch::{FetchError, ModsetSource, SharedModsetSource, WinHttpModsetSource, host_and_port, request_path, status_error};
pub use import::{ArmmImport, ImportError, ImportedGameServer, MAX_IMPORT_BYTES, normalize_base_url, parse_sync_file};
pub use modset::{MAX_MODSET_BYTES, ModsetError, parse_modset};

use serde::{Deserialize, Serialize};

/// Wert von `format` in einer `armm-sync.json`.
pub const SYNC_FORMAT: &str = "armm-sync";

/// Höchste `formatVersion`, die diese App versteht.
pub const SYNC_FORMAT_VERSION: u64 = 1;

/// Gespeicherte Verbindung eines importierten Servers. Der Zugangsschlüssel liegt nicht hier,
/// sondern im Anmeldeinformationsspeicher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmmLink {
    /// `https://host` oder `https://host:port`, ohne `/` am Ende.
    pub base_url: String,
    pub modset_id: u64,
}

impl ArmmLink {
    /// Adresse, unter der ARMM die Mod-Liste des Modsets liefert.
    pub fn modset_url(&self) -> String {
        format!("{}/api/sync/modsets/{}", self.base_url, self.modset_id)
    }

    /// Nur der Host mit Port, für Anzeige und Logs.
    pub fn host(&self) -> &str {
        self.base_url.strip_prefix("https://").unwrap_or(&self.base_url)
    }

    /// Dieselbe Verbindung, etwa bei einem erneuten Import.
    pub fn same_as(&self, other: &Self) -> bool {
        self.base_url.eq_ignore_ascii_case(&other.base_url) && self.modset_id == other.modset_id
    }
}
