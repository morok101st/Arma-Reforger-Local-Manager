//! Übergabe des Server-Passworts an die Hilfs-Mod. Bewusst nicht als Startparameter, weil der Client seine
//! Startparameter ins Log schreibt. Die Hilfs-Mod liest die Datei aus `$logs:` oder `$profile:` und löscht sie dabei.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::addons::client_default_folder;
use crate::paths::AppPaths;

pub const FILE_NAME: &str = "rms_server_password.txt";

/// Im eigenen Profil des Clients, das Liste und Download nutzen.
pub fn for_client_profile(client_profile_dir: &Path) -> PathBuf {
    client_profile_dir.join("profile").join(FILE_NAME)
}

/// Im Log-Ordner eines Laufs (`$logs:`). So liegt sie beim Beitritt nicht in „Dokumente“, das oft in OneDrive liegt.
pub fn for_logs_folder(logs_dir: &Path) -> PathBuf {
    logs_dir.join(FILE_NAME)
}

/// Früherer Ort beim Beitritt, im echten Profil des Spielers. Wird nur noch aufgeräumt.
pub fn legacy_player_profile() -> PathBuf {
    client_default_folder().join("profile").join(FILE_NAME)
}

/// Entfernt immer zuerst eine alte Datei und schreibt nur bei einem Passwort eine neue. Sonst fände die
/// Hilfs-Mod das Passwort eines anderen Servers, etwa nach einem Absturz oder einem Beitritt kurz davor.
pub fn prepare(path: &Path, password: Option<&str>) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }

    let Some(password) = password.filter(|password| !password.is_empty()) else {
        return Ok(());
    };

    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, password)
}

/// Absicherung, falls die Hilfs-Mod nicht zum Lesen kommt. Scheitert das Löschen, folgt der nächste Versuch
/// beim nächsten Start. `true`, wenn die Datei danach nicht mehr existiert.
pub fn try_delete(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(error) => error.kind() == io::ErrorKind::NotFound,
    }
}

/// Beim App-Start: Reste aus abgestürzten Läufen an allen Orten entfernen.
pub fn clean_up_leftovers(paths: &AppPaths) {
    try_delete(&for_client_profile(&paths.client_profile()));
    try_delete(&legacy_player_profile());
    if let Ok(runs) = fs::read_dir(paths.client_logs()) {
        for run in runs.filter_map(Result::ok).filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir())) {
            try_delete(&for_logs_folder(&run.path()));
        }
    }
}

/// Wartet, bis die Datei weg ist, längstens `max_wait`. Blockiert, nur für das App-Ende gedacht.
/// `true`, wenn die Datei nicht mehr existiert.
pub fn wait_until_removed(path: &Path, max_wait: Duration) -> bool {
    let started = Instant::now();
    while path.exists() {
        if started.elapsed() >= max_wait {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}
