//! Findet Steam und den Spielclient und prüft, ob beide laufen.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};

use crate::paths::full_path;
use crate::win::{OwnedHandle, pcwstr, wide};

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

/// Client aus den Einstellungen, sonst über Steam erkannt. `None`: nicht gefunden.
pub fn resolve_client_exe(configured: Option<&str>) -> Option<PathBuf> {
    match configured {
        Some(configured) => Some(PathBuf::from(configured)),
        None => find_client_exe(&read_steam_path_from_registry()?),
    }
}

/// `HKCU\Software\Valve\Steam\SteamPath`.
pub fn read_steam_path_from_registry() -> Option<PathBuf> {
    let key = wide(r"Software\Valve\Steam");
    let value = wide("SteamPath");
    let mut buffer = vec![0u16; 1024];
    let mut size = u32::try_from(buffer.len() * 2).expect("klein");
    // SAFETY: Puffer und Größe passen zusammen, der Wert wird als nullterminierter Text gelesen.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            pcwstr(&key),
            pcwstr(&value),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&raw mut size),
        )
    };
    if result != ERROR_SUCCESS {
        return None;
    }

    let length = buffer.iter().position(|&unit| unit == 0).unwrap_or(buffer.len());
    let path = String::from_utf16_lossy(&buffer[..length]);
    (!path.is_empty()).then(|| full_path(Path::new(&path)))
}

pub fn is_steam_running() -> bool {
    is_process_running("steam.exe")
}

pub fn is_game_running() -> bool {
    is_process_running(CLIENT_EXE_NAME)
}

fn is_process_running(exe_name: &str) -> bool {
    // SAFETY: Schnappschuss der Prozessliste, das Handle schließt die Hülle.
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return false;
    };
    let snapshot = OwnedHandle(snapshot);
    let mut entry = PROCESSENTRY32W { dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).expect("klein"), ..Default::default() };
    // SAFETY: `entry` hat die angegebene Größe.
    let mut more = unsafe { Process32FirstW(snapshot.0, &raw mut entry) }.is_ok();
    while more {
        let length = entry.szExeFile.iter().position(|&unit| unit == 0).unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..length]).eq_ignore_ascii_case(exe_name) {
            return true;
        }
        // SAFETY: wie oben.
        more = unsafe { Process32NextW(snapshot.0, &raw mut entry) }.is_ok();
    }
    false
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
