//! Prüft die Mod-Liste, die ARMM für ein Modset liefert. Sie bestimmt auch, was der exakte Abgleich
//! löscht. Deshalb verwirft jeder Fehler die ganze Liste.

use std::collections::HashSet;

use serde_json::Value;

use crate::addons::ModEntry;

/// Mehr als genug für Hunderte Mods. Alles darüber ist keine Mod-Liste.
pub const MAX_MODSET_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModsetError {
    #[error("Die Antwort von ARMM ist zu groß.")]
    TooLarge,
    #[error("Die Antwort von ARMM ist keine gültige Mod-Liste.")]
    NotAList,
    #[error("Das Modset in ARMM enthält keine Mods.")]
    Empty,
    #[error("Die Mod-Liste aus ARMM ist fehlerhaft (Eintrag {position}: {problem}). Es wurde nichts geändert.")]
    InvalidEntry { position: usize, problem: &'static str },
    #[error("Die Mod-Liste aus ARMM enthält die Mod {0} doppelt. Es wurde nichts geändert.")]
    Duplicate(String),
}

/// Liest die Liste `[{ "modId", "name", "version" }]` oder eine Serverkonfiguration mit `game.mods`.
pub fn parse_modset(bytes: &[u8]) -> Result<Vec<ModEntry>, ModsetError> {
    if bytes.len() > MAX_MODSET_BYTES {
        return Err(ModsetError::TooLarge);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ModsetError::NotAList)?;

    let entries = match &value {
        Value::Array(entries) => entries,
        Value::Object(_) => value.pointer("/game/mods").and_then(Value::as_array).ok_or(ModsetError::NotAList)?,
        _ => return Err(ModsetError::NotAList),
    };
    if entries.is_empty() {
        return Err(ModsetError::Empty);
    }

    let mut seen = HashSet::new();
    let mut mods = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let position = index + 1;
        let invalid = |problem| ModsetError::InvalidEntry { position, problem };
        let text = |key: &str| entry.get(key).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty());

        let id = text("modId").ok_or(invalid("modId fehlt"))?.to_ascii_uppercase();
        if id.len() != 16 || !id.chars().all(|character| character.is_ascii_hexdigit()) {
            return Err(invalid("modId ist keine Workshop-ID"));
        }
        let name = text("name").ok_or(invalid("name fehlt"))?;
        let version = text("version").ok_or(invalid("version fehlt"))?;
        if name.contains(['|', '\n', '\r']) || version.contains(['|', '\n', '\r', ':', ';']) {
            return Err(invalid("unerlaubtes Zeichen"));
        }
        if !seen.insert(id.clone()) {
            return Err(ModsetError::Duplicate(id));
        }

        mods.push(ModEntry::new(&id, name, Some(version)));
    }

    Ok(mods)
}
