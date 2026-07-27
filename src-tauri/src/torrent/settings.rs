use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::settings::de_limit;

/// BT 專用設定 — 現為 AppSettings 的 bt 區塊（存 app_settings.json），
/// 舊 bt_settings.json 僅在首次啟動時遷移用（settings.rs）。
/// 目錄空 = 系統下載資料夾、限速 bytes/s 且 0 = 不限，與 web/http 分頁同語意。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BtSettings {
    /// 新任務預設下載目錄（空 = 系統下載資料夾）
    #[serde(alias = "default_download_dir")]
    pub default_dir: String,
    /// 固定 BT 監聽 port。None = librqbit 預設 range 4240..4260
    pub listen_port: Option<u16>,
    /// 全域上傳限速 bytes/s，0 = 不限
    #[serde(deserialize_with = "de_limit")]
    pub upload_limit_bps: u64,
    /// 全域下載限速 bytes/s，0 = 不限
    #[serde(deserialize_with = "de_limit")]
    pub download_limit_bps: u64,
}

impl BtSettings {
    pub fn load(path: &Path) -> BtSettings {
        crate::utils::jsonfile::load_json(path).unwrap_or_default()
    }
}
