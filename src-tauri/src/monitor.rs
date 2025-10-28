// src/monitor.rs

use clipboard::ClipboardProvider;
use reqwest;
use select::document::Document;
use select::predicate::{Attr, Descendant, Name};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::runtime::Runtime;

use crate::commands::common::is_valid_wnacg_url;

const MONITOR_INTERVAL_MS: u64 = 500;

#[derive(Clone, serde::Serialize)]
pub struct ClipboardPayload {
    pub url: String,
    pub title: String, // 依照 url 使用 reqwest get 取得的其一資訊
    pub image: String, // 依照 url 使用 reqwest get 取得的其二資訊
}

// 假設您有一個異步函數來獲取額外資訊
// 為了簡化，我們在 monitor.rs 中定義一個模擬或實際的函數
// 由於這是網絡操作，它必須是 async 的
async fn fetch_payload_details(
    url: String,
) -> Result<ClipboardPayload, Box<dyn std::error::Error + Send + Sync>> {
    println!("Rust Monitor: 正在從 URL 獲取詳細資訊: {}", url);

    // 實際的 reqwest 請求範例：
    let client = reqwest::Client::new();
    let res = client.get(&url).send().await?;

    // 檢查響應狀態
    if !res.status().is_success() {
        return Err(format!("網絡請求失敗，狀態碼: {}", res.status()).into());
    }

    // 實際應用中，您會解析 HTML 內容來獲取 title 和 image URL/ID
    let html_content = res.text().await?;

    let document = Document::from(html_content.as_str());

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

    let title = extracted_title;
    let image_id = format!("123456789"); // 替換為實際解析邏

    Ok(ClipboardPayload {
        url,
        title,
        image: image_id,
    })
}

/// 啟動剪貼簿監控線程
pub fn start_clipboard_monitor(app_handle: AppHandle) {
    thread::spawn(move || {
        // 在這個 OS 線程中創建一個 Tokio 運行時
        // 這樣就可以運行 async/await 程式碼 (如 reqwest)
        let rt = match Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("無法創建 Tokio 運行時，停止監控: {}", e);
                return;
            }
        };

        let mut last_content = String::new();
        let mut ctx: clipboard::ClipboardContext = match ClipboardProvider::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("無法初始化剪貼簿上下文，停止監控: {}", e);
                return;
            }
        };

        loop {
            let current_content = match ctx.get_contents() {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("讀取剪貼簿錯誤: {}", e);
                    std::thread::sleep(Duration::from_millis(MONITOR_INTERVAL_MS));
                    continue;
                }
            };

            if current_content != last_content {
                match is_valid_wnacg_url(&current_content) {
                    Ok(parsed_url) => {
                        println!("Rust Monitor: 偵測到新的有效 URL: {}", &parsed_url);

                        let url_clone = parsed_url.clone();
                        let app_handle_clone = app_handle.clone();

                        // ✨ 使用 rt.spawn 執行異步任務
                        // rt.spawn 不會阻塞當前的 loop 線程
                        rt.spawn(async move {
                            match fetch_payload_details(url_clone).await {
                                Ok(payload) => {
                                    println!("Rust Monitor: 成功獲取詳細資訊，推送事件。");
                                    // 在異步任務中發送事件
                                    if let Err(e) =
                                        app_handle_clone.emit("new-valid-url-payload", payload)
                                    {
                                        eprintln!("發送 new-valid-url-payload 事件錯誤: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("獲取 URL 詳細資訊失敗: {}", e);
                                }
                            }
                        });
                    }
                    Err(_) => {}
                }
                last_content = current_content;
            }

            // 使用 std::thread::sleep 進行阻塞等待
            thread::sleep(Duration::from_millis(MONITOR_INTERVAL_MS));
        }
    });
}
