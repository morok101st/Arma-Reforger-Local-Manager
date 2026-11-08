//! Kommandozeile der Vorschau. Server, Passwörter und Zielordner werden in der App hinterlegt.

use std::process::ExitCode;
use std::sync::Arc;

use rms_core::addons::{ModState, ModSummary, compare, resolve_mods_root, scan, sort_open_first, state_text};
use rms_core::credentials::{ProfilePasswords, WindowsCredentialStore};
use rms_core::paths::{AppPaths, default_settings_file};
use rms_core::settings::{AppSettings, ServerProfile, SettingsStore};
use rms_core::steam::{is_game_running, is_steam_running, read_steam_path_from_registry, resolve_client_exe};
use rms_core::sync::{ModListCache, Progress, SyncOrchestrator, SyncOutcome};
use tokio_util::sync::CancellationToken;

const USAGE: &str = "Verwendung: reforger-sync-cli status | check [--server <Name>] | sync [--server <Name>]\n\
                     Server, Passwörter und Zielordner werden in der App hinterlegt.";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, server) = match args.as_slice() {
        [] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [command] => (command.as_str(), None),
        [command, flag, name] if flag == "--server" => (command.as_str(), Some(name.as_str())),
        _ => return usage_error(),
    };

    let settings = match SettingsStore::new(default_settings_file()).load() {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("Fehler: {error}");
            return ExitCode::FAILURE;
        }
    };

    match (command, server) {
        ("status", None) => status(&settings),
        ("check" | "sync", _) => sync(&settings, server, command == "check").await,
        _ => usage_error(),
    }
}

fn usage_error() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::FAILURE
}

fn find_profile<'a>(settings: &'a AppSettings, name: Option<&str>) -> Option<&'a ServerProfile> {
    match name {
        Some(name) => settings.profiles.iter().find(|profile| profile.name.eq_ignore_ascii_case(name)),
        None => settings
            .profiles
            .iter()
            .find(|profile| Some(&profile.id) == settings.selected_profile_id.as_ref())
            .or_else(|| settings.profiles.first()),
    }
}

async fn sync(settings: &AppSettings, server: Option<&str>, only_check: bool) -> ExitCode {
    let Some(profile) = find_profile(settings, server) else {
        eprintln!(
            "{}",
            server.map_or_else(|| "Es ist noch kein Server angelegt.".into(), |name| format!("Server „{name}“ nicht gefunden."))
        );
        return ExitCode::FAILURE;
    };

    let cancel = CancellationToken::new();
    let on_ctrl_c = cancel.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            on_ctrl_c.cancel();
        }
    });

    println!("Server: {}", profile.name);
    let progress: Progress = Arc::new(|status| println!("{}", status.message));
    let orchestrator = SyncOrchestrator::new(Arc::new(WindowsCredentialStore), AppPaths::default_for_user());
    let result = if only_check {
        orchestrator.check(settings, profile, &progress, &cancel).await
    } else {
        orchestrator.run(settings, profile, &progress, &cancel).await
    };
    println!("{}", result.message);
    if matches!(result.outcome, SyncOutcome::AlreadyCurrent | SyncOutcome::Completed | SyncOutcome::Checked) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Startet keinen Client: zeigt den Vergleich mit der zuletzt vom Server geholten Liste.
fn status(settings: &AppSettings) -> ExitCode {
    let running = |yes: bool| if yes { "läuft" } else { "läuft nicht" };
    let steam = read_steam_path_from_registry().map_or_else(|| "nicht gefunden".into(), |path| path.display().to_string());
    let client =
        resolve_client_exe(settings.reforger_exe.as_deref()).map_or_else(|| "nicht gefunden".into(), |path| path.display().to_string());
    println!("Steam:  {steam} ({})", running(is_steam_running()));
    println!("Client: {client} ({})", running(is_game_running()));
    println!("Server: {}", settings.profiles.len());

    let store = WindowsCredentialStore;
    let passwords = ProfilePasswords::new(&store);
    let cache = ModListCache::new(AppPaths::default_for_user().mod_lists());
    for profile in &settings.profiles {
        let has_password = passwords.has_password(profile.server_address.as_deref(), &profile.id).unwrap_or(false);
        let mods_root = resolve_mods_root(profile.download_dir.as_deref());
        let addons = scan(&mods_root);
        println!();
        println!("[{}]", profile.name);
        println!("  Adresse:    {}", profile.server_address.as_deref().unwrap_or("nicht hinterlegt"));
        println!("  Passwort:   {}", if has_password { "hinterlegt" } else { "nicht hinterlegt" });
        println!("  Mod-Ordner: {} ({} Mods)", mods_root.display(), addons.len());

        let Some(list) = cache.load(&profile.id) else {
            println!("  Modset:     noch nicht vom Server geholt");
            continue;
        };

        let mut comparison = compare(&list.mods, &addons);
        println!("  Modset:     {} (Stand {})", ModSummary::from_comparison(&comparison).text, list.fetched_at.format("%d.%m.%Y %H:%M"));
        sort_open_first(&mut comparison);
        for mod_state in comparison.iter().filter(|mod_state| mod_state.state != ModState::Current) {
            println!(
                "    [{}] {}: benötigt {}, lokal {}",
                state_text(mod_state.state),
                mod_state.required.name,
                mod_state.required.version.as_deref().unwrap_or("beliebig"),
                mod_state.installed.as_ref().and_then(|addon| addon.version.as_deref()).unwrap_or("-")
            );
        }
    }

    ExitCode::SUCCESS
}
