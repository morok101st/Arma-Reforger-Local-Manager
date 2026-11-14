//! Befehle, die die Oberfläche aufrufen darf. Die Logik liegt in `rms-core`, hier wird nur verbunden.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Local;
use rms_core::addons::{ModEntry, compare, normalize_download_dir, resolve_mods_root, scan};
use rms_core::client::args::{combine_server_address, is_valid_server_address, steam_launch_options};
use rms_core::client::{launcher, password_file};
use rms_core::credentials::{CredentialStore, ProfilePasswords, WindowsCredentialStore};
use rms_core::paths::AppPaths;
use rms_core::settings::{
    AppSettings, BasicSettingsInput, BasicSettingsResult, ServerProfile, SettingsStore, apply_basic_settings, validate_profile_input,
};
use rms_core::steam::{is_game_running, resolve_client_exe};
use rms_core::sync::{
    CleanupPlan, ModListCache, Progress, SyncOrchestrator, SyncOutcome, SyncResult, delete_surplus, format_size, plan_cleanup,
};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::dto::{
    AppView, BasicSettingsSaveResult, BasicSettingsView, CleanupQuestion, Message, Overview, ProfileInput, ProfileView, RunResult,
    RunStatus, SaveProfileResult,
};

const PASSWORD_CLEANUP_DELAY: Duration = Duration::from_secs(120);

/// Zustand der App. Gespeicherte Einstellungen liegen nur hier, die Oberfläche hält Entwürfe.
pub struct AppState {
    store: SettingsStore,
    settings: Mutex<AppSettings>,
    credentials: Arc<WindowsCredentialStore>,
    cache: ModListCache,
    orchestrator: SyncOrchestrator,
    paths: AppPaths,
    busy: AtomicBool,
    cancel: Mutex<Option<CancellationToken>>,
    pending_cleanup: Mutex<Option<PendingCleanup>>,
    join_password: Mutex<Option<(PathBuf, Instant)>>,
    load_problem: Mutex<Option<String>>,
}

struct PendingCleanup {
    profile_id: String,
    plan: CleanupPlan,
    result_message: String,
}

impl AppState {
    pub fn new(store: SettingsStore, paths: AppPaths) -> Self {
        let (settings, load_problem) = match store.load() {
            Ok(settings) => (settings, None),
            Err(error) => (AppSettings::default(), Some(format!("{error} Beim Speichern wird sie ersetzt."))),
        };
        let credentials = Arc::new(WindowsCredentialStore);
        Self {
            store,
            settings: Mutex::new(settings),
            cache: ModListCache::new(paths.mod_lists()),
            orchestrator: SyncOrchestrator::new(credentials.clone(), paths.clone()),
            credentials,
            paths,
            busy: AtomicBool::new(false),
            cancel: Mutex::new(None),
            pending_cleanup: Mutex::new(None),
            join_password: Mutex::new(None),
            load_problem: Mutex::new(load_problem),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn request_cancel(&self) {
        if let Some(cancel) = self.cancel.lock().unwrap().as_ref() {
            cancel.cancel();
        }
    }

    /// Passwortdatei des letzten Beitritts und wie lange noch auf sie zu achten ist.
    pub fn pending_join_password(&self) -> Option<(PathBuf, Duration)> {
        let guard = self.join_password.lock().unwrap();
        let (path, due) = guard.as_ref()?;
        let left = due.saturating_duration_since(Instant::now());
        (!left.is_zero()).then(|| (path.clone(), left))
    }

    fn settings(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }

    fn profile(&self, id: &str) -> Option<ServerProfile> {
        self.settings.lock().unwrap().profiles.iter().find(|profile| profile.id == id).cloned()
    }

    fn passwords(&self) -> ProfilePasswords<'_> {
        ProfilePasswords::new(self.credentials.as_ref())
    }

    fn has_password(&self, profile: &ServerProfile) -> bool {
        self.passwords().has_password(profile.server_address.as_deref(), &profile.id).unwrap_or(false)
    }

    fn cached_overview(&self, profile: &ServerProfile) -> Option<Overview> {
        let list = self.cache.load(&profile.id)?;
        let installed = scan(&resolve_mods_root(profile.download_dir.as_deref()));
        Some(Overview::from_comparison(&compare(&list.mods, &installed), format!("Liste vom {}", list.fetched_at.format("%d.%m.%Y %H:%M"))))
    }

    fn view(&self, profile: &ServerProfile) -> ProfileView {
        ProfileView::new(profile, self.has_password(profile), self.cached_overview(profile))
    }

    /// Speichert die Einstellungen. Liefert eine Meldung, wenn das scheitert.
    fn persist(&self, change: impl FnOnce(&mut AppSettings)) -> Result<(), String> {
        let mut settings = self.settings.lock().unwrap();
        let mut next = settings.clone();
        change(&mut next);
        self.store.save(&next).map_err(|error| {
            warn!("Einstellungen konnten nicht gespeichert werden: {error}");
            format!("Die Einstellungen konnten nicht gespeichert werden: {error}")
        })?;
        *settings = next;
        Ok(())
    }

    /// Markiert den Beginn eines Laufs. `None`, wenn schon einer läuft.
    fn begin_run(&self) -> Option<RunGuard<'_>> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return None;
        }
        let cancel = CancellationToken::new();
        *self.cancel.lock().unwrap() = Some(cancel.clone());
        Some(RunGuard { state: self, cancel })
    }
}

/// Gibt den Lauf beim Verlassen wieder frei.
struct RunGuard<'a> {
    state: &'a AppState,
    cancel: CancellationToken,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        *self.state.cancel.lock().unwrap() = None;
        self.state.busy.store(false, Ordering::SeqCst);
    }
}

type CommandResult<T> = Result<T, String>;

fn busy_error() -> String {
    "Es läuft bereits ein Vorgang.".into()
}

#[tauri::command]
pub fn load_app(state: State<'_, AppState>) -> AppView {
    let settings = state.settings();
    AppView {
        version: rms_core::VERSION.into(),
        profiles: settings.profiles.iter().map(|profile| state.view(profile)).collect(),
        selected_profile_id: settings.selected_profile_id.clone(),
        load_problem: state.load_problem.lock().unwrap().take(),
    }
}

#[tauri::command]
pub fn select_profile(state: State<'_, AppState>, id: Option<String>) -> CommandResult<()> {
    state.persist(|settings| settings.selected_profile_id = id)
}

/// Prüft die Eingaben. `None`, wenn sie gespeichert werden dürfen.
fn validate(input: &ProfileInput) -> Option<&'static str> {
    validate_profile_input(Some(&input.name), Some(&input.ip), Some(&input.port))
}

fn to_profile(input: &ProfileInput, previous: Option<&ServerProfile>) -> ServerProfile {
    ServerProfile {
        id: input.id.clone(),
        name: input.name.trim().to_string(),
        server_address: combine_server_address(Some(&input.ip), Some(&input.port)),
        download_dir: normalize_download_dir(Some(&input.download_dir)),
        // Felder, die nur die C#-Vorgängerversion kennt, bleiben erhalten.
        extra: previous.map(|profile| profile.extra.clone()).unwrap_or_default(),
    }
}

/// Die Adresse wurde geändert, ohne ein neues Passwort einzugeben: Soll das gespeicherte mitwandern?
#[tauri::command]
pub fn password_transfer_needed(state: State<'_, AppState>, input: ProfileInput, password_entered: bool) -> bool {
    let Some(previous) = state.profile(&input.id) else {
        return false;
    };
    validate(&input).is_none()
        && ProfilePasswords::needs_transfer_decision(
            state.has_password(&previous),
            previous.server_address.as_deref(),
            to_profile(&input, Some(&previous)).server_address.as_deref(),
            if password_entered { "x" } else { "" },
        )
}

/// Speichert ohne Verbindung zum Server, damit sich auch ein Server einrichten lässt, der gerade offline ist.
#[tauri::command]
pub fn save_profile(state: State<'_, AppState>, input: ProfileInput, password: String, keep_password: bool) -> SaveProfileResult {
    let fail = |message: String, profile: Option<ProfileView>| SaveProfileResult { stored: false, message, is_error: true, profile };
    if state.is_busy() {
        return fail("Während eines Vorgangs lässt sich nichts speichern.".into(), None);
    }
    if let Some(problem) = validate(&input) {
        return fail(problem.into(), None);
    }

    let previous = state.profile(&input.id);
    let saved = to_profile(&input, previous.as_ref());
    if let Err(problem) = state.persist(|settings| {
        match settings.profiles.iter_mut().find(|profile| profile.id == saved.id) {
            Some(existing) => *existing = saved.clone(),
            None => settings.profiles.push(saved.clone()),
        }
        settings.selected_profile_id = Some(saved.id.clone());
    }) {
        return fail(problem, None);
    }

    let address = saved.server_address.clone().unwrap_or_default();
    if let Some(previous_address) = previous.as_ref().and_then(|profile| profile.server_address.clone())
        && previous_address != address
    {
        // Die gemerkte Liste gehört zum alten Server und würde in der Übersicht falsche Mods zeigen.
        let _ = state.cache.delete(&saved.id);
        if let Err(error) = state.passwords().move_to_new_address(&saved.id, &previous_address, &address, keep_password) {
            return fail(format!("Das Passwort konnte nicht übernommen werden: {error}"), Some(state.view(&saved)));
        }
    }

    if let Err(error) = state.passwords().store_entered(&address, &saved.id, &password) {
        return fail(format!("Das Passwort konnte nicht gespeichert werden: {error}"), Some(state.view(&saved)));
    }

    info!("Server {} gespeichert", saved.name);
    SaveProfileResult {
        stored: true,
        message: "Gespeichert. Prüfen und synchronisieren kannst du in der Übersicht.".into(),
        is_error: false,
        profile: Some(state.view(&saved)),
    }
}

/// Löscht Profil, Passwort und gemerkte Liste. Heruntergeladene Mods bleiben auf der Festplatte.
#[tauri::command]
pub fn remove_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    if state.is_busy() {
        return Err(busy_error());
    }
    if let Some(profile) = state.profile(&id) {
        if let Some(address) = profile.server_address.as_deref() {
            let _ = state.credentials.delete_password(address, &id);
        }
        info!("Server entfernt: {}", profile.name);
    }
    let _ = state.cache.delete(&id);
    state.persist(|settings| {
        settings.profiles.retain(|profile| profile.id != id);
        if settings.selected_profile_id.as_deref() == Some(id.as_str()) {
            settings.selected_profile_id = settings.profiles.first().map(|profile| profile.id.clone());
        }
    })
}

#[tauri::command]
pub fn delete_password(state: State<'_, AppState>, id: String) -> CommandResult<Message> {
    let profile = state.profile(&id).ok_or("Der Server ist noch nicht gespeichert.")?;
    if let Some(address) = profile.server_address.as_deref() {
        state.credentials.delete_password(address, &id).map_err(|error| error.to_string())?;
    }
    Ok(Message::info("Das gespeicherte Passwort dieses Servers wurde gelöscht."))
}

#[tauri::command]
pub fn steam_options(download_dir: String) -> String {
    steam_launch_options(Some(&download_dir))
}

fn emit_status(app: &AppHandle, profile: &ServerProfile) -> Progress {
    let app = app.clone();
    let profile_id = profile.id.clone();
    let name = profile.name.clone();
    Arc::new(move |status| {
        let is_client_line = status.message.starts_with("[client]");
        if !is_client_line {
            info!("[{name}] {}", status.message);
        }
        let overview = status.mods.as_deref().map(|mods| Overview::from_comparison(mods, String::new()));
        let _ = app.emit(
            "run-status",
            RunStatus {
                profile_id: profile_id.clone(),
                message: status.message,
                is_client_line,
                done: status.done,
                total: status.total,
                overview,
            },
        );
    })
}

fn now_stand() -> String {
    format!("Liste vom {}", Local::now().format("%d.%m.%Y %H:%M"))
}

fn finish(result: &SyncResult, ok_outcome: bool) -> RunResult {
    let cancelled = result.outcome == SyncOutcome::Cancelled;
    let has_mods = !result.mods.is_empty();
    let overview = has_mods.then(|| Overview::from_comparison(&result.mods, now_stand()));
    let list_status = match (&overview, result.outcome) {
        (Some(overview), _) => overview.summary.list_status.clone(),
        (None, SyncOutcome::Cancelled) => "abgebrochen".into(),
        (None, _) => "Fehler".into(),
    };
    RunResult {
        ok: ok_outcome,
        cancelled,
        message: result.message.clone(),
        is_error: matches!(result.outcome, SyncOutcome::Failed | SyncOutcome::Incomplete),
        list_status,
        overview,
        cleanup_question: None,
    }
}

/// Holt nur die Liste vom Server und vergleicht.
#[tauri::command]
pub async fn check_profile(app: AppHandle, state: State<'_, AppState>, id: String) -> CommandResult<RunResult> {
    let profile = state.profile(&id).ok_or("Bitte den Server zuerst in den Einstellungen speichern.")?;
    let run = state.begin_run().ok_or_else(busy_error)?;
    let settings = state.settings();
    let result = state.orchestrator.check(&settings, &profile, &emit_status(&app, &profile), &run.cancel).await;
    let mut view = finish(&result, result.outcome == SyncOutcome::Checked);
    if view.ok {
        view.message = String::new();
    }
    Ok(view)
}

#[tauri::command]
pub async fn sync_profile(app: AppHandle, state: State<'_, AppState>, id: String) -> CommandResult<RunResult> {
    let profile = state.profile(&id).ok_or("Bitte den Server zuerst in den Einstellungen speichern.")?;
    let run = state.begin_run().ok_or_else(busy_error)?;
    let settings = state.settings();
    let result = state.orchestrator.run(&settings, &profile, &emit_status(&app, &profile), &run.cancel).await;
    info!("Sync {} beendet: {:?} - {}", profile.name, result.outcome, result.message);
    let mut view = finish(&result, matches!(result.outcome, SyncOutcome::Completed | SyncOutcome::AlreadyCurrent));

    // Nur nach einem vollständigen Lauf: Sonst ist nicht sicher, dass die Liste und der Ordner stimmen.
    // Der Lauf bleibt belegt, bis auch das Bereinigen durch ist.
    if view.ok {
        apply_cleanup_step(&state, &settings, &profile, &result, &mut view);
    }
    drop(run);
    Ok(view)
}

fn apply_cleanup_step(state: &AppState, settings: &AppSettings, profile: &ServerProfile, result: &SyncResult, view: &mut RunResult) {
    let required: Vec<ModEntry> = result.mods.iter().map(|mod_state| mod_state.required.clone()).collect();
    let plan = plan_cleanup(&state.cache, profile, &required, &settings.profiles);
    let note = |view: &mut RunResult, note: &str, is_error: bool| {
        info!("[{}] {note}", profile.name);
        view.message = format!("{} {note}", result.message);
        view.is_error = is_error;
    };

    if let Some(blocker) = &plan.blocker {
        note(view, blocker, false);
        return;
    }
    if plan.surplus.is_empty() {
        return;
    }
    if is_game_running() {
        note(view, "Überzählige Mods wurden nicht gelöscht, weil das Spiel läuft.", false);
        return;
    }

    if settings.cleanup_approved_folders.contains(&plan.folder_key) {
        let summary = run_cleanup(&plan);
        note(view, &summary.0, summary.1);
        return;
    }

    const MAX_LISTED: usize = 8;
    let count = plan.surplus.len();
    let mut names: Vec<String> = plan.surplus.iter().take(MAX_LISTED).map(|mod_entry| format!("•  {}", mod_entry.name)).collect();
    if count > MAX_LISTED {
        names.push(format!("•  und {} weitere", count - MAX_LISTED));
    }
    view.cleanup_question = Some(CleanupQuestion {
        title: if count == 1 { "1 überzählige Mod löschen?".into() } else { format!("{count} überzählige Mods löschen?") },
        text: format!(
            "Kein eingetragener Server mit diesem Zielordner braucht sie ({}):\n\n{}\n\nOrdner: {}\n\nNach deiner Zustimmung löscht der Sync überzählige Mods in diesem Ordner künftig automatisch.",
            format_size(plan.total_bytes()),
            names.join("\n"),
            plan.mods_root.display()
        ),
    });
    *state.pending_cleanup.lock().unwrap() =
        Some(PendingCleanup { profile_id: profile.id.clone(), plan, result_message: result.message.clone() });
}

/// Löscht und liefert Text und ob etwas scheiterte.
fn run_cleanup(plan: &CleanupPlan) -> (String, bool) {
    let cleaned = delete_surplus(plan);
    for mod_entry in &cleaned.deleted {
        info!("Überzählige Mod gelöscht: {} ({})", mod_entry.name, mod_entry.folder.display());
    }
    if !cleaned.failed.is_empty() {
        warn!("Nicht gelöscht: {}", cleaned.failed.iter().map(|mod_entry| mod_entry.name.as_str()).collect::<Vec<_>>().join(", "));
    }
    (cleaned.summary(), !cleaned.failed.is_empty())
}

/// Antwort auf die Rückfrage zum Bereinigen.
#[tauri::command]
pub fn answer_cleanup(state: State<'_, AppState>, id: String, approved: bool) -> CommandResult<Message> {
    let pending = state.pending_cleanup.lock().unwrap().take().filter(|pending| pending.profile_id == id);
    let Some(pending) = pending else {
        return Err("Es wartet keine Rückfrage.".into());
    };

    if !approved {
        return Ok(Message::info(format!(
            "{} Überzählige Mods bleiben erhalten, die App fragt beim nächsten Sync erneut.",
            pending.result_message
        )));
    }

    let key = pending.plan.folder_key.clone();
    state.persist(|settings| {
        if !settings.cleanup_approved_folders.contains(&key) {
            settings.cleanup_approved_folders.push(key);
        }
    })?;
    let (summary, is_error) = run_cleanup(&pending.plan);
    let text = format!("{} {summary}", pending.result_message);
    Ok(if is_error { Message::error(text) } else { Message::info(text) })
}

/// Bricht den laufenden Vorgang ab. Der Spielclient wird beendet.
#[tauri::command]
pub fn cancel_run(state: State<'_, AppState>) {
    state.request_cancel();
}

/// Startet das Spiel und tritt dem gewählten Server direkt bei.
#[tauri::command]
pub async fn join_server(state: State<'_, AppState>, id: String) -> CommandResult<Message> {
    if state.is_busy() {
        return Err(busy_error());
    }
    let profile = state.profile(&id).ok_or("Bitte den Server zuerst in den Einstellungen speichern.")?;
    if !is_valid_server_address(profile.server_address.as_deref()) {
        return Ok(Message::error("Es ist keine gültige Server-Adresse hinterlegt (Adresse und Port)."));
    }

    let address = profile.server_address.clone().unwrap_or_default();
    let password = state.credentials.read_password(&address, &id).map_err(|error| error.to_string())?;
    let started = launcher::start_and_join(&state.settings(), &profile, password.as_deref(), &state.paths);
    drop(password);
    let start = match started {
        Ok(start) => start,
        Err(problem) => return Ok(Message::error(problem)),
    };

    info!("Direkter Beitritt gestartet für Server {}", profile.name);
    if let Some(file) = start.password_file {
        // Die Hilfs-Mod löscht die Passwortdatei beim Start. Das ist die Absicherung, falls sie nicht dazu kommt.
        // Wird die App vorher geschlossen, übernimmt das App-Ende diese Aufgabe.
        *state.join_password.lock().unwrap() = Some((file.clone(), Instant::now() + PASSWORD_CLEANUP_DELAY));
        tokio::spawn(async move {
            tokio::time::sleep(PASSWORD_CLEANUP_DELAY).await;
            password_file::try_delete(&file);
        });
    }
    Ok(Message::info(
        "Das Spiel startet und tritt dem Server bei. Klappt das nicht, bleibt es im Hauptmenü, und du kannst selbst beitreten.",
    ))
}

#[tauri::command]
pub fn basic_settings(state: State<'_, AppState>) -> BasicSettingsView {
    let hint = match resolve_client_exe(None) {
        Some(exe) => format!("Leer lassen, um den Client über Steam zu finden. Gefunden: {}", exe.display()),
        None => {
            "Leer lassen, um den Client über Steam zu finden. Über Steam wurde gerade kein Client gefunden, dann hier eintragen.".into()
        }
    };
    BasicSettingsView {
        input: BasicSettingsInput::from_settings(&state.settings()),
        defaults: BasicSettingsInput::defaults(),
        detected_exe_hint: hint,
    }
}

#[tauri::command]
pub fn save_basic_settings(state: State<'_, AppState>, input: BasicSettingsInput) -> BasicSettingsSaveResult {
    let fail = |message: &str| BasicSettingsSaveResult { ok: false, errors: Default::default(), message: message.into() };
    if state.is_busy() {
        return fail("Während eines Vorgangs lassen sich die Grundeinstellungen nicht ändern.");
    }

    match apply_basic_settings(&state.settings(), &input) {
        BasicSettingsResult::Invalid(errors) => {
            BasicSettingsSaveResult { ok: false, errors, message: "Bitte die markierten Felder prüfen.".into() }
        }
        BasicSettingsResult::Valid(next) => match state.persist(|settings| *settings = next.clone()) {
            Err(problem) => fail(&problem),
            Ok(()) => {
                info!(
                    "Grundeinstellungen gespeichert: Client {}, höchstens {} min, Neustart nach {} min",
                    next.reforger_exe.as_deref().unwrap_or("automatisch"),
                    next.timeouts.total_minutes,
                    next.timeouts.idle_minutes
                );
                BasicSettingsSaveResult { ok: true, errors: Default::default(), message: String::new() }
            }
        },
    }
}

#[tauri::command]
pub async fn browse_folder(app: AppHandle, current: String) -> Option<String> {
    let mut dialog = app.dialog().file().set_title("Zielordner für die Mods wählen");
    let current = PathBuf::from(current.trim());
    if current.is_dir() {
        dialog = dialog.set_directory(current);
    }
    dialog.blocking_pick_folder().and_then(|path| path.into_path().ok()).map(|path| path.display().to_string())
}

#[tauri::command]
pub async fn browse_exe(app: AppHandle, current: String) -> Option<String> {
    let mut dialog =
        app.dialog().file().set_title("Spielclient von Arma Reforger wählen").add_filter("Arma Reforger (ArmaReforgerSteam.exe)", &["exe"]);
    if let Some(folder) = PathBuf::from(current.trim().trim_matches('"')).parent().filter(|folder| folder.is_dir()) {
        dialog = dialog.set_directory(folder);
    }
    dialog.blocking_pick_file().and_then(|path| path.into_path().ok()).map(|path| path.display().to_string())
}

/// Nur die beiden festen Adressen der App, nie eine von der Oberfläche übergebene.
#[tauri::command]
pub fn open_link(target: String) -> CommandResult<()> {
    let url = match target.as_str() {
        "repository" => rms_core::REPOSITORY_URL,
        "releases" => rms_core::RELEASES_URL,
        _ => return Err("Unbekanntes Ziel.".into()),
    };
    launcher::open_url(url)
}

/// Beenden nach Rückfrage, auch während eines Laufs.
#[tauri::command]
pub fn quit_app(app: AppHandle, state: State<'_, AppState>) {
    state.request_cancel();
    app.exit(0);
}
