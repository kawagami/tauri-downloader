// src/lib.rs

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Manager, WindowEvent};

use crate::{db::init_db, state::AppState};

pub mod commands;
pub mod db;
pub mod download_core;
pub mod monitor;
pub mod providers;
pub mod state;
pub mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let monitor_running = Arc::new(AtomicBool::new(true));

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let db = init_db(app.handle())?;
            let state = AppState::new(db, Arc::clone(&monitor_running));
            app.manage(state);

            // 啟動剪貼簿監控邏輯
            let app_handle = app.handle().clone();
            monitor::start_clipboard_monitor(app_handle, Arc::clone(&monitor_running));

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                if let Some(state) = window.try_state::<AppState>() {
                    state.monitor_running.store(false, Ordering::Relaxed);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::network::download_with_progress,
            commands::common::read_clipboard,
            commands::common::load_all_tasks,
            commands::common::remove_task,
            commands::common::remove_all_tasks,
            commands::common::cancel_download,
            commands::common::set_monitor_paused,
            commands::common::update_task_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
