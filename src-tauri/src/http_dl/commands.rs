use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::State;

use super::manager::{filename_from_url, HttpManager, HttpStatus};

/// 極端情況的後備檔名時間戳(URL 取不出檔名時)。
fn fallback_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 新增 HTTP 直鏈下載並立即開跑。檔名先取 URL path 最後一段,首次回應的
/// Content-Disposition 會再覆蓋。out_dir 空 = 系統下載資料夾。
#[tauri::command]
pub fn add_http_download(
    state: State<'_, Arc<HttpManager>>,
    url: String,
    out_dir: Option<String>,
) -> Result<Value, String> {
    let url = url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("無效的下載連結".to_string());
    }
    let parsed = reqwest::Url::parse(&url).map_err(|_| "無效的下載連結".to_string())?;

    // 同一條 URL 還在跑/暫停/失敗中 → 不重複加。
    if let Some(existing) = state.find_active_by_url(&url) {
        return Ok(json!({ "already_exists": true, "id": existing.id }));
    }

    let dest_dir = out_dir
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(dirs::download_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    let file_name =
        filename_from_url(&parsed).unwrap_or_else(|| format!("download-{}", fallback_timestamp()));

    let task = state.add(url, dest_dir, file_name);
    let id = task.id;
    state.spawn_run(task);
    Ok(json!({ "id": id }))
}

#[tauri::command]
pub fn pause_http_download(state: State<'_, Arc<HttpManager>>, id: u64) -> Result<(), String> {
    state.pause(id);
    Ok(())
}

/// 續跑暫停或失敗的任務(連結未過期時 Range 續傳)。
#[tauri::command]
pub fn resume_http_download(state: State<'_, Arc<HttpManager>>, id: u64) -> Result<(), String> {
    let task = state.find(id).ok_or("任務不存在")?;
    if task.status() == HttpStatus::Running {
        return Ok(());
    }
    state.spawn_run(task);
    Ok(())
}

/// token 過期後使用者貼新連結(指向同一檔案),換掉 URL 後接著續傳。
#[tauri::command]
pub fn update_http_url(
    state: State<'_, Arc<HttpManager>>,
    id: u64,
    url: String,
) -> Result<(), String> {
    let url = url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("無效的下載連結".to_string());
    }
    reqwest::Url::parse(&url).map_err(|_| "無效的下載連結".to_string())?;
    let task = state.find(id).ok_or("任務不存在")?;
    *task.url.lock().unwrap() = url;
    if task.status() != HttpStatus::Running {
        state.spawn_run(task);
    }
    Ok(())
}

#[tauri::command]
pub fn delete_http_download(
    state: State<'_, Arc<HttpManager>>,
    id: u64,
    delete_files: bool,
) -> Result<(), String> {
    state.remove(id, delete_files);
    Ok(())
}
