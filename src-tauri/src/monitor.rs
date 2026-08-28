use crate::utils::lock::LockExt;
use crate::db;
use crate::providers::Site;
use crate::state::AppState;
use clipboard::{ClipboardContext, ClipboardProvider};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Instant;
use std::{thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

const MONITOR_INTERVAL_MS: u64 = 500;
const URL_THROTTLE_SECS: u64 = 30;

pub fn start_clipboard_monitor(app_handle: AppHandle, running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut ctx: ClipboardContext = match ClipboardProvider::new() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Monitor: 剪貼簿初始化失敗: {}", e);
                return;
            }
        };
        // 啟動時先讀取當前剪貼簿，避免把舊內容當新內容處理
        let mut last_content = ctx.get_contents().unwrap_or_default();
        // 共享 map：fetch 失敗時從節流名單移除，讓使用者能立即重試
        let recent_urls: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

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
                // 內容變化且非空時才處理
                if !current_content.is_empty() && current_content != last_content {
                    let trimmed = current_content.trim();
                    if trimmed.starts_with("magnet:") {
                        // magnet 連結 → 交給 BT 引擎，沿用同一節流 map（key 為 magnet 字串）
                        let now = Instant::now();
                        let should_add = {
                            let mut map = recent_urls.lock_safe();
                            map.retain(|_, t| now.duration_since(*t).as_secs() < URL_THROTTLE_SECS);
                            if map.contains_key(trimmed) {
                                false
                            } else {
                                map.insert(trimmed.to_string(), now);
                                true
                            }
                        };
                        if should_add {
                            tracing::info!("Monitor: 偵測到 magnet 連結");
                            let handle = app_handle.clone();
                            let magnet = trimmed.to_string();
                            let recent_urls = Arc::clone(&recent_urls);
                            tauri::async_runtime::spawn(async move {
                                match crate::torrent::commands::add_magnet_inner(
                                    handle.clone(),
                                    magnet.clone(),
                                    None,
                                    false,
                                )
                                .await
                                {
                                    // 新加入（非重複）才通知前端播 ding
                                    Ok(v) if v.get("pending").is_some() => {
                                        let name = v.get("name").cloned();
                                        let _ = handle.emit("new-magnet-added", name);
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("Magnet add error: {}", e);
                                        // 加入失敗（如引擎未就緒）移出節流名單讓使用者可立即重試，
                                        // 並通知前端顯示 toast（與站台 URL fetch 失敗行為一致）
                                        recent_urls.lock_safe().remove(&magnet);
                                        let _ = handle.emit("magnet-add-error", e);
                                    }
                                }
                            });
                        }
                    } else if let Ok(site) = Site::from_url(&current_content) {
                        if let Ok(normalized_url) = site.validate(&current_content) {
                            // 節流：30 秒內同一 URL 不重複抓取
                            let now = Instant::now();
                            let should_fetch = {
                                let mut map = recent_urls.lock_safe();
                                map.retain(|_, t| now.duration_since(*t).as_secs() < URL_THROTTLE_SECS);
                                if map.contains_key(&normalized_url) {
                                    false
                                } else {
                                    map.insert(normalized_url.clone(), now);
                                    true
                                }
                            };

                            if should_fetch {
                                tracing::info!("Monitor: 偵測到有效 {} 連結: {}", site.to_string(), normalized_url);

                                let handle = app_handle.clone();
                                let url_to_fetch = normalized_url.clone();
                                let recent_urls = Arc::clone(&recent_urls);

                                // 使用 Tauri 內建的 runtime 執行異步抓取
                                tauri::async_runtime::spawn(async move {
                                    match site.fetch_details(&handle, &url_to_fetch).await {
                                        Ok(payload) => {
                                            // 檢查下載目錄是否已有同名檔案（含 _N 後綴變體）；命名規則與
                                            // get_unique_save_path 共用同一個函式，不再各寫一份。
                                            // 目錄來源與實際下載一致：AppSettings.web.default_dir（空 = 系統下載夾）
                                            let web_dir = crate::utils::fs::resolve_dir(
                                                &handle
                                                    .state::<crate::settings::SettingsState>()
                                                    .get()
                                                    .web
                                                    .default_dir,
                                            );
                                            if crate::utils::fs::already_downloaded(&web_dir, &payload.title) {
                                                tracing::info!("已下載過，略過: {}", payload.title);
                                                return;
                                            }

                                            match db::insert_task(&handle, &payload) {
                                                Ok(true) => {
                                                    let _ = handle.emit("new-valid-url-payload", payload);
                                                }
                                                Ok(false) => {}
                                                Err(e) => tracing::error!("DB Error: {:?}", e),
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Fetch Error: {}", e);
                                            // 抓取失敗，移出節流名單讓使用者可立即重試
                                            recent_urls.lock_safe().remove(&url_to_fetch);
                                            // 與 magnet-add-error 對稱：不通知的話使用者只會看到
                                            // 「複製了但沒反應」，分不出是網路掛了、站台改版、
                                            // 還是這個 provider 根本還沒實作（nhentai）
                                            let _ = handle.emit("url-fetch-error", e);
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                // 無論內容是否有效，都更新 last_content（含空字串），
                // 避免清空剪貼簿後再次複製同一 URL 無法觸發的問題
                last_content = current_content;
            }
            thread::sleep(Duration::from_millis(MONITOR_INTERVAL_MS));
        }
    });
}
