//! Holt die Mod-Liste, vergleicht sie mit dem Zielordner und lädt die offenen Mods.
//!
//! - Manuell angelegter Server: Die Hilfs-Mod liest die Liste aus dem Server-Eintrag im Backend und lädt
//!   daraus. Dafür läuft der Spielclient zweimal kurz.
//! - Aus ARMM importierter Server: Die Liste kommt per HTTPS aus ARMM, ganz ohne Spielstart. Geladen wird
//!   ohne Server-Eintrag (`-rmsModset`). Nur für Mods, die so nicht erreichbar sind, dient der Server-Eintrag
//!   als Rückfall.
//!
//! In keinem Fall entsteht eine Verbindung zum Spielserver oder ein Beitritt.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use zeroize::{Zeroize, Zeroizing};

use super::watcher::{DownloadWatch, DownloadWatcher, WatchEnd};
use super::{ModListCache, ServerListError, ServerListReader, SyncLock};
use crate::addons::{ModComparison, ModEntry, ModState, compare, helper_root, resolve_mods_root, scan};
use crate::armm::{ArmmLink, FetchError, ModsetError, SharedModsetSource, WinHttpModsetSource, parse_modset};
use crate::client::args::{build_download, build_list_request, build_modset_download, is_valid_server_address, resolve_server_address};
use crate::client::{ClientProcess, ClientStarter, default_starter, modset_file, password_file, precheck, retention};
use crate::credentials::CredentialStore;
use crate::paths::AppPaths;
use crate::settings::{AppSettings, ServerProfile};

const MINIMUM_FREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SyncOutcome {
    /// Nur geprüft: Die Liste kam vom Server oder aus ARMM, es wurde nichts geladen.
    Checked,
    AlreadyCurrent,
    Completed,
    Incomplete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncStatus {
    pub message: String,
    pub done: usize,
    pub total: usize,
    pub mods: Option<Vec<ModComparison>>,
}

impl SyncStatus {
    pub fn message(message: impl Into<String>) -> Self {
        Self { message: message.into(), done: 0, total: 0, mods: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncResult {
    pub outcome: SyncOutcome,
    pub message: String,
    pub mods: Vec<ModComparison>,
    /// Die Liste ist zweifelhaft (ARMM weicht vom laufenden Server ab). Überzählige Mods dann nie löschen.
    pub keep_surplus: bool,
}

impl SyncResult {
    fn new(outcome: SyncOutcome, message: impl Into<String>, mods: Vec<ModComparison>) -> Self {
        Self { outcome, message: message.into(), mods, keep_surplus: false }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self::new(SyncOutcome::Failed, message, Vec::new())
    }
}

/// Meldet Fortschritt an Oberfläche oder Kommandozeile.
pub type Progress = Arc<dyn Fn(SyncStatus) + Send + Sync>;

/// Vorprüfung vor dem Start. Tests ersetzen sie, weil die echte Steam und die Hilfs-Mod verlangt.
pub type Precheck = Arc<dyn Fn(&AppSettings, &ServerProfile) -> Result<PathBuf, String> + Send + Sync>;

enum Stop {
    Failed(String),
    Cancelled,
}

impl From<ServerListError> for Stop {
    fn from(error: ServerListError) -> Self {
        match error {
            ServerListError::Failed(message) => Self::Failed(message),
            ServerListError::Cancelled => Self::Cancelled,
        }
    }
}

impl From<std::io::Error> for Stop {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<FetchError> for Stop {
    fn from(error: FetchError) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<ModsetError> for Stop {
    fn from(error: ModsetError) -> Self {
        Self::Failed(error.to_string())
    }
}

/// Woher die Hilfs-Mod beim Laden ihre Mods bezieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Aus dem Server-Eintrag (`-rmsServer`, `-rmsMods`), mit Passwort.
    Server,
    /// Aus der Listendatei (`-rmsModset`), ohne Server und ohne Passwort.
    Modset,
}

/// Alles, was ein Lauf durchreicht.
struct SyncRun<'a> {
    settings: &'a AppSettings,
    profile: &'a ServerProfile,
    client_exe: PathBuf,
    /// Adresse für den Spielclient, ein Name ist hier schon in die IPv4-Adresse aufgelöst. Bei einem
    /// ARMM-Lauf leer, bis der Rückfall über den Server-Eintrag sie braucht.
    client_address: String,
    progress: &'a Progress,
    cancel: &'a CancellationToken,
}

impl SyncRun<'_> {
    /// Gespeicherte Adresse wie eingegeben, daran hängt das Passwort.
    fn address(&self) -> &str {
        self.profile.server_address.as_deref().unwrap_or_default().trim()
    }

    /// Für Meldungen: der Name und, falls aufgelöst, die IP dahinter.
    fn display_address(&self) -> String {
        if self.address() == self.client_address {
            self.client_address.clone()
        } else {
            format!("{} ({})", self.address(), self.client_address)
        }
    }

    fn mods_root(&self) -> PathBuf {
        resolve_mods_root(self.profile.download_dir.as_deref())
    }

    fn report(&self, message: impl Into<String>) {
        (self.progress)(SyncStatus::message(message));
    }

    /// Derselbe Lauf mit aufgelöster Server-Adresse, für den Rückfall.
    fn with_address(&self, client_address: String) -> SyncRun<'_> {
        SyncRun {
            settings: self.settings,
            profile: self.profile,
            client_exe: self.client_exe.clone(),
            client_address,
            progress: self.progress,
            cancel: self.cancel,
        }
    }
}

pub struct SyncOrchestrator {
    credentials: Arc<dyn CredentialStore>,
    cache: ModListCache,
    paths: AppPaths,
    start_client: ClientStarter,
    precheck: Precheck,
    list_reader: ServerListReader,
    watcher: DownloadWatcher,
    modset_source: SharedModsetSource,
    helper_root: PathBuf,
    lock_name: String,
}

impl SyncOrchestrator {
    pub fn new(credentials: Arc<dyn CredentialStore>, paths: AppPaths) -> Self {
        Self {
            credentials,
            cache: ModListCache::new(paths.mod_lists()),
            paths,
            start_client: default_starter(),
            precheck: Arc::new(precheck::check),
            list_reader: ServerListReader::default(),
            watcher: DownloadWatcher::default(),
            modset_source: Arc::new(WinHttpModsetSource),
            helper_root: helper_root(),
            lock_name: super::SYNC_LOCK_NAME.into(),
        }
    }

    /// Für Tests: eigener Client, eigene Vorprüfung, kurze Wartezeiten, eigene Sperre.
    #[must_use]
    pub fn with_test_parts(
        mut self,
        start_client: ClientStarter,
        precheck: Precheck,
        list_reader: ServerListReader,
        watcher: DownloadWatcher,
        lock_name: &str,
    ) -> Self {
        self.start_client = start_client;
        self.precheck = precheck;
        self.list_reader = list_reader;
        self.watcher = watcher;
        lock_name.clone_into(&mut self.lock_name);
        self
    }

    /// Für Tests: ARMM durch eine Attrappe ersetzen.
    #[must_use]
    pub fn with_modset_source(mut self, source: SharedModsetSource) -> Self {
        self.modset_source = source;
        self
    }

    pub fn cache(&self) -> &ModListCache {
        &self.cache
    }

    /// Holt nur die Liste und vergleicht. Bei einem ARMM-Server ohne Spielstart.
    pub async fn check(
        &self,
        settings: &AppSettings,
        profile: &ServerProfile,
        progress: &Progress,
        cancel: &CancellationToken,
    ) -> SyncResult {
        self.run_exclusive(cancel, async {
            if let Some(link) = &profile.armm {
                let required = self.fetch_armm(profile, link, progress, cancel).await?;
                let comparison = compare(&required, &scan(&resolve_mods_root(profile.download_dir.as_deref())));
                let message = format!("Liste aus ARMM geholt: {} Mods.", comparison.len());
                return Ok(SyncResult::new(SyncOutcome::Checked, message, comparison));
            }

            let run = self.start_run(settings, profile, progress, cancel)?;
            let required = self.fetch_list(&run, true).await?;
            let comparison = compare(&required, &scan(&run.mods_root()));
            let message = format!("Liste vom Server geholt: {} Mods.", comparison.len());
            Ok(SyncResult::new(SyncOutcome::Checked, message, comparison))
        })
        .await
    }

    /// Fehler führen nie zu einem Abbruch, sondern zu einem Ergebnis mit klarer Meldung.
    pub async fn run(
        &self,
        settings: &AppSettings,
        profile: &ServerProfile,
        progress: &Progress,
        cancel: &CancellationToken,
    ) -> SyncResult {
        self.run_exclusive(cancel, async {
            progress(SyncStatus::message("Vorprüfung …"));
            if let Some(link) = &profile.armm {
                let client_exe = (self.precheck)(settings, profile).map_err(Stop::Failed)?;
                let run = SyncRun { settings, profile, client_exe, client_address: String::new(), progress, cancel };
                let required = self.fetch_armm(profile, link, progress, cancel).await?;
                return self.download_armm(&run, &required).await;
            }

            let run = self.start_run(settings, profile, progress, cancel)?;
            let required = self.fetch_list(&run, true).await?;
            self.download(&run, &required).await
        })
        .await
    }

    fn start_run<'a>(
        &self,
        settings: &'a AppSettings,
        profile: &'a ServerProfile,
        progress: &'a Progress,
        cancel: &'a CancellationToken,
    ) -> Result<SyncRun<'a>, Stop> {
        let client_exe = (self.precheck)(settings, profile).map_err(Stop::Failed)?;
        let client_address = resolve_server_address(profile.server_address.as_deref().unwrap_or_default()).map_err(Stop::Failed)?;
        let run = SyncRun { settings, profile, client_exe, client_address, progress, cancel };
        if run.address() != run.client_address {
            run.report(format!("{} zeigt auf {}.", run.address(), run.client_address));
        }
        Ok(run)
    }

    /// Es kann immer nur ein Client laufen, die Sperre gilt deshalb profil- und app-übergreifend.
    async fn run_exclusive(&self, cancel: &CancellationToken, run: impl Future<Output = Result<SyncResult, Stop>>) -> SyncResult {
        let Some(_lock) = SyncLock::try_acquire_named(&self.lock_name) else {
            return SyncResult::failed("Es läuft bereits eine Synchronisierung.");
        };

        match run.await {
            Ok(result) => result,
            Err(Stop::Cancelled) => cancelled(),
            Err(Stop::Failed(_)) if cancel.is_cancelled() => cancelled(),
            Err(Stop::Failed(message)) => SyncResult::failed(message),
        }
    }

    /// Mod-Liste aus ARMM. Der Abruf blockiert, er läuft deshalb neben der Oberfläche in einem eigenen Faden.
    async fn fetch_armm(
        &self,
        profile: &ServerProfile,
        link: &ArmmLink,
        progress: &Progress,
        cancel: &CancellationToken,
    ) -> Result<Vec<ModEntry>, Stop> {
        progress(SyncStatus::message(format!("Mod-Liste wird aus ARMM geholt ({}) …", link.host())));
        let key = Zeroizing::new(self.credentials.read_access_key(&profile.id)?.ok_or(FetchError::NoAccessKey)?);
        let source = Arc::clone(&self.modset_source);
        let link = link.clone();
        let task = tokio::task::spawn_blocking(move || source.fetch(&link, &key));
        let bytes = tokio::select! {
            () = cancel.cancelled() => return Err(Stop::Cancelled),
            joined = task => joined.map_err(|error| Stop::Failed(error.to_string()))??,
        };

        let mods = parse_modset(&bytes)?;
        self.cache.save(&profile.id, &mods)?;
        Ok(mods)
    }

    /// Liste aus dem Server-Eintrag. `save`: als gemerkte Liste des Profils ablegen. Beim Rückfall eines
    /// ARMM-Servers dient sie nur der Gegenprüfung und ersetzt die Liste aus ARMM nicht.
    async fn fetch_list(&self, run: &SyncRun<'_>, save: bool) -> Result<Vec<ModEntry>, Stop> {
        run.report("Mod-Liste wird vom Server geholt …");
        let (logs_dir, profile_dir) = self.prepare_run_folders()?;
        let arguments = build_list_request(
            &run.client_address,
            run.profile.download_dir.as_deref(),
            &run.settings.client,
            &logs_dir.to_string_lossy(),
            &profile_dir.to_string_lossy(),
            &self.helper_root.to_string_lossy(),
        );
        let client = self.start_client(run, &arguments, &profile_dir, Source::Server)?;
        let mods = self.list_reader.read(client.client.as_ref(), &logs_dir, &run.display_address(), run.cancel).await;
        client.finish().await;
        let mods = mods?;
        if save {
            self.cache.save(&run.profile.id, &mods)?;
        }
        Ok(mods)
    }

    async fn download(&self, run: &SyncRun<'_>, required: &[ModEntry]) -> Result<SyncResult, Stop> {
        let mods_root = run.mods_root();
        let comparison = compare(required, &scan(&mods_root));
        if comparison.iter().all(|mod_state| mod_state.state == ModState::Current) {
            return Ok(SyncResult::new(SyncOutcome::AlreadyCurrent, "Alles aktuell.", comparison));
        }
        ensure_free_space(&mods_root)?;

        let deadline = deadline(run);
        let mut failures = HashMap::new();
        let (end, logs_dir) = self.download_with_restart(run, required, deadline, &mut failures, Source::Server).await?;
        Ok(conclude(compare(required, &scan(&mods_root)), end, &logs_dir, &failures, None))
    }

    /// Laden für einen ARMM-Server: erst ohne Server, dann für übrig gebliebene Mods über den Server-Eintrag.
    async fn download_armm(&self, run: &SyncRun<'_>, required: &[ModEntry]) -> Result<SyncResult, Stop> {
        let mods_root = run.mods_root();
        let comparison = compare(required, &scan(&mods_root));
        if comparison.iter().all(|mod_state| mod_state.state == ModState::Current) {
            return Ok(SyncResult::new(SyncOutcome::AlreadyCurrent, "Alles aktuell.", comparison));
        }
        ensure_free_space(&mods_root)?;

        let deadline = deadline(run);
        let mut failures = HashMap::new();
        let (mut end, mut logs_dir) = self.download_with_restart(run, required, deadline, &mut failures, Source::Modset).await?;

        let open = count_open(&compare(required, &scan(&mods_root)));
        let mut note = None;
        if open > 0 {
            match self.fallback(run, required, deadline, &mut failures, open).await? {
                Fallback::Done { end: fallback_end, logs_dir: fallback_logs, mismatch } => {
                    end = fallback_end;
                    logs_dir = fallback_logs;
                    note = mismatch;
                }
                Fallback::Skipped(reason) => note = Some(reason),
            }
        }

        let keep_surplus = note.as_deref().is_some_and(|text| text.starts_with(MISMATCH_PREFIX));
        let mut result = conclude(compare(required, &scan(&mods_root)), end, &logs_dir, &failures, note);
        result.keep_surplus = keep_surplus;
        Ok(result)
    }

    /// Rückfall über den Server-Eintrag. Läuft der Server nicht oder fehlt die Adresse, bleibt es beim
    /// Ergebnis ohne Server, mit Hinweis.
    async fn fallback(
        &self,
        run: &SyncRun<'_>,
        required: &[ModEntry],
        deadline: Instant,
        failures: &mut HashMap<String, String>,
        open: usize,
    ) -> Result<Fallback, Stop> {
        if !is_valid_server_address(run.profile.server_address.as_deref()) {
            return Ok(Fallback::Skipped(format!(
                "{open} Mods sind ohne Server-Eintrag nicht erreichbar. Mit eingetragener Adresse des Spielservers versucht die App sie beim nächsten Sync über den Server."
            )));
        }

        run.report(format!("{open} Mods sind ohne Server nicht erreichbar, Rückfall über den Server-Eintrag …"));
        let client_address = match resolve_server_address(run.address()) {
            Ok(address) => address,
            Err(problem) => return Ok(Fallback::Skipped(format!("Rückfall über den Server nicht möglich: {problem}"))),
        };
        let server_run = run.with_address(client_address);
        let server_list = match self.fetch_list(&server_run, false).await {
            Ok(list) => list,
            Err(Stop::Cancelled) => return Err(Stop::Cancelled),
            Err(Stop::Failed(problem)) => return Ok(Fallback::Skipped(format!("Rückfall über den Server nicht möglich: {problem}"))),
        };

        let mismatch = list_mismatch(required, &server_list);
        if let Some(text) = &mismatch {
            run.report(text.clone());
        }
        let (end, logs_dir) = self.download_once(&server_run, required, deadline, failures, Source::Server).await?;
        Ok(Fallback::Done { end, logs_dir, mismatch })
    }

    /// Ein Start für alle offenen Mods, bei Stillstand ein zweiter.
    async fn download_with_restart(
        &self,
        run: &SyncRun<'_>,
        required: &[ModEntry],
        deadline: Instant,
        failures: &mut HashMap<String, String>,
        source: Source,
    ) -> Result<(WatchEnd, PathBuf), Stop> {
        let (end, logs_dir) = self.download_once(run, required, deadline, failures, source).await?;
        if end != WatchEnd::Idle {
            return Ok((end, logs_dir));
        }
        run.report("Keine Aktivität mehr, der Client wird einmal neu gestartet …");
        self.download_once(run, required, deadline, failures, source).await
    }

    /// Ein Start des Clients für alle noch offenen Mods.
    async fn download_once(
        &self,
        run: &SyncRun<'_>,
        required: &[ModEntry],
        deadline: Instant,
        failures: &mut HashMap<String, String>,
        source: Source,
    ) -> Result<(WatchEnd, PathBuf), Stop> {
        let mods_root = run.mods_root();
        let open: Vec<ModEntry> = compare(required, &scan(&mods_root))
            .into_iter()
            .filter(|mod_state| mod_state.state != ModState::Current)
            .map(|mod_state| mod_state.required)
            .collect();
        run.report(format!("Spielclient lädt {} Mods …", open.len()));
        let (logs_dir, profile_dir) = self.prepare_run_folders()?;

        let list_file = ListFileGuard(modset_file::for_client_profile(&profile_dir));
        let arguments = match source {
            Source::Server => build_download(
                &run.client_address,
                &open,
                run.profile.download_dir.as_deref(),
                &run.settings.client,
                &logs_dir.to_string_lossy(),
                &profile_dir.to_string_lossy(),
                &self.helper_root.to_string_lossy(),
            ),
            Source::Modset => {
                let open_ids: HashSet<String> = open.iter().map(|mod_entry| mod_entry.id.to_uppercase()).collect();
                modset_file::write(&list_file.0, required, &open_ids)?;
                build_modset_download(
                    run.profile.download_dir.as_deref(),
                    &run.settings.client,
                    &logs_dir.to_string_lossy(),
                    &profile_dir.to_string_lossy(),
                    &self.helper_root.to_string_lossy(),
                )
            }
        };

        let idle_minutes = u64::try_from(run.settings.timeouts.idle_minutes).unwrap_or(10);
        let watch = DownloadWatch {
            required,
            mods_root: &mods_root,
            logs_dir: &logs_dir,
            idle_limit: Duration::from_secs(idle_minutes * 60),
            deadline,
            failures,
            progress: run.progress,
        };
        let client = self.start_client(run, &arguments, &profile_dir, source)?;
        let end = self.watcher.watch(client.client.as_ref(), watch, run.cancel).await;
        client.finish().await;
        drop(list_file);
        Ok((end.ok_or(Stop::Cancelled)?, logs_dir))
    }

    /// Schreibt bei Bedarf die Passwortdatei und startet den Client. Der Lauf ruft danach in jedem Fall
    /// [`RunningClient::finish`] auf. Wird er vorher verworfen, löscht der Wächter die Datei trotzdem,
    /// und das Job Object beendet den Client. Der Modus ohne Server braucht kein Passwort.
    fn start_client(&self, run: &SyncRun<'_>, arguments: &[String], profile_dir: &Path, source: Source) -> Result<RunningClient, Stop> {
        let guard = PasswordFileGuard(password_file::for_client_profile(profile_dir));
        let mut password = match source {
            Source::Server => self.credentials.read_password(run.address(), &run.profile.id)?,
            Source::Modset => None,
        };
        let prepared = password_file::prepare(&guard.0, password.as_deref());
        password.zeroize();
        prepared?;

        let client = (self.start_client)(&run.client_exe, arguments, run.settings.client.hide_window)
            .map_err(|error| Stop::Failed(format!("Der Spielclient konnte nicht gestartet werden: {error}")))?;
        Ok(RunningClient { client, _password_file: guard })
    }

    fn prepare_run_folders(&self) -> Result<(PathBuf, PathBuf), Stop> {
        let logs_dir = retention::create_run_folder(&self.paths.client_logs())?;
        let profile_dir = self.paths.client_profile();
        fs::create_dir_all(&profile_dir)?;
        Ok((logs_dir, profile_dir))
    }
}

enum Fallback {
    Done { end: WatchEnd, logs_dir: PathBuf, mismatch: Option<String> },
    Skipped(String),
}

/// Anfang der Warnung, wenn ARMM und Server-Eintrag verschiedene Listen haben.
const MISMATCH_PREFIX: &str = "Achtung: Die Liste in ARMM weicht vom laufenden Server ab";

/// Vergleicht ARMM-Liste und Server-Eintrag nach ID und Version. `None`: gleich.
pub fn list_mismatch(armm: &[ModEntry], server: &[ModEntry]) -> Option<String> {
    let key = |mod_entry: &ModEntry| (mod_entry.id.to_uppercase(), mod_entry.version.clone().unwrap_or_default());
    let armm_keys: HashSet<_> = armm.iter().map(key).collect();
    let server_keys: HashSet<_> = server.iter().map(key).collect();
    let mut differing: Vec<&str> = armm
        .iter()
        .filter(|mod_entry| !server_keys.contains(&key(mod_entry)))
        .chain(server.iter().filter(|mod_entry| !armm_keys.contains(&key(mod_entry))))
        .map(|mod_entry| mod_entry.name.as_str())
        .collect();
    differing.dedup();
    if differing.is_empty() {
        return None;
    }

    let examples = differing.iter().take(3).copied().collect::<Vec<_>>().join(", ");
    Some(format!(
        "{MISMATCH_PREFIX} ({} Unterschiede, z. B. {examples}). Überzählige Mods werden deshalb nicht gelöscht. Bitte den Export in ARMM prüfen.",
        differing.len()
    ))
}

fn count_open(comparison: &[ModComparison]) -> usize {
    comparison.iter().filter(|mod_state| mod_state.state != ModState::Current).count()
}

fn deadline(run: &SyncRun<'_>) -> Instant {
    let minutes = u64::try_from(run.settings.timeouts.total_minutes).unwrap_or(120);
    Instant::now() + Duration::from_secs(minutes * 60)
}

fn ensure_free_space(mods_root: &Path) -> Result<(), Stop> {
    fs::create_dir_all(mods_root)?;
    let free = free_bytes(mods_root);
    if free < MINIMUM_FREE_BYTES {
        return Err(Stop::Failed(format!("Zu wenig freier Speicher im Zielordner ({} MB frei).", free / (1024 * 1024))));
    }
    Ok(())
}

/// Ergebnis nach dem Laden. `note`: Zusatz für den Bericht, etwa eine Warnung zur Liste.
fn conclude(
    comparison: Vec<ModComparison>,
    end: WatchEnd,
    logs_dir: &Path,
    failures: &HashMap<String, String>,
    note: Option<String>,
) -> SyncResult {
    let suffix = note.map(|note| format!(" {note}")).unwrap_or_default();
    let remaining: Vec<&ModComparison> = comparison.iter().filter(|mod_state| mod_state.state != ModState::Current).collect();
    if remaining.is_empty() {
        let message = format!("Fertig: alle {} Mods sind aktuell.{suffix}", comparison.len());
        return SyncResult::new(SyncOutcome::Completed, message, comparison);
    }

    let reason = match end {
        WatchEnd::ClientExited if contains_crash_log(logs_dir) => "Der Spielclient ist abgestürzt.",
        WatchEnd::ClientExited => "Der Spielclient hat sich vorzeitig beendet.",
        WatchEnd::Idle => "Der Download zeigt keine Aktivität mehr.",
        WatchEnd::TotalTimeout => "Die maximale Laufzeit wurde erreicht.",
        WatchEnd::AllCurrent | WatchEnd::HelperDone => "Der Download wurde nicht abgeschlossen.",
    };
    let names: Vec<String> = remaining
        .iter()
        .take(5)
        .map(|mod_state| match failures.get(&mod_state.required.id.to_uppercase()) {
            Some(failure) => format!("{} ({})", mod_state.required.name, failure_text(failure)),
            None => mod_state.required.name.clone(),
        })
        .collect();
    let more = if remaining.len() > 5 { format!(" und {} weitere", remaining.len() - 5) } else { String::new() };
    let message =
        format!("{} Mods fehlen noch: {}{more}. {reason}{suffix} Client-Logs: {}", remaining.len(), names.join(", "), logs_dir.display());
    SyncResult::new(SyncOutcome::Incomplete, message, comparison)
}

/// Gründe der Hilfs-Mod in Worten.
fn failure_text(reason: &str) -> &str {
    match reason {
        "not_found" => "im Workshop nicht gefunden",
        "version_unavailable" => "Version nicht mehr im Workshop",
        other => other,
    }
}

/// Löscht die Passwortdatei, sobald der Lauf mit dem Client endet, auch bei Abbruch oder Fehler.
struct PasswordFileGuard(PathBuf);

impl Drop for PasswordFileGuard {
    fn drop(&mut self) {
        password_file::try_delete(&self.0);
    }
}

/// Räumt die Listendatei für `-rmsModset` weg. Sie ist nicht geheim, soll aber keinen alten Stand hinterlassen.
struct ListFileGuard(PathBuf);

impl Drop for ListFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct RunningClient {
    client: Box<dyn ClientProcess>,
    _password_file: PasswordFileGuard,
}

impl RunningClient {
    /// Client beenden, danach räumt der Wächter die Passwortdatei weg.
    async fn finish(self) {
        self.client.stop().await;
    }
}

fn cancelled() -> SyncResult {
    SyncResult::new(SyncOutcome::Cancelled, "Abgebrochen. Bereits geladene Mods bleiben erhalten.", Vec::new())
}

fn contains_crash_log(folder: &Path) -> bool {
    fs::read_dir(folder).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let path = entry.path();
            if path.is_dir() { contains_crash_log(&path) } else { entry.file_name().eq_ignore_ascii_case("crash.log") }
        })
    })
}

fn free_bytes(folder: &Path) -> u64 {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let path = crate::win::wide(crate::paths::full_path(folder).as_os_str());
    let mut free = 0u64;
    // SAFETY: Pfad nullterminiert, Ausgabe ist eine lokale Zahl.
    match unsafe { GetDiskFreeSpaceExW(crate::win::pcwstr(&path), Some(&raw mut free), None, None) } {
        Ok(()) => free,
        // Unbekannt: nicht am Platz scheitern lassen, der Client meldet selbst, wenn die Platte voll ist.
        Err(_) => u64::MAX,
    }
}
