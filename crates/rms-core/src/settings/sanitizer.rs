//! Macht eine geladene Einstellungsdatei sicher benutzbar. Die Datei kann von Hand geändert oder beschädigt
//! sein: Werte außerhalb der Grenzen fallen auf den Standard zurück. Ein ungültiger Spielclient bleibt stehen,
//! damit die Vorprüfung ihn meldet, statt still einen anderen zu nehmen.

use super::basic::{MAX_IDLE_MINUTES, MAX_TOTAL_MINUTES, MIN_IDLE_MINUTES, MIN_TOTAL_MINUTES};
use super::{AppSettings, ClientWindowSettings, TimeoutSettings};

pub const MIN_WIDTH: i32 = 320;
pub const MAX_WIDTH: i32 = 3840;
pub const MIN_HEIGHT: i32 = 180;
pub const MAX_HEIGHT: i32 = 2160;
pub const MIN_FPS: i32 = 5;
pub const MAX_FPS: i32 = 60;

pub fn sanitize(mut settings: AppSettings) -> AppSettings {
    let window_defaults = ClientWindowSettings::default();
    let timeout_defaults = TimeoutSettings::default();

    let mut total = within(settings.timeouts.total_minutes, MIN_TOTAL_MINUTES, MAX_TOTAL_MINUTES, timeout_defaults.total_minutes);
    let mut idle = within(settings.timeouts.idle_minutes, MIN_IDLE_MINUTES, MAX_IDLE_MINUTES, timeout_defaults.idle_minutes);
    if idle >= total {
        total = timeout_defaults.total_minutes;
        idle = timeout_defaults.idle_minutes;
    }
    settings.timeouts.total_minutes = total;
    settings.timeouts.idle_minutes = idle;

    let client = &mut settings.client;
    client.width = within(client.width, MIN_WIDTH, MAX_WIDTH, window_defaults.width);
    client.height = within(client.height, MIN_HEIGHT, MAX_HEIGHT, window_defaults.height);
    client.max_fps = within(client.max_fps, MIN_FPS, MAX_FPS, window_defaults.max_fps);

    // Ohne ID hinge das Passwort an keinem eindeutigen Eintrag.
    settings.profiles.retain(|profile| !profile.id.trim().is_empty());
    settings.cleanup_approved_folders.retain(|folder| !folder.trim().is_empty());
    settings.reforger_exe = settings.reforger_exe.as_deref().map(str::trim).filter(|exe| !exe.is_empty()).map(String::from);

    settings
}

fn within(value: i32, min: i32, max: i32, fallback: i32) -> i32 {
    if (min..=max).contains(&value) { value } else { fallback }
}
