// src/commands/common.rs

use crate::db;
use crate::http_dl::manager::HttpManager;
use crate::ingest::IngestSummary;
use crate::providers::{ClipboardPayload, Site};
use crate::settings::{AppSettings, SettingsState};
use crate::state::AppState;

use clipboard::{ClipboardContext, ClipboardProvider};
use std::sync::atomic::Ordering;
use std::sync::Arc;
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

/// 停止所有進行中的網站下載（工具列「停止下載」）。
/// 旗標已是 per-task，這裡只是「全部取消」的入口。
#[tauri::command]
pub fn cancel_download(state: State<'_, AppState>) {
    state.cancel_all_downloads();
}

#[tauri::command]
pub fn get_app_settings(settings: State<'_, SettingsState>) -> AppSettings {
    settings.get()
}

/// 存 app 設定並即時套用 runtime 旗標（網站/直鏈限速、監控開關）。
/// BT port/限速仍是重啟生效（session 建立時讀取）。
#[tauri::command]
pub async fn save_app_settings(
    state: State<'_, AppState>,
    settings_state: State<'_, SettingsState>,
    http: State<'_, Arc<HttpManager>>,
    settings: AppSettings,
) -> Result<(), String> {
    settings_state
        .save(settings.clone())
        .await
        .map_err(|e| format!("儲存設定失敗: {:?}", e))?;
    apply_runtime_settings(&state, &http, &settings);
    Ok(())
}

/// 設定 → runtime：啟動與存檔共用同一條套用路徑，避免兩邊各寫一次而漂移。
pub fn apply_runtime_settings(state: &AppState, http: &Arc<HttpManager>, s: &AppSettings) {
    state.limiter.set_limit(s.web.limit_bps);
    http.set_limit(s.http.limit_bps);
    state
        .monitor_paused
        .store(!s.monitor_clipboard, Ordering::Relaxed);
}

#[tauri::command]
pub fn reorder_tasks(app_handle: AppHandle, urls: Vec<String>) -> Result<(), String> {
    db::reorder_tasks(&app_handle, &urls).map_err(|e| format!("排序失敗: {:?}", e))
}

/// 手動新增任務（拖曳連結觸發）：與剪貼簿共用 `ingest`
/// （辨識站台 → 驗證 → 抓元資料 → 寫 DB → emit），合集在裡面展開成每話一筆。
/// 前端事件 listener 不受監控開關控制，所以拖曳進來的同樣經事件進清單；回傳值只有統計。
#[tauri::command]
pub async fn add_url_manually(
    app_handle: AppHandle,
    url: String,
) -> Result<IngestSummary, String> {
    let url = url.trim().to_string();
    let site = Site::from_url(&url)?;
    let normalized = site.validate(&url)?;
    // 與剪貼簿共用 ingest：任務經事件送到前端，這裡只回統計（前端用來提示「任務已存在」）。
    // 手動加不做檔案已存在檢查（使用者明示意圖）
    crate::ingest::ingest(&app_handle, &site, &normalized, false).await
}
