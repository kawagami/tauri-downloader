use std::fmt;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::error::DownloadError;

pub mod nhentai;
pub mod wnacg;

pub enum Site {
    Wnacg,
    NHentai,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipboardPayload {
    pub url: String,
    pub title: String,
    pub image: String,
    pub download_page_href: String,
    pub file_url: String,
    pub file_size: i64, // 檔案位元組數，-1 = 未知（探測失敗或站台未回報）
    pub created_at: i64,
    pub db_status: String,
}

#[derive(Serialize, Clone)]
pub struct DownloadProgress {
    pub url: String,
    pub progress: f64,
    pub speed_bytes_per_sec: f64,
    pub time_remaining_secs: f64,
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Site::Wnacg => write!(f, "wnacg"),
            Site::NHentai => write!(f, "nhentai"),
        }
    }
}

/// host 是否屬於這個站（裸域或任何子網域）。
/// `from_url` 與各 provider 的 `validate` 共用同一份規則 —— 兩邊各寫一套的話
/// 會出現「認得出站台、驗證卻拒絕」的靜默失敗（例：沒有 www 的 wnacg.com 連結）。
pub(crate) fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

impl Site {
    /// 根據 host 辨識屬於哪個網站
    pub fn from_url(url: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(url).map_err(|_| "無效的 URL 格式".to_string())?;
        match parsed.host_str() {
            Some(h) if host_matches(h, wnacg::DOMAIN) => Ok(Site::Wnacg),
            Some(h) if host_matches(h, nhentai::DOMAIN) => Ok(Site::NHentai),
            _ => Err("不支援的網站域名".to_string()),
        }
    }

    /// 驗證是否該站以一部作品為單位的網址路徑
    pub fn validate(&self, url: &str) -> Result<String, String> {
        match self {
            Site::Wnacg => wnacg::validate(url),
            Site::NHentai => nhentai::validate(url),
        }
    }

    /// 解析下載頁面 取得 ClipboardPayload 所需的資料
    pub async fn fetch_details(
        &self,
        handle: &AppHandle,
        url: &str,
    ) -> Result<ClipboardPayload, String> {
        match self {
            Site::Wnacg => wnacg::fetch_payload_details(handle, url.to_string())
                .await
                .map_err(|e| e.to_string()),
            Site::NHentai => Err("NHentai fetch 尚未實作".to_string()),
        }
    }

    /// 從作品頁解析出真正的檔案下載連結。
    /// 實際下載交給共用引擎（crate::dl），provider 只負責「連結在哪」。
    pub async fn resolve_file_url(
        &self,
        app_handle: &AppHandle,
        source_url: &str,
    ) -> Result<String, DownloadError> {
        match self {
            Site::Wnacg => wnacg::get_file_url(app_handle, source_url).await,
            Site::NHentai => Err(DownloadError::Other("NHentai 下載尚未實作".to_string())),
        }
    }
}
