//! Mods des Servers und die lokal installierten Mods.

mod comparer;
mod paths;
mod scanner;
mod summary;

pub use comparer::{ModComparison, ModState, compare, compare_versions};
pub use paths::{client_default_folder, folder_key, helper_root, is_default_folder, normalize_download_dir, resolve_mods_root};
pub use scanner::{AddonHealth, InstalledAddon, id_of_folder, scan, scan_only};
pub use summary::{ModSummary, sort_open_first, state_text};

use serde::{Deserialize, Serialize};

/// Eine Mod aus der Liste des Servers. Ohne Version zählt jede vollständige lokale Version als aktuell.
///
/// Die Feldnamen entsprechen dem Zwischenspeicher der WPF-App (`modlists\<id>.json`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

impl ModEntry {
    pub fn new(id: &str, name: &str, version: Option<&str>) -> Self {
        Self { id: id.into(), name: name.into(), version: version.map(String::from) }
    }
}
