// src/error.rs

use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::fmt;

/// 下載流程的結構化錯誤。取代散落各處的魔字串（"NOT_FOUND"、"下載已取消"），
/// 經 IPC 序列化為 `{ code, message }`，前端比對 code 而非錯誤訊息子字串。
#[derive(Debug)]
pub enum DownloadError {
    /// HTTP 404/410 — 永久性失效，不可重試
    NotFound,
    /// 使用者取消下載
    Cancelled,
    /// 其他暫時性錯誤（網路、IO、解析），可重試
    Other(String),
}

impl DownloadError {
    pub fn code(&self) -> &'static str {
        match self {
            DownloadError::NotFound => "NOT_FOUND",
            DownloadError::Cancelled => "CANCELLED",
            DownloadError::Other(_) => "OTHER",
        }
    }
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::NotFound => write!(f, "找不到檔案 (404/410)"),
            DownloadError::Cancelled => write!(f, "下載已取消"),
            DownloadError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        DownloadError::Other(e.to_string())
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Other(e.to_string())
    }
}

impl Serialize for DownloadError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut st = serializer.serialize_struct("DownloadError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}
