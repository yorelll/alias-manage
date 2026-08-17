// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::startup_status,
            commands::list_aliases,
            commands::search_aliases,
            commands::tag_counts,
            commands::create_alias,
            commands::update_alias,
            commands::doctor_status,
            commands::generated_preview,
            commands::reload_command,
            commands::import_preview,
            commands::import_confirm,
            commands::config_get,
            commands::config_save,
            commands::uninstall_preview,
            commands::uninstall_confirm,
            commands::overridden_definitions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
