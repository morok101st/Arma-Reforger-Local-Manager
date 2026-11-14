mod common;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Duration;

use common::FakeClient;
use rms_core::addons::ModEntry;
use rms_core::sync::{ServerListError, ServerListReader};
use tokio_util::sync::CancellationToken;

const ADDRESS: &str = "203.0.113.10:2001";

fn reader() -> ServerListReader {
    ServerListReader::new(Duration::from_millis(10), Duration::from_secs(5))
}

async fn read_sample(sample: &str) -> Result<Vec<ModEntry>, ServerListError> {
    let temp = common::temp_dir();
    common::write_log(temp.path(), &common::sample(sample));
    reader().read(&FakeClient::default(), temp.path(), ADDRESS, &CancellationToken::new()).await
}

fn message(result: Result<Vec<ModEntry>, ServerListError>) -> String {
    match result {
        Err(ServerListError::Failed(message)) => message,
        other => panic!("Fehlermeldung erwartet, bekommen: {other:?}"),
    }
}

#[tokio::test]
async fn reads_all_mods_including_one_without_version() {
    let mods = read_sample("list-done.log").await.unwrap();

    assert_eq!(
        mods,
        [
            ModEntry::new("1A2B3C4D5E6F7001", "Beispiel Fahrzeuge", Some("1.4.0")),
            ModEntry::new("1A2B3C4D5E6F7002", "Beispiel Karte Nord", Some("2.1.3")),
            ModEntry::new("1A2B3C4D5E6F7003", "Beispiel ohne Version", None),
        ]
    );
}

#[tokio::test]
async fn an_incomplete_list_never_counts() {
    let message = message(read_sample("list-incomplete.log").await);

    assert!(message.starts_with("Die Mod-Liste kam unvollständig an (1 von 2 Mods)"), "{message}");
}

#[tokio::test]
async fn missing_password_gets_a_clear_message() {
    assert_eq!(
        message(read_sample("list-password-required.log").await),
        "Der Server verlangt ein Passwort. Bitte in den Einstellungen eintragen."
    );
}

#[tokio::test]
async fn a_room_with_another_address_counts_as_not_found() {
    let message = message(read_sample("list-address-mismatch.log").await);

    assert!(message.starts_with("Der Server 203.0.113.10:2001 wurde nicht gefunden."), "{message}");
}

#[tokio::test]
async fn lines_written_while_reading_are_picked_up() {
    let temp = common::temp_dir();
    let sample = common::sample("list-done.log");
    let lines: Vec<&str> = sample.split('\n').collect();
    common::write_log(temp.path(), &(lines[..7].join("\n") + "\n"));
    let client = FakeClient::default();
    let cancel = CancellationToken::new();
    let reader = reader();

    let reading = reader.read(&client, temp.path(), ADDRESS, &cancel);
    let writing = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut file = OpenOptions::new().append(true).open(temp.path().join("console.log")).unwrap();
        file.write_all(lines[7..].join("\n").as_bytes()).unwrap();
    };
    let (result, ()) = tokio::join!(reading, writing);

    assert_eq!(result.unwrap().len(), 3);
}

#[tokio::test]
async fn a_client_that_exits_early_is_reported_with_its_logs() {
    let temp = common::temp_dir();
    common::write_log(temp.path(), "14:53:09.618 ENGINE       : Game successfully created.\n");
    fs::write(temp.path().join("crash.log"), "").unwrap();

    let result = reader().read(&FakeClient::exited(), temp.path(), ADDRESS, &CancellationToken::new()).await;

    assert_eq!(message(result), format!("Der Spielclient ist abgestürzt. Client-Logs: {}", temp.path().display()));
}

#[tokio::test]
async fn a_client_that_exits_without_crash_is_reported_too() {
    let temp = common::temp_dir();

    let result = reader().read(&FakeClient::exited(), temp.path(), ADDRESS, &CancellationToken::new()).await;

    assert!(message(result).starts_with("Der Spielclient wurde beendet, bevor die Liste da war."));
}

#[tokio::test]
async fn gives_up_after_the_timeout() {
    let temp = common::temp_dir();
    let reader = ServerListReader::new(Duration::from_millis(10), Duration::from_millis(100));

    let result = reader.read(&FakeClient::default(), temp.path(), ADDRESS, &CancellationToken::new()).await;

    assert!(message(result).starts_with("Die Mod-Liste kam nicht innerhalb von"));
}

#[tokio::test]
async fn cancelling_stops_reading() {
    let temp = common::temp_dir();
    let cancel = CancellationToken::new();
    let stopper = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        stopper.cancel();
    });

    let result = reader().read(&FakeClient::default(), temp.path(), ADDRESS, &cancel).await;

    assert_eq!(result, Err(ServerListError::Cancelled));
}
