//! Daten, die zwischen Backend und Oberfläche wandern. Passwörter stehen nie darin.

use std::collections::BTreeMap;

use rms_core::addons::{ModComparison, ModState, ModSummary, sort_open_first, state_text};
use rms_core::settings::{BasicSettingsField, BasicSettingsInput, ServerProfile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRow {
    pub name: String,
    pub required: String,
    pub local: String,
    pub state: &'static str,
    pub is_open: bool,
    pub is_damaged: bool,
}

/// Stand eines Servers für die Übersicht.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub rows: Vec<ModRow>,
    pub summary: ModSummary,
    pub open: usize,
    /// z. B. „Liste vom 08.11.2026 17:05“.
    pub stand: String,
}

impl Overview {
    pub fn from_comparison(comparison: &[ModComparison], stand: String) -> Self {
        let mut sorted = comparison.to_vec();
        sort_open_first(&mut sorted);
        let rows = sorted
            .iter()
            .map(|mod_state| ModRow {
                name: mod_state.required.name.clone(),
                required: mod_state.required.version.clone().unwrap_or_else(|| "beliebig".into()),
                local: mod_state.installed.as_ref().and_then(|addon| addon.version.clone()).unwrap_or_else(|| "–".into()),
                state: state_text(mod_state.state),
                is_open: mod_state.state != ModState::Current,
                is_damaged: mod_state.state == ModState::Damaged,
            })
            .collect();
        let summary = ModSummary::from_comparison(comparison);
        Self { rows, open: summary.open(), summary, stand }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub server_address: Option<String>,
    pub download_dir: Option<String>,
    pub has_password: bool,
    pub overview: Option<Overview>,
}

impl ProfileView {
    pub fn new(profile: &ServerProfile, has_password: bool, overview: Option<Overview>) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            server_address: profile.server_address.clone(),
            download_dir: profile.download_dir.clone(),
            has_password,
            overview,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub version: String,
    pub profiles: Vec<ProfileView>,
    pub selected_profile_id: Option<String>,
    pub load_problem: Option<String>,
}

/// Eingaben eines Servers aus den Einstellungen.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: String,
    pub download_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileResult {
    /// `false`: Das eingegebene Passwort bleibt im Feld stehen.
    pub stored: bool,
    pub message: String,
    pub is_error: bool,
    pub profile: Option<ProfileView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub message: String,
    pub is_error: bool,
}

impl Message {
    pub fn info(message: impl Into<String>) -> Self {
        Self { message: message.into(), is_error: false }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self { message: message.into(), is_error: true }
    }
}

/// Fortschritt eines Laufs, als Ereignis `run-status`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatus {
    pub profile_id: String,
    pub message: String,
    pub is_client_line: bool,
    pub done: usize,
    pub total: usize,
    pub overview: Option<Overview>,
}

/// Rückfrage vor dem ersten Löschen überzähliger Mods in einem Ordner.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupQuestion {
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub ok: bool,
    pub cancelled: bool,
    pub message: String,
    pub is_error: bool,
    pub list_status: String,
    pub overview: Option<Overview>,
    /// Gesetzt, wenn vor dem Bereinigen gefragt werden muss. Antwort über `answer_cleanup`.
    pub cleanup_question: Option<CleanupQuestion>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicSettingsView {
    pub input: BasicSettingsInput,
    pub defaults: BasicSettingsInput,
    pub detected_exe_hint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicSettingsSaveResult {
    pub ok: bool,
    pub errors: BTreeMap<BasicSettingsField, String>,
    pub message: String,
}
