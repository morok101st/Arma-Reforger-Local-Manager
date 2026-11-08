//! Beobachtet einen laufenden Download über das Log des Clients und die `meta`-Dateien im Zielordner.
//! Gelesen werden nur die Mods, die noch nicht aktuell sind: Eine fertige Mod ändert sich nicht mehr.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::{Progress, SyncStatus};
use crate::addons::{InstalledAddon, ModEntry, ModState, compare, id_of_folder, scan_only};
use crate::client::ClientProcess;
use crate::client::log::{ClientLogEvent, LogTail, classify, parse_mod_failed};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEnd {
    AllCurrent,
    HelperDone,
    ClientExited,
    Idle,
    TotalTimeout,
}

/// Was ein Download-Lauf überwacht. `failures`: gescheiterte Mods mit Grund aus `[RMS] MOD_FAILED`,
/// bleibt über einen Neustart erhalten (Schlüssel in Großbuchstaben).
pub struct DownloadWatch<'a> {
    pub required: &'a [ModEntry],
    pub mods_root: &'a Path,
    pub logs_dir: &'a Path,
    pub idle_limit: Duration,
    pub deadline: Instant,
    pub failures: &'a mut HashMap<String, String>,
    pub progress: &'a Progress,
}

#[derive(Debug, Clone)]
pub struct DownloadWatcher {
    poll_interval: Duration,
    temp_addons_dir: PathBuf,
}

impl Default for DownloadWatcher {
    fn default() -> Self {
        Self::new(Duration::from_secs(3), std::env::temp_dir().join("addons"))
    }
}

impl DownloadWatcher {
    pub fn new(poll_interval: Duration, temp_addons_dir: impl Into<PathBuf>) -> Self {
        Self { poll_interval, temp_addons_dir: temp_addons_dir.into() }
    }

    /// `None`: abgebrochen.
    pub async fn watch(&self, client: &dyn ClientProcess, watch: DownloadWatch<'_>, cancel: &CancellationToken) -> Option<WatchEnd> {
        let mut tail = LogTail::new(watch.logs_dir.join("console.log"));
        let mut finished: HashMap<String, InstalledAddon> = HashMap::new();
        let mut last_activity = Instant::now();
        let mut last_bytes = None;
        let mut last_done = None;
        let mut helper_done = false;

        loop {
            tokio::select! {
                () = cancel.cancelled() => return None,
                () = tokio::time::sleep(self.poll_interval) => {}
            }

            for line in tail.read_new_lines() {
                let event = classify(&line);
                if event == ClientLogEvent::None {
                    continue;
                }

                if event != ClientLogEvent::BackendError {
                    last_activity = Instant::now();
                }

                if let Some((id, reason)) = parse_mod_failed(&line) {
                    watch.failures.insert(id.to_uppercase(), reason);
                }

                helper_done |= event == ClientLogEvent::HelperDone;
                (watch.progress)(SyncStatus::message(format!("[client] {line}")));
            }

            let open: HashSet<String> = watch
                .required
                .iter()
                .map(|mod_entry| mod_entry.id.clone())
                .filter(|id| !finished.contains_key(&id.to_uppercase()))
                .collect();
            let mut installed = scan_only(watch.mods_root, &open);
            installed.extend(finished.values().cloned());
            let comparison = compare(watch.required, &installed);
            for mod_state in comparison.iter().filter(|mod_state| mod_state.state == ModState::Current) {
                if let Some(addon) = &mod_state.installed {
                    finished.insert(mod_state.required.id.to_uppercase(), addon.clone());
                }
            }

            let done = comparison.iter().filter(|mod_state| mod_state.state == ModState::Current).count();
            if last_done != Some(done) {
                last_done = Some(done);
                last_activity = Instant::now();
                let total = comparison.len();
                (watch.progress)(SyncStatus {
                    message: format!("{done} von {total} Mods fertig"),
                    done,
                    total,
                    mods: Some(comparison.clone()),
                });
            }

            if done == comparison.len() {
                return Some(WatchEnd::AllCurrent);
            }

            if helper_done {
                return Some(WatchEnd::HelperDone);
            }

            if client.has_exited() {
                return Some(WatchEnd::ClientExited);
            }

            // Große Mods schreiben lange keine Logzeile, wachsende Dateien zählen deshalb als Aktivität.
            let bytes = open_mod_bytes(watch.mods_root, &open) + folder_bytes(&self.temp_addons_dir);
            if last_bytes != Some(bytes) {
                last_bytes = Some(bytes);
                last_activity = Instant::now();
            }

            if last_activity.elapsed() > watch.idle_limit {
                return Some(WatchEnd::Idle);
            }

            if Instant::now() > watch.deadline {
                return Some(WatchEnd::TotalTimeout);
            }
        }
    }
}

fn open_mod_bytes(mods_root: &Path, open_ids: &HashSet<String>) -> u64 {
    let upper: HashSet<String> = open_ids.iter().map(|id| id.to_uppercase()).collect();
    fs::read_dir(mods_root)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|folder| id_of_folder(folder).is_some_and(|id| upper.contains(&id.to_uppercase())))
                .map(|folder| folder_bytes(&folder))
                .sum()
        })
        .unwrap_or(0)
}

/// Größe aller Dateien eines Ordners. Unlesbares wird übersprungen, Verknüpfungen werden nicht verfolgt.
pub fn folder_bytes(folder: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(folder) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() && !kind.is_symlink() => folder_bytes(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|info| info.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}
