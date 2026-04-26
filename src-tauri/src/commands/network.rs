use crate::{providers::Site, state::AppState, utils};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn download_with_progress(
    url: String,
    title: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    // 每次下載前重設取消旗標
    state.download_cancelled.store(false, Ordering::Relaxed);

    // 1. 準備路徑
    let download_dir = app_handle
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&download_dir).map_err(|e| e.to_string())?;
    let save_path = utils::fs::get_unique_save_path(download_dir, &title);

    // 2. 自動識別網站並獲取檔案 URL (核心邏輯封裝在 Site 裡了)
    let site = Site::from_url(&url)?;
    let bandwidth_limit_bps = state.bandwidth_limit_bps.load(Ordering::Relaxed);

    // 3. 下載
    site.download(
        &state.client,
        &app_handle,
        url,
        save_path.clone(),
        state.download_cancelled.clone(),
        bandwidth_limit_bps,
    )
    .await?;

    Ok(save_path.to_string_lossy().to_string())
}
