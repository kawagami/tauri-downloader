// src/commands/common.rs

use crate::db;
use crate::monitor::ClipboardPayload;
use clipboard::{ClipboardContext, ClipboardProvider};
use tauri::command;
use url::Url;

// =========================================================
// ✨ 輔助函數：URL 驗證邏輯 (提供給 monitor.rs 使用)
// =========================================================
pub fn is_valid_wnacg_url(content: &str) -> Result<String, String> {
    let parsed_url = Url::parse(content).map_err(|_| "非有效 URL".to_string())?;

    // 檢查 Scheme (必須是 HTTPS)
    if parsed_url.scheme() != "https" {
        return Err("連結不符合格式：必須使用 https 協定。".to_string());
    }

    // 檢查 Host (必須是 www.wnacg.com)
    if parsed_url.host_str() != Some("www.wnacg.com") {
        return Err("連結不符合格式：域名錯誤。".to_string());
    }

    // 檢查 Path 格式：必須是 /photos-index-aid-{ID}.html
    let path = parsed_url.path();
    let path_prefix = "/photos-index-aid-";
    let path_suffix = ".html";

    if !(path.starts_with(path_prefix) && path.ends_with(path_suffix)) {
        return Err("連結不符合格式：路徑錯誤。".to_string());
    }

    // 驗證 {ID} 部分是否為純數字
    let id_segment = path
        .trim_start_matches(path_prefix)
        .trim_end_matches(path_suffix);

    if id_segment.is_empty() || !id_segment.chars().all(|c| c.is_ascii_digit()) {
        return Err("連結不符合格式：ID 部分非數字。".to_string());
    }

    Ok(parsed_url.to_string())
}

/// 簡單的問候命令
#[command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

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
