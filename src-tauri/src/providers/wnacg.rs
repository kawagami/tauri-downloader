use crate::state::AppState;

use regex::Regex;
use scraper::Selector;
use tauri::{AppHandle, Manager};
use url::Url;

/// 驗證 wnacg URL 並回傳規範化的 URL 字串
pub fn is_valid_wnacg_url(content: &str) -> Result<String, String> {
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
