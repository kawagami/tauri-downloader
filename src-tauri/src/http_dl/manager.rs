use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use percent_encoding::percent_decode_str;
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_RANGE, RANGE};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinSet;

use crate::torrent::commands::sanitize_folder_name;

/// 分段並行數(伺服器無連線數限制時 4 段)。
const SEGMENT_COUNT: u64 = 4;
/// 小於此大小不分段,單一連線下載即可。
const MIN_SPLIT_BYTES: u64 = 8 * 1024 * 1024;
/// Content-Length 未知時 segment.end 的哨兵值。
const UNBOUNDED: u64 = u64::MAX;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HttpStatus {
    Running,
    Paused,
    Finished,
    Error,
}

/// 半開區間 [start, end)。written 為已寫入 bytes,由 worker 累加。
pub struct Segment {
    pub start: u64,
    pub end: u64,
    pub written: Arc<AtomicU64>,
}

pub struct HttpTask {
    pub id: u64,
    pub url: Mutex<String>,
    pub file_name: Mutex<String>,
    pub dest_dir: PathBuf,
    pub total_bytes: AtomicU64, // 0 = 未知
    pub range_supported: AtomicBool,
    pub segments: Mutex<Vec<Segment>>,
    pub status: Mutex<HttpStatus>,
    pub error: Mutex<Option<String>>,
    pub retryable: AtomicBool,
    /// 本輪執行的停止旗標;暫停/刪除設為 true,resume 換新的一顆。
    stop: Mutex<Arc<AtomicBool>>,
}

impl HttpTask {
    pub fn downloaded(&self) -> u64 {
        self.segments
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.written.load(Ordering::Relaxed))
            .sum()
    }

    pub fn status(&self) -> HttpStatus {
        *self.status.lock().unwrap()
    }

    fn part_path(&self) -> PathBuf {
        // 檔名摻 task id：不同 URL 同檔名的任務才不會互寫同一個 .part
        let name = self.file_name.lock().unwrap().clone();
        self.dest_dir.join(format!("{name}.{}.part", self.id))
    }

    fn final_path(&self) -> PathBuf {
        let name = self.file_name.lock().unwrap().clone();
        self.dest_dir.join(name)
    }

    fn signal_stop(&self) {
        self.stop.lock().unwrap().store(true, Ordering::Relaxed);
    }
}

/// 下載失敗原因。message 給 UI 直接顯示;retryable = 網路類問題,重試即可
/// (token 過期/權限/404 則需要使用者處理)。錯誤訊息一律不含 URL(token 在
/// query string 裡,不可外洩到 log 或畫面)。
struct TaskError {
    message: String,
    retryable: bool,
}

impl TaskError {
    fn net(e: reqwest::Error) -> Self {
        // without_url:reqwest 錯誤字串預設帶完整 URL(含 token),必須剝掉。
        TaskError {
            message: format!("網路中斷,可重試({})", e.without_url()),
            retryable: true,
        }
    }

    fn io(e: std::io::Error) -> Self {
        TaskError {
            message: format!("寫檔失敗:{e}"),
            retryable: true,
        }
    }
}

async fn map_status_error(status: StatusCode, resp: reqwest::Response) -> TaskError {
    let canned = match status.as_u16() {
        401 => Some("連結已過期,請重新複製下載連結"),
        403 => Some("權限不足,請重新複製下載連結"),
        404 => Some("檔案已不存在"),
        _ => None,
    };
    if let Some(msg) = canned {
        return TaskError {
            message: msg.to_string(),
            retryable: false,
        };
    }
    // 其他狀態碼:body 可能是 {code,message} JSON,也可能是 nginx HTML,別假設。
    let detail = resp
        .text()
        .await
        .ok()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|v| v["message"].as_str().map(|s| format!(":{s}")))
        .unwrap_or_default();
    TaskError {
        message: format!("HTTP {}{detail}", status.as_u16()),
        retryable: status.is_server_error(),
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
}

impl HttpManager {
    pub fn load(state_path: PathBuf) -> Arc<Self> {
        let persisted: Vec<PersistedTask> = std::fs::read(&state_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();

        let next_id = persisted.iter().map(|t| t.id + 1).max().unwrap_or(1);
        let tasks = persisted
            .into_iter()
            .map(|p| {
                Arc::new(HttpTask {
                    id: p.id,
                    url: Mutex::new(p.url),
                    file_name: Mutex::new(p.file_name),
                    dest_dir: PathBuf::from(p.dest_dir),
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
                    status: Mutex::new(p.status),
                    error: Mutex::new(p.error),
                    retryable: AtomicBool::new(p.retryable),
                    stop: Mutex::new(Arc::new(AtomicBool::new(false))),
                })
            })
            .collect::<Vec<Arc<HttpTask>>>();

        // 舊版 .part 檔名不含 task id，載入時搬到新命名，續傳進度不丟
        for t in &tasks {
            let name = t.file_name.lock().unwrap().clone();
            let old = t.dest_dir.join(format!("{name}.part"));
            let new = t.part_path();
            if old.exists() && !new.exists() {
                let _ = std::fs::rename(&old, &new);
            }
        }

        Arc::new(HttpManager {
            tasks: Mutex::new(tasks),
            next_id: AtomicU64::new(next_id),
            state_path,
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        })
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
            .find(|t| t.status() != HttpStatus::Finished && *t.url.lock().unwrap() == url)
            .cloned()
    }

    pub fn add(&self, url: String, dest_dir: PathBuf, file_name: String) -> Arc<HttpTask> {
        let task = Arc::new(HttpTask {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            url: Mutex::new(url),
            file_name: Mutex::new(file_name),
            dest_dir,
            total_bytes: AtomicU64::new(0),
            range_supported: AtomicBool::new(false),
            segments: Mutex::new(Vec::new()),
            status: Mutex::new(HttpStatus::Paused),
            error: Mutex::new(None),
            retryable: AtomicBool::new(false),
            stop: Mutex::new(Arc::new(AtomicBool::new(false))),
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
        if delete_files {
            let _ = std::fs::remove_file(task.part_path());
            let _ = std::fs::remove_file(task.final_path());
        }
        self.persist();
    }

    pub fn persist(&self) {
        let data: Vec<PersistedTask> = self
            .tasks
            .lock()
            .unwrap()
            .iter()
            .map(|t| PersistedTask {
                id: t.id,
                url: t.url.lock().unwrap().clone(),
                file_name: t.file_name.lock().unwrap().clone(),
                dest_dir: t.dest_dir.to_string_lossy().into_owned(),
                total_bytes: t.total_bytes.load(Ordering::Relaxed),
                range_supported: t.range_supported.load(Ordering::Relaxed),
                segments: t
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
        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&self.state_path, json);
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
        tauri::async_runtime::spawn(async move {
            let result = mgr.run_task(&task, &stop).await;
            if stop.load(Ordering::Relaxed) {
                // 暫停/刪除已由指令端處理狀態,這裡不能覆寫。
                return;
            }
            match result {
                Ok(()) => {
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
    }

    async fn run_task(&self, task: &Arc<HttpTask>, stop: &Arc<AtomicBool>) -> Result<(), TaskError> {
        let url = task.url.lock().unwrap().clone();

        // 續傳前置檢查:.part 不見了就只能從頭來。
        if task.downloaded() > 0 && !task.part_path().exists() {
            task.segments.lock().unwrap().clear();
            task.total_bytes.store(0, Ordering::Relaxed);
        }

        if task.segments.lock().unwrap().is_empty() {
            self.probe_and_prepare(task, &url).await?;
        } else if !task.range_supported.load(Ordering::Relaxed) {
            // 無 Range 支援的任務只能整檔重來。
            for s in task.segments.lock().unwrap().iter() {
                s.written.store(0, Ordering::Relaxed);
            }
            // .part 要清空重配置:total 未知時完整性檢查不會跑,
            // 上次殘留的尾巴資料會混進最終檔
            let file = tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(task.part_path())
                .await
                .map_err(TaskError::io)?;
            let total = task.total_bytes.load(Ordering::Relaxed);
            if total > 0 {
                file.set_len(total).await.map_err(TaskError::io)?;
            }
        }

        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        let part_path = task.part_path();
        let ranged = task.range_supported.load(Ordering::Relaxed);
        // 任一段失敗時令其他段也停下,與使用者暫停分開計。
        let abort = Arc::new(AtomicBool::new(false));

        let mut set = JoinSet::new();
        for seg in task.segments.lock().unwrap().iter() {
            let written = seg.written.load(Ordering::Relaxed);
            if seg.end != UNBOUNDED && seg.start + written >= seg.end {
                continue;
            }
            set.spawn(download_segment(
                self.client.clone(),
                url.clone(),
                part_path.clone(),
                seg.start,
                seg.end,
                seg.written.clone(),
                stop.clone(),
                abort.clone(),
                ranged,
            ));
        }

        let mut first_err: Option<TaskError> = None;
        while let Some(res) = set.join_next().await {
            let seg_result = res.unwrap_or_else(|e| {
                Err(TaskError {
                    message: format!("下載執行緒異常:{e}"),
                    retryable: true,
                })
            });
            if let Err(e) = seg_result {
                if first_err.is_none() {
                    first_err = Some(e);
                    abort.store(true, Ordering::Relaxed);
                }
            }
        }
        if let Some(e) = first_err {
            return Err(e);
        }
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        // 完整性檢查:全部段都到位才算完成(stream 提前斷線不會回 Err)。
        let total = task.total_bytes.load(Ordering::Relaxed);
        if total > 0 && task.downloaded() < total {
            return Err(TaskError {
                message: "網路中斷,可重試(下載不完整)".to_string(),
                retryable: true,
            });
        }

        let final_path = unique_path(&task.final_path());
        tokio::fs::rename(&part_path, &final_path)
            .await
            .map_err(TaskError::io)?;
        if let Some(name) = final_path.file_name() {
            *task.file_name.lock().unwrap() = name.to_string_lossy().into_owned();
        }
        Ok(())
    }

    /// 首次請求:確認狀態碼、取檔名(Content-Disposition)、總大小與 Range
    /// 支援度,然後預配置 .part 檔並切段。
    async fn probe_and_prepare(&self, task: &Arc<HttpTask>, url: &str) -> Result<(), TaskError> {
        let resp = self
            .client
            .get(url)
            .header(RANGE, "bytes=0-")
            .send()
            .await
            .map_err(TaskError::net)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(map_status_error(status, resp).await);
        }

        if let Some(name) = resp
            .headers()
            .get(CONTENT_DISPOSITION)
            .and_then(|v| filename_from_content_disposition(&String::from_utf8_lossy(v.as_bytes())))
        {
            let cleaned = sanitize_folder_name(&name);
            if !cleaned.is_empty() {
                *task.file_name.lock().unwrap() = cleaned;
            }
        }

        let ranged = status == StatusCode::PARTIAL_CONTENT;
        let total = resp
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(total_from_content_range)
            .or(resp.content_length())
            .unwrap_or(0);
        drop(resp); // 內容用分段請求重抓,這條連線只要 header

        task.range_supported.store(ranged, Ordering::Relaxed);
        task.total_bytes.store(total, Ordering::Relaxed);

        let segments = if ranged && total >= MIN_SPLIT_BYTES {
            let n = SEGMENT_COUNT;
            let base = total / n;
            (0..n)
                .map(|i| Segment {
                    start: i * base,
                    end: if i == n - 1 { total } else { (i + 1) * base },
                    written: Arc::new(AtomicU64::new(0)),
                })
                .collect()
        } else {
            vec![Segment {
                start: 0,
                end: if total > 0 { total } else { UNBOUNDED },
                written: Arc::new(AtomicU64::new(0)),
            }]
        };

        tokio::fs::create_dir_all(&task.dest_dir)
            .await
            .map_err(TaskError::io)?;
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(task.part_path())
            .await
            .map_err(TaskError::io)?;
        if total > 0 {
            file.set_len(total).await.map_err(TaskError::io)?;
        }

        *task.segments.lock().unwrap() = segments;
        self.persist();
        Ok(())
    }
}

/// 單一分段:從 start+written 處發 Range 請求,串流寫入 .part 對應偏移。
/// 整包資料不落記憶體,逐 chunk 寫盤。
#[allow(clippy::too_many_arguments)]
async fn download_segment(
    client: reqwest::Client,
    url: String,
    part_path: PathBuf,
    start: u64,
    end: u64,
    written: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    abort: Arc<AtomicBool>,
    ranged: bool,
) -> Result<(), TaskError> {
    let pos = start + written.load(Ordering::Relaxed);
    let mut req = client.get(&url);
    if ranged {
        let range = if end == UNBOUNDED {
            format!("bytes={pos}-")
        } else {
            format!("bytes={pos}-{}", end - 1)
        };
        req = req.header(RANGE, range);
    }
    let resp = req.send().await.map_err(TaskError::net)?;
    match resp.status() {
        StatusCode::PARTIAL_CONTENT => {}
        StatusCode::OK if !ranged || pos == 0 => {}
        StatusCode::OK => {
            // 要求 Range 卻回整檔 → 伺服器行為變了,續傳資料不可信。
            return Err(TaskError {
                message: "伺服器不再支援續傳,請刪除任務後重新加入".to_string(),
                retryable: false,
            });
        }
        status => return Err(map_status_error(status, resp).await),
    }

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&part_path)
        .await
        .map_err(TaskError::io)?;
    file.seek(std::io::SeekFrom::Start(pos))
        .await
        .map_err(TaskError::io)?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if stop.load(Ordering::Relaxed) || abort.load(Ordering::Relaxed) {
            let _ = file.flush().await;
            return Ok(());
        }
        let chunk = chunk.map_err(TaskError::net)?;
        file.write_all(&chunk).await.map_err(TaskError::io)?;
        written.fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }
    file.flush().await.map_err(TaskError::io)?;
    Ok(())
}

/// `filename*=UTF-8''…`(RFC 5987)優先,退回 `filename="…"`。
fn filename_from_content_disposition(value: &str) -> Option<String> {
    for part in value.split(';') {
        if let Some(rest) = part.trim().strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            let rest = rest
                .strip_prefix("UTF-8''")
                .or_else(|| rest.strip_prefix("utf-8''"))
                .unwrap_or(rest);
            if let Ok(s) = percent_decode_str(rest).decode_utf8() {
                if !s.is_empty() {
                    return Some(s.into_owned());
                }
            }
        }
    }
    for part in value.split(';') {
        if let Some(rest) = part.trim().strip_prefix("filename=") {
            let name = rest.trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// `Content-Range: bytes 0-499/1234` → 1234。
fn total_from_content_range(value: &str) -> Option<u64> {
    value.rsplit('/').next()?.trim().parse().ok()
}

/// URL path 最後一段(去 query、percent-decode)當預設檔名。
pub fn filename_from_url(url: &reqwest::Url) -> Option<String> {
    let last = url.path_segments()?.filter(|s| !s.is_empty()).last()?;
    let decoded = percent_decode_str(last)
        .decode_utf8()
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| last.to_string());
    let cleaned = sanitize_folder_name(&decoded);
    (!cleaned.is_empty()).then_some(cleaned)
}

/// 目標已存在時改用 "name (1).ext"、"name (2).ext"…
fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new(""));
    for i in 1.. {
        let candidate = dir.join(format!("{stem} ({i}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_disposition_plain() {
        assert_eq!(
            filename_from_content_disposition(r#"attachment; filename="movie.mkv""#),
            Some("movie.mkv".to_string())
        );
    }

    #[test]
    fn parses_content_disposition_rfc5987() {
        assert_eq!(
            filename_from_content_disposition(
                r#"attachment; filename="fallback.bin"; filename*=UTF-8''%E5%BD%B1%E7%89%87.mkv"#
            ),
            Some("影片.mkv".to_string())
        );
    }

    #[test]
    fn parses_content_range_total() {
        assert_eq!(total_from_content_range("bytes 0-499/1234"), Some(1234));
        assert_eq!(total_from_content_range("bytes */5000"), Some(5000));
        assert_eq!(total_from_content_range("bytes 0-499/*"), None);
    }

    #[test]
    fn filename_from_url_strips_query() {
        let url = reqwest::Url::parse(
            "https://example.com/files/movie.mkv?token=secret",
        )
        .unwrap();
        assert_eq!(filename_from_url(&url), Some("movie.mkv".to_string()));
    }
}
