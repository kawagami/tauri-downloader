// 站台 URL → 任務入庫的共用流程（剪貼簿監控與拖曳共用同一條路）。
// 一般作品 = 一筆；合集展開成每話一筆，逐話抓取。
//
// 任務一律經事件送到前端（不靠指令回傳值），剪貼簿與拖曳進來的長得一樣：
// - `new-valid-url-payload`：一般作品新增（前端播 ding）
// - `series-start` / `series-item-added` / `series-done`：合集開始、每話新增（靜音）、結束統計（整批一次 ding）

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::db;
use crate::providers::{ClipboardPayload, Fetched, Site, SERIES_REQUEST_GAP};

/// 所有站台抓取排成一列（併發 1）：剪貼簿與拖曳同時進來、或同一個合集在抓取途中
/// 被再複製一次時，對站台的請求仍然一次一個。第二次進來的會等前一次跑完，
/// 那時各話都已在 DB，`task_exists` 直接略過、不會再打站台。
static FETCH_LOCK: Mutex<()> = Mutex::const_new(());

/// 一次加入的結果統計（合集才有意義；一般作品各欄位非 0 即 1）
#[derive(Serialize, Clone, Default)]
pub struct IngestSummary {
    /// 合集標題；一般作品為 None
    pub series_title: Option<String>,
    pub added: usize,
    /// 清單裡已經有了
    pub existed: usize,
    /// 下載目錄已有同名檔（只有剪貼簿監控會檢查）
    pub skipped: usize,
    /// 抓取失敗（合集的個別章節；一般作品失敗直接回 Err）
    pub failed: usize,
}

#[derive(Serialize, Clone)]
struct SeriesStart {
    title: String,
    total: usize,
}

/// `url` 須是 `Site::validate` 過的規範化 URL（DB 主鍵）。
/// `skip_downloaded`：下載目錄已有同名檔就略過（剪貼簿監控用；手動拖曳是明示意圖，不檢查）
pub async fn ingest(
    handle: &AppHandle,
    site: &Site,
    url: &str,
    skip_downloaded: bool,
) -> Result<IngestSummary, String> {
    let _guard = FETCH_LOCK.lock().await;
    let mut summary = IngestSummary::default();

    // 已在清單裡就不必再打站台（合集頁本身不入庫，這裡不會擋到它）
    if db::task_exists(handle, url).map_err(db_err)? {
        summary.existed = 1;
        return Ok(summary);
    }

    match site.fetch_details(handle, url).await? {
        Fetched::Work(payload) => {
            store(handle, payload, skip_downloaded, "new-valid-url-payload", &mut summary)?;
        }
        Fetched::Series { title, chapters } => {
            let _ = handle.emit(
                "series-start",
                SeriesStart { title: title.clone(), total: chapters.len() },
            );
            for ch in &chapters {
                // 部分失敗後重新複製，只補抓缺的那幾話
                if db::task_exists(handle, ch).unwrap_or(false) {
                    summary.existed += 1;
                    continue;
                }
                tokio::time::sleep(SERIES_REQUEST_GAP).await;
                let result = match site.fetch_details(handle, ch).await {
                    Ok(Fetched::Work(payload)) => {
                        store(handle, payload, skip_downloaded, "series-item-added", &mut summary)
                    }
                    Ok(Fetched::Series { .. }) => Err("章節本身又是合集".to_string()),
                    Err(e) => Err(e),
                };
                if let Err(e) = result {
                    tracing::warn!("合集《{}》章節加入失敗 {}: {}", title, ch, e);
                    summary.failed += 1;
                }
            }
            tracing::info!(
                "合集《{}》：新增 {}、已存在 {}、已下載過 {}、失敗 {}",
                title, summary.added, summary.existed, summary.skipped, summary.failed
            );
            summary.series_title = Some(title);
            let _ = handle.emit("series-done", summary.clone());
        }
    }
    Ok(summary)
}

/// 單筆入庫：已下載過檢查 → insert → 新增才 emit
fn store(
    handle: &AppHandle,
    payload: ClipboardPayload,
    skip_downloaded: bool,
    event: &str,
    summary: &mut IngestSummary,
) -> Result<(), String> {
    if skip_downloaded {
        // 命名規則與 get_unique_save_path 共用同一個函式；
        // 目錄來源與實際下載一致：AppSettings.web.default_dir（空 = 系統下載夾）
        let web_dir = crate::utils::fs::resolve_dir(
            &handle.state::<crate::settings::SettingsState>().get().web.default_dir,
        );
        if crate::utils::fs::already_downloaded(&web_dir, &payload.title) {
            tracing::info!("已下載過，略過: {}", payload.title);
            summary.skipped += 1;
            return Ok(());
        }
    }

    if db::insert_task(handle, &payload).map_err(db_err)? {
        let _ = handle.emit(event, &payload);
        summary.added += 1;
    } else {
        summary.existed += 1;
    }
    Ok(())
}

fn db_err(e: rusqlite::Error) -> String {
    format!("資料庫錯誤: {:?}", e)
}
