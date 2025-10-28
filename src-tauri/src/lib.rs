// src/lib.rs (調整後)

// 引入我們定義的模塊
pub mod commands;
pub mod download_core;
pub mod monitor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // ✨ 在設定invoke_handler之前，先啟動監控線程
        .setup(|app| {
            // 由於 AppHandle 不能直接跨越異步邊界傳遞給 tokio::spawn_blocking，
            // 我們使用 app.handle() 獲得一個 AppHandle 的 Clone，並傳入 monitor 模塊。
            let app_handle = app.handle().clone();
            monitor::start_clipboard_monitor(app_handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ✨ 註冊 commands.rs 中的函數
            // 現在需要指定模塊路徑
            commands::common::greet,
            commands::network::download_url,  // 來自 network.rs
            commands::common::read_clipboard, // 保持舊的命令
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
