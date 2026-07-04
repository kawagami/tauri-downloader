// src/commands/common.rs

use crate::db;
use crate::providers::{ClipboardPayload, Site};
use crate::settings::{AppSettings, SettingsState};
use crate::state::AppState;

use clipboard::{ClipboardContext, ClipboardProvider};
use std::sync::atomic::Ordering;
use tauri::command;
use tauri::{AppHandle, State};

/// 讀取剪貼簿內容
#[command]
pub fn read_clipboard() -> Result<String, String> {
    let mut ctx: ClipboardContext =
        ClipboardProvider::new().map_err(|e| format!("Error creating clipboard context: {}", e))?;

    ctx.get_contents()
        .map_err(|e| format!("Error reading clipboard: {}", e))
}

/// 取得所有任務列表
#[tauri::command]
pub fn load_all_tasks(app_handle: tauri::AppHandle) -> Result<Vec<ClipboardPayload>, String> {
    db::get_all_tasks(&app_handle).map_err(|e| format!("讀取資料庫失敗: {:?}", e))
}

#[tauri::command]
pub fn remove_task(app_handle: AppHandle, url: String) -> Result<(), String> {
    db::delete_task_by_url(&app_handle, &url).map_err(|e| format!("刪除任務失敗: {:?}", e))
}

#[tauri::command]
pub fn remove_all_tasks(app_handle: AppHandle) -> Result<(), String> {
    db::clear_all_tasks(&app_handle).map_err(|e| format!("刪除全部任務失敗: {:?}", e))
}

#[tauri::command]
pub fn update_task_status(app_handle: AppHandle, url: String, status: String) -> Result<(), String> {
    db::update_task_status(&app_handle, &url, &status)
        .map_err(|e| format!("更新狀態失敗: {:?}", e))
}

#[tauri::command]
pub fn cancel_download(state: State<'_, AppState>) {
    state.download_cancelled.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn get_app_settings(settings: State<'_, SettingsState>) -> AppSettings {
    settings.get()
}

/// 存 app 設定並即時套用 runtime 旗標（頻寬限制、監控開關）。
/// BT port/限速仍是重啟生效（session 建立時讀取）。
#[tauri::command]
pub fn save_app_settings(
    state: State<'_, AppState>,
    settings_state: State<'_, SettingsState>,
    settings: AppSettings,
) -> Result<(), String> {
    settings_state
        .save(settings.clone())
        .map_err(|e| format!("儲存設定失敗: {:?}", e))?;
    state
        .bandwidth_limit_bps
        .store(settings.bandwidth_limit_kbps * 1024, Ordering::Relaxed);
    state
        .monitor_paused
        .store(!settings.monitor_clipboard, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn reorder_tasks(app_handle: AppHandle, urls: Vec<String>) -> Result<(), String> {
    db::reorder_tasks(&app_handle, &urls).map_err(|e| format!("排序失敗: {:?}", e))
}

/// 手動新增任務（拖曳連結觸發）：複用剪貼簿同一條 pipeline
/// （辨識站台 → 驗證 → 抓元資料 → 寫 DB），回傳 payload 讓前端直接 addTask。
/// 不 emit 事件，避免與剪貼簿監控的 listener double-add；前端 addTask 與 DB UNIQUE 各自去重。
#[tauri::command]
pub async fn add_url_manually(
    app_handle: AppHandle,
    url: String,
) -> Result<ClipboardPayload, String> {
    let url = url.trim().to_string();
    let site = Site::from_url(&url)?;
    let normalized = site.validate(&url)?;
    let payload = site.fetch_details(&app_handle, &normalized).await?;
    // 重複 url 回 Ok(false)，仍回傳 payload（前端去重）；手動加不做檔案已存在檢查（使用者明示意圖）
    db::insert_task(&app_handle, &payload).map_err(|e| format!("寫入資料庫失敗: {:?}", e))?;
    Ok(payload)
}
