use crate::{download_core::DownloadManager, providers::{ClipboardPayload, DownloadProgress}, state::AppState};

use futures_util::StreamExt;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

static RE_VALIDATE: OnceLock<Regex> = OnceLock::new();

fn select_first<'a>(document: &'a Html, selectors: &[&str]) -> Option<ElementRef<'a>> {
    for sel in selectors {
        if let Ok(parsed) = Selector::parse(sel) {
            if let Some(el) = document.select(&parsed).next() {
                return Some(el);
            }
        }
    }
    None
}

/// 驗證 wnacg URL 並回傳規範化的 URL 字串
pub fn validate(content: &str) -> Result<String, String> {
    // 1. 初步解析 URL
    let parsed_url = Url::parse(content).map_err(|_| "無效的 URL 格式".to_string())?;

    // 2. 驗證 Scheme 與 Host (快速過濾)
    if parsed_url.scheme() != "https" {
        return Err("必須使用 https 協定".to_string());
    }

    if parsed_url.host_str() != Some("www.wnacg.com") {
        return Err("域名必須為 www.wnacg.com".to_string());
    }

    // 3. 使用 Regex 驗證 Path 並提取 ID (兼顧檢查與提取)
    let re = RE_VALIDATE.get_or_init(|| Regex::new(r"^/photos-index-aid-(\d+)\.html$").unwrap());

    if !re.is_match(parsed_url.path()) {
        return Err("路徑格式錯誤，應為 /photos-index-aid-{ID}.html".to_string());
    }

    // 回傳規範化後的字串
    Ok(parsed_url.to_string())
}

/// 輔助用函數
pub async fn get_file_url(
    app_handle: &AppHandle,
    url: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    println!("Rust Monitor: 正在從 URL 獲取詳細資訊: {}", url);

    // 取 state 中的 client 執行 reqwest get 請求
    let state = app_handle.state::<AppState>();
    let client = &state.client;
    let res = client.get(url).send().await?;

    if matches!(res.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err("NOT_FOUND".into());
    }
    if !res.status().is_success() {
        return Err(format!("網絡請求失敗，狀態碼: {}", res.status()).into());
    }

    let html_content = res.text().await?;
    let document = Html::parse_document(&html_content);

    let raw = select_first(&document, &["#ads > a", "a.ads", "a[href*='down']"])
        .and_then(|el| el.value().attr("href"))
        .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
            "wnacg: 無法找到下載連結".into()
        })?;

    let href = if raw.starts_with("http") {
        raw.to_string()
    } else if raw.starts_with("//") {
        format!("https:{}", raw)
    } else {
        format!("https://www.wnacg.com{}", raw)
    };

    Ok(href)
}

pub async fn fetch_payload_details(
    app_handle: &AppHandle,
    url: String,
) -> Result<ClipboardPayload, Box<dyn std::error::Error + Send + Sync>> {
    println!("Rust Monitor: 正在從 URL 獲取詳細資訊: {}", url);

    // 取 state 中的 client 執行 reqwest get 請求
    let state = app_handle.state::<AppState>();
    let client = &state.client;
    let res = client.get(&url).send().await?;

    if matches!(res.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err("NOT_FOUND".into());
    }
    if !res.status().is_success() {
        return Err(format!("網絡請求失敗，狀態碼: {}", res.status()).into());
    }

    // 實際應用中，您會解析 HTML 內容來獲取 title 和 image URL/ID
    let html_content = res.text().await?;
    let document = Html::parse_document(&html_content);

    let title = select_first(&document, &["#bodywrap > h2", "#bodywrap h2", "h1", "h2"])
        .map(|el| el.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| "無法找到標題".to_string());

    let image = select_first(&document, &[
        "#bodywrap .pic_box img",
        ".pic_box img",
        ".grid img",
    ])
    .and_then(|el| el.value().attr("src"))
    .map(|s| s.to_string())
    .unwrap_or_else(|| "placeholder.png".to_string());

    let download_page_href_raw = select_first(&document, &["#ads > a", "a.ads", "a[href*='down']"])
        .and_then(|el| el.value().attr("href"))
        .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
            "wnacg: 無法找到下載頁面連結".into()
        })?;

    let download_page_href = if download_page_href_raw.starts_with("http") {
        download_page_href_raw.to_string()
    } else if download_page_href_raw.starts_with("//") {
        format!("https:{}", download_page_href_raw)
    } else {
        format!("https://www.wnacg.com{}", download_page_href_raw)
    };

    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    Ok(ClipboardPayload {
        url,
        title,
        image,
        download_page_href,
        created_at,
        db_status: "idle".to_string(),
    })
}

pub async fn download(
    client: &reqwest::Client,
    app_handle: &AppHandle,
    source_url: String, // 原始網頁網址 (用於進度事件辨識)
    file_url: String,   // 實際檔案下載網址
    save_path: PathBuf,
    cancelled: Arc<AtomicBool>,
    bandwidth_limit_bps: u64, // 0 = 無限制
) -> Result<(), String> {
    let resp = client
        .get(&file_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if matches!(resp.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err("NOT_FOUND".to_string());
    }
    if !resp.status().is_success() {
        return Err(format!("下載失敗 status: {}, Url: {}", resp.status(), file_url));
    }

    let total_size = resp.content_length().unwrap_or(0);
    let mut file = File::create(&save_path).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    let mut manager = DownloadManager::new();
    manager.start_download(total_size);
    let throttle_start = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&save_path);
            return Err("下載已取消".to_string());
        }

        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        if bandwidth_limit_bps > 0 {
            let expected = std::time::Duration::from_secs_f64(
                downloaded as f64 / bandwidth_limit_bps as f64,
            );
            let actual = throttle_start.elapsed();
            if expected > actual {
                tokio::time::sleep(expected - actual).await;
            }
        }

        let metrics = manager.calculate_metrics(downloaded, total_size);
        app_handle
            .emit(
                "download_progress",
                DownloadProgress {
                    url: source_url.clone(),
                    progress: metrics.percentage,
                    speed_bytes_per_sec: metrics.speed_bytes_per_sec,
                    time_remaining_secs: metrics.time_remaining_secs,
                },
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
