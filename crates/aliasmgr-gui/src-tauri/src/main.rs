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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
