use crate::{providers::Site, state::AppState, utils};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn download_with_progress(
    url: String,
    title: String,
    file_url: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    state.download_cancelled.store(false, Ordering::Relaxed);

    let download_dir = app_handle
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&download_dir).map_err(|e| e.to_string())?;
    let save_path = utils::fs::get_unique_save_path(download_dir, &title);

    let site = Site::from_url(&url)?;
    site.download(
        &state.client,
        &app_handle,
        url,
        file_url,
        save_path.clone(),
        state.download_cancelled.clone(),
        state.bandwidth_limit_bps.clone(),
    )
    .await?;

    Ok(save_path.to_string_lossy().to_string())
}
