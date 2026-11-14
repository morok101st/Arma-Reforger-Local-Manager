mod common;

use std::fs::{self, OpenOptions};
use std::io::Write;

use rms_core::addons::ModEntry;
use rms_core::client::args::{
    HELPER_GUID, build_download, build_join, build_list_request, clean_port_input, combine_server_address, is_valid_host, is_valid_ip,
    is_valid_port, is_valid_server_address, normalize_address_input, resolve_server_address, split_server_address, steam_launch_options,
    to_command_line,
};
use rms_core::client::log::{ClientLogEvent, LogTail, ServerListEnd, classify, parse_mod_failed, parse_server_list_end, parse_server_mod};
use rms_core::settings::ClientWindowSettings;

fn mods() -> [ModEntry; 2] {
    [ModEntry::new("65AD7C139EB4C1A1", "ACE Backblast Dev", Some("1.5.36")), ModEntry::new("64EE818E08AFCF94", "MFD Framework", None)]
}

fn download(download_dir: Option<&str>) -> Vec<String> {
    build_download("203.0.113.10:2001", &mods(), download_dir, &ClientWindowSettings::default(), r"C:\logs", r"C:\profil", r"C:\app\helper")
}

fn list_request() -> Vec<String> {
    build_list_request("203.0.113.10:2001", Some(r"D:\Mods"), &ClientWindowSettings::default(), r"C:\logs", r"C:\profil", r"C:\app\helper")
}

fn value<'a>(args: &'a [String], name: &str) -> &'a str {
    let index = args.iter().position(|arg| arg == name).unwrap_or_else(|| panic!("{name} fehlt"));
    &args[index + 1]
}

fn contains(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

// --- Log ---

#[test]
fn real_log_lines_are_classified() {
    let cases = [
        ("11:46:47.145 BACKEND      : Addon Download started 65AD7C139EB4C1A1 - ACE Backblast Dev", ClientLogEvent::Activity),
        ("11:46:48.347 BACKEND      : Downloading 65AD7C139EB4C1A1 version 1.5.36", ClientLogEvent::Activity),
        ("11:47:52.528 BACKEND      : Updating 64EE818E08AFCF94 from 0.5.6 to 0.5.8", ClientLogEvent::Activity),
        ("11:46:47.145 SCRIPT       : [RMS] DOWNLOAD_STARTED 65AD7C139EB4C1A1 1.5.36", ClientLogEvent::Activity),
        ("11:46:50.746 SCRIPT       : [RMS] DONE failed=0", ClientLogEvent::HelperDone),
        ("11:46:50.746 SCRIPT       : [RMS] MOD_FAILED 65AD7C139EB4C1A1 not found", ClientLogEvent::ModFailed),
        (r"09:49:27.425 BACKEND   (E): Cannot move a file from C:\a to E:\b", ClientLogEvent::BackendError),
        ("09:54:40.597 BACKEND      : Addon Remove 6AAD0E4883FE0B85 - ROG CQB Training Simulation", ClientLogEvent::None),
    ];
    for (line, expected) in cases {
        assert_eq!(classify(line), expected, "{line}");
    }
}

#[test]
fn failed_mod_line_gives_id_and_reason() {
    assert_eq!(
        parse_mod_failed("11:46:50 SCRIPT : [RMS] MOD_FAILED 65AD7C139EB4C1A1 version 1.5.36 not available"),
        Some(("65AD7C139EB4C1A1".into(), "version 1.5.36 not available".into()))
    );
}

#[test]
fn server_mod_line_gives_id_version_and_name_with_spaces() {
    assert_eq!(
        parse_server_mod("12:35:56 SCRIPT       : [RMS] SERVER_MOD 1337C0DE5DABBEEF 0.16.5236 RHS - Content Pack 01"),
        Some(ModEntry::new("1337C0DE5DABBEEF", "RHS - Content Pack 01", Some("0.16.5236")))
    );
}

#[test]
fn server_mod_line_without_version_means_any_version() {
    assert_eq!(
        parse_server_mod("12:35:56 SCRIPT : [RMS] SERVER_MOD 1337C0DE5DABBEEF  Pack"),
        Some(ModEntry::new("1337C0DE5DABBEEF", "Pack", None))
    );
}

#[test]
fn server_list_end_is_recognized() {
    let cases = [
        ("[RMS] LIST_DONE mods=76", Some(ServerListEnd::Done)),
        ("[RMS] ROOM_NOT_FOUND", Some(ServerListEnd::NotFound)),
        ("[RMS] ROOM_PASSWORD_REQUIRED", Some(ServerListEnd::PasswordRequired)),
        ("[RMS] ROOM_PASSWORD_REJECTED http=403", Some(ServerListEnd::PasswordRejected)),
        ("[RMS] ROOM_SEARCH_FAILED http=500", Some(ServerListEnd::Failed)),
        ("[RMS] ROOM_LIST_FAILED http=500", Some(ServerListEnd::Failed)),
        ("[RMS] ROOM_FOUND rooms=1", None),
        ("[RMS] SERVER_MOD 1337C0DE5DABBEEF 1.0 ROOM_NOT_FOUND Pack", None),
    ];
    for (line, expected) in cases {
        assert_eq!(parse_server_list_end(line), expected, "{line}");
    }
}

#[test]
fn tail_returns_only_new_complete_lines() {
    let temp = common::temp_dir();
    let path = temp.path().join("console.log");
    let mut tail = LogTail::new(&path);
    assert!(tail.read_new_lines().is_empty());

    fs::write(&path, "eins\r\nzw").unwrap();
    assert_eq!(tail.read_new_lines(), ["eins"]);

    OpenOptions::new().append(true).open(&path).unwrap().write_all(b"ei\ndrei\n").unwrap();
    assert_eq!(tail.read_new_lines(), ["zwei", "drei"]);
    assert!(tail.read_new_lines().is_empty());
}

#[test]
fn tail_keeps_a_character_split_between_two_reads() {
    let temp = common::temp_dir();
    let path = temp.path().join("console.log");
    let mut tail = LogTail::new(&path);
    let text = "Größe\n".as_bytes();

    fs::write(&path, &text[..3]).unwrap();
    assert!(tail.read_new_lines().is_empty());
    OpenOptions::new().append(true).open(&path).unwrap().write_all(&text[3..]).unwrap();

    assert_eq!(tail.read_new_lines(), ["Größe"]);
}

#[test]
fn tail_can_read_while_the_client_keeps_the_file_open() {
    let temp = common::temp_dir();
    let path = temp.path().join("console.log");
    let mut writer = OpenOptions::new().create(true).write(true).truncate(true).open(&path).unwrap();
    writer.write_all(b"zeile\n").unwrap();
    writer.flush().unwrap();

    assert_eq!(LogTail::new(&path).read_new_lines(), ["zeile"]);
}

// --- Startparameter ---

#[test]
fn download_arguments_load_the_helper_and_keep_the_players_profile_untouched() {
    let args = download(Some(r"D:\Meine Mods"));

    assert_eq!(value(&args, "-addonsDir"), r"D:\Meine Mods,C:\app\helper");
    assert_eq!(value(&args, "-addonDownloadDir"), r"D:\Meine Mods");
    assert_eq!(value(&args, "-addons"), HELPER_GUID);
    assert_eq!(value(&args, "-profile"), r"C:\profil");
    assert_eq!(value(&args, "-logsDir"), r"C:\logs");
    assert_eq!(value(&args, "-rmsServer"), "203.0.113.10:2001");
    assert_eq!(value(&args, "-rmsMods"), "65AD7C139EB4C1A1:1.5.36;64EE818E08AFCF94");
    assert!(contains(&args, "-noSound"));
}

#[test]
fn list_request_asks_for_the_server_and_contains_no_password() {
    let args = list_request();

    assert_eq!(value(&args, "-rmsServer"), "203.0.113.10:2001");
    assert!(!contains(&args, "-rmsMods"));
    assert!(!args.iter().any(|arg| arg.to_lowercase().contains("password")));
}

#[test]
fn the_client_never_connects_to_the_game_server() {
    assert!(!contains(&download(Some(r"D:\Mods")), "-client"));
    assert!(!contains(&list_request(), "-client"));
}

#[test]
fn without_folder_the_clients_default_is_set_explicitly_because_of_the_own_profile() {
    let args = download(None);

    assert!(value(&args, "-addonDownloadDir").ends_with(r"My Games\ArmaReforger"));
    assert!(value(&args, "-addonsDir").ends_with(r"My Games\ArmaReforger,C:\app\helper"));
}

#[test]
fn join_uses_the_helper_and_the_players_own_profile() {
    let args = build_join("203.0.113.10:2001", Some(r"D:\Meine Mods"), r"C:\app\helper", r"C:\Daten\clientlogs\lauf");

    assert_eq!(value(&args, "-addonsDir"), r"D:\Meine Mods,C:\app\helper");
    assert_eq!(value(&args, "-addonDownloadDir"), r"D:\Meine Mods");
    assert_eq!(value(&args, "-addons"), HELPER_GUID);
    assert_eq!(value(&args, "-rmsJoin"), "203.0.113.10:2001");
    assert_eq!(value(&args, "-logsDir"), r"C:\Daten\clientlogs\lauf");
    assert!(!contains(&args, "-profile"));
    assert!(!contains(&args, "-client"));
}

#[test]
fn join_without_folder_keeps_the_games_own_mod_folder() {
    let args = build_join("203.0.113.10:2001", None, r"C:\app\helper", r"C:\Daten\clientlogs\lauf");

    assert_eq!(value(&args, "-addonsDir"), r"C:\app\helper");
    assert!(!contains(&args, "-addonDownloadDir"));
}

#[test]
fn command_line_quotes_values_with_spaces() {
    assert_eq!(
        to_command_line(&["-exe", "ArmaReforgerSteam.exe", "-addonsDir", r"D:\Meine Mods,C:\app helper\x"]),
        r#"-exe ArmaReforgerSteam.exe -addonsDir "D:\Meine Mods,C:\app helper\x""#
    );
}

#[test]
fn simple_values_stay_unquoted() {
    assert_eq!(
        to_command_line(&["-addons", "7A3C1E5D9B2F4860", "-rmsJoin", "203.0.113.10:2001"]),
        "-addons 7A3C1E5D9B2F4860 -rmsJoin 203.0.113.10:2001"
    );
}

#[test]
fn a_trailing_backslash_does_not_swallow_the_closing_quote() {
    assert_eq!(to_command_line(&[r"D:\Meine Mods\"]), r#""D:\Meine Mods\\""#);
}

#[test]
fn steam_launch_options_are_quoted_for_paths_with_spaces() {
    assert_eq!(steam_launch_options(Some(r"D:\Meine Mods")), r#"-addonsDir "D:\Meine Mods" -addonDownloadDir "D:\Meine Mods""#);
}

#[test]
fn default_folder_needs_no_steam_launch_options() {
    assert_eq!(steam_launch_options(None), "");
}

// --- Adresse ---

#[test]
fn server_address_needs_host_and_port() {
    let cases = [
        (Some("203.0.113.10:2001"), true),
        (Some(" 203.0.113.10:2001 "), true),
        (Some("server.example.org:2001"), true),
        (Some("203.0.113.10"), false),
        (Some(""), false),
        (None, false),
        (Some("999.1.1.1:2001"), false),
        (Some("arma.example.de:2001"), true),
        (Some("arma.example.de"), false),
        (Some("203.0.113.10:70000"), false),
        (Some("203.0.113.10:0"), false),
        (Some("203.0.113.10:"), false),
        (Some(":2001"), false),
    ];
    for (address, valid) in cases {
        assert_eq!(is_valid_server_address(address), valid, "{address:?}");
    }
}

#[test]
fn ip_must_be_a_real_ipv4_address() {
    let cases = [
        (Some("0.0.0.0"), true),
        (Some("255.255.255.255"), true),
        (Some(" 203.0.113.10 "), true),
        (Some("256.1.1.1"), false),
        (Some("1.2.3"), false),
        (Some("1.2.3.4.5"), false),
        (Some("01.2.3.4"), false),
        (Some("server.example.org"), false),
        (Some(""), false),
        (None, false),
    ];
    for (ip, valid) in cases {
        assert_eq!(is_valid_ip(ip), valid, "{ip:?}");
    }
}

#[test]
fn host_may_be_an_ipv4_address_or_a_name() {
    let cases = [
        (Some("203.0.113.10"), true),
        (Some("arma.example.de"), true),
        (Some("ARMA.Example.DE"), true),
        (Some("server-1.example.org"), true),
        (Some("localhost"), true),
        (Some("xn--mnchen-3ya.de"), true),
        (Some("1.2.3"), false),
        (Some("256.1.1.1"), false),
        (Some("-arma.example.de"), false),
        (Some("arma-.example.de"), false),
        (Some("arma..example.de"), false),
        (Some("arma example.de"), false),
        (Some("arma_server.de"), false),
        (Some("münchen.de"), false),
        (Some(&"a".repeat(64)), false),
        (Some(""), false),
        (None, false),
    ];
    for (host, valid) in cases {
        assert_eq!(is_valid_host(host), valid, "{host:?}");
    }
}

#[test]
fn pasted_urls_are_reduced_to_name_and_port() {
    let cases = [
        ("https://Arma.Example.de:2001/", "arma.example.de:2001"),
        ("http://arma.example.de/server?x=1", "arma.example.de"),
        (" arma.example.de. ", "arma.example.de"),
        ("203.0.113.10:2001", "203.0.113.10:2001"),
        ("", ""),
    ];
    for (input, expected) in cases {
        assert_eq!(normalize_address_input(input), expected, "{input:?}");
    }
    assert_eq!(split_server_address(Some("https://arma.example.de:2001/")), ("arma.example.de".into(), "2001".into()));
    assert_eq!(combine_server_address(Some("Arma.Example.de"), Some("2001")).as_deref(), Some("arma.example.de:2001"));
}

#[test]
fn resolving_keeps_an_ip_and_looks_up_a_name() {
    assert_eq!(resolve_server_address("203.0.113.10:2001").as_deref(), Ok("203.0.113.10:2001"));
    // localhost löst Windows ohne Netzwerk auf.
    assert_eq!(resolve_server_address("localhost:2001").as_deref(), Ok("127.0.0.1:2001"));
    assert!(resolve_server_address("arma.example.de").is_err());
    let missing = resolve_server_address("gibt-es-nicht.invalid:2001").unwrap_err();
    assert!(missing.contains("gibt-es-nicht.invalid"), "{missing}");
}

#[test]
fn port_must_be_in_range() {
    let cases = [
        (Some("1"), true),
        (Some("2001"), true),
        (Some("65535"), true),
        (Some("0"), false),
        (Some("65536"), false),
        (Some("abc"), false),
        (Some(""), false),
        (None, false),
    ];
    for (port, valid) in cases {
        assert_eq!(is_valid_port(port), valid, "{port:?}");
    }
}

#[test]
fn splits_address_for_the_input_fields() {
    let cases = [
        (Some("203.0.113.10:2001"), "203.0.113.10", "2001"),
        (Some(" 203.0.113.10:2001 "), "203.0.113.10", "2001"),
        (Some("203.0.113.10"), "203.0.113.10", ""),
        (Some("203.0.113.10:"), "203.0.113.10", ""),
        (Some(""), "", ""),
        (None, "", ""),
    ];
    for (address, ip, port) in cases {
        assert_eq!(split_server_address(address), (ip.to_string(), port.to_string()), "{address:?}");
    }
}

#[test]
fn combines_fields_to_the_stored_address() {
    let cases = [
        (Some("203.0.113.10"), Some("2001"), Some("203.0.113.10:2001")),
        (Some(" 203.0.113.10 "), Some(" 2001 "), Some("203.0.113.10:2001")),
        (Some("203.0.113.10"), Some("02001"), Some("203.0.113.10:2001")),
        (Some("203.0.113.10"), Some(""), Some("203.0.113.10:")),
        (Some(""), Some(""), None),
        (None, None, None),
    ];
    for (ip, port, expected) in cases {
        assert_eq!(combine_server_address(ip, port).as_deref(), expected, "{ip:?} {port:?}");
    }
}

#[test]
fn split_and_combine_keep_a_saved_address_unchanged() {
    // Das Passwort hängt an diesem Wert. Ändert er sich beim Laden, ginge die Zuordnung verloren.
    for address in ["203.0.113.10:2001", "1.1.1.1:65535", "arma.example.de:2001"] {
        let (ip, port) = split_server_address(Some(address));
        assert_eq!(combine_server_address(Some(&ip), Some(&port)).as_deref(), Some(address));
    }
}

#[test]
fn port_field_keeps_only_digits() {
    let cases =
        [(Some("2001"), "2001"), (Some("20x01 "), "2001"), (Some("abc"), ""), (Some("123456789"), "12345"), (Some(""), ""), (None, "")];
    for (input, expected) in cases {
        assert_eq!(clean_port_input(input), expected, "{input:?}");
    }
}
