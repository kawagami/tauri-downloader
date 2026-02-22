// src/commands/common.rs

use crate::db;
use crate::monitor::ClipboardPayload;
use clipboard::{ClipboardContext, ClipboardProvider};
use tauri::command;
use tauri::AppHandle;

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
