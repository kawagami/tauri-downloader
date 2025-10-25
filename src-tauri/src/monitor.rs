// src/monitor.rs

use clipboard::ClipboardProvider;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

// 移除 tokio 相關的 use 語句，改用 std::thread::spawn
use std::thread; // ✨ 新增 std::thread

use crate::commands::common::is_valid_wnacg_url;

const MONITOR_INTERVAL_MS: u64 = 500;

/// 啟動剪貼簿監控線程
/// 使用 std::thread::spawn 確保它在一個獨立的 OS 線程上運行，
/// 繞過 Tauri/Tokio 運行時上下文的問題。
pub fn start_clipboard_monitor(app_handle: AppHandle) {
    // ✨ 將 tokio::task::spawn_blocking 替換為 std::thread::spawn
    thread::spawn(move || {
        let mut last_content = String::new();
        let mut ctx: clipboard::ClipboardContext = match ClipboardProvider::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("無法初始化剪貼簿上下文，停止監控: {}", e);
                return;
            }
        };

        loop {
            let current_content = match ctx.get_contents() {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("讀取剪貼簿錯誤: {}", e);
                    std::thread::sleep(Duration::from_millis(MONITOR_INTERVAL_MS));
                    continue;
                }
            };

            if current_content != last_content {
                match is_valid_wnacg_url(&current_content) {
                    Ok(parsed_url) => {
                        println!("Rust Monitor: 偵測到新的有效 URL，推送事件。");

                        // 雖然在 std::thread 中，但 AppHandle::windows() 仍然可用
                        app_handle.emit("new-valid-url", parsed_url).unwrap();
                    }
                    Err(_) => {}
                }
                last_content = current_content;
            }

            // 使用 std::thread::sleep 進行阻塞等待
            thread::sleep(Duration::from_millis(MONITOR_INTERVAL_MS));
        }
    });
}
