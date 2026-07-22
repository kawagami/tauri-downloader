// src/settings.rs
// 統一 app 設定：後端會用到的設定都收在這（純 UI 偏好留前端 localStorage）。
// 存 app_data_dir/app_settings.json，獨立於 BT 引擎 manage — 引擎掛掉設定照常可讀寫。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::torrent::settings::BtSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// 剪貼簿監控開關（啟動時套用到 AppState.monitor_paused）
    pub monitor_clipboard: bool,
    /// 網站下載頻寬限制 KB/s，0 = 不限（啟動時套用到 AppState.bandwidth_limit_bps）
    pub bandwidth_limit_kbps: u64,
    /// 直鏈下載預設目錄，空 = 系統下載資料夾
    pub http_default_dir: String,
    /// BT 設定（port/限速重啟生效，預設目錄即時生效）
    pub bt: BtSettings,
    /// 工作需求遊戲設定分頁掃描的根目錄（code.*.php 所在）
    pub jin_roots: Vec<String>,
}

/// jin 分頁預設掃這兩個根目錄（compose + k8s overlays）
const DEFAULT_JIN_ROOTS: [&str; 2] = [
    r"\\wsl.localhost\Ubuntu-Project\home\kawa\job\gameriver\jinbaba\compose\configs\sites",
    r"\\wsl.localhost\Ubuntu-Project\home\kawa\another-job-env\gameriver\jinbaba\compose-k8s\kustomize\overlays",
];

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            monitor_clipboard: true,
            bandwidth_limit_kbps: 0,
            http_default_dir: String::new(),
            bt: BtSettings::default(),
            jin_roots: DEFAULT_JIN_ROOTS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

pub struct SettingsState {
    inner: Mutex<AppSettings>,
    path: PathBuf,
}

impl SettingsState {
    /// 載入 app_settings.json；檔案不存在時從舊 bt_settings.json 遷移 bt 區塊
    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join("app_settings.json");
        let settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| AppSettings {
                bt: BtSettings::load(&app_data_dir.join("bt_settings.json")),
                ..Default::default()
            });
        Self {
            inner: Mutex::new(settings),
            path,
        }
    }

    pub fn get(&self) -> AppSettings {
        self.inner.lock().unwrap().clone()
    }

    pub fn save(&self, settings: AppSettings) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, serde_json::to_string_pretty(&settings)?)?;
        *self.inner.lock().unwrap() = settings;
        Ok(())
    }
}
