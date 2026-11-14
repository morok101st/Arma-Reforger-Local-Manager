//! Gemeinsame Hilfen der Tests. Die Beispiel-Logs sind dieselben wie bei den C#-Tests.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rms_core::client::ClientProcess;

/// Steht in Tests für den Spielclient.
#[derive(Default)]
pub struct FakeClient {
    pub exited: AtomicBool,
}

impl FakeClient {
    pub fn exited() -> Self {
        Self { exited: AtomicBool::new(true) }
    }
}

impl ClientProcess for FakeClient {
    fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }
}

pub fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/samples")
}

pub fn sample(name: &str) -> String {
    fs::read_to_string(samples_dir().join(name)).expect("Beispiel-Log vorhanden")
}

/// Legt `console.log` an, wie der Client es im Log-Ordner tut.
pub fn write_log(logs_dir: &Path, content: &str) {
    fs::write(logs_dir.join("console.log"), content).unwrap();
}

pub fn temp_dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("ReforgerModSyncTests").tempdir().unwrap()
}
