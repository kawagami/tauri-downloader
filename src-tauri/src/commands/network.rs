// src/commands/network.rs

// 這裡需要引入外部的 download_core 模塊
// 只需要 DownloadManager 的 new 和 start_download 方法
use crate::state::AppState;

// 引入 Tauri 核心
use futures_util::StreamExt;
use scraper::Selector;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

// 由於移除了異步循環，暫時不需要 tokio::time
// use tokio::time::{sleep, Duration};
// use tauri::Manager; // 暫時移除，避免 windows() 錯誤

#[tauri::command]
pub async fn check_file_available(url: String, state: State<'_, AppState>) -> Result<bool, String> {
    // 使用全域共用的 reqwest client
    let client = &state.client;

    let resp = client
        .head(&url)
        .send()
        .await
        .map_err(|e| format!("request error: {}", e))?;

    // 狀態碼 200 表示可用
    Ok(resp.status().is_success())
}

#[tauri::command]
pub async fn download_with_progress(
    url: String,
    title: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let client = &state.client;

    let file_name = format!("{}.zip", title);

    // 取得應用資料夾路徑
    let mut save_path: PathBuf = app_handle
        .path()
        .download_dir()
        .map_err(|e| format!("無法取得 download_dir: {}", e))?;

    // 建立路徑（如果資料夾不存在）
    std::fs::create_dir_all(&save_path).map_err(|e| e.to_string())?;

    // 組合完整檔案路徑
    save_path.push(file_name);

    // 取得檔案路徑
    let file_url = get_file_url(&app_handle, url)
        .await
        .map_err(|e| format!("無法取得 file_url: {}", e))?;

    let https_file_url = format!("https:{}", file_url);
    // 發送請求
    let resp = client
        .get(https_file_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // 檢查 HTTP 狀態碼
    if !resp.status().is_success() {
        return Err(format!("下載失敗，HTTP 狀態碼: {}", resp.status()));
    }

    // 取得檔案總大小
    let total_size = resp
        .content_length()
        .ok_or("無法取得檔案大小 (Content-Length)")?;

    // 建立輸出檔案
    let mut file = File::create(&save_path).map_err(|e| e.to_string())?;

    // 下載進度統計
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    // 逐塊下載並寫入
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        let progress = (downloaded as f64 / total_size as f64) * 100.0;

        // 🔥 發送事件給前端
        app_handle
            .emit("download_progress", progress)
            .map_err(|e| e.to_string())?;
    }

    Ok(save_path.to_string_lossy().to_string())
}

/// 輔助用函數
async fn get_file_url(
    app_handle: &AppHandle,
    url: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    println!("Rust Monitor: 正在從 URL 獲取詳細資訊: {}", url);

    // 取 state 中的 client 執行 reqwest get 請求
    let state = app_handle.state::<AppState>();
    let client = &state.client;
    let res = client.get(&url).send().await?;

    // 檢查響應狀態
    if !res.status().is_success() {
        return Err(format!("網絡請求失敗，狀態碼: {}", res.status()).into());
    }

    // 實際應用中，您會解析 HTML 內容來獲取 title 和 image URL/ID
    let html_content = res.text().await?;
    let document = scraper::Html::parse_document(&html_content);

    // 取得下載路徑
    let download_page_href_selector = Selector::parse("#adsbox > a:nth-child(1)").unwrap();
    let download_page_href_raw = document
        .select(&download_page_href_selector)
        .next()
        .and_then(|element| element.value().attr("href"))
        .map(|href| href.to_string())
        .unwrap_or_else(|| {
            eprintln!("Rust Monitor: 無法找到下載頁面的 href。");
            "".to_string() // 找不到時使用空字串
        });

    Ok(download_page_href_raw)
}
