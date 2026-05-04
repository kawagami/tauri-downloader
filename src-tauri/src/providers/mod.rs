use std::fmt;
use std::path::PathBuf;
use std::sync::{atomic::{AtomicBool, AtomicU64}, Arc};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

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

impl Site {
    /// 根據 host 辨識屬於哪個網站
    pub fn from_url(url: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(url).map_err(|_| "無效的 URL 格式".to_string())?;
        match parsed.host_str() {
            Some(h) if h == "wnacg.com" || h.ends_with(".wnacg.com") => Ok(Site::Wnacg),
            Some(h) if h == "nhentai.net" || h.ends_with(".nhentai.net") => Ok(Site::NHentai),
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

    pub async fn download(
        &self,
        client: &reqwest::Client,
        app_handle: &AppHandle,
        source_url: String,
        cached_file_url: String,
        save_path: PathBuf,
        cancelled: Arc<AtomicBool>,
        bandwidth_limit_bps: Arc<AtomicU64>,
    ) -> Result<(), String> {
        match self {
            Site::Wnacg => {
                let file_url = if !cached_file_url.is_empty() {
                    cached_file_url
                } else {
                    wnacg::get_file_url(app_handle, &source_url)
                        .await
                        .map_err(|e| e.to_string())?
                };
                wnacg::download(client, app_handle, source_url, file_url, save_path, cancelled, bandwidth_limit_bps)
                    .await
            }
            Site::NHentai => Err("NHentai 下載尚未實作".to_string()),
        }
    }
}
