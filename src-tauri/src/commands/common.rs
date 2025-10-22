// src/commands/common.rs

use clipboard::{ClipboardContext, ClipboardProvider};
use reqwest;
use select::document::Document;
use select::predicate::{Attr, Descendant, Name};
use tauri::command;
use url::Url;

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
    // ... 讀取剪貼簿內容 (content) ...
    let mut ctx: ClipboardContext =
        ClipboardProvider::new().map_err(|e| format!("Error creating clipboard context: {}", e))?;

    let content = ctx
        .get_contents()
        .map_err(|e| format!("Error reading clipboard: {}", e))?;

    // 1. 判斷是否為有效連結
    let parsed_url = match Url::parse(&content) {
        Ok(url) => url,
        Err(_) => return Ok(format!("非有效 URL: {}", content)),
    };

    // =========================================================
    // ✨ 核心驗證邏輯：限定 URL 格式
    // =========================================================

    // 檢查 Scheme (必須是 HTTPS)
    if parsed_url.scheme() != "https" {
        return Ok(format!(
            "連結不符合格式：必須使用 https 協定。原始: {}",
            content
        ));
    }

    // 檢查 Host (必須是 www.wnacg.com)
    if parsed_url.host_str() != Some("www.wnacg.com") {
        return Ok(format!("連結不符合格式：域名錯誤。原始: {}", content));
    }

    // 檢查 Path 格式：必須是 /photos-index-aid-{ID}.html
    let path = parsed_url.path();
    let path_prefix = "/photos-index-aid-";
    let path_suffix = ".html";

    if !(path.starts_with(path_prefix) && path.ends_with(path_suffix)) {
        return Ok(format!("連結不符合格式：路徑錯誤。原始: {}", content));
    }

    // 額外檢查：驗證 {ID} 部分是否為純數字
    // 提取 ID 部分
    let id_segment = path
        .trim_start_matches(path_prefix)
        .trim_end_matches(path_suffix);

    if id_segment.is_empty() || !id_segment.chars().all(|c| c.is_ascii_digit()) {
        return Ok(format!("連結不符合格式：ID 部分非數字。原始: {}", content));
    }

    // =========================================================
    // 驗證通過，繼續執行
    // =========================================================

    println!("有效 URL (wnacg 格式): {}", parsed_url.as_str());

    // 3. 執行 reqwest 請求
    // ... (後續的 reqwest 和 HTML 解析邏輯保持不變)
    let response = reqwest::get(parsed_url.as_str())
        .await
        // ... (省略後續解析邏輯)
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
        content,
        extracted_title // 使用新提取的標題
    ))
}
