//! Liste für den Modus `-rmsModset` der Hilfs-Mod. Sie steht in einer Datei im Client-Profil, weil Namen
//! Leerzeichen enthalten und der Client seine Startparameter ins Log schreibt. Sie enthält nichts Geheimes.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::addons::ModEntry;

const FILE_NAME: &str = "rms_modset.txt";

/// `$profile:` der Hilfs-Mod im eigenen Client-Profil der App.
pub fn for_client_profile(client_profile_dir: &Path) -> PathBuf {
    client_profile_dir.join("profile").join(FILE_NAME)
}

/// Eine Zeile je Mod: `id|version|name|get` (laden) oder `|have` (nur als Anker für nicht gelistete Mods).
/// Die ganze Liste kommt hinein, denn ein Anker liegt oft schon aktuell im Ordner.
pub fn render(required: &[ModEntry], open_ids: &HashSet<String>) -> String {
    let mut text = String::new();
    for mod_entry in required {
        let kind = if open_ids.contains(&mod_entry.id.to_uppercase()) { "get" } else { "have" };
        let version = mod_entry.version.as_deref().unwrap_or_default();
        text.push_str(&format!("{}|{version}|{}|{kind}\n", mod_entry.id, mod_entry.name));
    }
    text
}

pub fn write(path: &Path, required: &[ModEntry], open_ids: &HashSet<String>) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, render(required, open_ids))
}
