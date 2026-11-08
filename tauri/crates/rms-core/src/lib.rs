//! Logik von Reforger Sync & Launch. Oberfläche und Kommandozeile rufen nur diese Bibliothek auf.

pub mod addons;
pub mod client;
pub mod credentials;
pub mod paths;
pub mod settings;
pub mod steam;
pub mod sync;
mod win;

/// Version, wie sie in der App angezeigt wird. Das Build-Skript setzt sie über `RMS_VERSION`, etwa `0.1.0-test`.
pub const VERSION: &str = match option_env!("RMS_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// Projektseite und Releases für die Anzeige in der App.
pub const REPOSITORY_URL: &str = "https://github.com/morok101st/Arma-Reforger-Local-Manager";
pub const RELEASES_URL: &str = "https://github.com/morok101st/Arma-Reforger-Local-Manager/releases";
