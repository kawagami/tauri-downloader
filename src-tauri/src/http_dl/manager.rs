// 直鏈下載的任務管理層 —— 狀態機、持久化、暫停/刪除、UI 需要的欄位。
// 實際的位元組搬運（Range 探測、分段並行、限速、`.part`、完成 rename）
// 在共用引擎 crate::dl，網站下載走同一套。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};

use crate::dl::{self, DownloadJob, JobConfig, Segment};
use crate::torrent::commands::sanitize_folder_name;
use crate::utils::{net::build_client, ratelimit::RateLimiter};

/// 分段並行數(伺服器無連線數限制時 4 段)。
const SEGMENT_COUNT: u64 = 4;
/// 小於此大小不分段,單一連線下載即可。
const MIN_SPLIT_BYTES: u64 = 8 * 1024 * 1024;
/// 刪檔前等 worker 收工的上限(比 READ_TIMEOUT 略長:最壞情況是卡在讀取逾時)。
const WORKER_JOIN_TIMEOUT: Duration = Duration::from_secs(35);
/// 刪檔重試次數/間隔 — 防毒軟體掃描中之類的短暫佔用。
const DELETE_RETRIES: u32 = 5;
const DELETE_RETRY_DELAY: Duration = Duration::from_millis(200);

/// 刪檔並重試:檔案不存在視為成功,連續失敗只記日誌不吵使用者。
async fn remove_file_retry(path: &Path) {
    for attempt in 1..=DELETE_RETRIES {
        match tokio::fs::remove_file(path).await {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                if attempt == DELETE_RETRIES {
                    tracing::warn!("刪除檔案失敗 {:?}: {}", path, e);
                    return;
                }
                tokio::time::sleep(DELETE_RETRY_DELAY).await;
            }
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HttpStatus {
    Running,
    Paused,
    Finished,
    Error,
}

/// 直鏈任務 = 共用引擎的 job + 這一層自己的狀態機。
pub struct HttpTask {
    pub id: u64,
    /// 下載狀態（url/檔名/大小/segments 都在裡面）
    pub job: DownloadJob,
    pub status: Mutex<HttpStatus>,
    pub error: Mutex<Option<String>>,
    pub retryable: AtomicBool,
    /// 本輪執行的停止旗標;暫停/刪除設為 true,resume 換新的一顆。
    stop: Mutex<Arc<AtomicBool>>,
    /// 本輪 worker 的 handle;刪除檔案前要等它收工放掉 .part 的檔案 handle。
    run_handle: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

/// 直鏈這邊的 job 設定：多段並行（偏移由本檔案持久化）、
/// 檔名可被 Content-Disposition 覆蓋、`.part` 摻 task id 防同名任務互寫。
fn job_config(id: u64) -> JobConfig {
    JobConfig {
        max_segments: SEGMENT_COUNT,
        min_split_bytes: MIN_SPLIT_BYTES,
        rename_from_headers: true,
        part_suffix: format!(".{id}.part"),
        // 多段的續傳靠 persisted segments.written，不是靠 .part 長度
        resume_from_part_len: false,
    }
}

impl HttpTask {
    pub fn downloaded(&self) -> u64 {
        self.job.downloaded()
    }

    pub fn status(&self) -> HttpStatus {
        *self.status.lock().unwrap()
    }

    pub fn file_name(&self) -> String {
        self.job.file_name.lock().unwrap().clone()
    }

    pub fn total_bytes(&self) -> u64 {
        self.job.total_bytes.load(Ordering::Relaxed)
    }

    fn signal_stop(&self) {
        self.stop.lock().unwrap().store(true, Ordering::Relaxed);
    }
}

// ---- 持久化(app_data_dir/http_tasks.json) ----

#[derive(Serialize, Deserialize)]
struct PersistedSegment {
    start: u64,
    end: u64,
    written: u64,
}

#[derive(Serialize, Deserialize)]
struct PersistedTask {
    id: u64,
    // token 即授權本體,但效期內續傳必須留著 URL;只存檔不進 log。
    url: String,
    file_name: String,
    dest_dir: String,
    total_bytes: u64,
    range_supported: bool,
    segments: Vec<PersistedSegment>,
    status: HttpStatus,
    error: Option<String>,
    retryable: bool,
}

pub struct HttpManager {
    pub tasks: Mutex<Vec<Arc<HttpTask>>>,
    next_id: AtomicU64,
    state_path: PathBuf,
    client: reqwest::Client,
    /// 全域限速器（bytes/s，0 = 不限），所有分段共用一顆桶
    limiter: Arc<RateLimiter>,
}

impl HttpManager {
    pub fn load(state_path: PathBuf, limit_bps: u64) -> Arc<Self> {
        // 解析失敗會把壞檔改名保留 + 記日誌，不再靜默把整批任務清空
        let persisted: Vec<PersistedTask> =
            crate::utils::jsonfile::load_json(&state_path).unwrap_or_default();

        let next_id = persisted.iter().map(|t| t.id + 1).max().unwrap_or(1);
        let tasks = persisted
            .into_iter()
            .map(|p| {
                Arc::new(HttpTask {
                    id: p.id,
                    job: DownloadJob {
                        url: Mutex::new(p.url),
                        dest_dir: PathBuf::from(p.dest_dir),
                        file_name: Mutex::new(p.file_name),
                        total_bytes: AtomicU64::new(p.total_bytes),
                        range_supported: AtomicBool::new(p.range_supported),
                        segments: Mutex::new(
                            p.segments
                                .into_iter()
                                .map(|s| Segment {
                                    start: s.start,
                                    end: s.end,
                                    written: Arc::new(AtomicU64::new(s.written)),
                                })
                                .collect(),
                        ),
                        cfg: job_config(p.id),
                    },
                    status: Mutex::new(p.status),
                    error: Mutex::new(p.error),
                    retryable: AtomicBool::new(p.retryable),
                    stop: Mutex::new(Arc::new(AtomicBool::new(false))),
                    run_handle: Mutex::new(None),
                })
            })
            .collect::<Vec<Arc<HttpTask>>>();

        // 舊版 .part 檔名不含 task id，載入時搬到新命名，續傳進度不丟
        for t in &tasks {
            let old = t.job.dest_dir.join(format!("{}.part", t.file_name()));
            let new = t.job.part_path();
            if old.exists() && !new.exists() {
                let _ = std::fs::rename(&old, &new);
            }
        }

        Arc::new(HttpManager {
            tasks: Mutex::new(tasks),
            next_id: AtomicU64::new(next_id),
            state_path,
            client: build_client(),
            limiter: RateLimiter::new(Arc::new(AtomicU64::new(limit_bps))),
        })
    }

    /// 限速即時生效（設定存檔時呼叫）
    pub fn set_limit(&self, bps: u64) {
        self.limiter.set_limit(bps);
    }

    /// app 啟動時把上次仍在跑的任務接回去(中途殺 app 重開要續傳)。
    pub fn resume_interrupted(self: &Arc<Self>) {
        let running: Vec<_> = self
            .tasks
            .lock()
            .unwrap()
            .iter()
            .filter(|t| t.status() == HttpStatus::Running)
            .cloned()
            .collect();
        for t in running {
            self.spawn_run(t);
        }
    }

    pub fn find(&self, id: u64) -> Option<Arc<HttpTask>> {
        self.tasks.lock().unwrap().iter().find(|t| t.id == id).cloned()
    }

    pub fn find_active_by_url(&self, url: &str) -> Option<Arc<HttpTask>> {
        self.tasks
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.status() != HttpStatus::Finished && *t.job.url.lock().unwrap() == url)
            .cloned()
    }

    pub fn add(&self, url: String, dest_dir: PathBuf, file_name: String) -> Arc<HttpTask> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let task = Arc::new(HttpTask {
            id,
            job: DownloadJob {
                url: Mutex::new(url),
                dest_dir,
                file_name: Mutex::new(file_name),
                total_bytes: AtomicU64::new(0),
                range_supported: AtomicBool::new(false),
                segments: Mutex::new(Vec::new()),
                cfg: job_config(id),
            },
            status: Mutex::new(HttpStatus::Paused),
            error: Mutex::new(None),
            retryable: AtomicBool::new(false),
            stop: Mutex::new(Arc::new(AtomicBool::new(false))),
            run_handle: Mutex::new(None),
        });
        self.tasks.lock().unwrap().push(task.clone());
        self.persist();
        task
    }

    pub fn pause(&self, id: u64) {
        if let Some(task) = self.find(id) {
            task.signal_stop();
            if task.status() == HttpStatus::Running {
                *task.status.lock().unwrap() = HttpStatus::Paused;
            }
            self.persist();
        }
    }

    pub fn remove(&self, id: u64, delete_files: bool) {
        let task = {
            let mut tasks = self.tasks.lock().unwrap();
            let Some(pos) = tasks.iter().position(|t| t.id == id) else {
                return;
            };
            tasks.remove(pos)
        };
        task.signal_stop();
        self.persist();
        if !delete_files {
            return;
        }

        // 不能立刻刪：worker 每收一個 chunk 才檢查 stop，此刻 .part 的檔案 handle
        // 還開著,Windows 會回 sharing violation 而刪不掉(以前被 `let _ =` 吞掉,
        // 結果勾了「刪除檔案」卻留下 .part)。等 worker 收工再刪,清單那邊已經移除,
        // UI 不用等。
        let handle = task.run_handle.lock().unwrap().take();
        tauri::async_runtime::spawn(async move {
            if let Some(h) = handle {
                // 上限比 READ_TIMEOUT 略長:worker 最壞情況是卡在讀取逾時才收工
                if tokio::time::timeout(WORKER_JOIN_TIMEOUT, h).await.is_err() {
                    tracing::warn!("等待任務 {} 收工逾時，仍嘗試刪除檔案", task.id);
                }
            }
            remove_file_retry(&task.job.part_path()).await;
            remove_file_retry(&task.job.final_path()).await;
        });
    }

    pub fn persist(&self) {
        let data: Vec<PersistedTask> = self
            .tasks
            .lock()
            .unwrap()
            .iter()
            .map(|t| PersistedTask {
                id: t.id,
                url: t.job.url.lock().unwrap().clone(),
                file_name: t.file_name(),
                dest_dir: t.job.dest_dir.to_string_lossy().into_owned(),
                total_bytes: t.total_bytes(),
                range_supported: t.job.range_supported.load(Ordering::Relaxed),
                segments: t
                    .job
                    .segments
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|s| PersistedSegment {
                        start: s.start,
                        end: s.end,
                        written: s.written.load(Ordering::Relaxed),
                    })
                    .collect(),
                status: t.status(),
                error: t.error.lock().unwrap().clone(),
                retryable: t.retryable.load(Ordering::Relaxed),
            })
            .collect();
        // 原子寫入：下載中每幾秒就寫一次，非原子寫法被砍掉會留半截 JSON → 任務全滅
        if let Err(e) = crate::utils::jsonfile::write_json_atomic(&self.state_path, &data) {
            tracing::error!("寫入 {:?} 失敗: {}", self.state_path, e);
        }
    }

    /// 啟動(或續跑)一個任務。狀態立即轉 Running;實際下載在背景 task。
    /// 結束時:使用者主動停(stop 旗標)→ 不動狀態;否則寫 Finished / Error。
    pub fn spawn_run(self: &Arc<Self>, task: Arc<HttpTask>) {
        let stop = Arc::new(AtomicBool::new(false));
        *task.stop.lock().unwrap() = stop.clone();
        *task.status.lock().unwrap() = HttpStatus::Running;
        *task.error.lock().unwrap() = None;
        self.persist();

        let mgr = self.clone();
        let worker_task = task.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let task = worker_task;
            let result = dl::run(&mgr.client, &task.job, &stop, mgr.limiter.clone()).await;
            if dl::is_stopped(&stop) {
                // 暫停/刪除已由指令端處理狀態,這裡不能覆寫。
                return;
            }
            match result {
                Ok(_) => {
                    *task.status.lock().unwrap() = HttpStatus::Finished;
                    *task.error.lock().unwrap() = None;
                }
                Err(e) => {
                    *task.status.lock().unwrap() = HttpStatus::Error;
                    *task.error.lock().unwrap() = Some(e.message);
                    task.retryable.store(e.retryable, Ordering::Relaxed);
                }
            }
            mgr.persist();
        });
        // 刪除任務時要等這條收工才動檔案（背景 task 可能已跑完，存進去也無妨：
        // await 一個已結束的 handle 會立刻返回）
        *task.run_handle.lock().unwrap() = Some(handle);
    }
}

/// URL path 最後一段(去 query、percent-decode)當預設檔名。
pub fn filename_from_url(url: &reqwest::Url) -> Option<String> {
    let last = url.path_segments()?.rfind(|s| !s.is_empty())?;
    let decoded = percent_decode_str(last)
        .decode_utf8()
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| last.to_string());
    let cleaned = sanitize_folder_name(&decoded);
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_from_url_strips_query() {
        let url = reqwest::Url::parse("https://example.com/files/movie.mkv?token=secret").unwrap();
        assert_eq!(filename_from_url(&url), Some("movie.mkv".to_string()));
    }

    /// `.part` 要摻 task id，不同任務同檔名才不會互寫
    #[test]
    fn part_path_includes_task_id() {
        let cfg = job_config(42);
        assert_eq!(cfg.part_suffix, ".42.part");
    }
}
