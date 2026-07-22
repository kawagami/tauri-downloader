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
pub mod http_dl;
pub mod jin;
pub mod monitor;
pub mod providers;
pub mod settings;
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
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            // 統一設定：後端啟動自己 load 並套用 runtime 旗標，不靠前端補推
            let settings_state = settings::SettingsState::load(&app_data_dir);
            let s = settings_state.get();

            let db = init_db(app.handle())?;
            let state = AppState::new(db, Arc::clone(&monitor_running));
            state
                .bandwidth_limit_bps
                .store(s.bandwidth_limit_kbps * 1024, Ordering::Relaxed);
            state
                .monitor_paused
                .store(!s.monitor_clipboard, Ordering::Relaxed);
            app.manage(state);
            app.manage(settings_state);

            // BT 引擎背景初始化 — 失敗（如 port 衝突）只讓 BT 分頁失效,不擋 app 啟動
            // （spawn_init 讀 SettingsState.bt，須在 manage 之後）
            app.manage(torrent::state::BtEngine::default());
            torrent::state::spawn_init(app.handle().clone());
            torrent::events::spawn_stats_task(app.handle().clone());

            // HTTP 直鏈下載（獨立於 BT 引擎與網站下載）
            let http_mgr = http_dl::manager::HttpManager::load(app_data_dir.join("http_tasks.json"));
            // 上次關閉時仍在跑的任務自動續傳
            http_mgr.resume_interrupted();
            app.manage(http_mgr);
            http_dl::events::spawn_http_stats_task(app.handle().clone());

            // 啟動剪貼簿監控邏輯
            let app_handle = app.handle().clone();
            monitor::start_clipboard_monitor(app_handle, Arc::clone(&monitor_running));

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { .. } => {
                    // 直鏈任務進度最後落地一次,重開時續傳才接得準
                    if let Some(mgr) = window.try_state::<std::sync::Arc<http_dl::manager::HttpManager>>() {
                        mgr.persist();
                    }
                    // 優雅關閉 BT session：暫停 torrents 讓 persistence flush 完再退出
                    // 先 clone Arc 再 block_on，不在鎖裡等待
                    let ts = window
                        .try_state::<torrent::state::BtEngine>()
                        .and_then(|e| e.inner.read().unwrap().clone());
                    if let Some(ts) = ts {
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
            commands::common::update_task_status,
            commands::common::get_app_settings,
            commands::common::save_app_settings,
            commands::common::reorder_tasks,
            commands::common::add_url_manually,
            torrent::commands::add_magnet,
            torrent::commands::remove_pending,
            torrent::commands::list_torrents,
            torrent::commands::torrent_details,
            torrent::commands::pause_torrent,
            torrent::commands::resume_torrent,
            torrent::commands::delete_torrent,
            torrent::commands::get_bt_engine_status,
            torrent::commands::retry_bt_init,
            http_dl::commands::add_http_download,
            http_dl::commands::pause_http_download,
            http_dl::commands::resume_http_download,
            http_dl::commands::update_http_url,
            http_dl::commands::delete_http_download,
            jin::commands::jin_preview,
            jin::commands::jin_apply,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
