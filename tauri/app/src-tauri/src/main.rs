// Kein Konsolenfenster neben der App im Release-Build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Version für die Anzeige in der Oberfläche.
#[tauri::command]
fn app_version() -> &'static str {
    rms_core::VERSION
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("Die Oberfläche konnte nicht gestartet werden");
}
