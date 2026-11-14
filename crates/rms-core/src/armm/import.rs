//! Liest und prüft eine `armm-sync.json`. Der Inhalt landet nie im Log, er enthält den Zugangsschlüssel.

use serde::Deserialize;
use serde_json::Value;
use zeroize::Zeroizing;

use super::{ArmmLink, SYNC_FORMAT, SYNC_FORMAT_VERSION};
use crate::client::args::{is_valid_host, is_valid_port};

/// Eine Importdatei ist klein. Alles darüber ist keine ARMM-Datei.
pub const MAX_IMPORT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("Die Datei ist zu groß für eine ARMM-Importdatei.")]
    TooLarge,
    #[error("Die Datei ist keine gültige JSON-Datei.")]
    NotJson,
    #[error("Das ist keine ARMM-Importdatei (armm-sync).")]
    WrongFormat,
    #[error("Die Importdatei stammt aus einer neueren ARMM-Version. Bitte die App aktualisieren.")]
    NewerVersion,
    #[error("In der Importdatei fehlt oder ist ungültig: {0}")]
    Invalid(&'static str),
    #[error("Die Importdatei enthält ein Server-Passwort. Passwörter gehören nicht in die Datei, du gibst es in der App ein.")]
    ContainsPassword,
}

/// Inhalt einer geprüften Importdatei.
pub struct ArmmImport {
    pub name: String,
    pub link: ArmmLink,
    /// Wird beim Verwerfen aus dem Speicher gelöscht.
    pub access_key: Zeroizing<String>,
    pub game_server: Option<ImportedGameServer>,
}

impl std::fmt::Debug for ArmmImport {
    // Der Schlüssel erscheint nie, auch nicht in Fehlermeldungen von Tests.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArmmImport")
            .field("name", &self.name)
            .field("link", &self.link)
            .field("access_key", &"***")
            .field("game_server", &self.game_server)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedGameServer {
    /// Name oder IPv4, klein geschrieben.
    pub host: String,
    pub port: u16,
    pub password_required: bool,
}

impl ImportedGameServer {
    /// Wie die App die Adresse speichert.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFile {
    format: Option<String>,
    format_version: Option<Value>,
    name: Option<String>,
    armm: Option<RawArmm>,
    game_server: Option<RawGameServer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawArmm {
    base_url: Option<String>,
    modset_id: Option<Value>,
    access_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGameServer {
    host: Option<String>,
    port: Option<Value>,
    #[serde(default)]
    password: Value,
    #[serde(default)]
    password_required: bool,
}

/// Prüft eine `armm-sync.json`. Jeder Fehler verwirft die ganze Datei.
pub fn parse_sync_file(bytes: &[u8]) -> Result<ArmmImport, ImportError> {
    if bytes.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut raw: RawFile = serde_json::from_slice(bytes).map_err(|_| ImportError::NotJson)?;

    if raw.format.as_deref() != Some(SYNC_FORMAT) {
        return Err(ImportError::WrongFormat);
    }
    match raw.format_version.as_ref().and_then(Value::as_u64) {
        Some(version) if version > SYNC_FORMAT_VERSION => return Err(ImportError::NewerVersion),
        Some(version) if version >= 1 => {}
        _ => return Err(ImportError::Invalid("formatVersion")),
    }

    let name = raw.name.as_deref().map(str::trim).filter(|name| !name.is_empty()).ok_or(ImportError::Invalid("name"))?.to_string();

    let armm = raw.armm.as_mut().ok_or(ImportError::Invalid("armm"))?;
    let base_url =
        normalize_base_url(armm.base_url.as_deref().unwrap_or_default()).ok_or(ImportError::Invalid("armm.baseUrl (https://…)"))?;
    let modset_id = armm.modset_id.as_ref().and_then(Value::as_u64).filter(|id| *id > 0).ok_or(ImportError::Invalid("armm.modsetId"))?;
    let access_key = Zeroizing::new(armm.access_key.take().unwrap_or_default());
    if access_key.trim().is_empty() || access_key.chars().any(char::is_control) {
        return Err(ImportError::Invalid("armm.accessKey"));
    }

    let game_server = raw.game_server.map(parse_game_server).transpose()?;

    Ok(ArmmImport { name, link: ArmmLink { base_url, modset_id }, access_key, game_server })
}

fn parse_game_server(raw: RawGameServer) -> Result<ImportedGameServer, ImportError> {
    // `null` heißt „kein Passwort hinterlegt“ und ist erlaubt. Ein Wert gehört nie in die Datei.
    match &raw.password {
        Value::Null => {}
        Value::String(text) if text.is_empty() => {}
        _ => return Err(ImportError::ContainsPassword),
    }

    let host = raw.host.as_deref().map(|host| host.trim().to_ascii_lowercase()).unwrap_or_default();
    if !is_valid_host(Some(&host)) {
        return Err(ImportError::Invalid("gameServer.host"));
    }
    let port = raw
        .port
        .as_ref()
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| is_valid_port(Some(&port.to_string())))
        .ok_or(ImportError::Invalid("gameServer.port"))?;

    Ok(ImportedGameServer { host, port, password_required: raw.password_required })
}

/// `https://host` oder `https://host:port`, Schema und Host klein, ohne `/` am Ende. Pfade, Parameter
/// und Zugangsdaten in der Adresse sind nicht erlaubt.
pub fn normalize_base_url(input: &str) -> Option<String> {
    let text = input.trim();
    let scheme = text.get(..8)?;
    if !scheme.eq_ignore_ascii_case("https://") {
        return None;
    }
    let authority = text[8..].trim_end_matches('/');
    if authority.is_empty() || authority.contains(['/', '?', '#', '@', ' ']) {
        return None;
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    if !is_valid_host(Some(host)) || port.is_some_and(|port| !is_valid_port(Some(port))) {
        return None;
    }

    let host = host.to_ascii_lowercase();
    Some(match port {
        Some(port) => format!("https://{host}:{}", port.parse::<u16>().ok()?),
        None => format!("https://{host}"),
    })
}
