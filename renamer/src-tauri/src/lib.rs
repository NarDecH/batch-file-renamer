pub mod cli;
pub mod commands;
pub mod core;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Detailed logging to <data>/batch-renamer/logs/; set RENAMER_LOG=debug for verbose
    crate::core::logger::init(cfg!(debug_assertions));
    log::info!(
        "=== Batch Renamer GUI starting (v{}) ===",
        env!("CARGO_PKG_VERSION")
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::scan_paths,
            commands::preview,
            commands::update_rules,
            commands::set_conflict_strategy,
            commands::set_apply_to,
            commands::set_manual_name,
            commands::set_selection,
            commands::remove_entries,
            commands::apply_renames,
            commands::undo_last,
            commands::list_undo_history,
            commands::list_unfinished_batches,
            commands::save_preset,
            commands::load_preset,
            commands::list_presets,
            commands::parse_import_list,
            commands::log_file_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
