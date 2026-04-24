use crate::db;
use crate::providers::Site;
use crate::state::AppState;
use clipboard::{ClipboardContext, ClipboardProvider};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

const MONITOR_INTERVAL_MS: u64 = 500;

pub fn start_clipboard_monitor(app_handle: AppHandle, running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut last_content = String::new();
        let mut ctx: ClipboardContext = ClipboardProvider::new().expect("Failed to init clipboard");

        while running.load(Ordering::Relaxed) {
            let paused = app_handle
                .try_state::<AppState>()
                .map(|s| s.monitor_paused.load(Ordering::Relaxed))
                .unwrap_or(false);

            if paused {
                thread::sleep(Duration::from_millis(MONITOR_INTERVAL_MS));
                continue;
            }

            // 1. 獲取剪貼簿內容
            if let Ok(current_content) = ctx.get_contents() {
                // 內容變化且非空
                if !current_content.is_empty() && current_content != last_content {
                    if let Ok(site) = Site::from_url(&current_content) {
                        // 3. 執行該網站的驗證
                        if let Ok(normalized_url) = site.validate(&current_content) {
                            println!(
                                "Monitor: 偵測到有效 {} 連結: {}",
                                site.to_string(),
                                normalized_url
                            );

                            let handle = app_handle.clone();
                            let url_to_fetch = normalized_url.clone();

                            // 4. 使用 Tauri 內建的 runtime 執行異步抓取
                            tauri::async_runtime::spawn(async move {
                                match site.fetch_details(&handle, &url_to_fetch).await {
                                    Ok(payload) => {
                                        match db::insert_task(&handle, &payload) {
                                            Ok(true) => {
                                                // 真正新增才通知前端
                                                let _ = handle.emit("new-valid-url-payload", payload);
                                            }
                                            Ok(false) => {} // 已存在，略過
                                            Err(e) => eprintln!("DB Error: {:?}", e),
                                        }
                                    }
                                    Err(e) => eprintln!("Fetch Error: {}", e),
                                }
                            });
                        }
                    }
                    last_content = current_content;
                }
            }
            thread::sleep(Duration::from_millis(MONITOR_INTERVAL_MS));
        }
    });
}
