//! Spielclient: Startparameter, Log-Auswertung und der laufende Prozess.

pub mod args;
pub mod log;

/// Ein laufender Spielclient, so wie Liste und Download ihn brauchen. Tests setzen einen eigenen ein.
pub trait ClientProcess: Send + Sync {
    fn has_exited(&self) -> bool;
}
