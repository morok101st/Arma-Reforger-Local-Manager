//! Liest die Mod-Liste, die die Hilfs-Mod mit `[RMS] SERVER_MOD` ins Log des Clients schreibt.

use std::path::Path;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use crate::addons::ModEntry;
use crate::client::ClientProcess;
use crate::client::log::{LogTail, ServerListEnd, parse_list_done_count, parse_server_list_end, parse_server_mod};

/// Ein Listenabruf ist gescheitert. Die Meldung ist für den Nutzer gedacht.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServerListError {
    #[error("{0}")]
    Failed(String),

    #[error("Abgebrochen.")]
    Cancelled,
}

#[derive(Debug, Clone, Copy)]
pub struct ServerListReader {
    poll_interval: Duration,
    timeout: Duration,
}

impl Default for ServerListReader {
    fn default() -> Self {
        Self::new(Duration::from_millis(500), Duration::from_secs(120))
    }
}

impl ServerListReader {
    pub fn new(poll_interval: Duration, timeout: Duration) -> Self {
        Self { poll_interval, timeout }
    }

    pub async fn read(
        &self,
        client: &dyn ClientProcess,
        logs_dir: &Path,
        address: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<ModEntry>, ServerListError> {
        let fail = |message: String| Err(ServerListError::Failed(message));
        let logs = logs_dir.display();
        let mut mods = Vec::new();
        let mut tail = LogTail::new(logs_dir.join("console.log"));
        let deadline = Instant::now() + self.timeout;

        while Instant::now() < deadline {
            tokio::select! {
                () = cancel.cancelled() => return Err(ServerListError::Cancelled),
                () = tokio::time::sleep(self.poll_interval) => {}
            }

            // Erst den Prozessstand lesen: Beendet er sich gerade, stehen seine letzten Zeilen schon im Log.
            let exited = client.has_exited();
            for line in tail.read_new_lines() {
                if let Some(mod_entry) = parse_server_mod(&line) {
                    mods.push(mod_entry);
                }

                match parse_server_list_end(&line) {
                    None => {}
                    Some(ServerListEnd::Done) => {
                        // Die Liste entscheidet auch darüber, was gelöscht wird. Eine unvollständige darf nie gelten.
                        if let Some(expected) = parse_list_done_count(&line)
                            && expected != mods.len()
                        {
                            return fail(format!(
                                "Die Mod-Liste kam unvollständig an ({} von {expected} Mods). Bitte erneut versuchen. Client-Logs: {logs}",
                                mods.len()
                            ));
                        }
                        return Ok(mods);
                    }
                    Some(ServerListEnd::NotFound) => {
                        return fail(format!("Der Server {address} wurde nicht gefunden. Läuft er, und stimmt die Adresse?"));
                    }
                    Some(ServerListEnd::PasswordRequired) => {
                        return fail("Der Server verlangt ein Passwort. Bitte in den Einstellungen eintragen.".into());
                    }
                    Some(ServerListEnd::PasswordRejected) => {
                        return fail("Der Server hat das Passwort abgelehnt. Bitte in den Einstellungen prüfen.".into());
                    }
                    Some(ServerListEnd::Failed) => {
                        return fail("Die Mod-Liste konnte nicht geladen werden. Bitte später erneut versuchen.".into());
                    }
                }
            }

            if exited {
                return fail(if logs_dir.join("crash.log").is_file() {
                    format!("Der Spielclient ist abgestürzt. Client-Logs: {logs}")
                } else {
                    format!("Der Spielclient wurde beendet, bevor die Liste da war. Client-Logs: {logs}")
                });
            }
        }

        fail(format!("Die Mod-Liste kam nicht innerhalb von {:.0} Minuten an. Client-Logs: {logs}", self.timeout.as_secs_f64() / 60.0))
    }
}
