use crate::client::args::{is_valid_host, is_valid_port, normalize_address_input};

/// Prüft die Eingaben eines Servers in den Einstellungen, bevor er gespeichert wird.
/// `None`, wenn gespeichert werden darf. Sonst die Meldung für den Nutzer.
pub fn validate_profile_input(name: Option<&str>, ip: Option<&str>, port: Option<&str>) -> Option<&'static str> {
    if name.is_none_or(|name| name.trim().is_empty()) {
        return Some("Bitte einen Namen für diesen Server eintragen.");
    }

    if !is_valid_host(ip.map(normalize_address_input).as_deref()) {
        return Some("Bitte die Adresse des Servers eintragen, zum Beispiel arma.example.de oder 203.0.113.10.");
    }

    if !is_valid_port(port) {
        return Some("Bitte den Port des Servers eintragen, eine Zahl von 1 bis 65535, zum Beispiel 2001.");
    }

    None
}
