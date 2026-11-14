// Kein Konsolenfenster neben der App im Release-Build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dto;

use rms_core::client::password_file;
use rms_core::paths::{AppPaths, default_settings_file};
use rms_core::settings::SettingsStore;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tracing::{error, info};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::PCWSTR;

use crate::commands::AppState;

const LOG_FILES_KEPT: usize = 14;

fn main() {
    // Ohne WebView2 kann die Oberfläche nicht starten. Lieber eine klare Meldung als ein stilles Ende.
    if tauri::webview_version().is_err() {
        show_error(
            "Microsoft Edge WebView2 fehlt",
            "Reforger Sync & Launch braucht die Microsoft Edge WebView2 Runtime. Unter Windows 11 ist sie dabei, \
             unter Windows 10 lässt sie sich kostenlos bei Microsoft herunterladen (\"WebView2 Runtime\").",
        );
        return;
    }

    let paths = AppPaths::default_for_user();
    let _log_guard = start_logging(&paths);
    info!("Reforger Sync & Launch {} gestartet", rms_core::VERSION);
    password_file::clean_up_leftovers(&paths);
    rms_core::paths::remove_legacy_webview_data();

    let state = AppState::new(SettingsStore::new(default_settings_file()), paths);
    let app = tauri::Builder::default()
        // Es läuft nur eine Instanz: Ein zweiter Start holt das vorhandene Fenster nach vorn.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title(&format!("Reforger Sync & Launch {}", rms_core::VERSION));
                // Nie höher als der Arbeitsbereich des Bildschirms.
                if let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) {
                    let available = monitor.work_area().size.height;
                    if size.height > available {
                        let _ = window.set_size(tauri::PhysicalSize::new(size.width, available));
                        let _ = window.center();
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_app,
            commands::select_profile,
            commands::password_transfer_needed,
            commands::save_profile,
            commands::remove_profile,
            commands::delete_password,
            commands::steam_options,
            commands::check_profile,
            commands::sync_profile,
            commands::answer_cleanup,
            commands::cancel_run,
            commands::join_server,
            commands::basic_settings,
            commands::save_basic_settings,
            commands::browse_folder,
            commands::browse_exe,
            commands::open_link,
            commands::quit_app,
        ])
        .on_window_event(|window, event| {
            // Läuft ein Vorgang, fragt die Oberfläche vorher nach, weil er sonst abbricht.
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.state::<AppState>().is_busy()
            {
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(problem) => {
            error!("Oberfläche konnte nicht starten: {problem}");
            show_error("Reforger Sync & Launch", &format!("Die Oberfläche konnte nicht gestartet werden: {problem}"));
            return;
        }
    };

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            let state = app.state::<AppState>();
            // Das Job Object beendet einen laufenden Client mit, spätestens wenn dieser Prozess endet.
            state.request_cancel();
            // Direkt nach dem Beitritt geschlossen: Das Spiel braucht die Datei noch, liegen bleiben darf sie aber nicht.
            if let Some((file, wait)) = state.pending_join_password() {
                password_file::wait_until_removed(&file, wait);
                password_file::try_delete(&file);
            }
            info!("Beendet");
        }
    });
}

/// Tägliche Logdatei `app.<Datum>.log`. Das Aufräumen erfasst alle Dateien `app*.log`, also auch die der
/// Vorgängerversionen (`app-<Datum>.log`, `app-tauri.<Datum>.log`), und behält insgesamt die neuesten 14.
fn start_logging(paths: &AppPaths) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("app")
        .filename_suffix("log")
        .max_log_files(LOG_FILES_KEPT)
        .build(paths.logs())
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt().with_writer(writer).with_ansi(false).with_target(false).with_timer(LocalTime).init();
    Some(guard)
}

/// Ortszeit wie in den Logs der C#-Vorgängerversion, nicht UTC.
struct LocalTime;

impl tracing_subscriber::fmt::time::FormatTime for LocalTime {
    fn format_time(&self, writer: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(writer, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

fn show_error(title: &str, message: &str) {
    let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
    let message: Vec<u16> = message.encode_utf16().chain([0]).collect();
    // SAFETY: Beide Texte sind nullterminiert und leben bis nach dem Aufruf.
    unsafe {
        MessageBoxW(None, PCWSTR(message.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
    }
}
