use crate::{
    error::DownloadError, providers::Site, settings::SettingsState, state::AppState, utils,
};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn download_with_progress(
    url: String,
    title: String,
    file_url: String,
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    app_handle: AppHandle,
) -> Result<String, DownloadError> {
    state.download_cancelled.store(false, Ordering::Relaxed);

    // 網站下載預設目錄（空 = 系統下載資料夾），與 BT/直鏈同一套語意
    let download_dir = utils::fs::resolve_dir(&settings.get().web.default_dir);
    std::fs::create_dir_all(&download_dir)?;
    let save_path = utils::fs::get_unique_save_path(download_dir, &title);

    let site = Site::from_url(&url).map_err(DownloadError::Other)?;
    site.download(
        &state.client,
        &app_handle,
        url,
        file_url,
        save_path.clone(),
        state.download_cancelled.clone(),
        state.limiter.clone(),
    )
    .await?;

    Ok(save_path.to_string_lossy().to_string())
}
