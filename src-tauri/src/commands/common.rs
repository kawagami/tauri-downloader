// src/commands/common.rs

use clipboard::{ClipboardContext, ClipboardProvider};
use reqwest;
use select::document::Document;
use select::predicate::{Attr, Descendant, Name};
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

#[command]
pub async fn process_clipboard_url() -> Result<String, String> {
    let mut ctx: ClipboardContext =
        ClipboardProvider::new().map_err(|e| format!("Error creating clipboard context: {}", e))?;

    let content = ctx
        .get_contents()
        .map_err(|e| format!("Error reading clipboard: {}", e))?;

    // 使用新的輔助函數進行驗證
    let validated_url = is_valid_wnacg_url(&content)?;

    println!("有效 URL (wnacg 格式): {}", validated_url);

    // 執行 reqwest 請求 (保持不變)
    let response = reqwest::get(&validated_url) // 注意這裡使用 validated_url
        .await
        .map_err(|e| format!("HTTP 請求失敗: {}", e))?;

    // ... (後續的 HTML 解析和結果回傳) ...

    // 返回結果 (省略細節)
    let body = response
        .text()
        .await
        .map_err(|e| format!("無法讀取回應內容: {}", e))?;

    let document = Document::from(body.as_str());

    let target_h2 = document
        .find(
            // 選擇器：尋找 id="bodywrap" 元素底下的 h2 標籤
            Descendant(
                Attr("id", "bodywrap"), // 父元素：div id="bodywrap"
                Name("h2"),             // 子元素：h2
            ),
        )
        .next(); // 只取第一個匹配項

    let extracted_title = target_h2
        .map(|node| node.text())
        .unwrap_or_else(|| "無法找到指定標題".to_string());

    // 6. 返回結果
    Ok(format!(
        "✅ 成功解析連結！\nURL: {}\n提取標題: {}",
        validated_url,
        extracted_title // 使用新提取的標題
    ))
}
