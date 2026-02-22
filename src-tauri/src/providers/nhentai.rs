use crate::state::AppState;

use regex::Regex;
use scraper::Selector;
use tauri::{AppHandle, Manager};
use url::Url;

/// 驗證 wnacg URL 並回傳規範化的 URL 字串
pub fn validate(content: &str) -> Result<String, String> {
    todo!()
}

/// 輔助用函數
pub async fn get_file_url(
    app_handle: &AppHandle,
    url: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    todo!()
}
