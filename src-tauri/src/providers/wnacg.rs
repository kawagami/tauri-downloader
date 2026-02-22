use crate::{providers::ClipboardPayload, state::AppState};

use futures_util::StreamExt;
use regex::Regex;
use scraper::Selector;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

#[derive(Serialize, Clone)]
struct DownloadProgress {
    url: String,
    progress: f64,
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
    let re = Regex::new(r"^/photos-index-aid-(\d+)\.html$").unwrap();

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

    // 檢查響應狀態
    if !res.status().is_success() {
        return Err(format!("網絡請求失敗，狀態碼: {}", res.status()).into());
    }

    // 實際應用中，您會解析 HTML 內容來獲取 title 和 image URL/ID
    let html_content = res.text().await?;
    let document = scraper::Html::parse_document(&html_content);

    // 1. 定義新的選擇器 (針對 a.ads)
    let download_selector = Selector::parse("a.ads").unwrap();

    // 2. 執行選取與提取
    let download_page_href_raw = document
        .select(&download_selector)
        .next()
        .and_then(|element| element.value().attr("href"))
        .map(|href| {
            if href.starts_with("http") {
                // 如果已經有 http 或 https，直接轉 String
                href.to_string()
            } else if href.starts_with("//") {
                // 如果是 // 開頭，補上 https:
                format!("https:{}", href)
            } else {
                href.to_string()
            }
        })
        .unwrap_or_else(|| {
            eprintln!("Rust Monitor: 無法找到下載連結 (a.ads)。");
            "".to_string()
        });

    // 返回結果
    Ok(download_page_href_raw)
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

    // 檢查響應狀態
    if !res.status().is_success() {
        return Err(format!("網絡請求失敗，狀態碼: {}", res.status()).into());
    }

    // 實際應用中，您會解析 HTML 內容來獲取 title 和 image URL/ID
    let html_content = res.text().await?;
    let document = scraper::Html::parse_document(&html_content);

    // --- 1. 抓取 Title (H2 標籤) ---
    let title_selector = Selector::parse("#bodywrap > h2").unwrap();
    let title = document
        .select(&title_selector)
        .next()
        .map(|element| element.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| "無法找到指定標題".to_string());

    // --- 2. 抓取 Image Path (String) ---
    // 圖片的 CSS 選擇器，我們需要提取 img 標籤的 src 屬性
    let image_selector = Selector::parse(
        "#bodywrap > div.grid > div > ul > li:nth-child(1) > div.pic_box.tb > a > img",
    )
    .unwrap();

    let image = document
        .select(&image_selector)
        .next()
        // 嘗試從 img 元素中提取 'src' 屬性
        .and_then(|element| element.value().attr("src"))
        // 將 &str 轉換為 String
        .map(|src| {
            // 注意：圖片路徑可能是一個相對路徑（例如 /img/abc.jpg）
            // 為了完整性，您可以選擇在這裡將其轉換為絕對 URL。
            // 這裡僅簡單地將其轉換為 String
            let path = src.to_string();
            println!("Rust Monitor: 成功提取圖片路徑: {}", path);
            path
        })
        // 如果找不到元素或 src 屬性，則使用預設值
        .unwrap_or_else(|| {
            eprintln!("Rust Monitor: 無法找到圖片元素或 src 屬性。");
            "placeholder.png".to_string() // 使用一個預設或錯誤圖片路徑
        });

    // --- 3. ✨ 取得下載頁面的 href ---
    let download_page_href_selector = Selector::parse("#ads > a").unwrap();
    let download_page_href_raw = document
        .select(&download_page_href_selector)
        .next()
        .and_then(|element| element.value().attr("href"))
        .map(|href| href.to_string())
        .unwrap_or_else(|| {
            eprintln!("Rust Monitor: 無法找到下載頁面的 href。");
            "".to_string() // 找不到時使用空字串
        });

    // 處理下載頁面路徑
    let download_page_href = format!("https://www.wnacg.com{}", download_page_href_raw);

    Ok(ClipboardPayload {
        url,
        title,
        image, // 使用提取到的圖片路徑 (String)
        download_page_href,
    })
}

pub async fn download(
    client: &reqwest::Client,
    app_handle: &AppHandle,
    source_url: String, // 原始網頁網址 (用於進度事件辨識)
    file_url: String,   // 實際檔案下載網址
    save_path: PathBuf,
) -> Result<(), String> {
    let resp = client
        .get(&file_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!(
            "下載失敗 status: {}, Url: {}",
            resp.status(),
            file_url
        ));
    }

    let total_size = resp.content_length().ok_or("無法取得檔案大小")?;
    let mut file = File::create(&save_path).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        let progress = (downloaded as f64 / total_size as f64) * 100.0;
        app_handle
            .emit(
                "download_progress",
                DownloadProgress {
                    url: source_url.clone(),
                    progress,
                },
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
