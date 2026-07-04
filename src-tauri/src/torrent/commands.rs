use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use librqbit::{AddTorrent, AddTorrentOptions, Magnet};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

use super::settings::BtSettings;
use super::state::{BtEngine, PendingAdd};

/// Windows 安全的資料夾名：去非法字元、尾端點/空格、保留裝置名，長度上限 120 bytes
pub(crate) fn sanitize_folder_name(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if (c as u32) < 0x20 => ' ',
            c => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    while cleaned.ends_with('.') || cleaned.ends_with(' ') {
        cleaned.pop();
    }
    if cleaned.len() > 120 {
        let mut end = 120;
        while !cleaned.is_char_boundary(end) {
            end -= 1;
        }
        cleaned.truncate(end);
    }

    let upper = cleaned.to_ascii_uppercase();
    let base = upper.split('.').next().unwrap_or("");
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&base) {
        cleaned.push('_');
    }
    cleaned
}

/// add_magnet 核心：command 與剪貼簿監控共用。同步驗證後 spawn 背景 add
/// 立即返回（librqbit 的 api_add_torrent 對 magnet 會等 metadata 抓完，
/// 冷門種子可能等不到，不能直接 await）。
pub async fn add_magnet_inner(
    app: AppHandle,
    magnet: String,
    out_dir: Option<String>,
    paused: bool,
) -> Result<Value, String> {
    let state = app.state::<BtEngine>().get()?;
    let magnet = magnet.trim().to_string();
    if !magnet.starts_with("magnet:") {
        return Err("無效的磁力連結".to_string());
    }
    let parsed = Magnet::parse(&magnet).map_err(|_| "無效的磁力連結".to_string())?;
    let hash = parsed.as_id20().map(|h| h.as_string());

    // 重複 infohash → 回報既有任務，不重複加
    if let Some(hash) = &hash {
        let list = state.api.api_torrent_list();
        if let Some(existing) = list
            .torrents
            .iter()
            .find(|t| t.info_hash.eq_ignore_ascii_case(hash))
        {
            return Ok(json!({ "already_exists": true, "id": existing.id }));
        }
        if state.pending.lock().unwrap().values().any(|p| {
            p.info_hash
                .as_deref()
                .is_some_and(|h| h.eq_ignore_ascii_case(hash))
        }) {
            return Ok(json!({ "already_exists": true, "id": null }));
        }
    }

    // 未指定目錄時用 BT 設定的預設下載目錄
    let out_dir = out_dir
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| state.settings.lock().unwrap().default_download_dir.clone());

    // 每個任務放同名子資料夾：magnet dn= 名稱 sanitize，無 dn 用 infohash。
    // librqbit 對明確給的 output_folder 不再套自己的子資料夾，不會雙層。
    let folder_name = parsed
        .name
        .as_deref()
        .map(sanitize_folder_name)
        .filter(|s| !s.is_empty())
        .or_else(|| hash.clone());
    let output_folder = match folder_name {
        Some(name) => PathBuf::from(&out_dir)
            .join(name)
            .to_string_lossy()
            .into_owned(),
        None => out_dir,
    };

    let opts = AddTorrentOptions {
        output_folder: Some(output_folder),
        overwrite: true,
        // paused = 只加入清單不開跑；metadata 仍會抓（需要檔案清單），但不下載內容
        paused,
        ..Default::default()
    };

    let dn_name = parsed.name.clone();
    let key = state.pending_seq.fetch_add(1, Ordering::Relaxed);
    state.pending.lock().unwrap().insert(
        key,
        PendingAdd {
            name: parsed.name.clone(),
            info_hash: hash,
            added_at: Instant::now(),
            error: None,
            handle: None,
        },
    );

    let ts = state.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let result = ts
            .api
            .api_add_torrent(AddTorrent::from_url(&magnet), Some(opts))
            .await;
        let mut pending = ts.pending.lock().unwrap();
        match result {
            // 任務已進正式清單，撤掉 placeholder
            Ok(_) => {
                pending.remove(&key);
            }
            Err(e) => {
                if let Some(p) = pending.get_mut(&key) {
                    p.error = Some(e.to_string());
                }
            }
        }
    });
    // 背景 task 可能已跑完並移除 entry；只有還在時才存 handle
    //（drop JoinHandle 不會 abort task）
    if let Some(p) = state.pending.lock().unwrap().get_mut(&key) {
        p.handle = Some(handle);
    }

    Ok(json!({ "pending": true, "key": key, "name": dn_name }))
}

#[tauri::command]
pub async fn add_magnet(
    app: AppHandle,
    magnet: String,
    out_dir: Option<String>,
    paused: Option<bool>,
) -> Result<Value, String> {
    add_magnet_inner(app, magnet, out_dir, paused.unwrap_or(false)).await
}

/// 取消抓取中 / 移除加入失敗的 pending 項
#[tauri::command]
pub fn remove_pending(state: State<'_, BtEngine>, key: u64) -> Result<(), String> {
    let ts = state.get()?;
    if let Some(p) = ts.pending.lock().unwrap().remove(&key) {
        if let Some(handle) = p.handle {
            handle.abort();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_torrents(state: State<'_, BtEngine>) -> Result<Value, String> {
    let ts = state.get()?;
    serde_json::to_value(ts.api.api_torrent_list()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn torrent_details(state: State<'_, BtEngine>, id: usize) -> Result<Value, String> {
    let ts = state.get()?;
    let details = ts
        .api
        .api_torrent_details(id.into())
        .map_err(|e| e.to_string())?;
    serde_json::to_value(details).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_torrent(state: State<'_, BtEngine>, id: usize) -> Result<(), String> {
    state
        .get()?
        .api
        .api_torrent_action_pause(id.into())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_torrent(state: State<'_, BtEngine>, id: usize) -> Result<(), String> {
    state
        .get()?
        .api
        .api_torrent_action_start(id.into())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_torrent(
    state: State<'_, BtEngine>,
    id: usize,
    delete_files: bool,
) -> Result<(), String> {
    let ts = state.get()?;
    let res = if delete_files {
        ts.api.api_torrent_action_delete(id.into()).await
    } else {
        ts.api.api_torrent_action_forget(id.into()).await
    };
    res.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_bt_settings(state: State<'_, BtEngine>) -> Result<BtSettings, String> {
    let ts = state.get()?;
    let settings = ts.settings.lock().unwrap().clone();
    Ok(settings)
}

/// 存 BT 設定。session 層設定（port、限速）重啟 app 後生效；
/// default_download_dir 對新任務即時生效。
#[tauri::command]
pub fn save_bt_settings(state: State<'_, BtEngine>, settings: BtSettings) -> Result<(), String> {
    let ts = state.get()?;
    settings
        .save(&ts.settings_path)
        .map_err(|e| e.to_string())?;
    *ts.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
pub fn get_bt_engine_status(state: State<'_, BtEngine>) -> Value {
    state.status()
}

/// 引擎啟動失敗後（如 port 衝突）手動重試
#[tauri::command]
pub fn retry_bt_init(app: AppHandle) {
    super::state::spawn_init(app);
}

#[cfg(test)]
mod tests {
    use super::sanitize_folder_name;

    #[test]
    fn strips_illegal_chars() {
        assert_eq!(
            sanitize_folder_name("Ubuntu 24.04: <amd64>?"),
            "Ubuntu 24.04 amd64"
        );
        assert_eq!(sanitize_folder_name(r#"a/b\c|d*e"f"#), "a b c d e f");
    }

    #[test]
    fn trims_trailing_dots_and_spaces() {
        assert_eq!(sanitize_folder_name("name... "), "name");
    }

    #[test]
    fn escapes_reserved_device_names() {
        assert_eq!(sanitize_folder_name("CON"), "CON_");
        assert_eq!(sanitize_folder_name("con.iso"), "con.iso_");
        assert_eq!(sanitize_folder_name("console"), "console");
    }

    #[test]
    fn empty_when_nothing_valid() {
        assert_eq!(sanitize_folder_name("???"), "");
        assert_eq!(sanitize_folder_name(""), "");
    }

    #[test]
    fn caps_length_at_char_boundary() {
        let long = "字".repeat(100); // 300 bytes
        let out = sanitize_folder_name(&long);
        assert!(out.len() <= 120);
        assert_eq!(out, "字".repeat(40));
    }
}
