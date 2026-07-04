// src/lib.rs

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Manager, WindowEvent};

use crate::{db::init_db, state::AppState};

pub mod commands;
pub mod db;
pub mod error;
pub mod download_core;
pub mod monitor;
pub mod providers;
pub mod state;
pub mod torrent;
pub mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let monitor_running = Arc::new(AtomicBool::new(true));

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let db = init_db(app.handle())?;
            let state = AppState::new(db, Arc::clone(&monitor_running));
            app.manage(state);

            // BT 引擎（librqbit session + 每秒 stats 推送）
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let torrent_state = tauri::async_runtime::block_on(torrent::state::init(app_data_dir))
                .map_err(|e| -> Box<dyn std::error::Error> { format!("{:#}", e).into() })?;
            app.manage(torrent_state);
            torrent::events::spawn_stats_task(app.handle().clone());

            // 啟動剪貼簿監控邏輯
            let app_handle = app.handle().clone();
            monitor::start_clipboard_monitor(app_handle, Arc::clone(&monitor_running));

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { .. } => {
                    // 優雅關閉 BT session：暫停 torrents 讓 persistence flush 完再退出
                    if let Some(ts) = window.try_state::<torrent::state::TorrentState>() {
                        tauri::async_runtime::block_on(ts.session.stop());
                    }
                }
                WindowEvent::Destroyed => {
                    if let Some(state) = window.try_state::<AppState>() {
                        state.monitor_running.store(false, Ordering::Relaxed);
                    }
                }
                _ => {}
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
            commands::common::set_bandwidth_limit,
            commands::common::reorder_tasks,
            commands::common::add_url_manually,
            torrent::commands::add_magnet,
            torrent::commands::remove_pending,
            torrent::commands::list_torrents,
            torrent::commands::torrent_details,
            torrent::commands::pause_torrent,
            torrent::commands::resume_torrent,
            torrent::commands::delete_torrent,
            torrent::commands::get_bt_settings,
            torrent::commands::save_bt_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
