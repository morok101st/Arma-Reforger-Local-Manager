//! Gemeinsame Vorprüfung vor jedem Start des Spielclients: Sync, Prüfen und Beitritt.

use std::path::{Path, PathBuf};

use crate::addons::helper_root;
use crate::client::args::is_valid_server_address;
use crate::settings::{AppSettings, ServerProfile};
use crate::steam::{check_client_exe, is_game_running, is_steam_running, resolve_client_exe};

pub fn is_helper_installed() -> bool {
    helper_root().join("ReforgerModSyncHelper").join("addon.gproj").is_file()
}

/// Liefert den Spielclient, wenn er starten darf. Sonst eine Meldung für den Nutzer.
pub fn check(settings: &AppSettings, profile: &ServerProfile) -> Result<PathBuf, String> {
    if !is_valid_server_address(profile.server_address.as_deref()) {
        return Err("Es ist keine gültige Server-Adresse hinterlegt (IP:Port).".into());
    }

    if !is_helper_installed() {
        return Err("Die Hilfs-Mod fehlt neben der App. Bitte die App neu installieren.".into());
    }

    if !is_steam_running() {
        return Err("Bitte zuerst Steam starten.".into());
    }

    if is_game_running() {
        return Err("Arma Reforger läuft bereits. Bitte das Spiel zuerst schließen.".into());
    }

    // Auch hier und nicht nur im Dialog: Die Einstellungsdatei könnte von Hand geändert worden sein.
    if let Some(configured) = settings.reforger_exe.as_deref()
        && let Some(problem) = check_client_exe(Path::new(configured))
    {
        return Err(format!("Der Spielclient in den Grundeinstellungen ist ungültig: {problem}"));
    }

    match resolve_client_exe(settings.reforger_exe.as_deref()) {
        Some(exe) if exe.is_file() => Ok(exe),
        _ => Err("Arma Reforger wurde nicht gefunden. Läuft die Erkennung über Steam ins Leere, den Spielclient in den Grundeinstellungen eintragen."
            .into()),
    }
}
