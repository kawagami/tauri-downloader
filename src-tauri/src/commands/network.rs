// src/commands/network.rs

use crate::{providers::wnacg, state::AppState};

use futures_util::StreamExt;
use sanitize_filename::sanitize;
use serde::Serialize;
use std::{fs::File, io::Write, path::PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize, Clone)]
struct DownloadProgress {
    url: String,
    progress: f64,
}

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
    // 取得應用資料夾路徑
    let mut save_path: PathBuf = app_handle
        .path()
        .download_dir()
        .map_err(|e| format!("無法取得 download_dir: {}", e))?;

    // 建立路徑（如果資料夾不存在）
    std::fs::create_dir_all(&save_path).map_err(|e| e.to_string())?;

    // 組合完整檔案路徑
    let base_name = sanitize(&title);
    let mut file_name = format!("{}.zip", base_name);
    save_path.push(&file_name);
    let mut counter = 1;
    while save_path.exists() {
        file_name = format!("{}_{}.zip", base_name, counter);
        save_path.set_file_name(&file_name);
        counter += 1;
    }

    // 取得檔案路徑
    let file_url = wnacg::get_file_url(&app_handle, &url)
        .await
        .map_err(|e| format!("無法取得 file_url: {}", e))?;

    // 發送請求
    let client = &state.client;
    let resp = client
        .get(file_url)
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

        // ✅ 發送事件（包含 URL）
        let payload = DownloadProgress {
            url: url.to_string(),
            progress,
        };

        // println!("emit progress: {}% for {}", progress, url);
        app_handle
            .emit("download_progress", payload)
            .map_err(|e| e.to_string())?;
    }

    Ok(save_path.to_string_lossy().to_string())
}
