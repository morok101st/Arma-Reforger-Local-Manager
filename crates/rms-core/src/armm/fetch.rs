//! Holt die Mod-Liste eines Modsets per HTTPS über WinHTTP. Zertifikate und Proxy kommen von Windows.
//! Die App stellt nur diese eine Art Anfrage, und nur an die `baseUrl` einer importierten Verbindung.

use std::ffi::c_void;
use std::sync::Arc;

use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_FLAG_SECURE,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_STATUS_CODE, WinHttpAddRequestHeaders, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption, WinHttpSetTimeouts,
};
use windows::core::{PCWSTR, w};
use zeroize::Zeroize;

use super::ArmmLink;
use super::modset::MAX_MODSET_BYTES;
use crate::win::{pcwstr, wide};

const TIMEOUT_MS: i32 = 15_000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    #[error("ARMM hat den Zugangsschlüssel abgelehnt. Er ist ungültig oder gesperrt, bitte eine neue Importdatei aus ARMM holen.")]
    Unauthorized,
    #[error("Das Modset wurde in ARMM nicht gefunden. Ist es noch freigegeben?")]
    NotFound,
    #[error("ARMM hat mit einem Fehler geantwortet (HTTP {0}). Bitte später erneut versuchen.")]
    Status(u32),
    #[error("ARMM ist nicht erreichbar ({0}). Besteht eine Internetverbindung?")]
    Network(String),
    #[error("Die Antwort von ARMM ist zu groß.")]
    TooLarge,
    #[error("Für diesen Server ist kein ARMM-Zugangsschlüssel gespeichert. Bitte die Importdatei erneut importieren.")]
    NoAccessKey,
}

/// Quelle der Mod-Liste eines importierten Servers. Tests setzen eine Attrappe ein.
pub trait ModsetSource: Send + Sync {
    /// Rohdaten der Antwort bei HTTP 200.
    fn fetch(&self, link: &ArmmLink, access_key: &str) -> Result<Vec<u8>, FetchError>;
}

pub type SharedModsetSource = Arc<dyn ModsetSource>;

/// Ordnet einen HTTP-Status einer Meldung zu. `None`: Erfolg.
pub fn status_error(status: u32) -> Option<FetchError> {
    match status {
        200 => None,
        401 | 403 => Some(FetchError::Unauthorized),
        404 => Some(FetchError::NotFound),
        other => Some(FetchError::Status(other)),
    }
}

/// Host und Port aus `https://host[:port]`.
pub fn host_and_port(link: &ArmmLink) -> (String, u16) {
    let authority = link.host();
    match authority.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(INTERNET_DEFAULT_HTTPS_PORT)),
        None => (authority.to_string(), INTERNET_DEFAULT_HTTPS_PORT),
    }
}

/// Pfad der Anfrage, ohne Host. Kommt aus der Importdatei.
pub fn request_path(link: &ArmmLink) -> String {
    link.export_path.clone()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WinHttpModsetSource;

/// Schließt ein WinHTTP-Handle beim Verlassen des Bereichs.
struct Handle(*mut c_void);

impl Handle {
    fn open(raw: *mut c_void, what: &str) -> Result<Self, FetchError> {
        if raw.is_null() {
            Err(FetchError::Network(format!("{what}: {}", windows::core::Error::from_thread().message())))
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: Das Handle stammt von WinHTTP und wird genau einmal geschlossen.
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

fn network(error: &windows::core::Error) -> FetchError {
    FetchError::Network(error.message().trim().to_string())
}

impl ModsetSource for WinHttpModsetSource {
    fn fetch(&self, link: &ArmmLink, access_key: &str) -> Result<Vec<u8>, FetchError> {
        let (host, port) = host_and_port(link);
        let host = wide(&host);
        let path = wide(request_path(link));
        let agent = wide(format!("ReforgerSyncLaunch/{}", crate::VERSION));

        // SAFETY: Alle Texte sind nullterminiert und leben bis nach den Aufrufen. Die Handles schließt `Handle`.
        unsafe {
            let session = Handle::open(
                WinHttpOpen(pcwstr(&agent), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0),
                "Sitzung",
            )?;
            WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS).map_err(|error| network(&error))?;
            let connection = Handle::open(WinHttpConnect(session.0, pcwstr(&host), port, 0), "Verbindung")?;
            let request = Handle::open(
                WinHttpOpenRequest(
                    connection.0,
                    w!("GET"),
                    pcwstr(&path),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    std::ptr::null(),
                    WINHTTP_FLAG_SECURE,
                ),
                "Anfrage",
            )?;

            // Nie von https auf http umleiten lassen, der Schlüssel ginge sonst unverschlüsselt hinaus.
            let policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP.to_le_bytes();
            WinHttpSetOption(Some(request.0), WINHTTP_OPTION_REDIRECT_POLICY, Some(&policy)).map_err(|error| network(&error))?;

            let mut header: Vec<u16> = format!("Authorization: Bearer {access_key}\r\nAccept: application/json").encode_utf16().collect();
            let added = WinHttpAddRequestHeaders(request.0, &header, WINHTTP_ADDREQ_FLAG_ADD);
            header.zeroize();
            added.map_err(|error| network(&error))?;

            WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|error| network(&error))?;
            WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|error| network(&error))?;

            let mut status: u32 = 0;
            let mut size = u32::try_from(size_of::<u32>()).unwrap_or(4);
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&raw mut status).cast()),
                &raw mut size,
                std::ptr::null_mut(),
            )
            .map_err(|error| network(&error))?;
            if let Some(error) = status_error(status) {
                return Err(error);
            }

            let mut body = Vec::new();
            let mut chunk = vec![0u8; 16 * 1024];
            loop {
                let mut read: u32 = 0;
                WinHttpReadData(request.0, chunk.as_mut_ptr().cast(), u32::try_from(chunk.len()).unwrap_or(u32::MAX), &raw mut read)
                    .map_err(|error| network(&error))?;
                if read == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..read as usize]);
                if body.len() > MAX_MODSET_BYTES {
                    return Err(FetchError::TooLarge);
                }
            }
            Ok(body)
        }
    }
}
