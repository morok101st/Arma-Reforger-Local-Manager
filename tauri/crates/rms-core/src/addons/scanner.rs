//! Liest die vom Client gepflegte Datei `meta` jedes Mod-Ordners, ohne etwas zu verändern.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AddonHealth {
    Ok,

    /// Eine Datei fehlt oder hat die falsche Größe, z. B. nach einem abgebrochenen Download.
    Incomplete,

    /// Der Client selbst hat die Mod als beschädigt markiert.
    Corrupted,

    /// `meta` fehlt, ist unlesbar oder gehört zu einer anderen Mod.
    UnknownMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledAddon {
    pub id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub folder: PathBuf,
    pub health: AddonHealth,
}

static ADDON_FOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new("^.+_(?<id>[0-9A-Fa-f]{16})$").unwrap());

/// Alle Mods im Ordner. Ein fehlender Ordner heißt: keine Mods.
pub fn scan(mods_root: &Path) -> Vec<InstalledAddon> {
    scan_filtered(mods_root, None)
}

/// Nur diese Mods lesen. Spart während eines Downloads das Lesen aller übrigen Mods.
pub fn scan_only(mods_root: &Path, only_ids: &HashSet<String>) -> Vec<InstalledAddon> {
    scan_filtered(mods_root, Some(only_ids))
}

fn scan_filtered(mods_root: &Path, only_ids: Option<&HashSet<String>>) -> Vec<InstalledAddon> {
    let Ok(entries) = fs::read_dir(mods_root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let folder = entry.path();
            let id = id_of_folder(&folder)?;
            only_ids.is_none_or(|ids| ids.contains(&id)).then(|| read_addon(folder, id))
        })
        .collect()
}

/// Die Mod-ID aus einem Ordnernamen wie `Name_0123456789ABCDEF`, sonst `None`.
pub fn id_of_folder(folder: &Path) -> Option<String> {
    let name = folder.file_name()?.to_string_lossy();
    ADDON_FOLDER.captures(&name).map(|captures| captures["id"].to_string())
}

fn read_addon(folder: PathBuf, folder_id: String) -> InstalledAddon {
    let unknown = |name: Option<String>, folder: PathBuf, id: String| InstalledAddon {
        id,
        name,
        version: None,
        folder,
        health: AddonHealth::UnknownMetadata,
    };

    // Der Client schreibt während des Downloads in diese Datei. Rust öffnet Dateien unter Windows
    // mit Freigabe für Lesen, Schreiben und Löschen, das Tool blockiert ihn also nie.
    let Some(meta) = fs::read(folder.join("meta")).ok().and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok()) else {
        return unknown(None, folder, folder_id);
    };

    match read_meta(&meta, &folder, &folder_id) {
        Some(Meta::Selected { name, version, health }) => InstalledAddon { id: folder_id, name, version, folder, health },
        Some(Meta::NoSelection { name }) => unknown(name, folder, folder_id),
        None => unknown(None, folder, folder_id),
    }
}

enum Meta {
    Selected { name: Option<String>, version: Option<String>, health: AddonHealth },
    NoSelection { name: Option<String> },
}

/// `None`: Aufbau passt nicht, die Mod gilt als unbekannt.
fn read_meta(document: &Value, folder: &Path, folder_id: &str) -> Option<Meta> {
    let meta = document.get("meta")?;
    if !meta.get("id")?.as_str()?.eq_ignore_ascii_case(folder_id) {
        return None;
    }

    let name = optional_string(meta.get("name"))?;
    let selected = match meta.get("selectedRev") {
        None => 0,
        Some(value) => i32::try_from(value.as_i64()?).ok()?,
    };
    let versions = meta.get("versions")?.as_array()?;
    let Some(version) = usize::try_from(selected).ok().and_then(|index| versions.get(index)) else {
        return Some(Meta::NoSelection { name });
    };

    let version_text = optional_string(version.get("version"))?;
    let health = check_health(version, folder)?;
    Some(Meta::Selected { name, version: version_text, health })
}

/// Ein fehlender oder `null`-Wert ist erlaubt, ein anderer Typ nicht.
fn optional_string(value: Option<&Value>) -> Option<Option<String>> {
    match value {
        None | Some(Value::Null) => Some(None),
        Some(Value::String(text)) => Some(Some(text.clone())),
        Some(_) => None,
    }
}

fn flag(value: &Value, key: &str) -> Option<Option<bool>> {
    match value.get(key) {
        None => Some(None),
        Some(Value::Bool(flag)) => Some(Some(*flag)),
        Some(_) => None,
    }
}

fn check_health(version: &Value, folder: &Path) -> Option<AddonHealth> {
    if flag(version, "corrupted")? == Some(true) {
        return Some(AddonHealth::Corrupted);
    }

    let files = version.get("package")?.get("files")?.as_array()?;
    if files.is_empty() {
        return Some(AddonHealth::Incomplete);
    }

    for file in files {
        if flag(file, "isCorrupted")? == Some(true) {
            return Some(AddonHealth::Corrupted);
        }

        // Bewusst nicht filePath aus meta: Der Pfad ist absolut und stimmt nach einem Umzug nicht mehr.
        let name = optional_string(file.get("name"))?.unwrap_or_default();
        if name.is_empty() || name.contains(['\\', '/', ':']) {
            return Some(AddonHealth::UnknownMetadata);
        }

        let size = file.get("size")?.as_i64()?;
        let on_disk = fs::metadata(folder.join(&name)).ok().filter(|info| info.is_file());
        if on_disk.is_none_or(|info| i64::try_from(info.len()).ok() != Some(size)) || flag(file, "isDownloaded")? == Some(false) {
            return Some(AddonHealth::Incomplete);
        }
    }

    Some(AddonHealth::Ok)
}
