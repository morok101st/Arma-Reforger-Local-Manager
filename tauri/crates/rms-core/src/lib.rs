//! Logik von Reforger Sync & Launch. Oberfläche und Kommandozeile rufen nur diese Bibliothek auf.

pub mod addons;
pub mod client;
pub mod credentials;
pub mod paths;
pub mod settings;
pub mod steam;
pub mod sync;
mod win;

/// Version des Pakets, wie sie in der App angezeigt wird.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Projektseite und Releases für die Anzeige in der App.
pub const REPOSITORY_URL: &str = "https://github.com/morok101st/Arma-Reforger-Local-Manager";
pub const RELEASES_URL: &str = "https://github.com/morok101st/Arma-Reforger-Local-Manager/releases";
