use std::path::PathBuf;

use crate::paths::{documents_dir, full_path};

/// Der Client legt die Mods in `<Zielordner>\addons` ab. Nutzer wählen oft direkt den Ordner mit den Mods.
/// Diese Endung wird entfernt, damit der Client sie nicht doppelt anhängt.
pub fn normalize_download_dir(download_dir: Option<&str>) -> Option<String> {
    let trimmed = download_dir?.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut path = trim_separators(trimmed);
    if let Some((parent, name)) = path.rsplit_once(['\\', '/'])
        && name.eq_ignore_ascii_case("addons")
        && !parent.is_empty()
    {
        path = trim_separators(parent);
    }

    Some(if path.ends_with(':') { format!("{path}\\") } else { path.to_string() })
}

fn trim_separators(path: &str) -> &str {
    path.trim_end_matches(['\\', '/'])
}

/// Basisordner des Clients, wenn der Nutzer keinen Zielordner gewählt hat.
pub fn client_default_folder() -> PathBuf {
    documents_dir().join("My Games").join("ArmaReforger")
}

/// Vergleichswert: Gleich, wenn zwei Angaben denselben Zielordner meinen, auch über den Standardordner.
pub fn folder_key(download_dir: Option<&str>) -> String {
    let folder = normalize_download_dir(download_dir).map_or_else(client_default_folder, PathBuf::from);
    let full = full_path(&folder).to_string_lossy().into_owned();
    // Wie Path.TrimEndingDirectorySeparator: Das Laufwerk selbst behält seinen Backslash.
    let trimmed = if full.len() > 3 { full.trim_end_matches(['\\', '/']) } else { &full };
    trimmed.to_uppercase()
}

pub fn is_default_folder(download_dir: Option<&str>) -> bool {
    folder_key(download_dir) == folder_key(None)
}

/// Ordner, der die mitgelieferte Hilfs-Mod enthält: `helper` neben der EXE.
pub fn helper_root() -> PathBuf {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(|folder| folder.join("helper"))).unwrap_or_else(|| PathBuf::from("helper"))
}

/// Ordner, in dem die Mod-Ordner liegen. Beobachtet an einer echten Installation.
pub fn resolve_mods_root(download_dir: Option<&str>) -> PathBuf {
    normalize_download_dir(download_dir).map_or_else(client_default_folder, PathBuf::from).join("addons")
}
