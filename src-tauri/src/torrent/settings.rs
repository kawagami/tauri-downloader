use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// BT 專用設定，獨立於主 app 狀態，存 app_data_dir/bt_settings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BtSettings {
    /// 新任務預設下載目錄
    pub default_download_dir: String,
    /// 固定 BT 監聽 port。None = librqbit 預設 range 4240..4260
    pub listen_port: Option<u16>,
    /// 全域上傳限速 bytes/sec。None = 不限
    pub upload_limit_bps: Option<u32>,
    /// 全域下載限速 bytes/sec。None = 不限
    pub download_limit_bps: Option<u32>,
}

impl Default for BtSettings {
    fn default() -> Self {
        Self {
            default_download_dir: dirs::download_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .to_string_lossy()
                .into_owned(),
            listen_port: None,
            upload_limit_bps: None,
            download_limit_bps: None,
        }
    }
}

impl BtSettings {
    pub fn load(path: &Path) -> BtSettings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
