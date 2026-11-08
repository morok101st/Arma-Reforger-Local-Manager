//! Logik von Reforger Sync & Launch. Oberfläche und Kommandozeile rufen nur diese Bibliothek auf.

/// Version des Pakets, wie sie in der App angezeigt wird.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Projektseite und Releases für die Anzeige in der App.
pub const REPOSITORY_URL: &str = "https://github.com/morok101st/Arma-Reforger-Local-Manager";
