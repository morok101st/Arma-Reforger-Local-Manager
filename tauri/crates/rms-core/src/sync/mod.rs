//! Ablauf eines Syncs: Mod-Liste lesen, zwischenspeichern, Download überwachen und Zielordner bereinigen.

mod cache;
mod cleanup;
mod list_reader;
mod lock;
mod orchestrator;
mod watcher;

pub use cache::{CachedModList, ModListCache};
pub use cleanup::{CleanupPlan, CleanupResult, SurplusMod, delete_surplus, format_size, plan_cleanup, select_surplus};
pub use list_reader::{ServerListError, ServerListReader};
pub use lock::{SYNC_LOCK_NAME, SyncLock};
pub use orchestrator::{Precheck, Progress, SyncOrchestrator, SyncOutcome, SyncResult, SyncStatus};
pub use watcher::{DownloadWatch, DownloadWatcher, WatchEnd, folder_bytes};
