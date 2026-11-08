//! Grenzen und Prüfung der Grundeinstellungen. Sie gelten für alle Server. Das Spielfenster beim Sync
//! ist bewusst nicht dabei, seine Werte sind schon klein und bleiben, wie sie sind.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::AppSettings;
use crate::paths::full_path;
use crate::steam::check_client_exe;

pub const MIN_TOTAL_MINUTES: i32 = 10;
pub const MAX_TOTAL_MINUTES: i32 = 600;
pub const MIN_IDLE_MINUTES: i32 = 2;
pub const MAX_IDLE_MINUTES: i32 = 60;

/// Eingaben aus den Grundeinstellungen, so wie sie in den Feldern stehen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicSettingsInput {
    pub reforger_exe: String,
    pub total_minutes: String,
    pub idle_minutes: String,
}

impl BasicSettingsInput {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            reforger_exe: settings.reforger_exe.clone().unwrap_or_default(),
            total_minutes: settings.timeouts.total_minutes.to_string(),
            idle_minutes: settings.timeouts.idle_minutes.to_string(),
        }
    }

    pub fn defaults() -> Self {
        Self::from_settings(&AppSettings::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BasicSettingsField {
    ReforgerExe,
    TotalMinutes,
    IdleMinutes,
}

/// Ergebnis der Prüfung: entweder neue Einstellungen oder je Feld eine Meldung.
#[derive(Debug, Clone, PartialEq)]
pub enum BasicSettingsResult {
    Valid(AppSettings),
    Invalid(BTreeMap<BasicSettingsField, String>),
}

impl BasicSettingsResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }

    pub fn settings(&self) -> Option<&AppSettings> {
        match self {
            Self::Valid(settings) => Some(settings),
            Self::Invalid(_) => None,
        }
    }

    pub fn errors(&self) -> BTreeMap<BasicSettingsField, String> {
        match self {
            Self::Valid(_) => BTreeMap::new(),
            Self::Invalid(errors) => errors.clone(),
        }
    }
}

/// Übernimmt gültige Eingaben in `current`. Alles andere bleibt unverändert.
pub fn apply_basic_settings(current: &AppSettings, input: &BasicSettingsInput) -> BasicSettingsResult {
    let mut errors = BTreeMap::new();

    let exe = input.reforger_exe.trim().trim_matches('"');
    if !exe.is_empty()
        && let Some(problem) = check_client_exe(Path::new(exe))
    {
        errors.insert(BasicSettingsField::ReforgerExe, problem);
    }

    let total = number(&input.total_minutes, MIN_TOTAL_MINUTES, MAX_TOTAL_MINUTES, BasicSettingsField::TotalMinutes, &mut errors);
    let idle = number(&input.idle_minutes, MIN_IDLE_MINUTES, MAX_IDLE_MINUTES, BasicSettingsField::IdleMinutes, &mut errors);

    if let (Some(total), Some(idle)) = (total, idle)
        && idle >= total
    {
        errors.insert(BasicSettingsField::IdleMinutes, "Muss kleiner als die Höchstdauer sein.".into());
    }

    match (total, idle) {
        (Some(total), Some(idle)) if errors.is_empty() => {
            let mut settings = current.clone();
            settings.reforger_exe = (!exe.is_empty()).then(|| full_path(Path::new(exe)).to_string_lossy().into_owned());
            settings.timeouts.total_minutes = total;
            settings.timeouts.idle_minutes = idle;
            BasicSettingsResult::Valid(settings)
        }
        _ => BasicSettingsResult::Invalid(errors),
    }
}

fn number(text: &str, min: i32, max: i32, field: BasicSettingsField, errors: &mut BTreeMap<BasicSettingsField, String>) -> Option<i32> {
    let trimmed = text.trim();
    let value = (!trimmed.is_empty() && trimmed.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| trimmed.parse::<i32>().ok())
        .flatten()
        .filter(|value| (min..=max).contains(value));
    if value.is_none() {
        errors.insert(field, format!("Bitte eine Zahl von {min} bis {max} eintragen."));
    }
    value
}
