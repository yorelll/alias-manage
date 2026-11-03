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
            commands::preview_alias,
            commands::create_alias,
            commands::update_alias,
            commands::delete_alias,
            commands::doctor_status,
            commands::generated_preview,
            commands::reload_command,
            commands::sync_aliases,
            commands::shell_status,
            commands::shell_startup_state,
            commands::shell_set_enabled,
            commands::shell_setup_plan,
            commands::shell_setup_apply,
            commands::shell_setup_skip,
            commands::preflight_alias_shells,
            commands::shell_install,
            commands::import_preview,
            commands::import_confirm,
            commands::import_preview_content,
            commands::import_confirm_content,
            commands::config_get,
            commands::config_save,
            commands::uninstall_preview,
            commands::uninstall_confirm,
            commands::overridden_definitions,
            commands::scan_overrides,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
