//! Spielclient: Startparameter, Log-Auswertung, Passwortdatei und der laufende Prozess.

pub mod args;
pub mod launcher;
pub mod log;
pub mod password_file;
pub mod precheck;
pub mod retention;
pub mod session;

use std::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

pub type StopFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Ein laufender Spielclient, so wie Liste und Download ihn brauchen. Tests setzen einen eigenen ein.
pub trait ClientProcess: Send + Sync {
    fn has_exited(&self) -> bool;

    /// Erst sauber schließen, sonst beenden.
    fn stop(&self) -> StopFuture<'_> {
        Box::pin(async {})
    }
}

/// Startet den Spielclient mit diesen Startparametern. `hide_window`: Spielfenster ausblenden.
pub type ClientStarter = Arc<dyn Fn(&Path, &[String], bool) -> io::Result<Box<dyn ClientProcess>> + Send + Sync>;

/// Der echte Start über [`session::ClientSession`].
pub fn default_starter() -> ClientStarter {
    Arc::new(|exe, arguments, hide_window| {
        Ok(Box::new(session::ClientSession::start(exe, arguments, hide_window)?) as Box<dyn ClientProcess>)
    })
}
