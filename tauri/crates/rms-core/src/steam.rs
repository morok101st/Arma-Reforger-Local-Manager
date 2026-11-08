//! Findet den Spielclient in den Steam-Bibliotheken. Registry und Prozessliste folgen mit den Windows-Teilen.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::paths::full_path;

const REFORGER_APP_ID: &str = "1874880";
pub const CLIENT_EXE_NAME: &str = "ArmaReforgerSteam.exe";

static LIBRARY_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)"path"\s+"(?<value>(?:[^"\\]|\\.)*)""#).unwrap());
static INSTALL_DIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)"installdir"\s+"(?<value>(?:[^"\\]|\\.)*)""#).unwrap());

/// Prüft einen von Hand eingetragenen Client-Pfad. `None`: in Ordnung.
pub fn check_client_exe(path: &Path) -> Option<String> {
    let name_matches = path.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(CLIENT_EXE_NAME));
    if !name_matches {
        return Some(format!("Bitte die Datei {CLIENT_EXE_NAME} aus dem Spielordner wählen."));
    }

    if path.is_file() { None } else { Some("Die Datei gibt es nicht.".into()) }
}

/// Durchsucht alle Steam-Bibliotheken nach dem Reforger-Client.
pub fn find_client_exe(steam_path: &Path) -> Option<PathBuf> {
    library_paths(steam_path).into_iter().find_map(|library| {
        let steam_apps = library.join("steamapps");
        let manifest = fs::read_to_string(steam_apps.join(format!("appmanifest_{REFORGER_APP_ID}.acf"))).ok()?;
        let install_dir = read_values(&manifest, &INSTALL_DIR).into_iter().next()?;
        let exe = steam_apps.join("common").join(install_dir).join(CLIENT_EXE_NAME);
        exe.is_file().then_some(exe)
    })
}

/// Steam-Ordner selbst und alle Bibliotheken aus `libraryfolders.vdf`, ohne doppelte Einträge.
pub fn library_paths(steam_path: &Path) -> Vec<PathBuf> {
    let from_file = fs::read_to_string(steam_path.join("steamapps").join("libraryfolders.vdf"))
        .map(|vdf| read_values(&vdf, &LIBRARY_PATH))
        .unwrap_or_default();

    let mut seen = HashSet::new();
    std::iter::once(steam_path.to_path_buf())
        .chain(from_file.into_iter().map(PathBuf::from))
        .map(|path| full_path(&path))
        .filter(|path| seen.insert(path.to_string_lossy().to_uppercase()))
        .collect()
}

fn read_values(vdf: &str, regex: &Regex) -> Vec<String> {
    regex.captures_iter(vdf).map(|captures| unescape(&captures["value"])).collect()
}

/// Steam maskiert in seinen Dateien `\` und `"` mit einem Backslash.
fn unescape(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            match characters.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some(next) => result.push(next),
                None => result.push('\\'),
            }
        } else {
            result.push(character);
        }
    }
    result
}
