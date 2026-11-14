//! Muster stammen aus echten Client-Logs und den `[RMS]`-Zeilen der Hilfs-Mod.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::addons::ModEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerListEnd {
    Done,
    NotFound,
    PasswordRequired,
    PasswordRejected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientLogEvent {
    None,
    Activity,
    BackendError,
    ModFailed,
    HelperDone,
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("Muster ist gültig")
}

static LIST_DONE_COUNT: LazyLock<Regex> = LazyLock::new(|| regex(r"\[RMS\] LIST_DONE mods=(?<count>[0-9]+)"));
static SERVER_MOD: LazyLock<Regex> = LazyLock::new(|| regex(r"\[RMS\] SERVER_MOD (?<id>[0-9A-Fa-f]{16}) (?<version>\S*) (?<name>.*)$"));
static SERVER_LIST_END: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"\[RMS\] (?<end>LIST_DONE|ROOM_NOT_FOUND|ROOM_PASSWORD_REQUIRED|ROOM_PASSWORD_REJECTED|ROOM_SEARCH_FAILED|ROOM_LIST_FAILED)\b")
});
static HELPER_DONE: LazyLock<Regex> = LazyLock::new(|| regex(r"\[RMS\] DONE\b"));
static MOD_FAILED: LazyLock<Regex> = LazyLock::new(|| regex(r"\[RMS\] MOD_FAILED (?<id>[0-9A-Fa-f]{16})(?<reason>.*)$"));
static BACKEND_DOWNLOAD: LazyLock<Regex> = LazyLock::new(|| regex(r"BACKEND\s*: (Addon Download started|Downloading|Updating) "));
static BACKEND_ERROR: LazyLock<Regex> = LazyLock::new(|| regex(r"BACKEND\s*\(E\):"));

pub fn classify(line: &str) -> ClientLogEvent {
    if HELPER_DONE.is_match(line) {
        ClientLogEvent::HelperDone
    } else if MOD_FAILED.is_match(line) {
        ClientLogEvent::ModFailed
    } else if line.contains("[RMS] ") || BACKEND_DOWNLOAD.is_match(line) {
        ClientLogEvent::Activity
    } else if BACKEND_ERROR.is_match(line) {
        ClientLogEvent::BackendError
    } else {
        ClientLogEvent::None
    }
}

/// Liefert die ID und den Grund einer Zeile `[RMS] MOD_FAILED id grund`.
pub fn parse_mod_failed(line: &str) -> Option<(String, String)> {
    let captures = MOD_FAILED.captures(line)?;
    Some((captures["id"].to_string(), captures["reason"].trim().to_string()))
}

/// Liest eine Zeile `[RMS] SERVER_MOD id version name`. Die Version kann leer sein.
pub fn parse_server_mod(line: &str) -> Option<ModEntry> {
    let captures = SERVER_MOD.captures(line)?;
    let version = &captures["version"];
    Some(ModEntry::new(&captures["id"], captures["name"].trim(), (!version.is_empty()).then_some(version)))
}

/// Wertet das Endergebnis eines Listenabrufs aus. `None`: noch kein Ergebnis in dieser Zeile.
pub fn parse_server_list_end(line: &str) -> Option<ServerListEnd> {
    let captures = SERVER_LIST_END.captures(line)?;
    Some(match &captures["end"] {
        "LIST_DONE" => ServerListEnd::Done,
        "ROOM_NOT_FOUND" => ServerListEnd::NotFound,
        "ROOM_PASSWORD_REQUIRED" => ServerListEnd::PasswordRequired,
        "ROOM_PASSWORD_REJECTED" => ServerListEnd::PasswordRejected,
        _ => ServerListEnd::Failed,
    })
}

/// Die Anzahl aus `[RMS] LIST_DONE mods=n`. Sie zeigt, ob die Liste vollständig gelesen wurde.
pub fn parse_list_done_count(line: &str) -> Option<usize> {
    LIST_DONE_COUNT.captures(line)?["count"].parse().ok()
}

/// Liest neue vollständige Zeilen einer Datei, die der Client gerade schreibt.
pub struct LogTail {
    path: PathBuf,
    position: u64,
    /// Rest ohne Zeilenende, als Bytes, damit ein geteiltes UTF-8-Zeichen nicht verloren geht.
    partial: Vec<u8>,
}

impl LogTail {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), position: 0, partial: Vec::new() }
    }

    pub fn read_new_lines(&mut self) -> Vec<String> {
        // Rust öffnet unter Windows mit Freigabe für Lesen, Schreiben und Löschen, der Client schreibt weiter.
        let Ok(mut file) = File::open(&self.path) else {
            return Vec::new();
        };
        let Ok(length) = file.metadata().map(|info| info.len()) else {
            return Vec::new();
        };
        if length <= self.position || file.seek(SeekFrom::Start(self.position)).is_err() {
            return Vec::new();
        }

        let mut buffer = Vec::new();
        if file.by_ref().take(length - self.position).read_to_end(&mut buffer).is_err() {
            return Vec::new();
        }
        self.position += buffer.len() as u64;
        self.partial.extend_from_slice(&buffer);

        let Some(last_newline) = self.partial.iter().rposition(|&byte| byte == b'\n') else {
            return Vec::new();
        };
        let complete: Vec<u8> = self.partial.drain(..=last_newline).collect();
        String::from_utf8_lossy(&complete[..complete.len() - 1]).split('\n').map(|line| line.trim_end_matches('\r').to_string()).collect()
    }
}
