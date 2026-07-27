// wnacg provider —— 只負責「爬」：驗證網址、抓元資料、解析出真正的檔案連結。
// 實際下載走共用引擎 crate::dl（與直鏈下載同一套），這裡不再有自己的串流迴圈。

use crate::{error::DownloadError, providers::ClipboardPayload, state::AppState};

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use url::Url;

/// 站台主網域（`Site::from_url` 的辨識與這裡的驗證共用）
pub const DOMAIN: &str = "wnacg.com";
/// 規範化用的主機名：裸域與各子網域都收斂到這個，同一部作品才不會因為
/// 有沒有 www（或帶 query）而在 DB 裡變成好幾筆。
const CANONICAL_HOST: &str = "www.wnacg.com";

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

    // host 規則與 Site::from_url 共用：裸域與子網域都收
    let host = parsed_url.host_str().unwrap_or_default();
    if !crate::providers::host_matches(host, DOMAIN) {
        return Err(format!("域名必須為 {}（或其子網域）", DOMAIN));
    }

    // 3. 使用 Regex 驗證 Path 並提取 ID (兼顧檢查與提取)
    let re = RE_VALIDATE.get_or_init(|| Regex::new(r"^/photos-index-aid-(\d+)\.html$").unwrap());

    let id = re
        .captures(parsed_url.path())
        .map(|c| c[1].to_string())
        .ok_or_else(|| "路徑格式錯誤，應為 /photos-index-aid-{ID}.html".to_string())?;

    // 由 ID 重建規範化 URL：丟掉 query/fragment、主機統一，
    // 同一部作品的各種寫法都收斂成同一個 DB 主鍵
    Ok(format!(
        "https://{}/photos-index-aid-{}.html",
        CANONICAL_HOST, id
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str = "https://www.wnacg.com/photos-index-aid-123.html";

    /// 各種寫法都要收斂成同一個規範化 URL —— DB 主鍵是 url，
    /// 不收斂的話同一部作品會變成好幾筆
    #[test]
    fn canonicalizes_host_and_query() {
        for input in [
            CANONICAL,
            "https://wnacg.com/photos-index-aid-123.html",
            "https://m.wnacg.com/photos-index-aid-123.html",
            "https://www.wnacg.com/photos-index-aid-123.html?from=list#top",
        ] {
            assert_eq!(validate(input).as_deref(), Ok(CANONICAL), "input={input}");
        }
    }

    /// from_url 認得的 host，validate 就不能拒絕（以前無 www 會靜默失敗）
    #[test]
    fn accepts_every_host_from_url_accepts() {
        let bare = "https://wnacg.com/photos-index-aid-123.html";
        assert!(crate::providers::Site::from_url(bare).is_ok());
        assert!(validate(bare).is_ok());
    }

    #[test]
    fn rejects_wrong_scheme_host_and_path() {
        assert!(validate("http://www.wnacg.com/photos-index-aid-123.html").is_err());
        assert!(validate("https://evil-wnacg.com/photos-index-aid-123.html").is_err());
        assert!(validate("https://www.wnacg.com/photos-slist-aid-123.html").is_err());
    }
}
