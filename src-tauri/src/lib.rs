// src/lib.rs

use crate::utils::lock::RwLockExt;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;

use crate::{db::init_db, state::AppState};

pub mod commands;
pub mod db;
pub mod error;
pub mod dl;
pub mod download_core;
pub mod http_dl;
pub mod ingest;
pub mod jin;
pub mod logging;
pub mod monitor;
pub mod providers;
pub mod settings;
pub mod state;
pub mod torrent;
pub mod utils;

/// 關閉收尾已開始（CloseRequested 只處理第一次）
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
/// 背景收尾上限 — 正常約 1 秒（librqbit 寫死的 sleep）
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let monitor_running = Arc::new(AtomicBool::new(true));

    tauri::Builder::default()
        // 必須是第一個 plugin：第二個實例在任何初始化前就退出，避免兩份剪貼簿監控、
        // 同寫 tasks.db / http_tasks.json / 同一個 .part、搶 bt-session。
        // 改把舊視窗叫到前景；舊實例正在關閉收尾（視窗已 hide）就不 show，免得閃一下又消失
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if SHUTTING_DOWN.load(Ordering::SeqCst) {
                return;
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        // 不記 VISIBLE：關閉時視窗先 hide 再背景收尾，exit 那一刻存下來的會是「隱藏」
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            // 最先做：之後每一步的 tracing 訊息才有地方落
            logging::init(&app_data_dir);
            tracing::info!("app 啟動，app_data_dir = {:?}", app_data_dir);

            // 統一設定：後端啟動自己 load 並套用 runtime 旗標，不靠前端補推
            let settings_state = settings::SettingsState::load(&app_data_dir);
            let s = settings_state.get();

            let db = init_db(app.handle())?;
            let state = AppState::new(db, Arc::clone(&monitor_running));

            // HTTP 直鏈下載（任務管理獨立於 BT 引擎與網站下載；位元組搬運與網站下載共用 crate::dl）
            let http_mgr =
                http_dl::manager::HttpManager::load(app_data_dir.join("http_tasks.json"), s.http.limit_bps);

            // 啟動與「儲存設定」走同一條套用路徑
            commands::common::apply_runtime_settings(&state, &http_mgr, &s);

            app.manage(state);
            app.manage(settings_state);

            // BT 引擎背景初始化 — 失敗（如 port 衝突）只讓 BT 分頁失效,不擋 app 啟動
            // （spawn_init 讀 SettingsState.bt，須在 manage 之後）
            app.manage(torrent::state::BtEngine::default());
            torrent::state::spawn_init(app.handle().clone());
            torrent::events::spawn_stats_task(app.handle().clone());

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
                WindowEvent::CloseRequested { api, .. } => {
                    // 收尾跑完後 app.exit() 可能再觸發一次 —— 第二次直接放行
                    if SHUTTING_DOWN.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    // 收尾要 1 秒以上（librqbit 的 Session::stop() 寫死睡 1s 等 DHT/persistence
                    // 收工），以前在主執行緒 block_on，視窗凍住等完才消失。
                    // 改成先藏視窗、背景收尾、做完才 exit：收尾內容不變，只是使用者看不到。
                    api.prevent_close();
                    let _ = window.hide();
                    let app = window.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        // 直鏈任務進度最後落地一次,重開時續傳才接得準
                        if let Some(mgr) = app.try_state::<Arc<http_dl::manager::HttpManager>>() {
                            mgr.persist_now();
                        }
                        // 優雅關閉 BT session：暫停 torrents 讓 persistence flush 完再退出
                        // 先 clone Arc 再 await，不在鎖裡等待
                        let ts = app
                            .try_state::<torrent::state::BtEngine>()
                            .and_then(|e| e.inner.read_safe().clone());
                        if let Some(ts) = ts {
                            // 上限防呆：收尾卡住也不能讓程序永遠掛在背景
                            if tokio::time::timeout(SHUTDOWN_TIMEOUT, ts.session.stop()).await.is_err() {
                                tracing::warn!("BT session 關閉逾時 {:?}，直接退出", SHUTDOWN_TIMEOUT);
                            }
                        }
                        app.exit(0);
                    });
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
