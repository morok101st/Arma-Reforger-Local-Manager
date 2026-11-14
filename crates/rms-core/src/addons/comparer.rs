use std::cmp::Ordering;
use std::collections::HashMap;

use serde::Serialize;

use super::{AddonHealth, InstalledAddon, ModEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ModState {
    Current,
    Missing,

    /// Lokal älter als vom Server verlangt.
    Outdated,

    /// Lokal neuer als vom Server verlangt, z. B. weil der Server noch nicht aktualisiert wurde.
    Newer,

    /// Lokal eine andere Version, die sich nicht einordnen lässt.
    Different,

    Damaged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModComparison {
    pub required: ModEntry,
    pub state: ModState,
    pub installed: Option<InstalledAddon>,
}

pub fn compare(required: &[ModEntry], installed: &[InstalledAddon]) -> Vec<ModComparison> {
    // Zwei Ordner mit derselben ID sind möglich (umbenannte Mod), daher die gesunde Variante bevorzugen.
    let mut by_id: HashMap<String, &InstalledAddon> = HashMap::new();
    for addon in installed {
        by_id
            .entry(addon.id.to_uppercase())
            .and_modify(|chosen| {
                if chosen.health != AddonHealth::Ok && addon.health == AddonHealth::Ok {
                    *chosen = addon;
                }
            })
            .or_insert(addon);
    }

    required
        .iter()
        .map(|mod_entry| {
            let addon = by_id.get(&mod_entry.id.to_uppercase()).copied();
            ModComparison { required: mod_entry.clone(), state: state_of(mod_entry, addon), installed: addon.cloned() }
        })
        .collect()
}

fn state_of(mod_entry: &ModEntry, addon: Option<&InstalledAddon>) -> ModState {
    let Some(addon) = addon else {
        return ModState::Missing;
    };

    if addon.health != AddonHealth::Ok {
        return ModState::Damaged;
    }

    let Some(required) = mod_entry.version.as_deref() else {
        return ModState::Current;
    };
    if addon.version.as_deref() == Some(required) {
        return ModState::Current;
    }

    match compare_versions(addon.version.as_deref(), Some(required)) {
        Some(Ordering::Less) => ModState::Outdated,
        Some(Ordering::Greater) => ModState::Newer,
        Some(Ordering::Equal) => ModState::Current,
        None => ModState::Different,
    }
}

/// Vergleicht Versionen wie `0.16.5236` Teil für Teil. `None`: nicht vergleichbar.
pub fn compare_versions(left: Option<&str>, right: Option<&str>) -> Option<Ordering> {
    let left = parse_version(left?)?;
    let right = parse_version(right?)?;
    let length = left.len().max(right.len());
    let part = |parts: &[i64], index: usize| parts.get(index).copied().unwrap_or(0);
    (0..length).map(|index| part(&left, index).cmp(&part(&right, index))).find(|ordering| ordering.is_ne()).or(Some(Ordering::Equal))
}

fn parse_version(version: &str) -> Option<Vec<i64>> {
    if version.trim().is_empty() {
        return None;
    }

    version.split('.').map(|part| part.trim().parse::<i64>().ok()).collect()
}
