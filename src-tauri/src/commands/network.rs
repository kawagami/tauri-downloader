// 網站下載指令 —— 位元組搬運交給共用引擎 crate::dl（與直鏈下載同一套），
// 這裡只負責：解析出真正的檔案連結、組 job、回報進度、把引擎的錯誤翻成 UI 用的分類。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};

use crate::dl::{self, DownloadJob, ErrorKind, JobConfig};
use crate::download_core::DownloadManager;
use crate::{
    error::DownloadError, providers::DownloadProgress, providers::Site, settings::SettingsState,
    state::AppState, utils,
};

/// 進度事件節流間隔
const EMIT_INTERVAL: Duration = Duration::from_millis(250);

/// 網站下載的 job 設定。
/// 單段是刻意的：`.part` 的長度本身就是續傳進度，不必另外持久化偏移
///（web 任務存在 SQLite，沒有 segments 欄位）。檔名鎖死 `{title}.zip`，
/// 不讓 Content-Disposition 蓋掉——清單和「已下載過」判斷都靠它。
fn web_job_config() -> JobConfig {
    JobConfig {
        max_segments: 1,
        min_split_bytes: 0,
        rename_from_headers: false,
        part_suffix: ".part".to_string(),
        resume_from_part_len: true,
    }
}

/// 背景每 250ms 讀 job 的計數器推一次進度事件，直到 `done` 被設起來。
/// 引擎本身不碰 UI，進度就由呼叫端自己取樣（直鏈那邊是每秒 tick 取同一組計數器）。
fn spawn_progress_emitter(
    app: AppHandle,
    source_url: String,
    job: Arc<DownloadJob>,
    done: Arc<AtomicBool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut manager = DownloadManager::new();
        manager.start_download(job.total_bytes.load(Ordering::Relaxed));
        while !done.load(Ordering::Relaxed) {
            tokio::time::sleep(EMIT_INTERVAL).await;
            let downloaded = job.downloaded();
            let total = job.total_bytes.load(Ordering::Relaxed);
            let metrics = manager.calculate_metrics(downloaded, total);
            if let Err(e) = app.emit(
                "download_progress",
                DownloadProgress {
                    url: source_url.clone(),
                    progress: metrics.percentage,
                    speed_bytes_per_sec: metrics.speed_bytes_per_sec,
                    time_remaining_secs: metrics.time_remaining_secs,
                },
            ) {
                tracing::warn!("進度事件 emit 失敗: {}", e);
            }
        }
    })
}

#[tauri::command]
pub async fn download_with_progress(
    url: String,
    title: String,
    file_url: String,
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    app_handle: AppHandle,
) -> Result<String, DownloadError> {
    // 網站下載預設目錄（空 = 系統下載資料夾），與 BT/直鏈同一套語意
    let download_dir = utils::fs::resolve_dir(&settings.get().web.default_dir);
    std::fs::create_dir_all(&download_dir)?;
    let save_path = utils::fs::get_unique_save_path(download_dir.clone(), &title);
    let file_name = save_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{title}.zip"));

    let site = Site::from_url(&url).map_err(DownloadError::Other)?;

    // 快取的 file_url 可能已過期；空的話現在去抓
    let had_cache = !file_url.is_empty();
    let resolved = if had_cache {
        file_url
    } else {
        site.resolve_file_url(&app_handle, &url).await?
    };

    let job = Arc::new(DownloadJob {
        url: Mutex::new(resolved),
        dest_dir: download_dir,
        file_name: Mutex::new(file_name),
        total_bytes: AtomicU64::new(0),
        range_supported: AtomicBool::new(false),
        segments: Mutex::new(Vec::new()),
        cfg: web_job_config(),
    });

    let stop = state.register_cancel(&url);
    let done = Arc::new(AtomicBool::new(false));
    let emitter = spawn_progress_emitter(app_handle.clone(), url.clone(), job.clone(), done.clone());

    let mut result = dl::run(&state.client, &job, &stop, state.limiter.clone()).await;

    // 快取的 file_url 失效：重抓最新連結再試一次。segments 保留著，
    // 所以是從斷點接續，不是整包重來（等同直鏈的「更新連結」但自動化）
    if had_cache
        && !dl::is_stopped(&stop)
        && matches!(&result, Err(e) if e.kind == ErrorKind::NotFound || e.kind == ErrorKind::Auth)
    {
        tracing::info!("快取 file_url 失效，重新抓取: {}", url);
        match site.resolve_file_url(&app_handle, &url).await {
            Ok(fresh) => {
                *job.url.lock().unwrap() = fresh;
                result = dl::run(&state.client, &job, &stop, state.limiter.clone()).await;
            }
            // 重抓本身就 404（下載頁整個消失）→ 維持原本的失效結論
            Err(e) => tracing::warn!("重新抓取 file_url 失敗: {}", e),
        }
    }

    done.store(true, Ordering::Relaxed);
    let _ = emitter.await;
    state.unregister_cancel(&url, &stop);

    // 使用者主動停 → Cancelled（`.part` 留著，下次從斷點接續）
    if dl::is_stopped(&stop) {
        return Err(DownloadError::Cancelled);
    }

    match result {
        Ok(final_path) => {
            // 補一次最終進度，確保前端顯示 100%
            let _ = app_handle.emit(
                "download_progress",
                DownloadProgress {
                    url,
                    progress: 100.0,
                    speed_bytes_per_sec: 0.0,
                    time_remaining_secs: 0.0,
                },
            );
            Ok(final_path.to_string_lossy().to_string())
        }
        Err(e) if e.kind == ErrorKind::NotFound => Err(DownloadError::NotFound),
        Err(e) => Err(DownloadError::Other(e.message)),
    }
}
