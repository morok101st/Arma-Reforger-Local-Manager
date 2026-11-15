//! Updates über den Installer. Die App fragt nur die feste Adresse aus `tauri.conf.json` (GitHub-Release)
//! und spielt ein Update nur ein, wenn es mit dem Schlüssel des Release-Workflows signiert ist.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tracing::{info, warn};

use crate::commands::AppState;

/// Gefundenes Update bis zur Bestätigung. Die Oberfläche bekommt nur die Versionsnummer.
#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub version: String,
    /// Nur eine mit dem Installer eingerichtete App kann sich selbst aktualisieren, ein entpacktes ZIP nicht.
    pub can_install: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Der Installer legt `uninstall.exe` neben die EXE, das ZIP nicht.
pub fn is_installed() -> bool {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(|folder| folder.join("uninstall.exe").is_file())).unwrap_or(false)
}

/// Sucht nach einer neueren Version. `None`: Die App ist aktuell.
#[tauri::command]
pub async fn check_update(app: AppHandle, update: State<'_, UpdateState>) -> Result<Option<UpdateView>, String> {
    let handle = app.clone();
    let updater = app
        .updater_builder()
        // Der Installer beendet die App mit `process::exit`, das normale App-Ende läuft dann nicht.
        .on_before_exit(move || crate::clean_up_before_exit(&handle))
        .build()
        .map_err(|error| {
            warn!("Update-Prüfung nicht möglich: {error}");
            "Die Suche nach Updates ist nicht möglich.".to_string()
        })?;

    let found = updater.check().await.map_err(|error| {
        warn!("Update-Prüfung fehlgeschlagen: {error}");
        "Die Suche nach Updates ist fehlgeschlagen. Besteht eine Internetverbindung?".to_string()
    })?;

    let Some(found) = found else {
        info!("Update-Prüfung: Version {} ist aktuell", rms_core::VERSION);
        *update.pending.lock().unwrap() = None;
        return Ok(None);
    };
    info!("Update verfügbar: {} (installiert: {})", found.version, rms_core::VERSION);
    let view = UpdateView { version: found.version.clone(), can_install: is_installed() };
    *update.pending.lock().unwrap() = Some(found);
    Ok(Some(view))
}

/// Lädt das gefundene Update, prüft die Signatur und startet den Installer. Der beendet die App und startet
/// sie danach neu. Kehrt nur bei einem Fehler zurück.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, AppState>, update: State<'_, UpdateState>) -> Result<(), String> {
    if !is_installed() {
        return Err("Diese App wurde aus dem ZIP entpackt und kann sich nicht selbst aktualisieren.".into());
    }
    // Wie ein Lauf: Solange das Update lädt, startet kein Sync.
    let Some(_run) = state.begin_run() else {
        return Err("Es läuft bereits ein Vorgang.".into());
    };
    let found = update.pending.lock().unwrap().take().ok_or("Es liegt kein Update vor. Bitte erneut suchen.")?;

    info!("Update auf {} wird geladen", found.version);
    let mut downloaded = 0u64;
    let result = found
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit("update-progress", UpdateProgress { downloaded, total });
            },
            // Die Signatur prüft das Plugin erst danach, vor dem Start des Installers.
            || info!("Update geladen"),
        )
        .await;

    match result {
        // Unter Windows endet der Prozess im Installer, hierher kommt die App nur ohne ihn.
        Ok(()) => {
            app.restart();
        }
        Err(error) => {
            warn!("Update fehlgeschlagen: {error}");
            Err(format!("Das Update konnte nicht installiert werden: {error}"))
        }
    }
}
