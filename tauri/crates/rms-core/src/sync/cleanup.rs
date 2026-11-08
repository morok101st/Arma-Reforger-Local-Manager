//! Hält einen eigenen Zielordner deckungsgleich mit den Servern, die ihn nutzen: Gelöscht werden nur Mods, die
//! keiner dieser Server braucht. Fehlt auch nur eine Liste, wird nichts gelöscht, denn ein Fehlgriff kostet einen
//! neuen Download. Der Standardordner bleibt unberührt, dort liegen auch Mods, die das Spiel selbst geladen hat.

use std::collections::HashSet;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::ModListCache;
use super::watcher::folder_bytes;
use crate::addons::{AddonHealth, InstalledAddon, ModEntry, folder_key, is_default_folder, resolve_mods_root, scan};
use crate::settings::ServerProfile;

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurplusMod {
    pub name: String,
    pub folder: PathBuf,
    pub bytes: u64,
}

/// `blocker`: Warum gerade nicht abgeglichen werden kann. Dann wird nichts gelöscht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub folder_key: String,
    pub mods_root: PathBuf,
    pub surplus: Vec<SurplusMod>,
    pub blocker: Option<String>,
}

impl CleanupPlan {
    pub fn total_bytes(&self) -> u64 {
        self.surplus.iter().map(|mod_entry| mod_entry.bytes).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupResult {
    pub deleted: Vec<SurplusMod>,
    pub failed: Vec<SurplusMod>,
}

impl CleanupResult {
    /// Ergebnis für Status und Protokoll, z. B. „3 überzählige Mods gelöscht (1,2 GB frei).“
    pub fn summary(&self) -> String {
        let freed = format_size(self.deleted.iter().map(|mod_entry| mod_entry.bytes).sum());
        let text = if self.deleted.len() == 1 {
            format!("1 überzählige Mod gelöscht ({freed} frei).")
        } else {
            format!("{} überzählige Mods gelöscht ({freed} frei).", self.deleted.len())
        };
        if self.failed.is_empty() {
            text
        } else {
            let names: Vec<&str> = self.failed.iter().map(|mod_entry| mod_entry.name.as_str()).collect();
            format!("{text} Nicht löschbar: {}.", names.join(", "))
        }
    }
}

pub fn plan_cleanup(cache: &ModListCache, current: &ServerProfile, current_mods: &[ModEntry], profiles: &[ServerProfile]) -> CleanupPlan {
    let download_dir = current.download_dir.as_deref();
    let key = folder_key(download_dir);
    let mods_root = resolve_mods_root(download_dir);
    let plan = |surplus, blocker| CleanupPlan { folder_key: key.clone(), mods_root: mods_root.clone(), surplus, blocker };
    if is_default_folder(download_dir) {
        return plan(Vec::new(), None);
    }

    // Eine leere Liste ist eher ein Fehler als ein Server ohne Mods und würde den ganzen Ordner leeren.
    if current_mods.is_empty() {
        return plan(Vec::new(), Some(format!("Der Server „{}“ meldet keine Mods, deshalb wird nichts gelöscht.", current.name)));
    }

    let mut required: HashSet<String> = current_mods.iter().map(|mod_entry| mod_entry.id.to_uppercase()).collect();
    for other in profiles.iter().filter(|profile| profile.id != current.id && folder_key(profile.download_dir.as_deref()) == key) {
        match cache.load(&other.id) {
            Some(list) if !list.mods.is_empty() => required.extend(list.mods.iter().map(|mod_entry| mod_entry.id.to_uppercase())),
            _ => {
                return plan(
                    Vec::new(),
                    Some(format!(
                        "Überzählige Mods wurden nicht gelöscht: Für den Server „{}“ mit demselben Zielordner fehlt die Mod-Liste. Bitte dort „Neu prüfen“ klicken.",
                        other.name
                    )),
                );
            }
        }
    }

    let surplus = select_surplus(scan(&mods_root), &required)
        .into_iter()
        .filter(|addon| !is_link(&addon.folder))
        .map(|addon| SurplusMod {
            name: addon.name.clone().unwrap_or_else(|| addon.folder.file_name().unwrap_or_default().to_string_lossy().into_owned()),
            bytes: folder_bytes(&addon.folder),
            folder: addon.folder,
        })
        .collect();
    plan(surplus, None)
}

/// Ein Ordner ohne lesbares `meta` ist nicht sicher eine Mod und bleibt deshalb liegen.
/// `required_ids` in Großbuchstaben.
pub fn select_surplus(installed: Vec<InstalledAddon>, required_ids: &HashSet<String>) -> Vec<InstalledAddon> {
    let mut surplus: Vec<InstalledAddon> = installed
        .into_iter()
        .filter(|addon| addon.health != AddonHealth::UnknownMetadata && !required_ids.contains(&addon.id.to_uppercase()))
        .collect();
    surplus.sort_by_key(|addon| addon.name.clone().unwrap_or_default().to_lowercase());
    surplus
}

/// Eine Mod, die sich nicht löschen lässt, hält die übrigen nicht auf.
pub fn delete_surplus(plan: &CleanupPlan) -> CleanupResult {
    let (deleted, failed) = plan.surplus.iter().cloned().partition(|mod_entry| fs::remove_dir_all(&mod_entry.folder).is_ok());
    CleanupResult { deleted, failed }
}

/// Größe mit deutschem Dezimalkomma, z. B. „1,5 GB“.
pub fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    #[allow(clippy::cast_precision_loss)]
    let bytes = bytes as f64;
    let (value, unit) = if bytes >= MB * 1024.0 { (bytes / (MB * 1024.0), "GB") } else { (bytes / MB, "MB") };
    format!("{value:.1} {unit}").replace('.', ",")
}

fn is_link(folder: &Path) -> bool {
    fs::symlink_metadata(folder).is_ok_and(|info| info.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
}
