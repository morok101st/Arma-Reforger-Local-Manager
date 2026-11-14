//! Hält unter `clientlogs` nur die Ordner der letzten Läufe. Gelöscht werden nur Ordner, deren Name ein
//! Zeitstempel eines Laufs ist, alles andere bleibt liegen.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::Local;
use regex::Regex;

pub const KEEP_RUNS: usize = 10;

/// Format der Ordnernamen je Lauf, sortiert sich als Text zeitlich richtig. Gleich wie bei der C#-Vorgängerversion.
pub const RUN_FOLDER_FORMAT: &str = "%Y%m%d-%H%M%S-%3f";

/// Auch das ältere Format ohne Millisekunden (vor 0.4.0), damit diese Ordner nicht ewig liegen bleiben.
static RUN_FOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]{8}-[0-9]{6}(-[0-9]{3})?$").unwrap());

/// Löscht die ältesten Lauf-Ordner, bis höchstens `keep` übrig sind. Ein gesperrter Ordner, etwa weil ein Client
/// noch schreibt, bleibt liegen und wird beim nächsten Lauf erneut versucht. Liefert die Anzahl gelöschter Ordner.
pub fn prune(client_logs_root: &Path, keep: usize) -> usize {
    let Ok(entries) = fs::read_dir(client_logs_root) else {
        return 0;
    };

    let mut runs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| RUN_FOLDER.is_match(&entry.file_name().to_string_lossy()))
        .map(|entry| entry.path())
        .collect();
    runs.sort();

    let surplus = runs.len().saturating_sub(keep);
    runs.into_iter().take(surplus).filter(|run| fs::remove_dir_all(run).is_ok()).count()
}

/// Legt den Log-Ordner für einen neuen Lauf an und lässt danach genau `KEEP_RUNS` Läufe übrig.
pub fn create_run_folder(client_logs_root: &Path) -> io::Result<PathBuf> {
    prune(client_logs_root, KEEP_RUNS - 1);
    let mut folder = client_logs_root.join(Local::now().format(RUN_FOLDER_FORMAT).to_string());
    // Zwei Läufe in derselben Millisekunde: den zweiten etwas später benennen statt Logs zu mischen.
    while folder.exists() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        folder = client_logs_root.join(Local::now().format(RUN_FOLDER_FORMAT).to_string());
    }
    fs::create_dir_all(&folder)?;
    Ok(folder)
}
