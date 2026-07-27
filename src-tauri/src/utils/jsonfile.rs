// 設定/狀態 JSON 檔的共用載入 —
// 以前三處（app_settings.json、bt_settings.json、http_tasks.json）都是
// `.ok().and_then(..).unwrap_or_default()`：一個欄位型別不合就整份靜默回預設，
// 直鏈任務甚至會整批消失，而且沒有任何訊息。這裡統一成「壞檔留副本 + 記日誌」。

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

/// 把解析失敗的檔案改名保留（`{name}.bad-{unix_secs}`），讓使用者/日誌還救得回來。
/// 改名失敗只記日誌 —— 呼叫端無論如何都要能繼續走預設值。
pub fn backup_bad_file(path: &Path, reason: &str) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".bad-{stamp}"));
    let backup: PathBuf = path.with_file_name(name);

    tracing::error!("{:?} 解析失敗（{}），改用預設值", path, reason);
    match std::fs::rename(path, &backup) {
        Ok(()) => tracing::error!("壞檔已保留為 {:?}", backup),
        Err(e) => tracing::error!("保留壞檔失敗 {:?}: {}", backup, e),
    }
}

/// 原子寫入 JSON。
///
/// `std::fs::write` 是「先 truncate 再寫」—— 進行中的直鏈下載每幾秒就寫一次任務檔，
/// 這時候斷電/被砍掉留下的就是半截 JSON，下次啟動整份被判定壞檔、任務全滅。
/// 先寫 `.tmp` 再 rename（同磁碟的 rename 是原子的）就不會有半截狀態：
/// 要嘛是舊的完整內容，要嘛是新的完整內容。
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    std::fs::write(&tmp, json)?;
    // Windows 的 rename 會覆蓋既有檔案（MOVEFILE_REPLACE_EXISTING）
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// 讀 JSON 檔。檔案不存在回 None（正常首次啟動，不記日誌）；
/// 讀取或解析失敗會記日誌並把壞檔改名保留，再回 None。
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::error!("讀取 {:?} 失敗: {}", path, e);
            return None;
        }
    };
    match serde_json::from_str::<T>(&raw) {
        Ok(v) => Some(v),
        Err(e) => {
            backup_bad_file(path, &e.to_string());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_none_without_backup() {
        let dir = std::env::temp_dir().join(format!("jsonfile-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nope.json");
        assert!(load_json::<serde_json::Value>(&path).is_none());
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn broken_file_is_moved_aside() {
        let dir = std::env::temp_dir().join(format!("jsonfile-broken-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        std::fs::write(&path, "{ not json").unwrap();

        assert!(load_json::<serde_json::Value>(&path).is_none());
        // 原檔不留在原位，但內容有被保留成 .bad-*
        assert!(!path.exists());
        let backups: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("state.json.bad-"))
            .collect();
        assert_eq!(backups.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_write_roundtrips_and_leaves_no_tmp() {
        let dir = std::env::temp_dir().join(format!("jsonfile-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");

        write_json_atomic(&path, &serde_json::json!({ "a": 1 })).unwrap();
        // 覆寫既有檔也要成功（Windows rename 會 replace）
        write_json_atomic(&path, &serde_json::json!({ "a": 2 })).unwrap();

        let v: serde_json::Value = load_json(&path).unwrap();
        assert_eq!(v["a"], 2);
        assert!(!dir.join("state.json.tmp").exists(), "暫存檔應該已被 rename 掉");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn valid_file_parses() {
        let dir = std::env::temp_dir().join(format!("jsonfile-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        std::fs::write(&path, r#"{"a":1}"#).unwrap();
        let v: serde_json::Value = load_json(&path).unwrap();
        assert_eq!(v["a"], 1);
        assert!(path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
