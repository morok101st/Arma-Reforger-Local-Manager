mod common;

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::FakeClient;
use rms_core::addons::{AddonHealth, InstalledAddon, ModEntry, client_default_folder, folder_key};
use rms_core::client::log::parse_list_done_count;
use rms_core::client::{ClientProcess, ClientStarter};
use rms_core::credentials::CredentialStore;
use rms_core::paths::AppPaths;
use rms_core::settings::{AppSettings, ServerProfile};
use rms_core::sync::{
    DownloadWatch, DownloadWatcher, ModListCache, Progress, ServerListReader, SyncOrchestrator, SyncOutcome, SyncStatus, WatchEnd,
    delete_surplus, format_size, plan_cleanup, select_surplus,
};
use tokio_util::sync::CancellationToken;

const ID_A: &str = "1A2B3C4D5E6F7001";
const ID_B: &str = "1A2B3C4D5E6F7002";

fn write_addon(mods_root: &Path, id: &str, name: &str, version: &str) -> PathBuf {
    let folder = mods_root.join(format!("Mod_{id}"));
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("data.pak"), "12345").unwrap();
    fs::write(
        folder.join("meta"),
        format!(
            r#"{{"meta":{{"id":"{id}","name":"{name}","versions":[{{"version":"{version}","package":{{"files":[{{"size":5,"name":"data.pak"}}]}}}}],"selectedRev":0}}}}"#
        ),
    )
    .unwrap();
    folder
}

fn collecting_progress() -> (Progress, Arc<Mutex<Vec<SyncStatus>>>) {
    let reports = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&reports);
    (Arc::new(move |status| sink.lock().unwrap().push(status)), reports)
}

// --- Download-Überwachung ---

struct Setup {
    temp: tempfile::TempDir,
    failures: HashMap<String, String>,
    progress: Progress,
    reports: Arc<Mutex<Vec<SyncStatus>>>,
}

impl Setup {
    fn new() -> Self {
        let temp = common::temp_dir();
        for folder in ["mods/addons", "logs", "tempaddons"] {
            fs::create_dir_all(temp.path().join(folder)).unwrap();
        }
        let (progress, reports) = collecting_progress();
        Self { temp, failures: HashMap::new(), progress, reports }
    }

    fn mods_root(&self) -> PathBuf {
        self.temp.path().join("mods").join("addons")
    }

    fn logs_dir(&self) -> PathBuf {
        self.temp.path().join("logs")
    }

    fn temp_addons(&self) -> PathBuf {
        self.temp.path().join("tempaddons")
    }

    fn watcher(&self) -> DownloadWatcher {
        DownloadWatcher::new(Duration::from_millis(10), self.temp_addons())
    }
}

fn required() -> Vec<ModEntry> {
    vec![ModEntry::new(ID_A, "Beispiel Fahrzeuge", Some("1.4.0")), ModEntry::new(ID_B, "Beispiel Karte Nord", Some("2.1.3"))]
}

async fn watch(setup: &mut Setup, client: &FakeClient, idle: Duration, deadline: Instant, cancel: &CancellationToken) -> Option<WatchEnd> {
    let required = required();
    let (mods_root, logs_dir, watcher) = (setup.mods_root(), setup.logs_dir(), setup.watcher());
    let watch = DownloadWatch {
        required: &required,
        mods_root: &mods_root,
        logs_dir: &logs_dir,
        idle_limit: idle,
        deadline,
        failures: &mut setup.failures,
        progress: &setup.progress,
    };
    watcher.watch(client, watch, cancel).await
}

fn later(seconds: u64) -> Instant {
    Instant::now() + Duration::from_secs(seconds)
}

#[tokio::test]
async fn ends_when_all_mods_are_current() {
    let mut setup = Setup::new();
    write_addon(&setup.mods_root(), ID_A, "Mod", "1.4.0");
    let mods_root = setup.mods_root();
    let writer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        write_addon(&mods_root, ID_B, "Mod", "2.1.3");
    });

    let end = watch(&mut setup, &FakeClient::default(), Duration::from_secs(30), later(300), &CancellationToken::new()).await;

    writer.await.unwrap();
    assert_eq!(end, Some(WatchEnd::AllCurrent));
    assert!(
        setup.reports.lock().unwrap().iter().any(|status| status.message == "2 von 2 Mods fertig" && status.done == 2 && status.total == 2)
    );
}

#[tokio::test]
async fn helper_done_ends_the_watch_and_failures_are_kept() {
    let mut setup = Setup::new();
    write_addon(&setup.mods_root(), ID_A, "Mod", "1.4.0");
    common::write_log(&setup.logs_dir(), &common::sample("download.log"));

    let end = watch(&mut setup, &FakeClient::default(), Duration::from_secs(30), later(300), &CancellationToken::new()).await;

    assert_eq!(end, Some(WatchEnd::HelperDone));
    assert_eq!(setup.failures[ID_B], "download failed");
    assert!(setup.reports.lock().unwrap().iter().any(|status| status.message.ends_with("[RMS] MOD_DONE 1A2B3C4D5E6F7001 1.4.0")));
}

#[tokio::test]
async fn an_outdated_mod_is_not_yet_finished() {
    let mut setup = Setup::new();
    write_addon(&setup.mods_root(), ID_A, "Mod", "1.4.0");
    write_addon(&setup.mods_root(), ID_B, "Mod", "2.0.0");

    let end = watch(&mut setup, &FakeClient::exited(), Duration::from_secs(30), later(300), &CancellationToken::new()).await;

    assert_eq!(end, Some(WatchEnd::ClientExited));
    assert!(setup.reports.lock().unwrap().iter().any(|status| status.done == 1 && status.total == 2));
}

#[tokio::test]
async fn without_any_activity_the_watch_reports_idle() {
    let mut setup = Setup::new();

    let end = watch(&mut setup, &FakeClient::default(), Duration::from_millis(100), later(300), &CancellationToken::new()).await;

    assert_eq!(end, Some(WatchEnd::Idle));
}

#[tokio::test]
async fn growing_files_count_as_activity() {
    let mut setup = Setup::new();
    let part = setup.temp_addons().join("part.tmp");
    // Ohne Logzeile, nur ein wachsender Download im Temp-Ordner: Das darf nicht als Stillstand gelten.
    let writer = tokio::spawn(async move {
        for _ in 0..9 {
            OpenOptions::new().create(true).append(true).open(&part).unwrap().write_all(&[b'x'; 100]).unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    let end = watch(
        &mut setup,
        &FakeClient::default(),
        Duration::from_millis(300),
        Instant::now() + Duration::from_millis(900),
        &CancellationToken::new(),
    )
    .await;

    writer.await.unwrap();
    assert_eq!(end, Some(WatchEnd::TotalTimeout));
}

#[tokio::test]
async fn cancelling_stops_the_watch() {
    let mut setup = Setup::new();
    let cancel = CancellationToken::new();
    let stopper = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        stopper.cancel();
    });

    assert_eq!(watch(&mut setup, &FakeClient::default(), Duration::from_secs(30), later(300), &cancel).await, None);
}

// --- Bereinigen ---

const ID_C: &str = "CCCCCCCCCCCCCCCC";
const ID_D: &str = "DDDDDDDDDDDDDDDD";

fn mod_entry(id: &str) -> ModEntry {
    ModEntry::new(id, &format!("Mod {}", &id[..1]), Some("1.0.0"))
}

fn profile(id: &str, dir: Option<&str>) -> ServerProfile {
    ServerProfile {
        server_address: Some("203.0.113.10:2001".into()),
        download_dir: dir.map(String::from),
        ..ServerProfile::new(id, &format!("Server {id}"))
    }
}

fn target_addon(target: &Path, id: &str) -> PathBuf {
    write_addon(&target.join("addons"), id, &format!("Mod {}", &id[..1]), "1.0.0")
}

#[test]
fn plans_only_mods_that_no_server_of_the_folder_needs() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("cache"));
    let target = temp.path().join("mods");
    for id in [ID_A, ID_B, ID_C] {
        target_addon(&target, id);
    }
    let surplus = target_addon(&target, ID_D);
    let target_text = target.to_string_lossy().into_owned();
    let current = profile("1", Some(&target_text));
    let other = profile("2", Some(&format!("{target_text}\\")));
    cache.save(&other.id, &[mod_entry(ID_B), mod_entry(ID_C)]).unwrap();

    let plan = plan_cleanup(&cache, &current, &[mod_entry(ID_A), mod_entry(ID_B)], &[current.clone(), other]);

    assert_eq!(plan.blocker, None);
    assert_eq!(plan.surplus.len(), 1);
    assert_eq!(plan.surplus[0].folder, surplus);
    assert_eq!(plan.surplus[0].bytes, 5 + fs::metadata(surplus.join("meta")).unwrap().len());
}

#[test]
fn server_with_another_folder_protects_nothing() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("cache"));
    let target = temp.path().join("mods");
    target_addon(&target, ID_A);
    let surplus = target_addon(&target, ID_B);
    let current = profile("1", Some(&target.to_string_lossy()));
    let other = profile("2", Some(&temp.path().join("andere").to_string_lossy()));
    cache.save(&other.id, &[mod_entry(ID_B)]).unwrap();

    let plan = plan_cleanup(&cache, &current, &[mod_entry(ID_A)], &[current.clone(), other]);

    assert_eq!(plan.surplus.iter().map(|mod_entry| &mod_entry.folder).collect::<Vec<_>>(), [&surplus]);
}

#[test]
fn missing_list_of_a_server_with_the_same_folder_blocks_deleting() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("cache"));
    let target = temp.path().join("mods");
    target_addon(&target, ID_A);
    target_addon(&target, ID_B);
    let current = profile("1", Some(&target.to_string_lossy()));
    let other = profile("2", Some(&target.to_string_lossy().to_uppercase()));

    let plan = plan_cleanup(&cache, &current, &[mod_entry(ID_A)], &[current.clone(), other]);

    assert!(plan.surplus.is_empty());
    assert!(plan.blocker.unwrap().contains("Server 2"));
}

#[test]
fn empty_lists_block_deleting() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("cache"));
    let target = temp.path().join("mods");
    target_addon(&target, ID_B);
    let current = profile("1", Some(&target.to_string_lossy()));
    let other = profile("2", Some(&target.to_string_lossy()));
    cache.save(&other.id, &[]).unwrap();

    let of_other = plan_cleanup(&cache, &current, &[mod_entry(ID_A)], &[current.clone(), other]);
    let of_current = plan_cleanup(&cache, &current, &[], std::slice::from_ref(&current));

    assert!(of_other.surplus.is_empty() && of_other.blocker.is_some());
    assert!(of_current.surplus.is_empty() && of_current.blocker.is_some());
}

#[test]
fn default_folder_is_never_touched() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("cache"));
    let explicit = client_default_folder().join("addons").to_string_lossy().into_owned();
    for dir in [None, Some(""), Some(explicit.as_str())] {
        let current = profile("1", dir);

        let plan = plan_cleanup(&cache, &current, &[mod_entry(ID_A)], std::slice::from_ref(&current));

        assert!(plan.surplus.is_empty() && plan.blocker.is_none(), "{dir:?}");
    }
}

#[test]
fn folders_that_are_not_clearly_mods_stay() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("cache"));
    let target = temp.path().join("mods");
    target_addon(&target, ID_A);
    fs::create_dir_all(target.join("addons").join("saves")).unwrap();
    fs::create_dir_all(target.join("addons").join(format!("OhneMeta_{ID_B}"))).unwrap();
    let current = profile("1", Some(&target.to_string_lossy()));

    let plan = plan_cleanup(&cache, &current, &[mod_entry(ID_A)], std::slice::from_ref(&current));

    assert!(plan.surplus.is_empty());
}

#[test]
fn damaged_surplus_mod_is_selected_but_unknown_folder_is_not() {
    let addon = |id: &str, name: Option<&str>, health| InstalledAddon {
        id: id.into(),
        name: name.map(String::from),
        version: Some("1".into()),
        folder: id.into(),
        health,
    };
    let installed = vec![
        addon(ID_A, Some("A"), AddonHealth::Incomplete),
        addon(ID_B, None, AddonHealth::UnknownMetadata),
        addon(ID_C, Some("C"), AddonHealth::Ok),
    ];

    let surplus = select_surplus(installed, &HashSet::from([ID_C.to_string()]));

    assert_eq!(surplus.iter().map(|addon| addon.id.as_str()).collect::<Vec<_>>(), [ID_A]);
}

#[test]
fn delete_removes_only_planned_folders_and_a_locked_mod_does_not_stop_the_others() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("cache"));
    let target = temp.path().join("mods");
    let keep = target_addon(&target, ID_A);
    let locked = target_addon(&target, ID_B);
    let free = target_addon(&target, ID_C);
    let current = profile("1", Some(&target.to_string_lossy()));
    let plan = plan_cleanup(&cache, &current, &[mod_entry(ID_A)], std::slice::from_ref(&current));

    let result = {
        let _file = OpenOptions::new().read(true).share_mode(0).open(locked.join("data.pak")).unwrap();
        delete_surplus(&plan)
    };

    assert!(keep.exists());
    assert_eq!(result.failed.iter().map(|mod_entry| &mod_entry.folder).collect::<Vec<_>>(), [&locked]);
    assert_eq!(result.deleted.iter().map(|mod_entry| &mod_entry.folder).collect::<Vec<_>>(), [&free]);
    assert!(!free.exists());
    assert_eq!(result.summary(), "1 überzählige Mod gelöscht (0,0 MB frei). Nicht löschbar: Mod 1.");
}

#[test]
fn folder_key_matches_equal_targets() {
    let cases = [
        (Some(r"D:\Mods"), Some(r"d:\mods\"), true),
        (Some(r"D:\Mods"), Some(r"D:\Mods\addons"), true),
        (Some(r"D:\Mods"), Some(r"D:\Mods2"), false),
        (None, Some(""), true),
    ];
    for (left, right, same) in cases {
        assert_eq!(folder_key(left) == folder_key(right), same, "{left:?} {right:?}");
    }
}

#[test]
fn reads_mod_count_of_the_list_end() {
    let cases = [
        ("12:00:01 SCRIPT : [RMS] LIST_DONE mods=42", Some(42)),
        ("[RMS] LIST_DONE mods=0", Some(0)),
        ("[RMS] LIST_DONE", None),
        ("[RMS] ROOM_NOT_FOUND", None),
    ];
    for (line, expected) in cases {
        assert_eq!(parse_list_done_count(line), expected);
    }
}

#[test]
fn formats_sizes_with_a_german_comma() {
    assert_eq!(format_size(5 * 1024 * 1024), "5,0 MB");
    assert_eq!(format_size(3 * 1024 * 1024 * 1024 / 2), "1,5 GB");
}

// --- Ablauf mit nachgebautem Client ---

#[derive(Default)]
struct NoPasswords;

impl CredentialStore for NoPasswords {
    fn read_password(&self, _: &str, _: &str) -> std::io::Result<Option<String>> {
        Ok(Some("geheim".into()))
    }

    fn save_password(&self, _: &str, _: &str, _: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn delete_password(&self, _: &str, _: &str) -> std::io::Result<bool> {
        Ok(false)
    }
}

/// Merkt sich, was der Client beim Start vorfand, und schreibt das Beispiel-Log in seinen Log-Ordner.
fn fake_starter(sample: &'static str, seen_password: Arc<Mutex<Option<String>>>, client_profile: PathBuf) -> ClientStarter {
    Arc::new(move |_exe, arguments, _hide| {
        let logs_dir = &arguments[arguments.iter().position(|arg| arg == "-logsDir").unwrap() + 1];
        common::write_log(Path::new(logs_dir), &common::sample(sample));
        let password_file = rms_core::client::password_file::for_client_profile(&client_profile);
        *seen_password.lock().unwrap() = fs::read_to_string(password_file).ok();
        assert!(!arguments.iter().any(|arg| arg.contains("geheim")), "Passwort in den Startparametern");
        Ok(Box::new(FakeClient::default()) as Box<dyn ClientProcess>)
    })
}

fn orchestrator(temp: &Path, sample: &'static str, seen: Arc<Mutex<Option<String>>>) -> SyncOrchestrator {
    let paths = AppPaths::new(temp);
    let lock_name = format!(r"Local\ReforgerModSync.Test.{}.{}", std::process::id(), temp.file_name().unwrap().to_string_lossy());
    SyncOrchestrator::new(Arc::new(NoPasswords), paths.clone()).with_test_parts(
        fake_starter(sample, seen, paths.client_profile()),
        Arc::new(|_, _| Ok(PathBuf::from(r"C:\Spiel\ArmaReforgerSteam.exe"))),
        ServerListReader::new(Duration::from_millis(10), Duration::from_secs(5)),
        DownloadWatcher::new(Duration::from_millis(10), temp.join("tempaddons")),
        &lock_name,
    )
}

fn server(temp: &Path) -> (AppSettings, ServerProfile) {
    let profile = ServerProfile {
        server_address: Some("203.0.113.10:2001".into()),
        download_dir: Some(temp.join("mods").to_string_lossy().into_owned()),
        ..ServerProfile::new("a", "Beispielserver")
    };
    (AppSettings { profiles: vec![profile.clone()], ..AppSettings::default() }, profile)
}

#[tokio::test]
async fn check_reads_the_list_saves_it_and_removes_the_password_file() {
    let temp = common::temp_dir();
    let seen = Arc::new(Mutex::new(None));
    let orchestrator = orchestrator(temp.path(), "list-done.log", Arc::clone(&seen));
    let (settings, profile) = server(temp.path());
    let (progress, _) = collecting_progress();

    let result = orchestrator.check(&settings, &profile, &progress, &CancellationToken::new()).await;

    assert_eq!(result.outcome, SyncOutcome::Checked, "{}", result.message);
    assert_eq!(result.message, "Liste vom Server geholt: 3 Mods.");
    assert_eq!(orchestrator.cache().load("a").unwrap().mods.len(), 3);
    assert_eq!(seen.lock().unwrap().as_deref(), Some("geheim"), "Passwortdatei lag beim Start bereit");
    assert!(!rms_core::client::password_file::for_client_profile(&AppPaths::new(temp.path()).client_profile()).exists());
}

#[tokio::test]
async fn a_failed_list_is_reported_and_leaves_no_password_file() {
    let temp = common::temp_dir();
    let orchestrator = orchestrator(temp.path(), "list-password-required.log", Arc::new(Mutex::new(None)));
    let (settings, profile) = server(temp.path());
    let (progress, _) = collecting_progress();

    let result = orchestrator.run(&settings, &profile, &progress, &CancellationToken::new()).await;

    assert_eq!(result.outcome, SyncOutcome::Failed);
    assert_eq!(result.message, "Der Server verlangt ein Passwort. Bitte in den Einstellungen eintragen.");
    assert!(!rms_core::client::password_file::for_client_profile(&AppPaths::new(temp.path()).client_profile()).exists());
}

#[tokio::test]
async fn a_failed_precheck_starts_no_client() {
    let temp = common::temp_dir();
    let started = Arc::new(Mutex::new(false));
    let flag = Arc::clone(&started);
    let paths = AppPaths::new(temp.path());
    let orchestrator = SyncOrchestrator::new(Arc::new(NoPasswords), paths).with_test_parts(
        Arc::new(move |_, _, _| {
            *flag.lock().unwrap() = true;
            Ok(Box::new(FakeClient::default()) as Box<dyn ClientProcess>)
        }),
        Arc::new(|_, _| Err("Bitte zuerst Steam starten.".into())),
        ServerListReader::default(),
        DownloadWatcher::default(),
        &format!(r"Local\ReforgerModSync.Test.{}.precheck", std::process::id()),
    );
    let (settings, profile) = server(temp.path());
    let (progress, _) = collecting_progress();

    let result = orchestrator.run(&settings, &profile, &progress, &CancellationToken::new()).await;

    assert_eq!((result.outcome, result.message.as_str()), (SyncOutcome::Failed, "Bitte zuerst Steam starten."));
    assert!(!*started.lock().unwrap());
}

#[tokio::test]
async fn already_current_mods_need_no_download() {
    let temp = common::temp_dir();
    let mods_root = temp.path().join("mods").join("addons");
    write_addon(&mods_root, "1A2B3C4D5E6F7001", "A", "1.4.0");
    write_addon(&mods_root, "1A2B3C4D5E6F7002", "B", "2.1.3");
    write_addon(&mods_root, "1A2B3C4D5E6F7003", "C", "9.9.9");
    let orchestrator = orchestrator(temp.path(), "list-done.log", Arc::new(Mutex::new(None)));
    let (settings, profile) = server(temp.path());
    let (progress, _) = collecting_progress();

    let result = orchestrator.run(&settings, &profile, &progress, &CancellationToken::new()).await;

    assert_eq!(result.outcome, SyncOutcome::AlreadyCurrent, "{}", result.message);
}
