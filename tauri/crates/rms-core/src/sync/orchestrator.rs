//! Holt die Mod-Liste über die Hilfs-Mod aus dem Server-Eintrag im Backend, vergleicht sie mit dem Zielordner
//! und lädt die offenen Mods. Dafür läuft der Spielclient zweimal kurz. Es entsteht keine Verbindung zum
//! Spielserver und kein Beitritt.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use zeroize::Zeroize;

use super::watcher::{DownloadWatch, DownloadWatcher, WatchEnd};
use super::{ModListCache, ServerListError, ServerListReader, SyncLock};
use crate::addons::{ModComparison, ModEntry, ModState, compare, helper_root, resolve_mods_root, scan};
use crate::client::args::{build_download, build_list_request};
use crate::client::{ClientProcess, ClientStarter, default_starter, password_file, precheck, retention};
use crate::credentials::CredentialStore;
use crate::paths::AppPaths;
use crate::settings::{AppSettings, ServerProfile};

const MINIMUM_FREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SyncOutcome {
    /// Nur geprüft: Die Liste kam vom Server, es wurde nichts geladen.
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
}

impl SyncResult {
    fn failed(message: impl Into<String>) -> Self {
        Self { outcome: SyncOutcome::Failed, message: message.into(), mods: Vec::new() }
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

/// Alles, was ein Lauf durchreicht.
struct SyncRun<'a> {
    settings: &'a AppSettings,
    profile: &'a ServerProfile,
    client_exe: PathBuf,
    progress: &'a Progress,
    cancel: &'a CancellationToken,
}

impl SyncRun<'_> {
    fn address(&self) -> &str {
        self.profile.server_address.as_deref().unwrap_or_default().trim()
    }

    fn mods_root(&self) -> PathBuf {
        resolve_mods_root(self.profile.download_dir.as_deref())
    }

    fn report(&self, message: impl Into<String>) {
        (self.progress)(SyncStatus::message(message));
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

    pub fn cache(&self) -> &ModListCache {
        &self.cache
    }

    /// Holt nur die Liste und vergleicht.
    pub async fn check(
        &self,
        settings: &AppSettings,
        profile: &ServerProfile,
        progress: &Progress,
        cancel: &CancellationToken,
    ) -> SyncResult {
        self.run_exclusive(cancel, async {
            let run = self.start_run(settings, profile, progress, cancel)?;
            let required = self.fetch_list(&run).await?;
            let comparison = compare(&required, &scan(&run.mods_root()));
            Ok(SyncResult {
                outcome: SyncOutcome::Checked,
                message: format!("Liste vom Server geholt: {} Mods.", comparison.len()),
                mods: comparison,
            })
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
            let run = self.start_run(settings, profile, progress, cancel)?;
            let required = self.fetch_list(&run).await?;
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
        Ok(SyncRun { settings, profile, client_exe, progress, cancel })
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

    async fn fetch_list(&self, run: &SyncRun<'_>) -> Result<Vec<ModEntry>, Stop> {
        run.report("Mod-Liste wird vom Server geholt …");
        let (logs_dir, profile_dir) = self.prepare_run_folders()?;
        let arguments = build_list_request(
            run.address(),
            run.profile.download_dir.as_deref(),
            &run.settings.client,
            &logs_dir.to_string_lossy(),
            &profile_dir.to_string_lossy(),
            &self.helper_root.to_string_lossy(),
        );
        let client = self.start_client(run, &arguments, &profile_dir)?;
        let mods = self.list_reader.read(client.client.as_ref(), &logs_dir, run.address(), run.cancel).await;
        client.finish().await;
        let mods = mods?;
        self.cache.save(&run.profile.id, &mods)?;
        Ok(mods)
    }

    async fn download(&self, run: &SyncRun<'_>, required: &[ModEntry]) -> Result<SyncResult, Stop> {
        let mods_root = run.mods_root();
        let mut comparison = compare(required, &scan(&mods_root));
        if comparison.iter().all(|mod_state| mod_state.state == ModState::Current) {
            return Ok(SyncResult { outcome: SyncOutcome::AlreadyCurrent, message: "Alles aktuell.".into(), mods: comparison });
        }

        fs::create_dir_all(&mods_root)?;
        let free = free_bytes(&mods_root);
        if free < MINIMUM_FREE_BYTES {
            return Err(Stop::Failed(format!("Zu wenig freier Speicher im Zielordner ({} MB frei).", free / (1024 * 1024))));
        }

        let minutes = u64::try_from(run.settings.timeouts.total_minutes).unwrap_or(120);
        let deadline = Instant::now() + Duration::from_secs(minutes * 60);
        let mut failures = HashMap::new();

        let (mut end, mut logs_dir) = self.download_once(run, required, &comparison, deadline, &mut failures).await?;
        if end == WatchEnd::Idle {
            run.report("Keine Aktivität mehr, der Client wird einmal neu gestartet …");
            comparison = compare(required, &scan(&mods_root));
            (end, logs_dir) = self.download_once(run, required, &comparison, deadline, &mut failures).await?;
        }

        comparison = compare(required, &scan(&mods_root));
        let remaining: Vec<&ModComparison> = comparison.iter().filter(|mod_state| mod_state.state != ModState::Current).collect();
        if remaining.is_empty() {
            return Ok(SyncResult {
                outcome: SyncOutcome::Completed,
                message: format!("Fertig: alle {} Mods sind aktuell.", comparison.len()),
                mods: comparison,
            });
        }

        let reason = match end {
            WatchEnd::ClientExited if contains_crash_log(&logs_dir) => "Der Spielclient ist abgestürzt.",
            WatchEnd::ClientExited => "Der Spielclient hat sich vorzeitig beendet.",
            WatchEnd::Idle => "Der Download zeigt keine Aktivität mehr.",
            WatchEnd::TotalTimeout => "Die maximale Laufzeit wurde erreicht.",
            WatchEnd::AllCurrent | WatchEnd::HelperDone => "Der Download wurde nicht abgeschlossen.",
        };
        let names: Vec<String> = remaining
            .iter()
            .take(5)
            .map(|mod_state| match failures.get(&mod_state.required.id.to_uppercase()) {
                Some(failure) => format!("{} ({failure})", mod_state.required.name),
                None => mod_state.required.name.clone(),
            })
            .collect();
        let more = if remaining.len() > 5 { format!(" und {} weitere", remaining.len() - 5) } else { String::new() };
        Ok(SyncResult {
            outcome: SyncOutcome::Incomplete,
            message: format!(
                "{} Mods fehlen noch: {}{more}. {reason} Client-Logs: {}",
                remaining.len(),
                names.join(", "),
                logs_dir.display()
            ),
            mods: comparison,
        })
    }

    /// Ein Start des Clients für alle noch offenen Mods.
    async fn download_once(
        &self,
        run: &SyncRun<'_>,
        required: &[ModEntry],
        comparison: &[ModComparison],
        deadline: Instant,
        failures: &mut HashMap<String, String>,
    ) -> Result<(WatchEnd, PathBuf), Stop> {
        let open: Vec<ModEntry> = comparison
            .iter()
            .filter(|mod_state| mod_state.state != ModState::Current)
            .map(|mod_state| mod_state.required.clone())
            .collect();
        run.report(format!("Spielclient lädt {} Mods …", open.len()));
        let (logs_dir, profile_dir) = self.prepare_run_folders()?;
        let arguments = build_download(
            run.address(),
            &open,
            run.profile.download_dir.as_deref(),
            &run.settings.client,
            &logs_dir.to_string_lossy(),
            &profile_dir.to_string_lossy(),
            &self.helper_root.to_string_lossy(),
        );
        let mods_root = run.mods_root();
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
        let client = self.start_client(run, &arguments, &profile_dir)?;
        let end = self.watcher.watch(client.client.as_ref(), watch, run.cancel).await;
        client.finish().await;
        Ok((end.ok_or(Stop::Cancelled)?, logs_dir))
    }

    /// Schreibt die Passwortdatei und startet den Client. Der Lauf ruft danach in jedem Fall
    /// [`RunningClient::finish`] auf. Wird er vorher verworfen, löscht der Wächter die Datei trotzdem,
    /// und das Job Object beendet den Client.
    fn start_client(&self, run: &SyncRun<'_>, arguments: &[String], profile_dir: &Path) -> Result<RunningClient, Stop> {
        let guard = PasswordFileGuard(password_file::for_client_profile(profile_dir));
        let mut password = self.credentials.read_password(run.address(), &run.profile.id)?;
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

/// Löscht die Passwortdatei, sobald der Lauf mit dem Client endet, auch bei Abbruch oder Fehler.
struct PasswordFileGuard(PathBuf);

impl Drop for PasswordFileGuard {
    fn drop(&mut self) {
        password_file::try_delete(&self.0);
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
    SyncResult { outcome: SyncOutcome::Cancelled, message: "Abgebrochen. Bereits geladene Mods bleiben erhalten.".into(), mods: Vec::new() }
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
