use crate::{download_core::DownloadManager, error::DownloadError, providers::{ClipboardPayload, DownloadProgress}, state::AppState};

use futures_util::StreamExt;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
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
) -> Result<String, DownloadError> {
    tracing::debug!("get_file_url: {}", url);

    // 取 state 中的 client 執行 reqwest get 請求
    let state = app_handle.state::<AppState>();
    let client = &state.client;
    let res = client.get(url).send().await?;

    if matches!(res.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err(DownloadError::NotFound);
    }
    if !res.status().is_success() {
        return Err(DownloadError::Other(format!("網絡請求失敗，狀態碼: {}", res.status())));
    }

    let html_content = res.text().await?;
    let document = Html::parse_document(&html_content);

    let raw = select_first(&document, &["#ads > a", "a.ads", "a[href*='down']"])
        .and_then(|el| el.value().attr("href"))
        .ok_or_else(|| DownloadError::Other("wnacg: 無法找到下載連結".to_string()))?;

    let href = if raw.starts_with("http") {
        raw.to_string()
    } else if raw.starts_with("//") {
        format!("https:{}", raw)
    } else {
        format!("https://www.wnacg.com{}", raw)
    };

    Ok(href)
}

/// Range 探測：對實際 ZIP 連結發 `Range: bytes=0-0`，驗證能否真的取到 bytes
/// 並回傳檔案總大小。比 HEAD 可靠（強制走真實下載路徑，有些 CDN 不支援 HEAD）。
/// - 206 Partial：從 `Content-Range: bytes 0-0/{total}` 解析總大小
/// - 200（伺服器忽略 Range）：退回 `Content-Length`
/// - 404/410：回 `NOT_FOUND`，代表連結預檢即失效
async fn probe_file_size(
    client: &reqwest::Client,
    file_url: &str,
) -> Result<i64, DownloadError> {
    let res = client
        .get(file_url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await?;
    let status = res.status();

    if matches!(status, reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err(DownloadError::NotFound);
    }

    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        if let Some(total) = res
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cr| cr.rsplit('/').next())
            .and_then(|s| s.trim().parse::<i64>().ok())
        {
            return Ok(total);
        }
    }

    if status.is_success() {
        // 伺服器忽略 Range（回 200），退回 Content-Length；拿不到則回 -1（未知）
        return Ok(res.content_length().map(|l| l as i64).unwrap_or(-1));
    }

    Err(DownloadError::Other(format!("探測失敗，狀態碼: {}", status)))
}

pub async fn fetch_payload_details(
    app_handle: &AppHandle,
    url: String,
) -> Result<ClipboardPayload, DownloadError> {
    tracing::info!("fetch_payload_details: {}", url);

    // 取 state 中的 client 執行 reqwest get 請求
    let state = app_handle.state::<AppState>();
    let client = &state.client;
    let res = client.get(&url).send().await?;

    if matches!(res.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err(DownloadError::NotFound);
    }
    if !res.status().is_success() {
        return Err(DownloadError::Other(format!("網絡請求失敗，狀態碼: {}", res.status())));
    }

    let html_content = res.text().await?;

    // 用 block 確保 Html（非 Send）在 await 前 drop
    let (title, image, download_page_href) = {
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
            .ok_or_else(|| DownloadError::Other("wnacg: 無法找到下載頁面連結".to_string()))?;

        let download_page_href = if download_page_href_raw.starts_with("http") {
            download_page_href_raw.to_string()
        } else if download_page_href_raw.starts_with("//") {
            format!("https:{}", download_page_href_raw)
        } else {
            format!("https://www.wnacg.com{}", download_page_href_raw)
        };

        (title, image, download_page_href)
    }; // document 在此 drop，之後才 await

    // 順帶抓實際 ZIP URL，快取進 DB 省掉下載時的額外請求；失敗不中斷
    let file_url = get_file_url(app_handle, &download_page_href)
        .await
        .unwrap_or_default();

    // Range 探測：驗證連結真的能下載並取得檔案大小
    let mut file_size: i64 = -1;
    let mut db_status = "idle".to_string();
    if file_url.is_empty() {
        tracing::warn!("fetch_payload_details: 無法預取 file_url，下載時將重新抓取");
    } else {
        match probe_file_size(client, &file_url).await {
            Ok(size) => file_size = size,
            Err(DownloadError::NotFound) => {
                // 預檢就確定 ZIP 連結已失效，直接標 not_found
                tracing::warn!("fetch_payload_details: ZIP 連結預檢 404/410: {}", file_url);
                db_status = "not_found".to_string();
            }
            Err(e) => {
                // 暫時性失敗，大小未知，仍以 idle 加入、下載時再試
                tracing::warn!("fetch_payload_details: 大小探測失敗（{}），標為未知", e);
            }
        }
    }

    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    Ok(ClipboardPayload {
        url,
        title,
        image,
        download_page_href,
        file_url,
        file_size,
        created_at,
        db_status,
    })
}

pub async fn download(
    client: &reqwest::Client,
    app_handle: &AppHandle,
    source_url: String, // 原始網頁網址 (用於進度事件辨識)
    file_url: String,   // 實際檔案下載網址
    save_path: PathBuf,
    cancelled: Arc<AtomicBool>,
    bandwidth_limit_bps: Arc<AtomicU64>, // 0 = 無限制
) -> Result<(), DownloadError> {
    let resp = client.get(&file_url).send().await?;

    if matches!(resp.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err(DownloadError::NotFound);
    }
    if !resp.status().is_success() {
        return Err(DownloadError::Other(format!(
            "下載失敗 status: {}, Url: {}",
            resp.status(),
            file_url
        )));
    }

    let total_size = resp.content_length().unwrap_or(0);

    // 串流寫檔包進 async block：任何失敗（含取消）統一在外層刪除殘檔，
    // 避免半截 .zip 留在下載目錄被 monitor 的存在檢查誤判
    let result: Result<(), DownloadError> = async {
        // tokio::fs 非阻塞寫檔：同步 I/O 會卡住 async runtime 的 worker thread，
        // 慢碟時拖累同 runtime 上的 BT stats / 直鏈下載 / IPC
        let mut file = tokio::fs::File::create(&save_path).await?;
        let mut downloaded: u64 = 0;
        let mut stream = resp.bytes_stream();
        let mut manager = DownloadManager::new();
        manager.start_download(total_size);

        let mut throttle_downloaded: u64 = 0;
        let mut throttle_start = std::time::Instant::now();
        let mut last_limit = bandwidth_limit_bps.load(Ordering::Relaxed);
        let mut last_emit = std::time::Instant::now();
        let emit_interval = std::time::Duration::from_millis(250);

        while let Some(chunk) = stream.next().await {
            if cancelled.load(Ordering::Relaxed) {
                return Err(DownloadError::Cancelled);
            }

            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            let current_limit = bandwidth_limit_bps.load(Ordering::Relaxed);
            if current_limit != last_limit {
                throttle_start = std::time::Instant::now();
                throttle_downloaded = 0;
                last_limit = current_limit;
            }
            throttle_downloaded += chunk.len() as u64;

            if current_limit > 0 {
                let expected = std::time::Duration::from_secs_f64(
                    throttle_downloaded as f64 / current_limit as f64,
                );
                let actual = throttle_start.elapsed();
                if expected > actual {
                    // 限速 sleep 切小段，期間仍能即時回應取消
                    let mut remaining = expected - actual;
                    let step = std::time::Duration::from_millis(250);
                    while remaining > std::time::Duration::ZERO {
                        if cancelled.load(Ordering::Relaxed) {
                            return Err(DownloadError::Cancelled);
                        }
                        let d = remaining.min(step);
                        tokio::time::sleep(d).await;
                        remaining -= d;
                    }
                }
            }

            // 節流：每 250ms 發一次進度事件；emit 失敗只記錄，不中斷下載
            if last_emit.elapsed() >= emit_interval {
                let metrics = manager.calculate_metrics(downloaded, total_size);
                if let Err(e) = app_handle.emit(
                    "download_progress",
                    DownloadProgress {
                        url: source_url.clone(),
                        progress: metrics.percentage,
                        speed_bytes_per_sec: metrics.speed_bytes_per_sec,
                        time_remaining_secs: metrics.time_remaining_secs,
                    },
                ) {
                    tracing::warn!("進度事件 emit 失敗: {}", e);
                }
                last_emit = std::time::Instant::now();
            }
        }

        file.flush().await?;

        // 下載完成後補發最終進度（確保前端顯示 100%）
        let metrics = manager.calculate_metrics(downloaded, total_size);
        if let Err(e) = app_handle.emit(
            "download_progress",
            DownloadProgress {
                url: source_url.clone(),
                progress: metrics.percentage,
                speed_bytes_per_sec: metrics.speed_bytes_per_sec,
                time_remaining_secs: metrics.time_remaining_secs,
            },
        ) {
            tracing::warn!("進度事件 emit 失敗: {}", e);
        }

        Ok(())
    }
    .await;

    if result.is_err() {
        let _ = std::fs::remove_file(&save_path);
    }
    result
}
