// src/lib.rs

pub mod commands;
pub mod db;
pub mod download_core;
pub mod monitor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化資料庫
            db::init_db(&app.handle())
                .map_err(|e| {
                    println!("❌ 資料庫初始化失敗: {:?}", e);
                    e
                })
                .ok();

            // 啟動剪貼簿監控邏輯
            let app_handle = app.handle().clone();
            monitor::start_clipboard_monitor(app_handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::common::greet,
            commands::network::download_url,
            commands::common::read_clipboard,
            commands::common::load_all_tasks,
            commands::common::remove_task,
            commands::common::remove_all_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
