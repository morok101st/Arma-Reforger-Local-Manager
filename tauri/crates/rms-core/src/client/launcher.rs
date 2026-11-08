//! Startet das Spiel zum Spielen, mit dem echten Profil des Spielers und über den BattlEye-Starter, wie es die
//! Installation des Spiels selbst vorsieht. Es läuft bewusst unabhängig von der App weiter.

use std::path::{Path, PathBuf};

use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::args::{build_join, to_command_line};
use super::{password_file, precheck, retention};
use crate::addons::helper_root;
use crate::paths::AppPaths;
use crate::settings::{AppSettings, ServerProfile};
use crate::win::{pcwstr, wide};

const BATTLEYE_STARTER: &str = "ArmaReforger_BE.exe";

/// Ergebnis eines Beitritts. `password_file`: die Datei, die der Aufrufer später sicherheitshalber löscht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinStart {
    pub password_file: Option<PathBuf>,
}

/// Startet das Spiel und lässt die Hilfs-Mod dem Server beitreten. Die Logs dieser Sitzung und die
/// Passwortdatei liegen in einem eigenen Lauf-Ordner der App, nicht in „Dokumente“.
pub fn start_and_join(
    settings: &AppSettings,
    profile: &ServerProfile,
    password: Option<&str>,
    paths: &AppPaths,
) -> Result<JoinStart, String> {
    // Erst alles prüfen, dann die Passwortdatei schreiben: Sie soll nur existieren, wenn das Spiel auch startet.
    let exe = precheck::check(settings, profile)?;
    let address = profile.server_address.as_deref().unwrap_or_default();

    let logs_dir = retention::create_run_folder(&paths.client_logs()).map_err(|error| error.to_string())?;
    let password_file = password_file::for_logs_folder(&logs_dir);
    password_file::prepare(&password_file, password).map_err(|error| error.to_string())?;

    // Die Hilfs-Mod liest zur Not auch noch den früheren Ort. Dort darf nie ein altes Passwort liegen.
    password_file::try_delete(&password_file::legacy_player_profile());

    let arguments = build_join(address, profile.download_dir.as_deref(), &helper_root().to_string_lossy(), &logs_dir.to_string_lossy());
    if let Err(problem) = launch(&exe, &arguments) {
        password_file::try_delete(&password_file);
        return Err(problem);
    }

    let has_password = password.is_some_and(|password| !password.is_empty());
    Ok(JoinStart { password_file: has_password.then_some(password_file) })
}

fn launch(exe: &Path, arguments: &[String]) -> Result<(), String> {
    let folder = exe.parent().unwrap_or(Path::new("."));
    let starter = folder.join(BATTLEYE_STARTER);
    let use_starter = starter.is_file();
    let mut all = Vec::new();
    if use_starter {
        all.extend(["-exe".to_string(), exe.file_name().unwrap_or_default().to_string_lossy().into_owned()]);
    }
    all.extend_from_slice(arguments);

    // ShellExecute, weil der BattlEye-Starter Administratorrechte anfordern kann.
    let file = wide(if use_starter { starter.as_os_str() } else { exe.as_os_str() });
    let parameters = wide(to_command_line(&all));
    let directory = wide(folder.as_os_str());
    let verb = wide("open");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).expect("klein"),
        lpVerb: pcwstr(&verb),
        lpFile: pcwstr(&file),
        lpParameters: pcwstr(&parameters),
        lpDirectory: pcwstr(&directory),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: Alle Texte sind nullterminiert und leben bis nach dem Aufruf.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|error| format!("Das Spiel konnte nicht gestartet werden: {}", error.message()))
}
