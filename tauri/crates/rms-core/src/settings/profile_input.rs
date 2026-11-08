use crate::client::args::{is_valid_ip, is_valid_port};

/// Prüft die Eingaben eines Servers in den Einstellungen, bevor er gespeichert wird.
/// `None`, wenn gespeichert werden darf. Sonst die Meldung für den Nutzer.
pub fn validate_profile_input(name: Option<&str>, ip: Option<&str>, port: Option<&str>) -> Option<&'static str> {
    if name.is_none_or(|name| name.trim().is_empty()) {
        return Some("Bitte einen Namen für diesen Server eintragen.");
    }

    if !is_valid_ip(ip) {
        return Some("Bitte die IP-Adresse des Servers eintragen, zum Beispiel 203.0.113.10.");
    }

    if !is_valid_port(port) {
        return Some("Bitte den Port des Servers eintragen, eine Zahl von 1 bis 65535, zum Beispiel 2001.");
    }

    None
}
