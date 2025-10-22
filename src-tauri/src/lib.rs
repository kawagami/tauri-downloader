// src/lib.rs (調整後)

// 引入我們定義的模塊
pub mod commands;
pub mod download_core;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // ✨ 註冊 commands.rs 中的函數
            // 現在需要指定模塊路徑
            commands::common::greet,
            commands::network::download_url,  // 來自 network.rs
            commands::common::read_clipboard  // 來自 common.rs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
