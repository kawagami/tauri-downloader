// src/settings.rs
// 統一 app 設定：後端會用到的設定都收在這（純 UI 偏好留前端 localStorage）。
// 存 app_data_dir/app_settings.json，獨立於 BT 引擎 manage — 引擎掛掉設定照常可讀寫。
//
// 三個下載分頁的「預設目錄 + 限速」共用 DownloadSettings：
// 目錄空字串 = 系統下載資料夾（utils::fs::resolve_dir），限速一律 bytes/s、0 = 不限。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Deserializer, Serialize};

use crate::torrent::settings::BtSettings;

/// 限速欄位反序列化：舊格式是 `null`（不限），新格式是 0。兩者都接受。
pub(crate) fn de_limit<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    Ok(Option::<u64>::deserialize(d)?.unwrap_or(0))
}

/// 下載分頁共用設定 — 預設目錄（空 = 系統下載資料夾）+ 限速 bytes/s（0 = 不限）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadSettings {
    pub default_dir: String,
    #[serde(deserialize_with = "de_limit")]
    pub limit_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// 剪貼簿監控開關（啟動時套用到 AppState.monitor_paused）
    pub monitor_clipboard: bool,
    /// 網站下載（wnacg 等站台爬取）
    pub web: DownloadSettings,
    /// 直鏈下載
    pub http: DownloadSettings,
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
            web: DownloadSettings::default(),
            http: DownloadSettings::default(),
            bt: BtSettings::default(),
            jin_roots: DEFAULT_JIN_ROOTS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// 舊版扁平欄位 → 新的分區結構。
/// `bandwidth_limit_kbps`（KB/s）→ `web.limit_bps`（bytes/s）、`http_default_dir` → `http.default_dir`。
fn migrate_legacy(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    if let Some(kbps) = obj.remove("bandwidth_limit_kbps").and_then(|v| v.as_u64()) {
        if kbps > 0 && !obj.contains_key("web") {
            obj.insert(
                "web".into(),
                serde_json::json!({ "default_dir": "", "limit_bps": kbps * 1024 }),
            );
        }
    }

    if let Some(dir) = obj
        .remove("http_default_dir")
        .and_then(|v| v.as_str().map(str::to_owned))
    {
        if !dir.is_empty() && !obj.contains_key("http") {
            obj.insert(
                "http".into(),
                serde_json::json!({ "default_dir": dir, "limit_bps": 0 }),
            );
        }
    }
}

pub struct SettingsState {
    inner: Mutex<AppSettings>,
    path: PathBuf,
}

impl SettingsState {
    /// 載入 app_settings.json（含舊欄位遷移）；檔案不存在時從舊 bt_settings.json 遷移 bt 區塊。
    /// 解析失敗不靜默吞掉：壞檔改名保留 + 記日誌（見 utils::jsonfile），再走預設值。
    fn read_file(path: &Path) -> Option<AppSettings> {
        let mut value: serde_json::Value = crate::utils::jsonfile::load_json(path)?;
        migrate_legacy(&mut value);
        match serde_json::from_value::<AppSettings>(value) {
            Ok(s) => Some(s),
            // JSON 本身合法但欄位對不上（型別改過、手改打錯）—— 同樣要留副本
            Err(e) => {
                crate::utils::jsonfile::backup_bad_file(path, &e.to_string());
                None
            }
        }
    }

    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join("app_settings.json");
        let settings = Self::read_file(&path).unwrap_or_else(|| AppSettings {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_flat_legacy_fields() {
        let mut v = serde_json::json!({
            "monitor_clipboard": false,
            "bandwidth_limit_kbps": 512,
            "http_default_dir": "D:\\dl",
        });
        migrate_legacy(&mut v);
        let s: AppSettings = serde_json::from_value(v).unwrap();
        assert!(!s.monitor_clipboard);
        assert_eq!(s.web.limit_bps, 512 * 1024);
        assert_eq!(s.http.default_dir, "D:\\dl");
        assert_eq!(s.http.limit_bps, 0);
    }

    #[test]
    fn accepts_null_limits_from_old_bt_block() {
        let v = serde_json::json!({
            "bt": { "default_download_dir": "E:\\bt", "listen_port": null,
                    "upload_limit_bps": null, "download_limit_bps": 1024 }
        });
        let s: AppSettings = serde_json::from_value(v).unwrap();
        assert_eq!(s.bt.default_dir, "E:\\bt");
        assert_eq!(s.bt.upload_limit_bps, 0);
        assert_eq!(s.bt.download_limit_bps, 1024);
    }
}
