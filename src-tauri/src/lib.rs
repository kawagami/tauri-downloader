// lib.rs

// 引入 Tauri commands 的說明
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

// 引入我們需要的函式庫
use reqwest;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

// 定義 greet 函數
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// 這是我們新增的 download_url 函數
// 請注意，它必須是 async 函數，因為 reqwest::get() 是一個非同步操作
#[tauri::command]
async fn download_url(url: String) -> Result<String, String> {
    // 這裡我們先不實作下載功能，只印出 URL 來驗證連線
    println!("從前端接收到的 URL: {}", url);
    Ok(format!("成功接收 URL: {}", url))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,         // 這裡保留原本的 greet 函數
            download_url   // 將新的 download_url 函數註冊進來
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}