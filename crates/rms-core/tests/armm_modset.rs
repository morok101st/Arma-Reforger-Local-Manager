//! Mod-Liste aus ARMM: Prüfung der Antwort und Zuordnung der HTTP-Fehler.

mod common;

use rms_core::addons::ModEntry;
use rms_core::armm::{
    ArmmLink, FetchError, MAX_MODSET_BYTES, ModsetError, ModsetSource, WinHttpModsetSource, host_and_port, parse_modset, request_path,
    status_error,
};

fn sample() -> Vec<u8> {
    std::fs::read(common::samples_dir().join("armm-modset.json")).unwrap()
}

#[test]
fn reads_the_complete_list_of_the_server() {
    let mods = parse_modset(&sample()).unwrap();

    assert_eq!(mods.len(), 91);
    assert!(mods.contains(&ModEntry::new("1337C0DE5DABBEEF", "RHS - Content Pack 01", Some("0.17.5281"))));
    assert!(mods.iter().all(|mod_entry| mod_entry.version.is_some()));
}

#[test]
fn reads_mods_from_a_server_config() {
    let config = br#"{ "bindPort": 2001, "game": { "name": "Beispiel", "mods": [
        { "modId": "65ad7d2e9866fa6e", "name": "ACE Finger Dev", "version": "1.5.40" } ] } }"#;

    let mods = parse_modset(config).unwrap();

    assert_eq!(mods, vec![ModEntry::new("65AD7D2E9866FA6E", "ACE Finger Dev", Some("1.5.40"))]);
}

#[test]
fn every_entry_is_checked() {
    let one = |entry: &str| format!("[{{\"modId\":\"65AD7D2E9866FA6E\",\"name\":\"A\",\"version\":\"1.0.0\"}},{entry}]");
    let cases = [
        (one(r#"{"name":"B","version":"1.0.0"}"#), "modId fehlt"),
        (one(r#"{"modId":"123","name":"B","version":"1.0.0"}"#), "modId ist keine Workshop-ID"),
        (one(r#"{"modId":"ZZZZZZZZZZZZZZZZ","name":"B","version":"1.0.0"}"#), "modId ist keine Workshop-ID"),
        (one(r#"{"modId":"5A3E0C1B9D7F6000","version":"1.0.0"}"#), "name fehlt"),
        (one(r#"{"modId":"5A3E0C1B9D7F6000","name":"B","version":" "}"#), "version fehlt"),
        (one(r#"{"modId":"5A3E0C1B9D7F6000","name":"B"}"#), "version fehlt"),
        (one(r#"{"modId":"5A3E0C1B9D7F6000","name":"B|C","version":"1.0.0"}"#), "unerlaubtes Zeichen"),
        (one(r#"{"modId":"5A3E0C1B9D7F6000","name":"B","version":"1.0;0"}"#), "unerlaubtes Zeichen"),
    ];
    for (text, problem) in cases {
        assert_eq!(parse_modset(text.as_bytes()), Err(ModsetError::InvalidEntry { position: 2, problem }), "{problem}");
    }
}

#[test]
fn whole_list_is_refused_on_doubtful_input() {
    let duplicate = r#"[{"modId":"65AD7D2E9866FA6E","name":"A","version":"1"},{"modId":"65ad7d2e9866fa6e","name":"A","version":"2"}]"#;
    assert_eq!(parse_modset(duplicate.as_bytes()), Err(ModsetError::Duplicate("65AD7D2E9866FA6E".into())));
    assert_eq!(parse_modset(b"[]"), Err(ModsetError::Empty));
    assert_eq!(parse_modset(b"{}"), Err(ModsetError::NotAList));
    assert_eq!(parse_modset(b"<html>"), Err(ModsetError::NotAList));
    assert_eq!(parse_modset(b"\"text\""), Err(ModsetError::NotAList));
    assert_eq!(parse_modset(&vec![b' '; MAX_MODSET_BYTES + 1]), Err(ModsetError::TooLarge));
    let with_bom = [b"\xEF\xBB\xBF".as_slice(), &sample()].concat();
    assert_eq!(parse_modset(&with_bom).unwrap().len(), 91);
}

#[test]
fn http_status_becomes_a_clear_message() {
    assert_eq!(status_error(200), None);
    assert_eq!(status_error(401), Some(FetchError::Unauthorized));
    assert_eq!(status_error(403), Some(FetchError::Unauthorized));
    assert_eq!(status_error(404), Some(FetchError::NotFound));
    assert_eq!(status_error(500), Some(FetchError::Status(500)));
    assert_eq!(status_error(302), Some(FetchError::Status(302)));
}

#[test]
fn request_goes_to_host_port_and_endpoint() {
    let link = ArmmLink { base_url: "https://armm.example.com:8443".into(), modset_id: 42 };
    assert_eq!(host_and_port(&link), ("armm.example.com".into(), 8443));
    assert_eq!(request_path(&link), "/api/sync/modsets/42");
    let link = ArmmLink { base_url: "https://armm.example.com".into(), modset_id: 7 };
    assert_eq!(host_and_port(&link), ("armm.example.com".into(), 443));
}

#[test]
fn messages_never_contain_the_key() {
    for error in [FetchError::Unauthorized, FetchError::NotFound, FetchError::Status(500), FetchError::Network("x".into())] {
        assert!(!error.to_string().contains("Bearer"));
    }
}

/// Prüft die WinHTTP-Kette gegen einen öffentlichen Server ohne ARMM-Endpunkt: `cargo test -- --ignored`.
#[test]
#[ignore = "braucht Internet"]
fn winhttp_reaches_a_real_https_server() {
    let link = ArmmLink { base_url: "https://example.com".into(), modset_id: 1 };

    let result = WinHttpModsetSource.fetch(&link, "kein-echter-schluessel");

    println!("Ergebnis: {:?}", result.as_ref().map(Vec::len));
    assert!(matches!(result, Err(FetchError::NotFound | FetchError::Status(_)) | Ok(_)), "{result:?}");
}
