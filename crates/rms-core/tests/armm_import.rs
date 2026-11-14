//! Importdatei `armm-sync.json` aus ARMM.

use rms_core::armm::{ArmmLink, ImportError, MAX_IMPORT_BYTES, parse_sync_file};

const VALID: &str = r#"{
  "format": "armm-sync",
  "formatVersion": 1,
  "name": "Community Server",
  "armm": {
    "baseUrl": "https://armm.example.com",
    "modsetId": 42,
    "accessKey": "armm_sync_BEISPIELSCHLUESSEL"
  },
  "gameServer": {
    "host": "gameserver.example.com",
    "port": 2001,
    "password": null
  }
}"#;

fn with(from: &str, to: &str) -> String {
    assert!(VALID.contains(from), "{from}");
    VALID.replace(from, to)
}

fn error_of(text: &str) -> ImportError {
    parse_sync_file(text.as_bytes()).unwrap_err()
}

#[test]
fn reads_the_example_from_armm() {
    let import = parse_sync_file(VALID.as_bytes()).unwrap();

    assert_eq!(import.name, "Community Server");
    assert_eq!(import.link, ArmmLink { base_url: "https://armm.example.com".into(), modset_id: 42 });
    assert_eq!(import.access_key.as_str(), "armm_sync_BEISPIELSCHLUESSEL");
    let server = import.game_server.unwrap();
    assert_eq!(server.address(), "gameserver.example.com:2001");
    assert!(!server.password_required);
}

#[test]
fn modset_url_follows_the_agreed_endpoint() {
    let link = ArmmLink { base_url: "https://armm.example.com:8443".into(), modset_id: 42 };

    assert_eq!(link.modset_url(), "https://armm.example.com:8443/api/sync/modsets/42");
    assert_eq!(link.host(), "armm.example.com:8443");
}

#[test]
fn base_url_is_normalized() {
    let cases = [
        ("https://armm.example.com", "https://armm.example.com"),
        ("https://ARMM.Example.com/", "https://armm.example.com"),
        ("HTTPS://armm.example.com:8443", "https://armm.example.com:8443"),
        (" https://203.0.113.5:08443/ ", "https://203.0.113.5:8443"),
    ];
    for (input, expected) in cases {
        let text = with("https://armm.example.com", input);
        assert_eq!(parse_sync_file(text.as_bytes()).unwrap().link.base_url, expected, "{input}");
    }
}

#[test]
fn base_url_must_be_plain_https() {
    for input in [
        "http://armm.example.com",
        "armm.example.com",
        "https://",
        "https://armm.example.com/api",
        "https://armm.example.com?x=1",
        "https://user:pw@armm.example.com",
        "https://armm.example.com:70000",
        "https://armm example.com",
    ] {
        let text = with("https://armm.example.com", input);
        assert!(matches!(error_of(&text), ImportError::Invalid(field) if field.starts_with("armm.baseUrl")), "{input}");
    }
}

#[test]
fn rejects_other_files() {
    assert_eq!(error_of("kein json"), ImportError::NotJson);
    assert_eq!(error_of("[]"), ImportError::NotJson);
    assert_eq!(error_of(&with("\"armm-sync\"", "\"etwas-anderes\"")), ImportError::WrongFormat);
    assert_eq!(error_of(&with("\"format\": \"armm-sync\",", "")), ImportError::WrongFormat);
    let large = format!("{VALID}{}", " ".repeat(MAX_IMPORT_BYTES));
    assert_eq!(error_of(&large), ImportError::TooLarge);
}

#[test]
fn newer_format_versions_ask_for_an_update() {
    assert_eq!(error_of(&with("\"formatVersion\": 1", "\"formatVersion\": 2")), ImportError::NewerVersion);
    assert!(matches!(error_of(&with("\"formatVersion\": 1", "\"formatVersion\": 0")), ImportError::Invalid("formatVersion")));
    assert!(matches!(error_of(&with("\"formatVersion\": 1", "\"formatVersion\": \"1\"")), ImportError::Invalid("formatVersion")));
}

#[test]
fn required_fields_are_checked() {
    let cases = [
        (with("\"Community Server\"", "\"  \""), "name"),
        (with("\"modsetId\": 42", "\"modsetId\": 0"), "armm.modsetId"),
        (with("\"modsetId\": 42", "\"modsetId\": -1"), "armm.modsetId"),
        (with("\"modsetId\": 42", "\"modsetId\": \"42\""), "armm.modsetId"),
        (with("\"armm_sync_BEISPIELSCHLUESSEL\"", "\"\""), "armm.accessKey"),
        (with("\"armm_sync_BEISPIELSCHLUESSEL\"", r#""a\u0007b""#), "armm.accessKey"),
        (with("\"gameserver.example.com\"", "\"203.0.113\""), "gameServer.host"),
        (with("\"port\": 2001", "\"port\": 70000"), "gameServer.port"),
        (with("\"port\": 2001", "\"port\": \"2001\""), "gameServer.port"),
    ];
    for (text, field) in cases {
        assert_eq!(error_of(&text), ImportError::Invalid(field), "{field}");
    }
}

#[test]
fn game_server_block_is_optional() {
    let start = VALID.find(",\n  \"gameServer\"").unwrap();
    let text = format!("{}\n}}", &VALID[..start]);

    let import = parse_sync_file(text.as_bytes()).unwrap();

    assert!(import.game_server.is_none());
}

#[test]
fn a_password_in_the_file_is_refused() {
    assert_eq!(error_of(&with("\"password\": null", "\"password\": \"geheim\"")), ImportError::ContainsPassword);
    assert!(parse_sync_file(with("\"password\": null", "\"password\": \"\"").as_bytes()).is_ok());
    let flagged = with("\"password\": null", "\"passwordRequired\": true");
    assert!(parse_sync_file(flagged.as_bytes()).unwrap().game_server.unwrap().password_required);
}

#[test]
fn host_is_stored_lowercase_and_may_be_an_ip() {
    let text = with("\"gameserver.example.com\"", "\"GameServer.Example.com\"");
    assert_eq!(parse_sync_file(text.as_bytes()).unwrap().game_server.unwrap().host, "gameserver.example.com");
    let text = with("\"gameserver.example.com\"", "\"203.0.113.10\"");
    assert_eq!(parse_sync_file(text.as_bytes()).unwrap().game_server.unwrap().address(), "203.0.113.10:2001");
}

#[test]
fn accepts_a_byte_order_mark() {
    let text = format!("\u{feff}{VALID}");
    assert!(parse_sync_file(text.as_bytes()).is_ok());
}

#[test]
fn debug_output_hides_the_access_key() {
    let import = parse_sync_file(VALID.as_bytes()).unwrap();

    let printed = format!("{import:?}");

    assert!(!printed.contains("BEISPIELSCHLUESSEL"), "{printed}");
}

#[test]
fn same_connection_is_recognized_on_reimport() {
    let a = ArmmLink { base_url: "https://armm.example.com".into(), modset_id: 42 };

    assert!(a.same_as(&ArmmLink { base_url: "https://ARMM.example.com".into(), modset_id: 42 }));
    assert!(!a.same_as(&ArmmLink { base_url: "https://armm.example.com".into(), modset_id: 43 }));
}
