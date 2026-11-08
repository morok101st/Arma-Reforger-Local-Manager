mod common;

use std::fs;

use rms_core::addons::{InstalledAddon, ModComparison, ModEntry, ModState, ModSummary, sort_open_first};
use rms_core::settings::validate_profile_input;
use rms_core::steam::find_client_exe;

// --- Steam ---

#[test]
fn finds_the_client_in_a_second_library() {
    let temp = common::temp_dir();
    let steam = temp.path().join("Steam");
    let library = temp.path().join("Other Library");
    fs::create_dir_all(steam.join("steamapps")).unwrap();
    let escape = |path: &std::path::Path| path.display().to_string().replace('\\', r"\\");
    fs::write(
        steam.join("steamapps").join("libraryfolders.vdf"),
        format!(
            "\"libraryfolders\"\n{{\n    \"0\" {{ \"path\"  \"{}\" }}\n    \"1\" {{ \"path\"  \"{}\" }}\n}}\n",
            escape(&steam),
            escape(&library)
        ),
    )
    .unwrap();

    let install = library.join("steamapps").join("common").join("Arma Reforger");
    fs::create_dir_all(&install).unwrap();
    fs::write(
        library.join("steamapps").join("appmanifest_1874880.acf"),
        r#""AppState" { "appid" "1874880" "installdir" "Arma Reforger" }"#,
    )
    .unwrap();
    let exe = install.join("ArmaReforgerSteam.exe");
    fs::write(&exe, "").unwrap();

    assert_eq!(find_client_exe(&steam), Some(exe));
}

#[test]
fn returns_none_without_client_or_library_file() {
    let temp = common::temp_dir();

    assert_eq!(find_client_exe(temp.path()), None);
}

// --- Eingaben eines Servers ---

#[test]
fn profile_input_is_checked_in_order() {
    let cases = [
        ("", "203.0.113.10", "2001", Some("Bitte einen Namen für diesen Server eintragen.")),
        ("Training", "server.example", "2001", Some("Bitte die IP-Adresse des Servers eintragen, zum Beispiel 203.0.113.10.")),
        ("Training", "203.0.113.10", "70000", Some("Bitte den Port des Servers eintragen, eine Zahl von 1 bis 65535, zum Beispiel 2001.")),
        ("Training", "203.0.113.10", "2001", None),
    ];
    for (name, ip, port, expected) in cases {
        assert_eq!(validate_profile_input(Some(name), Some(ip), Some(port)), expected, "{name} {ip} {port}");
    }
}

// --- Zusammenfassung ---

fn comparison(name: &str, state: ModState) -> ModComparison {
    let id = format!("{name:0<16}");
    ModComparison { required: ModEntry::new(&id, name, Some("1.0.0")), state, installed: None::<InstalledAddon> }
}

#[test]
fn summary_counts_every_kind_of_open_mod() {
    let summary = ModSummary::from_comparison(&[
        comparison("A", ModState::Current),
        comparison("B", ModState::Current),
        comparison("C", ModState::Missing),
        comparison("D", ModState::Outdated),
        comparison("E", ModState::Damaged),
    ]);

    assert_eq!((summary.total, summary.current, summary.open()), (5, 2, 3));
    assert_eq!(summary.list_status, "3 von 5 offen");
    assert_eq!(summary.text, "2 von 5 Mods aktuell · 1 fehlt · 1 veraltet · 1 beschädigt");
}

#[test]
fn summary_of_a_current_list() {
    let summary = ModSummary::from_comparison(&[comparison("A", ModState::Current)]);

    assert_eq!(summary.list_status, "aktuell");
    assert_eq!(summary.text, "Alles aktuell: 1 Mods");
}

#[test]
fn open_mods_come_first() {
    let mut sorted = [comparison("A", ModState::Current), comparison("Z", ModState::Missing), comparison("B", ModState::Newer)];

    sort_open_first(&mut sorted);

    assert_eq!(sorted.map(|mod_state| mod_state.required.name), ["B", "Z", "A"]);
}
