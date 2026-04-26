use crate::db;
use crate::providers::Site;
use crate::state::AppState;
use clipboard::{ClipboardContext, ClipboardProvider};
use sanitize_filename::sanitize;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use std::{thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

const MONITOR_INTERVAL_MS: u64 = 500;
const URL_THROTTLE_SECS: u64 = 30;

pub fn start_clipboard_monitor(app_handle: AppHandle, running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut ctx: ClipboardContext = ClipboardProvider::new().expect("Failed to init clipboard");
        // 啟動時先讀取當前剪貼簿，避免把舊內容當新內容處理
        let mut last_content = ctx.get_contents().unwrap_or_default();
        let mut recent_urls: HashMap<String, Instant> = HashMap::new();

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
                        if let Ok(normalized_url) = site.validate(&current_content) {
                            // 節流：30 秒內同一 URL 不重複抓取
                            let now = Instant::now();
                            recent_urls.retain(|_, t| now.duration_since(*t).as_secs() < URL_THROTTLE_SECS);

                            if recent_urls.contains_key(&normalized_url) {
                                last_content = current_content;
                                continue;
                            }
                            recent_urls.insert(normalized_url.clone(), now);
                            println!("Monitor: 偵測到有效 {} 連結: {}", site.to_string(), normalized_url);

                            let handle = app_handle.clone();
                            let url_to_fetch = normalized_url.clone();

                            // 4. 使用 Tauri 內建的 runtime 執行異步抓取
                            tauri::async_runtime::spawn(async move {
                                match site.fetch_details(&handle, &url_to_fetch).await {
                                    Ok(payload) => {
                                        // 檢查下載目錄是否已有同名檔案（含 _N 後綴變體）
                                        let already_exists = handle
                                            .path()
                                            .download_dir()
                                            .ok()
                                            .and_then(|dir| std::fs::read_dir(dir).ok())
                                            .map(|entries| {
                                                let prefix = sanitize(&payload.title);
                                                entries.filter_map(|e| e.ok()).any(|e| {
                                                    let name = e.file_name();
                                                    let name = name.to_string_lossy();
                                                    name.starts_with(prefix.as_str()) && name.ends_with(".zip")
                                                })
                                            })
                                            .unwrap_or(false);

                                        if already_exists {
                                            return;
                                        }

                                        match db::insert_task(&handle, &payload) {
                                            Ok(true) => {
                                                let _ = handle.emit("new-valid-url-payload", payload);
                                            }
                                            Ok(false) => {}
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
