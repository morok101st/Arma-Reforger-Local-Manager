use serde::Serialize;

use super::{ModComparison, ModState};

/// Stand eines Abgleichs in Zahlen und als Text für Übersicht und Server-Liste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSummary {
    pub total: usize,
    pub current: usize,

    /// Kurz für die Server-Liste, z. B. „3 von 10 offen“.
    pub list_status: String,

    /// Ausführlich für die Übersicht, z. B. „7 von 10 Mods aktuell · 2 fehlen · 1 veraltet“.
    pub text: String,
}

impl ModSummary {
    pub fn open(&self) -> usize {
        self.total - self.current
    }

    pub fn from_comparison(comparison: &[ModComparison]) -> Self {
        let total = comparison.len();
        let count = |state: ModState| comparison.iter().filter(|mod_state| mod_state.state == state).count();
        let current = count(ModState::Current);
        let open = total - current;
        if open == 0 {
            return Self { total, current, list_status: "aktuell".into(), text: format!("Alles aktuell: {total} Mods") };
        }

        let missing = count(ModState::Missing);
        let parts = [
            format!("{current} von {total} Mods aktuell"),
            part(missing, if missing == 1 { "fehlt" } else { "fehlen" }),
            part(count(ModState::Outdated), "veraltet"),
            part(count(ModState::Newer), "neuer als Server"),
            part(count(ModState::Different), "andere Version"),
            part(count(ModState::Damaged), "beschädigt"),
        ];
        Self {
            total,
            current,
            list_status: format!("{open} von {total} offen"),
            text: parts.into_iter().filter(|text| !text.is_empty()).collect::<Vec<_>>().join(" · "),
        }
    }
}

fn part(count: usize, label: &str) -> String {
    if count == 0 { String::new() } else { format!("{count} {label}") }
}

/// Offene Mods zuerst, dann nach Name ohne Rücksicht auf Groß- und Kleinschreibung.
pub fn sort_open_first(comparison: &mut [ModComparison]) {
    comparison.sort_by(|left, right| {
        (left.state == ModState::Current)
            .cmp(&(right.state == ModState::Current))
            .then_with(|| left.required.name.to_lowercase().cmp(&right.required.name.to_lowercase()))
            .then_with(|| left.required.name.cmp(&right.required.name))
    });
}

pub fn state_text(state: ModState) -> &'static str {
    match state {
        ModState::Current => "aktuell",
        ModState::Missing => "fehlt",
        ModState::Outdated => "veraltet",
        ModState::Newer => "neuer als Server",
        ModState::Different => "andere Version",
        ModState::Damaged => "beschädigt",
    }
}
