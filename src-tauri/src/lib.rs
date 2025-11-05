// src/lib.rs

use tauri::Manager;

use crate::{db::init_db, state::AppState};

pub mod commands;
pub mod db;
pub mod download_core;
pub mod monitor;
pub mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db = init_db(app.handle())?;
            let state = AppState::new(db);
            app.manage(state);

            // 啟動剪貼簿監控邏輯
            let app_handle = app.handle().clone();
            monitor::start_clipboard_monitor(app_handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::common::greet,
            commands::network::download_with_progress,
            commands::common::read_clipboard,
            commands::common::load_all_tasks,
            commands::common::remove_task,
            commands::common::remove_all_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
