use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use super::manager::{HttpManager, HttpStatus};

/// 每秒收集直鏈任務狀態推 "http-stats" event;速度 = 兩次 tick 的
/// downloaded 差值。finished 轉換時推 "http-finished"(首 tick 不發,
/// 避免重啟恢復的已完成任務誤報)。清單空且上一輪也空時不 emit。
pub fn spawn_http_stats_task(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut prev_bytes: HashMap<u64, u64> = HashMap::new();
        let mut was_finished: HashMap<u64, bool> = HashMap::new();
        let mut first_tick = true;
        let mut prev_empty = true;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let mgr = app.state::<Arc<HttpManager>>();
            let tasks: Vec<_> = mgr.tasks.lock().unwrap().clone();

            if tasks.is_empty() && prev_empty {
                first_tick = false;
                continue;
            }
            prev_empty = tasks.is_empty();

            let mut payload_tasks = Vec::with_capacity(tasks.len());
            let mut total_down_bps: u64 = 0;
            for t in &tasks {
                let downloaded = t.downloaded();
                let total = t.total_bytes.load(Ordering::Relaxed);
                let prev = prev_bytes.insert(t.id, downloaded).unwrap_or(downloaded);
                let status = t.status();
                let bps = if status == HttpStatus::Running {
                    downloaded.saturating_sub(prev)
                } else {
                    0
                };
                total_down_bps += bps;

                let finished = status == HttpStatus::Finished;
                let prev_fin = was_finished.insert(t.id, finished).unwrap_or(false);
                if !first_tick && !prev_fin && finished {
                    let name = t.file_name.lock().unwrap().clone();
                    let _ = app.emit("http-finished", json!({ "id": t.id, "name": name }));
                }

                let progress = if total > 0 {
                    downloaded as f64 / total as f64 * 100.0
                } else if finished {
                    100.0
                } else {
                    0.0
                };
                payload_tasks.push(json!({
                    "id": t.id,
                    "name": t.file_name.lock().unwrap().clone(),
                    "state": match status {
                        HttpStatus::Running => "running",
                        HttpStatus::Paused => "paused",
                        HttpStatus::Finished => "finished",
                        HttpStatus::Error => "error",
                    },
                    "progress_percent": progress,
                    "downloaded_bytes": downloaded,
                    "total_bytes": total,
                    "down_speed_bps": bps,
                    "error": t.error.lock().unwrap().clone(),
                    "retryable": t.retryable.load(Ordering::Relaxed),
                }));
            }
            prev_bytes.retain(|k, _| tasks.iter().any(|t| t.id == *k));
            was_finished.retain(|k, _| tasks.iter().any(|t| t.id == *k));

            // 跑動中的任務每秒落地一次進度,殺掉 app 也只丟最後一秒。
            if tasks.iter().any(|t| t.status() == HttpStatus::Running) {
                mgr.persist();
            }

            first_tick = false;

            let _ = app.emit(
                "http-stats",
                json!({ "tasks": payload_tasks, "total_down_bps": total_down_bps }),
            );
        }
    });
}
