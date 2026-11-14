//! Merkt sich die zuletzt geholte Liste, damit die Übersicht beim App-Start ohne Spielfenster auskommt.
//! Gleiches Format wie die C#-Vorgängerversion (`modlists\<Profil-ID>.json`), damit beide denselben Stand zeigen.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::addons::ModEntry;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CachedModList {
    #[serde(serialize_with = "write_time", deserialize_with = "read_time")]
    pub fetched_at: DateTime<FixedOffset>,
    pub mods: Vec<ModEntry>,
}

/// Mikrosekunden mit Zeitzone, so liest auch die C#-Vorgängerversion den Wert.
fn write_time<S: Serializer>(time: &DateTime<FixedOffset>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&time.to_rfc3339_opts(SecondsFormat::Micros, false))
}

/// Die C#-Vorgängerversion schreibt bis zu sieben Nachkommastellen und die Zeitzone, ältere Einträge auch ohne Zeitzone.
fn read_time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTime<FixedOffset>, D::Error> {
    let text = String::deserialize(deserializer)?;
    DateTime::parse_from_rfc3339(&text)
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(&text, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|naive| naive.and_local_timezone(Local).single().map(|local| local.fixed_offset()))
                .ok()
                .flatten()
                .ok_or(())
        })
        .map_err(|_| serde::de::Error::custom(format!("ungültige Zeit: {text}")))
}

pub struct ModListCache {
    folder: PathBuf,
}

impl ModListCache {
    pub fn new(folder: impl Into<PathBuf>) -> Self {
        Self { folder: folder.into() }
    }

    pub fn save(&self, profile_id: &str, mods: &[ModEntry]) -> io::Result<()> {
        fs::create_dir_all(&self.folder)?;
        let path = self.path_for(profile_id);
        let mut temp_path = path.clone().into_os_string();
        temp_path.push(".tmp");
        let entry = CachedModList { fetched_at: Local::now().fixed_offset(), mods: mods.to_vec() };
        fs::write(&temp_path, serde_json::to_vec(&entry)?)?;
        fs::rename(&temp_path, &path)
    }

    /// Eine fehlende oder beschädigte Datei ist kein Fehler, die Liste wird dann neu geholt.
    pub fn load(&self, profile_id: &str) -> Option<CachedModList> {
        let bytes = fs::read(self.path_for(profile_id)).ok()?;
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        serde_json::from_slice(bytes).ok()
    }

    pub fn delete(&self, profile_id: &str) -> io::Result<()> {
        match fs::remove_file(self.path_for(profile_id)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    fn path_for(&self, profile_id: &str) -> PathBuf {
        Path::new(&self.folder).join(format!("{profile_id}.json"))
    }
}
