// src/commands/common.rs

use clipboard::{ClipboardContext, ClipboardProvider};
use tauri::command;

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
