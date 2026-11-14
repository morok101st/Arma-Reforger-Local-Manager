mod common;

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rms_core::client::args::{steam_launch_options, to_command_line};
use rms_core::client::session::ClientSession;
use rms_core::client::{ClientProcess, password_file, retention};
use rms_core::credentials::{CredentialStore, ProfilePasswords, WindowsCredentialStore};
use rms_core::paths::AppPaths;
use rms_core::sync::SyncLock;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::UI::Shell::CommandLineToArgvW;

/// Öffnet eine Datei so, wie ein Programm sie exklusiv sperrt.
fn lock(path: &Path) -> File {
    OpenOptions::new().read(true).share_mode(0).open(path).unwrap()
}

// --- Aufbewahrung der Client-Logs ---

fn run(root: &Path, minute: u32) -> PathBuf {
    let folder = root.join(format!("20261107-12{minute:02}00-000"));
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("console.log"), "log").unwrap();
    folder
}

#[test]
fn keeps_only_the_newest_runs() {
    let temp = common::temp_dir();
    let runs: Vec<_> = (0..12).map(|minute| run(temp.path(), minute)).collect();

    assert_eq!(retention::prune(temp.path(), 10), 2);

    assert!(!runs[0].exists() && !runs[1].exists());
    assert!(runs[2..].iter().all(|run| run.exists()));
}

#[test]
fn old_run_folders_without_milliseconds_are_pruned_first() {
    let temp = common::temp_dir();
    let old = temp.path().join("20261104-112808");
    fs::create_dir(&old).unwrap();
    let newer = run(temp.path(), 1);

    assert_eq!(retention::prune(temp.path(), 1), 1);

    assert!(!old.exists());
    assert!(newer.exists());
}

#[test]
fn leaves_other_folders_and_files_alone() {
    let temp = common::temp_dir();
    let foreign = temp.path().join("eigene Notizen");
    fs::create_dir(&foreign).unwrap();
    let file = temp.path().join("20261107-120000-000.txt");
    fs::write(&file, "x").unwrap();
    run(temp.path(), 1);
    run(temp.path(), 2);

    retention::prune(temp.path(), 0);

    assert!(foreign.exists() && file.exists());
    assert!(!temp.path().join("20261107-120100-000").exists());
}

#[test]
fn skips_a_locked_folder() {
    let temp = common::temp_dir();
    let locked = run(temp.path(), 1);
    let free = run(temp.path(), 2);
    run(temp.path(), 3);

    let deleted = {
        let _file = lock(&locked.join("console.log"));
        retention::prune(temp.path(), 1)
    };

    assert_eq!(deleted, 1);
    assert!(locked.exists());
    assert!(!free.exists());
}

#[test]
fn prune_does_nothing_when_the_folder_is_missing() {
    let temp = common::temp_dir();

    assert_eq!(retention::prune(&temp.path().join("fehlt"), 10), 0);
}

#[test]
fn create_run_folder_keeps_exactly_the_newest_runs() {
    let temp = common::temp_dir();
    (0..12).for_each(|minute| {
        run(temp.path(), minute);
    });

    let folder = retention::create_run_folder(temp.path()).unwrap();

    assert!(folder.is_dir());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), retention::KEEP_RUNS);
    // Neue Ordner haben dasselbe Namensmuster und werden später selbst aufgeräumt.
    assert_eq!(retention::prune(temp.path(), 0), retention::KEEP_RUNS);
}

// --- Passwortdatei ---

#[test]
fn writes_the_password_into_the_profile_folder() {
    let temp = common::temp_dir();
    let path = password_file::for_client_profile(temp.path());

    password_file::prepare(&path, Some("geheim")).unwrap();

    assert_eq!(path, temp.path().join("profile").join("rms_server_password.txt"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "geheim");
}

#[test]
fn without_password_an_old_file_of_another_server_is_removed() {
    let temp = common::temp_dir();
    let path = password_file::for_client_profile(temp.path());
    password_file::prepare(&path, Some("passwort-von-server-a")).unwrap();

    password_file::prepare(&path, None).unwrap();

    assert!(!path.exists());
}

#[test]
fn a_new_password_replaces_the_old_one() {
    let temp = common::temp_dir();
    let path = password_file::for_client_profile(temp.path());
    password_file::prepare(&path, Some("ein-sehr-langes-altes-passwort")).unwrap();

    password_file::prepare(&path, Some("neu")).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "neu");
}

#[test]
fn empty_password_writes_no_file_and_needs_no_folder() {
    let temp = common::temp_dir();
    let path = password_file::for_client_profile(&temp.path().join("fehlt"));

    password_file::prepare(&path, Some("")).unwrap();

    assert!(!path.exists());
    assert!(!temp.path().join("fehlt").exists());
}

#[test]
fn leftovers_are_removed_at_startup_but_logs_stay() {
    let temp = common::temp_dir();
    let paths = AppPaths::new(temp.path());
    let leftover = password_file::for_client_profile(&paths.client_profile());
    password_file::prepare(&leftover, Some("geheim")).unwrap();
    let run = retention::create_run_folder(&paths.client_logs()).unwrap();
    let console = run.join("console.log");
    fs::write(&console, "log").unwrap();
    password_file::prepare(&password_file::for_logs_folder(&run), Some("geheim")).unwrap();

    password_file::clean_up_leftovers(&paths);

    assert!(!leftover.exists());
    assert!(!password_file::for_logs_folder(&run).exists());
    assert!(console.exists());
}

#[test]
fn try_delete_reports_a_locked_file() {
    let temp = common::temp_dir();
    let path = temp.path().join(password_file::FILE_NAME);
    fs::write(&path, "geheim").unwrap();

    {
        let _file = lock(&path);
        assert!(!password_file::try_delete(&path));
    }

    assert!(password_file::try_delete(&path));
    assert!(password_file::try_delete(&path));
}

#[test]
fn wait_until_removed_returns_at_once_or_after_the_timeout() {
    let temp = common::temp_dir();
    let stays = temp.path().join("bleibt.txt");
    fs::write(&stays, "x").unwrap();

    assert!(password_file::wait_until_removed(&temp.path().join("fehlt.txt"), Duration::ZERO));
    assert!(!password_file::wait_until_removed(&stays, Duration::from_millis(150)));
    assert!(stays.exists());
}

#[test]
fn wait_until_removed_ends_early_when_the_file_goes() {
    let temp = common::temp_dir();
    let path = temp.path().join("wird-geloescht.txt");
    fs::write(&path, "x").unwrap();
    let remover = {
        let path = path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            fs::remove_file(path).unwrap();
        })
    };

    let started = Instant::now();
    assert!(password_file::wait_until_removed(&path, Duration::from_secs(30)));
    assert!(started.elapsed() < Duration::from_secs(10));
    remover.join().unwrap();
}

// --- Anmeldeinformationsspeicher (echt, mit eindeutigen Test-IDs) ---

const ADDRESS: &str = "203.0.113.10:2001";
const OTHER_ADDRESS: &str = "203.0.113.20:2001";

struct TestProfiles(Vec<String>);

impl TestProfiles {
    fn new(count: usize) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self((0..count).map(|index| format!("test-rust-{stamp}-{index}")).collect())
    }
}

impl Drop for TestProfiles {
    fn drop(&mut self) {
        for profile in &self.0 {
            for address in [ADDRESS, OTHER_ADDRESS] {
                let _ = WindowsCredentialStore.delete_password(address, profile);
            }
        }
    }
}

#[test]
fn saved_password_can_be_read_and_deleted() {
    let profiles = TestProfiles::new(1);
    let store = WindowsCredentialStore;
    let profile = &profiles.0[0];

    store.save_password(ADDRESS, profile, "pässwört-€-123").unwrap();

    assert_eq!(store.read_password(ADDRESS, profile).unwrap().as_deref(), Some("pässwört-€-123"));
    assert!(store.delete_password(ADDRESS, profile).unwrap());
    assert_eq!(store.read_password(ADDRESS, profile).unwrap(), None);
    assert!(!store.delete_password(ADDRESS, profile).unwrap());
}

#[test]
fn two_profiles_on_the_same_server_have_separate_passwords() {
    let profiles = TestProfiles::new(2);
    let store = WindowsCredentialStore;

    store.save_password(ADDRESS, &profiles.0[0], "geheim-a").unwrap();
    store.save_password(ADDRESS, &profiles.0[1], "geheim-b").unwrap();

    assert_eq!(store.read_password(ADDRESS, &profiles.0[0]).unwrap().as_deref(), Some("geheim-a"));
    assert_eq!(store.read_password(ADDRESS, &profiles.0[1]).unwrap().as_deref(), Some("geheim-b"));
    assert_eq!(store.read_password(OTHER_ADDRESS, &profiles.0[0]).unwrap(), None);
}

/// Gleicher Eintrag wie bei der C#-Vorgängerversion: Ein dort gespeichertes Passwort gilt auch hier.
#[test]
fn target_name_matches_the_wpf_app() {
    assert_eq!(rms_core::credentials::target_name(" 203.0.113.10:2001 ", "abc"), "ReforgerModSync:Server:203.0.113.10:2001:abc");
}

// --- Regeln für Passwörter (Speicher im Arbeitsspeicher) ---

#[derive(Default)]
struct MemoryStore(Mutex<HashMap<(String, String), String>>);

impl CredentialStore for MemoryStore {
    fn read_password(&self, address: &str, id: &str) -> io::Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(&(address.into(), id.into())).cloned())
    }

    fn save_password(&self, address: &str, id: &str, password: &str) -> io::Result<()> {
        self.0.lock().unwrap().insert((address.into(), id.into()), password.into());
        Ok(())
    }

    fn delete_password(&self, address: &str, id: &str) -> io::Result<bool> {
        Ok(self.0.lock().unwrap().remove(&(address.into(), id.into())).is_some())
    }
}

const OLD: &str = "203.0.113.10:2001";
const NEW: &str = "203.0.113.20:2002";

#[test]
fn transfer_decision_only_when_a_saved_password_would_move() {
    let cases = [
        ("", true, Some(OLD), Some(NEW), true),
        ("neu", true, Some(OLD), Some(NEW), false),
        ("", false, Some(OLD), Some(NEW), false),
        ("", true, Some(OLD), Some(OLD), false),
        ("", true, None, Some(NEW), false),
    ];
    for (entered, has_saved, saved, next, expected) in cases {
        assert_eq!(ProfilePasswords::needs_transfer_decision(has_saved, saved, next, entered), expected);
    }
}

#[test]
fn moving_with_consent_keeps_the_password_under_the_new_address_only() {
    let store = MemoryStore::default();
    store.save_password(OLD, "a", "geheim").unwrap();

    ProfilePasswords::new(&store).move_to_new_address("a", OLD, NEW, true).unwrap();

    assert_eq!(store.read_password(NEW, "a").unwrap().as_deref(), Some("geheim"));
    assert_eq!(store.read_password(OLD, "a").unwrap(), None);
}

#[test]
fn moving_without_consent_leaves_no_password_behind() {
    let store = MemoryStore::default();
    store.save_password(OLD, "a", "geheim").unwrap();

    ProfilePasswords::new(&store).move_to_new_address("a", OLD, NEW, false).unwrap();

    assert!(store.0.lock().unwrap().is_empty());
}

#[test]
fn an_empty_entry_keeps_the_saved_password() {
    let store = MemoryStore::default();
    let passwords = ProfilePasswords::new(&store);
    store.save_password(OLD, "a", "geheim").unwrap();

    passwords.store_entered(OLD, "a", "").unwrap();

    assert_eq!(store.read_password(OLD, "a").unwrap().as_deref(), Some("geheim"));
    assert!(passwords.has_password(Some(OLD), "a").unwrap());
    assert!(!passwords.has_password(Some(OLD), "b").unwrap());
    assert!(!passwords.has_password(None, "a").unwrap());
}

// --- Befehlszeile: Rundlauf durch CommandLineToArgvW ---

/// Zerlegt eine Befehlszeile so, wie Windows-Programme es tun.
fn split(command_line: &str) -> Vec<String> {
    // Das erste Element gilt als Programmname und hat eigene Regeln, deshalb ein fester Platzhalter davor.
    let text: Vec<u16> = format!("programm.exe {command_line}").encode_utf16().chain([0]).collect();
    let mut count = 0;
    // SAFETY: Text ist nullterminiert, das Ergebnis wird mit LocalFree freigegeben.
    unsafe {
        let argv = CommandLineToArgvW(windows::core::PCWSTR(text.as_ptr()), &raw mut count);
        let arguments = (1..usize::try_from(count).unwrap()).map(|index| (*argv.add(index)).to_string().unwrap()).collect();
        LocalFree(Some(HLOCAL(argv.cast())));
        arguments
    }
}

#[test]
fn every_value_survives_the_round_trip() {
    let values = [
        r"D:\Meine Mods,C:\app helper\x",
        r"D:\Mods\",
        r"D:\Meine Mods\",
        r"C:\",
        "",
        "mit \"Anführungszeichen\"",
        r#"Backslash vor \"Zeichen"#,
        "Tab\tdarin",
        "1A2B3C4D5E6F7001:1.4.0;1A2B3C4D5E6F7002",
    ];
    for value in values {
        let arguments = ["-exe", "ArmaReforgerSteam.exe", "-addonsDir", value, "-rmsJoin", "203.0.113.10:2001"];
        assert_eq!(split(&to_command_line(&arguments)), arguments, "{value:?}");
    }
}

#[test]
fn steam_launch_options_split_back_into_the_folder() {
    for folder in [r"D:\", r"D:\Meine Mods"] {
        assert_eq!(split(&steam_launch_options(Some(folder))), ["-addonsDir", folder, "-addonDownloadDir", folder]);
    }
}

// --- Sperre gegen zwei gleichzeitige Syncs ---

#[test]
fn only_one_sync_can_hold_the_lock() {
    let name = format!(r"Local\ReforgerModSync.Test.{}", std::process::id());

    let first = SyncLock::try_acquire_named(&name).expect("frei");
    assert!(SyncLock::try_acquire_named(&name).is_none());

    drop(first);
    assert!(SyncLock::try_acquire_named(&name).is_some());
}

// --- Laufender Client ---

#[tokio::test]
async fn stopping_ends_a_process_without_window() {
    let ping = PathBuf::from(std::env::var("SystemRoot").unwrap()).join(r"System32\PING.EXE");
    let session = ClientSession::start(&ping, &["-n".into(), "60".into(), "127.0.0.1".into()], true).unwrap();
    assert!(!session.has_exited());

    session.stop().await;

    assert!(session.has_exited());
}

/// Der WebView2-Ordner der Vorschau wird entfernt, andere Ordner daneben bleiben.
#[test]
fn removes_legacy_webview_folder_only() {
    let dir = common::temp_dir();
    let legacy = dir.path().join(rms_core::paths::LEGACY_WEBVIEW_FOLDER).join("EBWebView");
    let current = dir.path().join("de.morok.reforgersynclaunch").join("EBWebView");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::create_dir_all(&current).unwrap();
    std::fs::write(legacy.join("Local State"), "{}").unwrap();

    rms_core::paths::remove_legacy_webview_data_in(dir.path());
    rms_core::paths::remove_legacy_webview_data_in(dir.path());

    assert!(!dir.path().join(rms_core::paths::LEGACY_WEBVIEW_FOLDER).exists());
    assert!(current.is_dir());
}
