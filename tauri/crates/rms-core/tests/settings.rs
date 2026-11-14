mod common;

use std::fs;

use rms_core::addons::ModEntry;
use rms_core::settings::{
    AppSettings, BasicSettingsField, BasicSettingsInput, ClientWindowSettings, MAX_IDLE_MINUTES, MAX_TOTAL_MINUTES, MIN_IDLE_MINUTES,
    MIN_TOTAL_MINUTES, ServerProfile, SettingsError, SettingsStore, TimeoutSettings, apply_basic_settings, sanitize,
};
use rms_core::steam::CLIENT_EXE_NAME;
use rms_core::sync::ModListCache;

fn profile(id: &str, name: &str, address: Option<&str>) -> ServerProfile {
    ServerProfile { server_address: address.map(String::from), ..ServerProfile::new(id, name) }
}

fn window(width: i32, height: i32, max_fps: i32, hide_window: bool) -> ClientWindowSettings {
    ClientWindowSettings { width, height, max_fps, hide_window, ..ClientWindowSettings::default() }
}

// --- Bereinigung ---

#[test]
fn valid_settings_stay_unchanged() {
    let settings = AppSettings {
        profiles: vec![profile("a", "Training", Some("203.0.113.10:2001"))],
        reforger_exe: Some(r"C:\Spiele\ArmaReforgerSteam.exe".into()),
        client: window(800, 450, 30, false),
        timeouts: TimeoutSettings::new(90, 5),
        cleanup_approved_folders: vec![r"D:\MODS".into()],
        ..AppSettings::default()
    };

    assert_eq!(sanitize(settings.clone()), settings);
}

#[test]
fn invalid_timeouts_fall_back_to_safe_values() {
    for (total, idle) in [(0, 10), (-5, 10), (120, 0), (100_000, 10), (30, 30), (20, 50)] {
        let clean = sanitize(AppSettings { timeouts: TimeoutSettings::new(total, idle), ..AppSettings::default() });

        assert!((MIN_TOTAL_MINUTES..=MAX_TOTAL_MINUTES).contains(&clean.timeouts.total_minutes));
        assert!((MIN_IDLE_MINUTES..=MAX_IDLE_MINUTES).contains(&clean.timeouts.idle_minutes));
        assert!(clean.timeouts.idle_minutes < clean.timeouts.total_minutes, "{total}/{idle}");
    }
}

#[test]
fn invalid_window_values_fall_back_to_the_defaults() {
    let clean = sanitize(AppSettings { client: window(0, 99_999, -1, false), ..AppSettings::default() });

    assert_eq!(clean.client, ClientWindowSettings { hide_window: false, ..ClientWindowSettings::default() });
}

#[test]
fn profiles_without_id_are_dropped_and_blank_exe_means_automatic() {
    let clean = sanitize(AppSettings {
        profiles: vec![profile(" ", "Kaputt", None), profile("a", "Gut", None)],
        reforger_exe: Some("   ".into()),
        ..AppSettings::default()
    });

    assert_eq!(clean.profiles.iter().map(|profile| profile.name.as_str()).collect::<Vec<_>>(), ["Gut"]);
    assert_eq!(clean.reforger_exe, None);
}

#[test]
fn store_sanitizes_a_hand_edited_file() {
    let temp = common::temp_dir();
    let path = temp.path().join("settings.json");
    fs::write(
        &path,
        r#"{ "profiles": null, "timeouts": { "totalMinutes": 0, "idleMinutes": -3 }, "client": null, "cleanupApprovedFolders": null }"#,
    )
    .unwrap();

    let settings = SettingsStore::new(&path).load().unwrap();

    assert!(settings.profiles.is_empty());
    assert!(settings.cleanup_approved_folders.is_empty());
    assert_eq!(settings.timeouts, TimeoutSettings::default());
    assert_eq!(settings.client, ClientWindowSettings::default());
}

// --- Grundeinstellungen ---

fn current() -> AppSettings {
    AppSettings {
        profiles: vec![profile("1", "Training", Some("203.0.113.10:2001"))],
        cleanup_approved_folders: vec![r"D:\Mods".into()],
        client: window(800, 450, 30, false),
        ..AppSettings::default()
    }
}

fn input(exe: &str, total: &str, idle: &str) -> BasicSettingsInput {
    BasicSettingsInput { reforger_exe: exe.into(), total_minutes: total.into(), idle_minutes: idle.into() }
}

#[test]
fn applies_values_and_keeps_everything_else() {
    let current = current();

    let result = apply_basic_settings(&current, &input("", "90", "5"));

    let settings = result.settings().expect("gültig");
    assert_eq!((settings.timeouts.total_minutes, settings.timeouts.idle_minutes), (90, 5));
    assert_eq!(settings.client, current.client);
    assert_eq!(settings.profiles, current.profiles);
    assert_eq!(settings.cleanup_approved_folders, current.cleanup_approved_folders);
    assert_eq!(settings.reforger_exe, None);
}

#[test]
fn defaults_match_a_fresh_installation() {
    assert_eq!(BasicSettingsInput::defaults(), input("", "120", "10"));
}

#[test]
fn rejects_invalid_total_time() {
    for total in ["9", "601", "", "abc", "-120", "1 20", "+120"] {
        let result = apply_basic_settings(&current(), &input("", total, "10"));

        assert!(!result.is_valid(), "{total}");
        assert_eq!(result.errors()[&BasicSettingsField::TotalMinutes], "Bitte eine Zahl von 10 bis 600 eintragen.");
    }
}

#[test]
fn accepts_the_limits_and_surrounding_spaces() {
    assert!(apply_basic_settings(&current(), &input("", " 600 ", "2")).is_valid());
}

#[test]
fn reports_every_invalid_field() {
    let result = apply_basic_settings(&current(), &input(r"C:\fehlt\ArmaReforgerSteam.exe", "0", "0"));

    assert_eq!(result.errors().len(), 3);
}

#[test]
fn idle_time_must_be_below_total_time() {
    let result = apply_basic_settings(&current(), &input("", "30", "30"));

    assert_eq!(result.errors()[&BasicSettingsField::IdleMinutes], "Muss kleiner als die Höchstdauer sein.");
}

#[test]
fn accepts_an_existing_client_exe() {
    let temp = common::temp_dir();
    let exe = temp.path().join(CLIENT_EXE_NAME);
    fs::write(&exe, "").unwrap();

    let result = apply_basic_settings(&current(), &input(&format!("  \"{}\" ", exe.display()), "120", "10"));

    assert_eq!(result.settings().unwrap().reforger_exe.as_deref(), Some(exe.to_str().unwrap()));
}

#[test]
fn rejects_a_missing_or_wrong_client_exe() {
    let temp = common::temp_dir();
    let other = temp.path().join("ArmaReforger_BE.exe");
    fs::write(&other, "").unwrap();

    let missing = apply_basic_settings(&current(), &input(temp.path().join(CLIENT_EXE_NAME).to_str().unwrap(), "120", "10"));
    let wrong = apply_basic_settings(&current(), &input(other.to_str().unwrap(), "120", "10"));

    assert_eq!(missing.errors()[&BasicSettingsField::ReforgerExe], "Die Datei gibt es nicht.");
    assert_eq!(wrong.errors()[&BasicSettingsField::ReforgerExe], "Bitte die Datei ArmaReforgerSteam.exe aus dem Spielordner wählen.");
}

#[test]
fn empty_client_path_means_automatic() {
    let with_exe = AppSettings { reforger_exe: Some(r"C:\Spiele\ArmaReforgerSteam.exe".into()), ..current() };

    let result = apply_basic_settings(&with_exe, &input("   ", "120", "10"));

    assert_eq!(result.settings().unwrap().reforger_exe, None);
}

#[test]
fn input_reflects_saved_settings() {
    let saved = AppSettings {
        reforger_exe: Some(r"C:\Spiele\ArmaReforgerSteam.exe".into()),
        timeouts: TimeoutSettings::new(90, 5),
        ..AppSettings::default()
    };

    assert_eq!(BasicSettingsInput::from_settings(&saved), input(r"C:\Spiele\ArmaReforgerSteam.exe", "90", "5"));
}

// --- Datei ---

#[test]
fn missing_file_gives_no_profiles_without_creating_it() {
    let temp = common::temp_dir();
    let path = temp.path().join("settings.json");

    let settings = SettingsStore::new(&path).load().unwrap();

    assert!(settings.profiles.is_empty());
    assert!(!path.exists());
}

#[test]
fn several_profiles_keep_their_own_values_and_the_selection() {
    let temp = common::temp_dir();
    let store = SettingsStore::new(temp.path().join("sub").join("settings.json"));
    let first = ServerProfile { download_dir: Some(r"D:\Mods".into()), ..profile("a", "Training", Some("203.0.113.10:2001")) };
    let second = profile("b", "Event", Some("203.0.113.20:2302"));

    store
        .save(&AppSettings {
            profiles: vec![first.clone(), second.clone()],
            selected_profile_id: Some("b".into()),
            timeouts: TimeoutSettings::new(120, 3),
            ..AppSettings::default()
        })
        .unwrap();
    let loaded = store.load().unwrap();

    assert_eq!(loaded.profiles, [first, second]);
    assert_eq!(loaded.selected_profile_id.as_deref(), Some("b"));
    assert_eq!(loaded.timeouts.idle_minutes, 3);
}

#[test]
fn passwords_cannot_end_up_in_the_settings_file() {
    let temp = common::temp_dir();
    let path = temp.path().join("settings.json");

    SettingsStore::new(&path).save(&AppSettings { profiles: vec![profile("a", "Training", None)], ..AppSettings::default() }).unwrap();

    assert!(!fs::read_to_string(&path).unwrap().to_lowercase().contains("password"));
}

#[test]
fn older_settings_file_with_armm_url_still_loads() {
    let temp = common::temp_dir();
    let path = temp.path().join("settings.json");
    fs::write(
        &path,
        r#"{ "profiles": [ { "id": "a", "name": "Training", "exportUrl": "https://x/api/modsets/1/export", "serverAddress": "203.0.113.10:2001" } ] }"#,
    )
    .unwrap();

    let settings = SettingsStore::new(&path).load().unwrap();

    assert_eq!(settings.profiles[0].server_address.as_deref(), Some("203.0.113.10:2001"));
}

#[test]
fn damaged_file_is_reported_and_left_untouched() {
    let temp = common::temp_dir();
    let path = temp.path().join("settings.json");
    fs::write(&path, "{ kaputt").unwrap();

    let error = SettingsStore::new(&path).load().unwrap_err();

    assert!(matches!(error, SettingsError::Damaged { .. }));
    assert!(error.to_string().ends_with("settings.json“ ist beschädigt."));
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ kaputt");
}

/// Eine Datei der C#-Vorgängerversion muss gleich gelesen und ohne Verlust geschrieben werden.
#[test]
fn a_file_written_by_the_wpf_app_round_trips_unchanged() {
    let wpf = r#"{
  "profiles": [
    {
      "id": "3f2b8c1e-0d4a-4b7e-9a51-2c6f0e8d7b13",
      "name": "Beispielserver Training",
      "serverAddress": "203.0.113.10:2001",
      "downloadDir": "D:\\Mods"
    },
    {
      "id": "8a7d6c5b-4e3f-4a1b-9c8d-7e6f5a4b3c2d",
      "name": "Event",
      "serverAddress": "203.0.113.20:2302",
      "downloadDir": null
    }
  ],
  "selectedProfileId": "8a7d6c5b-4e3f-4a1b-9c8d-7e6f5a4b3c2d",
  "reforgerExe": null,
  "client": {
    "width": 640,
    "height": 360,
    "maxFps": 15,
    "hideWindow": true
  },
  "timeouts": {
    "totalMinutes": 120,
    "idleMinutes": 10
  },
  "cleanupApprovedFolders": [
    "D:\\MODS"
  ]
}"#;
    let temp = common::temp_dir();
    let path = temp.path().join("settings.json");
    fs::write(&path, wpf).unwrap();
    let store = SettingsStore::new(&path);

    let loaded = store.load().unwrap();
    store.save(&loaded).unwrap();

    let written: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let original: serde_json::Value = serde_json::from_str(wpf).unwrap();
    assert_eq!(written, original);
    assert_eq!(loaded.profiles[0].download_dir.as_deref(), Some(r"D:\Mods"));
}

/// Prüft die echte Datei des Nutzers, nur lesend: `cargo test -p rms-core -- --ignored`.
#[test]
#[ignore = "liest die echte Einstellungsdatei dieses Rechners"]
fn the_real_settings_file_round_trips_unchanged() {
    let path = rms_core::paths::default_settings_file();
    let Ok(original) = fs::read_to_string(&path) else {
        return;
    };

    let loaded = SettingsStore::new(&path).load().unwrap();

    let original: serde_json::Value = serde_json::from_str(original.trim_start_matches('\u{feff}')).unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), original);
}

#[test]
fn unknown_fields_survive_saving() {
    let temp = common::temp_dir();
    let path = temp.path().join("settings.json");
    fs::write(&path, r#"{ "profiles": [ { "id": "a", "name": "A", "spaeter": 1 } ], "neuesFeld": { "x": true } }"#).unwrap();
    let store = SettingsStore::new(&path);

    store.save(&store.load().unwrap()).unwrap();

    let written: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written["neuesFeld"]["x"], true);
    assert_eq!(written["profiles"][0]["spaeter"], 1);
}

// --- Zwischenspeicher der Mod-Liste ---

#[test]
fn mod_list_cache_keeps_the_list_per_profile() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path().join("listen"));
    let mods =
        [ModEntry::new("65AD7C139EB4C1A1", "ACE Backblast Dev", Some("1.5.36")), ModEntry::new("64EE818E08AFCF94", "MFD Framework", None)];

    assert!(cache.load("a").is_none());
    cache.save("a", &mods).unwrap();

    assert_eq!(cache.load("a").unwrap().mods, mods);
    assert!(cache.load("b").is_none());

    cache.delete("a").unwrap();
    assert!(cache.load("a").is_none());
    cache.delete("a").unwrap();
}

#[test]
fn damaged_cache_counts_as_not_fetched() {
    let temp = common::temp_dir();
    fs::write(temp.path().join("a.json"), "{ kaputt").unwrap();

    assert!(ModListCache::new(temp.path()).load("a").is_none());
}

/// So schreibt die C#-Vorgängerversion den Zwischenspeicher (System.Text.Json ohne Optionen).
#[test]
fn reads_the_cache_of_the_wpf_app() {
    let temp = common::temp_dir();
    fs::write(
        temp.path().join("a.json"),
        r#"{"FetchedAt":"2026-11-07T14:53:13.5301234+01:00","Mods":[{"Id":"1A2B3C4D5E6F7001","Name":"Beispiel Fahrzeuge","Version":"1.4.0"},{"Id":"1A2B3C4D5E6F7003","Name":"Beispiel ohne Version","Version":null}]}"#,
    )
    .unwrap();

    let cached = ModListCache::new(temp.path()).load("a").unwrap();

    assert_eq!(cached.fetched_at.to_rfc3339(), "2026-11-07T14:53:13.530123400+01:00");
    assert_eq!(cached.mods[1], ModEntry::new("1A2B3C4D5E6F7003", "Beispiel ohne Version", None));
}

#[test]
fn writes_the_cache_in_the_format_of_the_wpf_app() {
    let temp = common::temp_dir();
    let cache = ModListCache::new(temp.path());

    cache.save("a", &[ModEntry::new("1A2B3C4D5E6F7001", "Beispiel Fahrzeuge", Some("1.4.0"))]).unwrap();

    let written: serde_json::Value = serde_json::from_str(&fs::read_to_string(temp.path().join("a.json")).unwrap()).unwrap();
    assert_eq!(written["Mods"][0]["Id"], "1A2B3C4D5E6F7001");
    let time = written["FetchedAt"].as_str().unwrap();
    assert!(chrono::DateTime::parse_from_rfc3339(time).is_ok(), "{time}");
}
