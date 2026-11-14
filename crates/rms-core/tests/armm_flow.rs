//! Ablauf für aus ARMM importierte Server: Liste aus ARMM, Laden ohne Server, Rückfall und Gegenprüfung.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::FakeClient;
use rms_core::addons::ModEntry;
use rms_core::armm::{ArmmLink, FetchError, ModsetSource};
use rms_core::client::{ClientProcess, ClientStarter, modset_file, password_file};
use rms_core::credentials::CredentialStore;
use rms_core::paths::AppPaths;
use rms_core::settings::{AppSettings, ServerProfile};
use rms_core::sync::{DownloadWatcher, Progress, ServerListReader, SyncOrchestrator, SyncOutcome, SyncStatus, list_mismatch};
use tokio_util::sync::CancellationToken;

const ID_A: &str = "1A2B3C4D5E6F7001";
const ID_B: &str = "1A2B3C4D5E6F7002";
const KEY: &str = "armm_sync_BEISPIEL";

const MODSET: &str = r#"[
  { "modId": "1A2B3C4D5E6F7001", "name": "Beispiel Fahrzeuge", "version": "1.4.0" },
  { "modId": "1A2B3C4D5E6F7002", "name": "Beispiel Karte Nord", "version": "2.1.3" }
]"#;

fn write_addon(mods_root: &Path, id: &str, version: &str) {
    let folder = mods_root.join(format!("Mod_{id}"));
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("data.pak"), "12345").unwrap();
    fs::write(
        folder.join("meta"),
        format!(r#"{{"meta":{{"id":"{id}","name":"Mod","versions":[{{"version":"{version}","package":{{"files":[{{"size":5,"name":"data.pak"}}]}}}}],"selectedRev":0}}}}"#),
    )
    .unwrap();
}

/// Schlüssel für ARMM, Passwort für den Server.
struct Secrets {
    access_key: Option<String>,
}

impl CredentialStore for Secrets {
    fn read_secret(&self, target: &str) -> std::io::Result<Option<String>> {
        Ok(if target.starts_with("ReforgerModSync:Armm:") { self.access_key.clone() } else { Some("geheim".into()) })
    }

    fn save_secret(&self, _: &str, _: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn delete_secret(&self, _: &str) -> std::io::Result<bool> {
        Ok(false)
    }
}

/// ARMM-Attrappe: liefert eine feste Antwort und merkt sich den Schlüssel.
struct FakeArmm {
    answer: Result<Vec<u8>, FetchError>,
    seen_key: Mutex<Option<String>>,
}

impl ModsetSource for FakeArmm {
    fn fetch(&self, link: &ArmmLink, access_key: &str) -> Result<Vec<u8>, FetchError> {
        assert_eq!(link.modset_id, 42);
        *self.seen_key.lock().unwrap() = Some(access_key.into());
        self.answer.clone()
    }
}

/// Was der nachgebaute Client bei jedem Start erlebt hat.
#[derive(Default)]
struct Calls {
    kinds: Vec<&'static str>,
    passwords: Vec<Option<String>>,
    list_files: Vec<String>,
}

/// Verhält sich wie die Hilfs-Mod: `-rmsModset` lädt nur `loadable`, `-rmsServer` liefert `server_list`,
/// `-rmsMods` lädt alles Verlangte.
fn fake_client(
    mods_root: PathBuf,
    client_profile: PathBuf,
    loadable: Vec<&'static str>,
    server_list: Vec<ModEntry>,
    calls: Arc<Mutex<Calls>>,
) -> ClientStarter {
    Arc::new(move |_exe, arguments, _hide| {
        let logs_dir = PathBuf::from(&arguments[arguments.iter().position(|arg| arg == "-logsDir").unwrap() + 1]);
        let mut calls = calls.lock().unwrap();
        calls.passwords.push(fs::read_to_string(password_file::for_client_profile(&client_profile)).ok());
        let mut log = String::new();

        if arguments.iter().any(|arg| arg == "-rmsModset") {
            calls.kinds.push("modset");
            let list = fs::read_to_string(modset_file::for_client_profile(&client_profile)).unwrap();
            for line in list.lines().filter(|line| line.ends_with("|get")) {
                let parts: Vec<&str> = line.split('|').collect();
                if loadable.contains(&parts[0]) {
                    write_addon(&mods_root, parts[0], parts[1]);
                } else {
                    log.push_str(&format!("[RMS] MOD_FAILED {} not_found\n", parts[0]));
                }
            }
            calls.list_files.push(list);
            log.push_str("[RMS] DONE failed=0\n");
        } else if let Some(position) = arguments.iter().position(|arg| arg == "-rmsMods") {
            calls.kinds.push("download");
            for pair in arguments[position + 1].split(';') {
                let (id, version) = pair.split_once(':').unwrap();
                write_addon(&mods_root, id, version);
            }
            log.push_str("[RMS] DONE failed=0\n");
        } else if server_list.is_empty() {
            // So meldet die Hilfs-Mod einen Server, der nicht läuft.
            calls.kinds.push("list");
            log.push_str("[RMS] ROOM_NOT_FOUND\n");
        } else {
            calls.kinds.push("list");
            for mod_entry in &server_list {
                log.push_str(&format!(
                    "[RMS] SERVER_MOD {} {} {}\n",
                    mod_entry.id,
                    mod_entry.version.as_deref().unwrap_or(""),
                    mod_entry.name
                ));
            }
            log.push_str(&format!("[RMS] LIST_DONE mods={}\n", server_list.len()));
        }

        common::write_log(&logs_dir, &log);
        Ok(Box::new(FakeClient::default()) as Box<dyn ClientProcess>)
    })
}

/// Antwort der ARMM-Attrappe.
type Answer<'a> = Result<&'a [u8], FetchError>;

struct Setup {
    _temp: tempfile::TempDir,
    root: PathBuf,
    orchestrator: SyncOrchestrator,
    armm: Arc<FakeArmm>,
    calls: Arc<Mutex<Calls>>,
    settings: AppSettings,
    profile: ServerProfile,
}

impl Setup {
    fn new(
        answer: Result<&[u8], FetchError>,
        access_key: Option<&str>,
        address: Option<&str>,
        loadable: Vec<&'static str>,
        server_list: Vec<ModEntry>,
    ) -> Self {
        let temp = common::temp_dir();
        let root = temp.path().to_path_buf();
        let paths = AppPaths::new(&root);
        let calls = Arc::new(Mutex::new(Calls::default()));
        let armm = Arc::new(FakeArmm { answer: answer.map(<[u8]>::to_vec), seen_key: Mutex::new(None) });
        let lock_name = format!(r"Local\ReforgerModSync.Test.{}.{}", std::process::id(), root.file_name().unwrap().to_string_lossy());
        let orchestrator = SyncOrchestrator::new(Arc::new(Secrets { access_key: access_key.map(String::from) }), paths.clone())
            .with_test_parts(
                fake_client(root.join("mods").join("addons"), paths.client_profile(), loadable, server_list, Arc::clone(&calls)),
                Arc::new(|_, _| Ok(PathBuf::from(r"C:\Spiel\ArmaReforgerSteam.exe"))),
                ServerListReader::new(Duration::from_millis(10), Duration::from_secs(5)),
                DownloadWatcher::new(Duration::from_millis(10), root.join("tempaddons")),
                &lock_name,
            )
            .with_modset_source(Arc::clone(&armm) as Arc<dyn ModsetSource>);
        let profile = ServerProfile {
            server_address: address.map(String::from),
            download_dir: Some(root.join("mods").to_string_lossy().into_owned()),
            armm: Some(ArmmLink {
                base_url: "https://armm.example.com".into(),
                modset_id: 42,
                export_path: "/api/sync/modsets/42/export".into(),
            }),
            ..ServerProfile::new("a", "Community")
        };
        let settings = AppSettings { profiles: vec![profile.clone()], ..AppSettings::default() };
        Self { _temp: temp, root, orchestrator, armm, calls, settings, profile }
    }

    fn mods_root(&self) -> PathBuf {
        self.root.join("mods").join("addons")
    }

    async fn check(&self) -> rms_core::sync::SyncResult {
        self.orchestrator.check(&self.settings, &self.profile, &silent(), &CancellationToken::new()).await
    }

    async fn run(&self) -> rms_core::sync::SyncResult {
        self.orchestrator.run(&self.settings, &self.profile, &silent(), &CancellationToken::new()).await
    }

    fn kinds(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().kinds.clone()
    }
}

fn silent() -> Progress {
    Arc::new(|_: SyncStatus| {})
}

fn server_list(version_a: &str) -> Vec<ModEntry> {
    vec![ModEntry::new(ID_A, "Beispiel Fahrzeuge", Some(version_a)), ModEntry::new(ID_B, "Beispiel Karte Nord", Some("2.1.3"))]
}

#[tokio::test]
async fn check_takes_the_list_from_armm_without_starting_the_game() {
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), None, vec![], vec![]);

    let result = setup.check().await;

    assert_eq!(result.outcome, SyncOutcome::Checked, "{}", result.message);
    assert_eq!(result.message, "Liste aus ARMM geholt: 2 Mods.");
    assert!(setup.kinds().is_empty(), "Spiel gestartet");
    assert_eq!(setup.orchestrator.cache().load("a").unwrap().mods.len(), 2);
    assert_eq!(setup.armm.seen_key.lock().unwrap().as_deref(), Some(KEY));
}

#[tokio::test]
async fn armm_problems_are_reported_clearly() {
    let cases: [(Answer, Option<&str>, &str); 4] = [
        (Err(FetchError::Unauthorized), Some(KEY), "Zugangsschlüssel abgelehnt"),
        (Err(FetchError::NotFound), Some(KEY), "nicht gefunden"),
        (Ok(MODSET.as_bytes()), None, "kein ARMM-Zugangsschlüssel gespeichert"),
        (Ok(b"<html>Wartung</html>".as_slice()), Some(KEY), "keine gültige Mod-Liste"),
    ];
    for (answer, key, expected) in cases {
        let setup = Setup::new(answer, key, None, vec![], vec![]);

        let result = setup.check().await;

        assert_eq!(result.outcome, SyncOutcome::Failed);
        assert!(result.message.contains(expected), "{}", result.message);
        assert!(setup.orchestrator.cache().load("a").is_none(), "Liste trotz Fehler gemerkt");
    }
}

#[tokio::test]
async fn sync_loads_without_server_and_without_password() {
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), Some("203.0.113.10:2001"), vec![ID_A, ID_B], vec![]);

    let result = setup.run().await;

    assert_eq!(result.outcome, SyncOutcome::Completed, "{}", result.message);
    assert_eq!(setup.kinds(), vec!["modset"]);
    assert_eq!(setup.calls.lock().unwrap().passwords, vec![None], "Passwortdatei beim Laden ohne Server");
    assert!(!modset_file::for_client_profile(&AppPaths::new(&setup.root).client_profile()).exists(), "Listendatei nicht aufgeräumt");
    assert!(!result.keep_surplus);
}

#[tokio::test]
async fn current_mods_go_into_the_list_only_as_anchors() {
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), None, vec![ID_B], vec![]);
    write_addon(&setup.mods_root(), ID_A, "1.4.0");

    let result = setup.run().await;

    assert_eq!(result.outcome, SyncOutcome::Completed, "{}", result.message);
    let list = setup.calls.lock().unwrap().list_files[0].clone();
    assert_eq!(list, format!("{ID_A}|1.4.0|Beispiel Fahrzeuge|have\n{ID_B}|2.1.3|Beispiel Karte Nord|get\n"));
}

#[tokio::test]
async fn nothing_to_load_starts_no_game() {
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), None, vec![], vec![]);
    write_addon(&setup.mods_root(), ID_A, "1.4.0");
    write_addon(&setup.mods_root(), ID_B, "2.1.3");

    let result = setup.run().await;

    assert_eq!(result.outcome, SyncOutcome::AlreadyCurrent, "{}", result.message);
    assert!(setup.kinds().is_empty());
}

#[tokio::test]
async fn without_server_address_unreachable_mods_are_named() {
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), None, vec![ID_A], vec![]);

    let result = setup.run().await;

    assert_eq!(result.outcome, SyncOutcome::Incomplete);
    assert!(result.message.contains("Beispiel Karte Nord (im Workshop nicht gefunden)"), "{}", result.message);
    assert!(result.message.contains("ohne Server-Eintrag nicht erreichbar"), "{}", result.message);
    assert_eq!(setup.kinds(), vec!["modset"]);
}

#[tokio::test]
async fn fallback_loads_the_rest_over_the_server_entry() {
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), Some("203.0.113.10:2001"), vec![ID_A], server_list("1.4.0"));

    let result = setup.run().await;

    assert_eq!(result.outcome, SyncOutcome::Completed, "{}", result.message);
    assert_eq!(setup.kinds(), vec!["modset", "list", "download"]);
    let passwords = setup.calls.lock().unwrap().passwords.clone();
    assert_eq!(passwords, vec![None, Some("geheim".into()), Some("geheim".into())]);
    assert!(!result.keep_surplus);
    assert_eq!(setup.orchestrator.cache().load("a").unwrap().mods[0].version.as_deref(), Some("1.4.0"), "Server-Liste hat ARMM ersetzt");
}

#[tokio::test]
async fn differing_server_list_warns_and_keeps_surplus_mods() {
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), Some("203.0.113.10:2001"), vec![ID_A], server_list("1.3.9"));

    let result = setup.run().await;

    assert!(result.keep_surplus);
    assert!(result.message.contains("weicht vom laufenden Server ab"), "{}", result.message);
}

#[tokio::test]
async fn offline_server_leaves_a_clear_note() {
    // Ohne Liste vom Server endet der Rückfall mit Hinweis, das Ergebnis ohne Server bleibt.
    let setup = Setup::new(Ok(MODSET.as_bytes()), Some(KEY), Some("203.0.113.10:2001"), vec![ID_A], vec![]);

    let result = setup.run().await;

    assert_eq!(result.outcome, SyncOutcome::Incomplete);
    assert!(result.message.contains("Rückfall über den Server nicht möglich"), "{}", result.message);
}

#[test]
fn list_mismatch_compares_id_and_version() {
    let armm = server_list("1.4.0");
    assert_eq!(list_mismatch(&armm, &server_list("1.4.0")), None);
    assert_eq!(list_mismatch(&armm, &server_list("1.4.0").into_iter().rev().collect::<Vec<_>>()), None);

    let text = list_mismatch(&armm, &server_list("1.3.9")).unwrap();
    assert!(text.contains("1 Unterschiede") && text.contains("Beispiel Fahrzeuge"), "{text}");

    let mut more = server_list("1.4.0");
    more.push(ModEntry::new("1A2B3C4D5E6F7003", "Zusatz", Some("1.0.0")));
    assert!(list_mismatch(&armm, &more).unwrap().contains("Zusatz"));
}
