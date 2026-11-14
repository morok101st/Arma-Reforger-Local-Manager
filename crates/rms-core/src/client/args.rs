//! Startparameter für den Spielclient und Prüfung der Server-Adresse. Das Passwort steht nie darin.

use std::net::{IpAddr, ToSocketAddrs};
use std::sync::LazyLock;

use regex::Regex;

use crate::addons::{ModEntry, client_default_folder, normalize_download_dir};
use crate::settings::ClientWindowSettings;

pub const HELPER_GUID: &str = "7A3C1E5D9B2F4860";

/// Startparameter, mit denen die Hilfs-Mod genau diese Mods in der Version des Servers lädt. Sie braucht
/// dafür den Server-Eintrag, die Workshop-Suche findet manche Mods nicht.
pub fn build_download(
    server_address: &str,
    mods: &[ModEntry],
    download_dir: Option<&str>,
    window: &ClientWindowSettings,
    logs_dir: &str,
    profile_dir: &str,
    helper_root: &str,
) -> Vec<String> {
    let mut args = build_common(download_dir, window, logs_dir, profile_dir, helper_root);
    args.extend(["-rmsServer".into(), server_address.trim().into(), "-rmsMods".into(), format_mods(mods)]);
    args
}

/// Startparameter, mit denen die Hilfs-Mod die Mod-Liste eines Servers abruft.
pub fn build_list_request(
    server_address: &str,
    download_dir: Option<&str>,
    window: &ClientWindowSettings,
    logs_dir: &str,
    profile_dir: &str,
    helper_root: &str,
) -> Vec<String> {
    let mut args = build_common(download_dir, window, logs_dir, profile_dir, helper_root);
    args.extend(["-rmsServer".into(), server_address.trim().into()]);
    args
}

/// Startparameter für den direkten Beitritt: mit Hilfs-Mod und echtem Profil des Spielers. Beim Verbinden
/// lädt das Spiel die Mods des Servers neu. `logs_dir` ist der Log-Ordner der App, dort liegt auch die
/// Passwortdatei, nicht in „Dokumente“, das oft in OneDrive liegt.
pub fn build_join(server_address: &str, download_dir: Option<&str>, helper_root: &str, logs_dir: &str) -> Vec<String> {
    let folder = normalize_download_dir(download_dir);
    let mut args =
        vec!["-addonsDir".to_string(), folder.as_ref().map_or_else(|| helper_root.to_string(), |folder| format!("{folder},{helper_root}"))];
    if let Some(folder) = folder {
        args.extend(["-addonDownloadDir".to_string(), folder]);
    }
    args.extend([
        "-addons".into(),
        HELPER_GUID.into(),
        "-logsDir".into(),
        logs_dir.into(),
        "-rmsJoin".into(),
        server_address.trim().into(),
    ]);
    args
}

/// Format der Hilfs-Mod: `id:version;id:version`, ohne Version die neueste.
pub fn format_mods(mods: &[ModEntry]) -> String {
    mods.iter()
        .map(|mod_entry| match &mod_entry.version {
            Some(version) => format!("{}:{version}", mod_entry.id),
            None => mod_entry.id.clone(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Text für die Steam-Startoptionen, damit das Spiel denselben Ordner nutzt.
pub fn steam_launch_options(download_dir: Option<&str>) -> String {
    normalize_download_dir(download_dir)
        .map(|folder| to_command_line(&["-addonsDir", &folder, "-addonDownloadDir", &folder]))
        .unwrap_or_default()
}

/// Baut eine Befehlszeile nach den Regeln, mit denen Windows-Programme sie wieder zerlegen
/// (`CommandLineToArgvW`): Werte mit Leerzeichen, Tabulator oder Anführungszeichen kommen in
/// Anführungszeichen, Backslashes vor einem Anführungszeichen werden verdoppelt.
pub fn to_command_line<S: AsRef<str>>(arguments: &[S]) -> String {
    arguments.iter().map(|argument| quote_argument(argument.as_ref())).collect::<Vec<_>>().join(" ")
}

fn quote_argument(argument: &str) -> String {
    if !argument.is_empty() && !argument.contains([' ', '\t', '"']) {
        return argument.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }

        // Backslashes vor einem Anführungszeichen verdoppeln, das Anführungszeichen selbst maskieren.
        let count = if character == '"' { backslashes * 2 + 1 } else { backslashes };
        quoted.extend(std::iter::repeat_n('\\', count));
        quoted.push(character);
        backslashes = 0;
    }

    // Am Ende stehen Backslashes vor dem schließenden Anführungszeichen und werden deshalb verdoppelt.
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

static IP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^((25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9])\.){3}(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9])$").unwrap()
});

static HOST_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$").unwrap());

/// Adresse (IPv4 oder Name) und Port. Ein Name wird erst beim Start aufgelöst, siehe [`resolve_server_address`].
pub fn is_valid_server_address(address: Option<&str>) -> bool {
    let (host, port) = split_server_address(address);
    is_valid_host(Some(&host)) && is_valid_port(Some(&port))
}

pub fn is_valid_ip(ip: Option<&str>) -> bool {
    ip.is_some_and(|ip| IP.is_match(ip.trim()))
}

/// IPv4-Adresse oder Name wie `arma.example.de`. Besteht die Eingabe nur aus Ziffern und Punkten,
/// muss sie eine gültige IPv4-Adresse sein, damit Tippfehler wie `203.0.113` auffallen.
pub fn is_valid_host(host: Option<&str>) -> bool {
    let Some(host) = host.map(|host| host.trim().to_ascii_lowercase()) else {
        return false;
    };
    if is_valid_ip(Some(&host)) {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    host.len() <= 253
        && labels.iter().all(|label| HOST_LABEL.is_match(label))
        && labels.last().is_some_and(|last| last.chars().any(|character| character.is_ascii_alphabetic()))
}

/// Macht aus einer eingefügten Adresse wie `https://arma.example.de:2001/` den Teil `arma.example.de:2001`.
pub fn normalize_address_input(input: &str) -> String {
    let text = input.trim();
    let text = text.split_once("://").map_or(text, |(_, rest)| rest);
    let text = text.split(['/', '?', '#']).next().unwrap_or_default();
    text.trim_end_matches('.').to_ascii_lowercase()
}

/// Liefert die Adresse für den Spielclient als `IPv4:Port`. Ein Name wird per DNS aufgelöst, denn
/// das Backend kennt den Server nur unter seiner IP. Die erste IPv4-Adresse gilt.
pub fn resolve_server_address(address: &str) -> Result<String, String> {
    let (host, port) = split_server_address(Some(address));
    if !is_valid_host(Some(&host)) || !is_valid_port(Some(&port)) {
        return Err("Es ist keine gültige Server-Adresse hinterlegt (Adresse und Port).".into());
    }
    if is_valid_ip(Some(&host)) {
        return Ok(format!("{host}:{port}"));
    }

    let port_number: u16 = port.parse().map_err(|_| "Ungültiger Port.".to_string())?;
    let found = (host.as_str(), port_number)
        .to_socket_addrs()
        .map_err(|_| format!("Der Name „{host}“ wurde nicht gefunden. Stimmt die Adresse, und besteht eine Internetverbindung?"))?;
    found
        .map(|socket| socket.ip())
        .find_map(|ip| match ip {
            IpAddr::V4(ip) => Some(format!("{ip}:{port_number}")),
            IpAddr::V6(_) => None,
        })
        .ok_or_else(|| format!("Der Name „{host}“ hat keine IPv4-Adresse. Bitte die IP-Adresse des Servers eintragen."))
}

pub fn is_valid_port(port: Option<&str>) -> bool {
    port.and_then(|port| port.trim().parse::<i32>().ok()).is_some_and(|number| (1..=65535).contains(&number))
}

/// Zerlegt `Adresse:Port` für die Eingabefelder. Ohne Doppelpunkt ist der Port leer.
pub fn split_server_address(address: Option<&str>) -> (String, String) {
    let text = normalize_address_input(address.unwrap_or_default());
    let text = text.as_str();
    match text.rsplit_once(':') {
        Some((ip, port)) => (ip.trim().to_string(), port.trim().to_string()),
        None => (text.to_string(), String::new()),
    }
}

/// Gespeichert wird `Adresse:Port` wie eingegeben, ein Name in Kleinbuchstaben. Daran hängt auch das
/// Passwort. `None`: beide Felder leer.
pub fn combine_server_address(ip: Option<&str>, port: Option<&str>) -> Option<String> {
    let ip = normalize_address_input(ip.unwrap_or_default());
    let port = port.unwrap_or_default().trim();
    if ip.is_empty() && port.is_empty() {
        return None;
    }

    let port = port.parse::<i32>().map_or_else(|_| port.to_string(), |number| number.to_string());
    Some(format!("{ip}:{port}"))
}

/// Lässt im Portfeld nur Ziffern zu, auch beim Einfügen, und höchstens die fünf eines gültigen Ports.
pub fn clean_port_input(input: Option<&str>) -> String {
    input.unwrap_or_default().chars().filter(char::is_ascii_digit).take(5).collect()
}

/// Ein eigenes Profil lässt die Einstellungen des Spielers unberührt, ändert aber auch den
/// Standard-Mod-Ordner des Clients. Der Download-Ordner wird deshalb immer ausdrücklich gesetzt.
fn build_common(
    download_dir: Option<&str>,
    window: &ClientWindowSettings,
    logs_dir: &str,
    profile_dir: &str,
    helper_root: &str,
) -> Vec<String> {
    let folder = normalize_download_dir(download_dir).unwrap_or_else(|| client_default_folder().to_string_lossy().into_owned());

    vec![
        "-window".into(),
        "-screenWidth".into(),
        window.width.to_string(),
        "-screenHeight".into(),
        window.height.to_string(),
        "-noSound".into(),
        "-noSplash".into(),
        "-noFocus".into(),
        "-maxFPS".into(),
        window.max_fps.to_string(),
        "-addonsDir".into(),
        format!("{folder},{helper_root}"),
        "-addonDownloadDir".into(),
        folder,
        "-addons".into(),
        HELPER_GUID.into(),
        "-profile".into(),
        profile_dir.into(),
        "-logsDir".into(),
        logs_dir.into(),
    ]
}
