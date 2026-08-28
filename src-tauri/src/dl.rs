// src/dl.rs
// 共用下載引擎 —— 網站下載與直鏈下載共用同一套位元組搬運：
// Range 探測、`.part` 預配置、分段並行、work stealing、限速、完整性檢查、完成 rename。
//
// 引擎只管「把 bytes 搬到磁碟」；任務管理（狀態機、錯誤呈現、持久化、UI 事件）
// 留在各自的呼叫端 —— 兩邊的任務模型差很多（web 有 DB/排序/縮圖，http 有 segments），
// 硬要共用同一個任務型別只會兩邊都長出對方的欄位。
//
// 兩邊的差異全部收在 JobConfig：
// - 直鏈：最多 4 段並行、用 Content-Disposition 覆蓋檔名、`.part` 摻 task id、
//   續傳偏移由呼叫端持久化
// - 網站：單段（`.part` 的長度本身就是續傳狀態，不必額外存）、檔名鎖死 `{title}.zip`

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use percent_encoding::percent_decode_str;
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_RANGE, RANGE};
use reqwest::StatusCode;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinSet;

use crate::utils::lock::LockExt;
use crate::utils::ratelimit::RateLimiter;

/// Content-Length 未知時 segment.end 的哨兵值。
pub const UNBOUNDED: u64 = u64::MAX;

// ---- 連線回收 / 分段看門狗 ----
//
// 大檔下載到一半掉速、手動暫停再恢復就回滿：長壽連線被伺服器配額型節流，
// 或遇丟包後擁塞窗口爬不回來。換一條新連線就好，所以引擎自己換，
// 不必等使用者發現。**兩者都只在支援 Range 時啟用**（否則接不回原位），
// 而且**限速開著時兩者都不做** —— 低速是設定要的，不是退化，
// 換線只是多付一次握手成本。

/// 單一連線搬完這麼多 bytes 就主動換新的。
const RECYCLE_BYTES: u64 = 512 * 1024 * 1024;
/// 單一連線活這麼久就主動換新的。
const RECYCLE_INTERVAL: Duration = Duration::from_secs(10 * 60);
/// 看門狗取樣窗口。
const WATCH_WINDOW: Duration = Duration::from_secs(5);
/// 窗口速率低於「本連線學到的峰值 × 此比例」算退化。
const DEGRADE_RATIO: f64 = 0.3;
/// 連續幾個窗口退化才換連線（單一窗口抖動不算）。
const DEGRADE_WINDOWS: u32 = 2;
/// 看門狗換連線的次數上限 —— 整體網路真的變慢時不該無限重連。
const MAX_WATCHDOG_RECONNECTS: u32 = 5;
/// 換了連線卻一個 byte 都沒拿到，最多再試幾次（防伺服器立刻關連線導致空轉）。
const MAX_IDLE_RECONNECTS: u32 = 3;
const IDLE_RECONNECT_DELAY: Duration = Duration::from_secs(1);

// ---- work stealing ----
//
// 固定等分切段的老問題（尾段效應）：四段一起開跑，先跑完的段就閒置，
// 最後幾 % 只剩最慢的那條連線在搬，速度階梯式下降。
// 解法是讓跑完的 worker 去「偷」還沒搬完的段的後半段：donor 的 end 縮小、
// 被偷走的區間變成一個新段。兩邊的位置都由 `written` 決定，所以偷完不丟進度；
// 持久化格式（start/end/written 的陣列）也完全不用改，只是陣列會變長。

/// 剩不到這麼多就不值得再切（多開一條連線的握手成本反而比較貴）。
const MIN_STEAL_BYTES: u64 = 4 * 1024 * 1024;

/// 換連線的原因（只影響計數與日誌）。
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Recycle {
    /// 到了 bytes/時間門檻，預防性換新
    Threshold,
    /// 看門狗判定速度退化
    Degraded,
    /// 伺服器提早關閉連線（段落還沒收完）
    EarlyClose,
}

impl Recycle {
    fn label(self) -> &'static str {
        match self {
            Recycle::Threshold => "連線回收門檻",
            Recycle::Degraded => "速度退化",
            Recycle::EarlyClose => "伺服器提早關閉連線",
        }
    }
}

/// 單條連線的結束原因。
enum SegOutcome {
    /// 這段的 bytes 都到位了
    Complete,
    /// 使用者暫停/刪除，或其他段失敗要求收工
    Stopped,
    Reconnect(Recycle),
}

/// 預防性換線判定（純函式，好測）。
fn recycle_due(conn_bytes: u64, conn_age: Duration) -> bool {
    conn_bytes >= RECYCLE_BYTES || conn_age >= RECYCLE_INTERVAL
}

/// 看門狗吃一個窗口的速率，回傳（新峰值, 新連續退化數, 是否該換線）。
///
/// 峰值是「這條連線自己」量到的最大值：換線後歸零重新學，
/// 整體網路真的變慢時新峰值就是那個慢速度，不會拿舊峰值反覆誤判。
fn watchdog_step(peak_bps: f64, degraded_windows: u32, window_bps: f64) -> (f64, u32, bool) {
    if window_bps > peak_bps {
        return (window_bps, 0, false);
    }
    if peak_bps > 0.0 && window_bps < peak_bps * DEGRADE_RATIO {
        let n = degraded_windows + 1;
        return (peak_bps, n, n >= DEGRADE_WINDOWS);
    }
    (peak_bps, 0, false)
}

/// 半開區間 [start, end)。
///
/// `end` 是 atomic 而不是普通欄位：work stealing 會在下載途中把它縮小，
/// 正在搬這一段的 worker 每個 chunk 都重讀一次，讀到縮小後的值就自己收工。
pub struct Segment {
    pub start: u64,
    end: AtomicU64,
    written: AtomicU64,
    /// 目前有沒有 worker 在搬這一段（純 runtime 狀態，不持久化）
    claimed: AtomicBool,
}

impl Segment {
    pub fn new(start: u64, end: u64, written: u64) -> Arc<Segment> {
        Arc::new(Segment {
            start,
            end: AtomicU64::new(end),
            written: AtomicU64::new(written),
            claimed: AtomicBool::new(false),
        })
    }

    pub fn end(&self) -> u64 {
        self.end.load(Ordering::Relaxed)
    }

    pub fn written(&self) -> u64 {
        self.written.load(Ordering::Relaxed)
    }

    /// 這一段還差多少 bytes（end 未知時回 0 —— 沒有中點可以拿來切）
    pub fn remaining(&self) -> u64 {
        let end = self.end();
        if end == UNBOUNDED {
            return 0;
        }
        end.saturating_sub(self.start + self.written())
    }

    fn is_complete(&self) -> bool {
        let end = self.end();
        end != UNBOUNDED && self.start + self.written() >= end
    }

    /// 認領這一段；已被別人認領則回 false。
    fn claim(&self) -> bool {
        self.claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn unclaim(&self) {
        self.claimed.store(false, Ordering::Release);
    }

    fn reset_written(&self) {
        self.written.store(0, Ordering::Relaxed);
    }
}

/// 分段清單。worker 要能在跑動中新增段（work stealing），所以整個 Vec 共享。
pub type Segments = Arc<Mutex<Vec<Arc<Segment>>>>;

/// 找一份新工作給剛跑完的 worker：
/// 1. 先撿沒人認領的既有段 —— 重啟續傳時 segments 可能多於 worker 數（上次偷過留下的），
///    這些段沒人撿就永遠不會被搬完。
/// 2. 沒有的話，從「剩最多」的那一段切一半搶過來。
///
/// 回 None 表示沒事可做，worker 收工。
fn steal_work(segments: &Mutex<Vec<Arc<Segment>>>) -> Option<Arc<Segment>> {
    let mut segs = segments.lock_safe();

    // `claim()` 有副作用（成功即認領），find 會停在第一個成功的那個
    if let Some(s) = segs.iter().find(|s| !s.is_complete() && s.claim()) {
        return Some(s.clone());
    }

    let donor = segs.iter().max_by_key(|s| s.remaining())?.clone();
    let end = donor.end();
    let remaining = donor.remaining();
    if end == UNBOUNDED || remaining < MIN_STEAL_BYTES {
        return None;
    }

    let mid = end - remaining / 2;
    donor.end.store(mid, Ordering::Relaxed);
    // donor 可能在我們讀 end 到 store 之間又寫了一個 chunk，越過了 mid。
    // 把 written 夾回新的段長：那一小段 bytes 由新段重下（同一個來源、內容一致），
    // 不夾的話同一段進度會被兩個 Segment 各算一次，downloaded() 就會超過 total。
    donor
        .written
        .fetch_min(mid.saturating_sub(donor.start), Ordering::Relaxed);

    let new = Segment::new(mid, end, 0);
    new.claim();
    segs.push(new.clone());
    Some(new)
}

/// 失敗分類 —— 呼叫端各自需要不同的分法：
/// http 看 `retryable`（決定顯示「重試」還是「更新連結」），
/// web 看 `kind == NotFound`（決定標 not_found 還是重抓 file_url）。
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ErrorKind {
    /// 404 / 410 — 永久失效
    NotFound,
    /// 401 / 403 — 授權過期，換連結才有救
    Auth,
    Other,
}

#[derive(Debug)]
pub struct EngineError {
    pub message: String,
    /// 網路類問題，原樣重試即可
    pub retryable: bool,
    pub kind: ErrorKind,
}

impl EngineError {
    fn net(e: reqwest::Error) -> Self {
        // without_url:reqwest 錯誤字串預設帶完整 URL(含 token),必須剝掉。
        EngineError {
            message: format!("網路中斷,可重試({})", e.without_url()),
            retryable: true,
            kind: ErrorKind::Other,
        }
    }

    fn io(e: std::io::Error) -> Self {
        EngineError {
            message: format!("寫檔失敗:{e}"),
            retryable: true,
            kind: ErrorKind::Other,
        }
    }

    fn other(message: impl Into<String>, retryable: bool) -> Self {
        EngineError {
            message: message.into(),
            retryable,
            kind: ErrorKind::Other,
        }
    }
}

/// 狀態碼 → 固定訊息與分類。抽出來是為了不用捏一個 `reqwest::Response` 就能測。
/// 回 None 表示要看 body 才知道細節。
fn classify_status(code: u16) -> Option<(&'static str, ErrorKind)> {
    match code {
        401 => Some(("連結已過期,請重新複製下載連結", ErrorKind::Auth)),
        403 => Some(("權限不足,請重新複製下載連結", ErrorKind::Auth)),
        // 410 Gone 與 404 一樣是永久失效（原本只認 404）
        404 | 410 => Some(("檔案已不存在", ErrorKind::NotFound)),
        _ => None,
    }
}

async fn map_status_error(status: StatusCode, resp: reqwest::Response) -> EngineError {
    if let Some((msg, kind)) = classify_status(status.as_u16()) {
        return EngineError {
            message: msg.to_string(),
            retryable: false,
            kind,
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
    EngineError {
        message: format!("HTTP {}{detail}", status.as_u16()),
        retryable: status.is_server_error(),
        kind: ErrorKind::Other,
    }
}

/// 兩個呼叫端的差異全部收在這裡。
pub struct JobConfig {
    /// 最多開幾條連線。>1 需要呼叫端自己持久化每段偏移，否則重啟接不回來。
    pub max_segments: u64,
    /// 小於此大小不分段（單一連線就夠快，切段只是多開連線）。
    pub min_split_bytes: u64,
    /// 首次回應的 Content-Disposition 是否可以覆蓋檔名。
    pub rename_from_headers: bool,
    /// `.part` 檔名後綴（接在 file_name 後）。同名任務靠它區分，不然會互寫。
    pub part_suffix: String,
    /// 單段模式下，用既有 `.part` 的長度當續傳起點。
    /// 免持久化的續傳：檔案長度本身就是狀態（多段就不行，偏移得另外存）。
    pub resume_from_part_len: bool,
}

/// 一次下載的全部狀態。呼叫端持有它，引擎讀寫它。
pub struct DownloadJob {
    pub url: Mutex<String>,
    pub dest_dir: PathBuf,
    pub file_name: Mutex<String>,
    /// 0 = 未知（伺服器沒給 Content-Length）
    pub total_bytes: AtomicU64,
    pub range_supported: AtomicBool,
    pub segments: Segments,
    pub cfg: JobConfig,
}

impl DownloadJob {
    pub fn downloaded(&self) -> u64 {
        self.segments.lock_safe().iter().map(|s| s.written()).sum()
    }

    pub fn part_path(&self) -> PathBuf {
        let name = self.file_name.lock_safe().clone();
        self.dest_dir.join(format!("{name}{}", self.cfg.part_suffix))
    }

    pub fn final_path(&self) -> PathBuf {
        let name = self.file_name.lock_safe().clone();
        self.dest_dir.join(name)
    }
}

/// 跑一次下載。成功回傳最終落地路徑（碰撞時會是 `name (1).ext`）。
///
/// `stop` 是使用者主動停（暫停/刪除/取消）：設了就提前返回 `Ok`，
/// 由呼叫端決定狀態，引擎不覆寫。失敗時 `.part` **保留**，下次可續傳。
pub async fn run(
    client: &reqwest::Client,
    job: &DownloadJob,
    stop: &Arc<AtomicBool>,
    limiter: Arc<RateLimiter>,
) -> Result<PathBuf, EngineError> {
    let url = job.url.lock_safe().clone();

    // 續傳前置檢查:.part 不見了就只能從頭來。
    if job.downloaded() > 0 && !job.part_path().exists() {
        job.segments.lock_safe().clear();
        job.total_bytes.store(0, Ordering::Relaxed);
    }

    let no_segments = job.segments.lock_safe().is_empty();
    if no_segments {
        probe_and_prepare(client, job, &url).await?;
    } else if !job.range_supported.load(Ordering::Relaxed) {
        // 無 Range 支援的任務只能整檔重來。
        for s in job.segments.lock_safe().iter() {
            s.reset_written();
        }
        // .part 要清空重來:total 未知時完整性檢查不會跑,
        // 上次殘留的尾巴資料會混進最終檔。
        // 不預配置大小：走到這裡一定是單段（分段的前提就是支援 Range），
        // 循序寫不需要，預配置還會毀掉「長度 = 進度」的續傳依據。
        tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(job.part_path())
            .await
            .map_err(EngineError::io)?;
    }

    if stop.load(Ordering::Relaxed) {
        return Err(EngineError::other("已停止", true));
    }

    let part_path = job.part_path();
    let ranged = job.range_supported.load(Ordering::Relaxed);
    // 任一段失敗時令其他段也停下,與使用者暫停分開計。
    let abort = Arc::new(AtomicBool::new(false));

    let initial: Vec<Arc<Segment>> = job.segments.lock_safe().clone();
    // 偷工作只在真的分段跑的任務上做：單段任務（小檔、無 Range、或網站下載那條
    // 靠 `.part` 長度續傳的路）一旦被切成多段，長度就不再等於進度。
    let steal_enabled = ranged && job.cfg.max_segments > 1 && initial.len() > 1;
    let max_workers = job.cfg.max_segments.max(1) as usize;

    // 上一輪（暫停前）留下的認領狀態要清掉，否則這輪誰都撿不到工作
    for s in &initial {
        s.unclaim();
    }

    let mut set = JoinSet::new();
    for seg in &initial {
        // 段數可能多於 worker 數（上次偷過）—— 剩下的留給先跑完的 worker 去撿
        if set.len() >= max_workers {
            break;
        }
        if seg.is_complete() || !seg.claim() {
            continue;
        }
        set.spawn(segment_worker(
            client.clone(),
            url.clone(),
            part_path.clone(),
            seg.clone(),
            job.segments.clone(),
            stop.clone(),
            abort.clone(),
            ranged,
            steal_enabled,
            limiter.clone(),
        ));
    }

    let mut first_err: Option<EngineError> = None;
    while let Some(res) = set.join_next().await {
        let seg_result =
            res.unwrap_or_else(|e| Err(EngineError::other(format!("下載執行緒異常:{e}"), true)));
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
        return Err(EngineError::other("已停止", true));
    }

    // 完整性檢查:全部段都到位才算完成(stream 提前斷線不會回 Err)。
    let total = job.total_bytes.load(Ordering::Relaxed);
    if total > 0 && job.downloaded() < total {
        return Err(EngineError::other("網路中斷,可重試(下載不完整)", true));
    }
    if total == 0 {
        // 沒有 Content-Length 就沒有東西可以比對，截斷的檔案看起來跟完整的一模一樣。
        // 至少讓日誌留下痕跡，事後查得到「這個檔從來沒被驗證過」。
        tracing::warn!("{:?}：伺服器未提供總大小，無法驗證下載完整性", part_path);
    }

    let final_path = unique_path(&job.final_path());
    tokio::fs::rename(&part_path, &final_path)
        .await
        .map_err(EngineError::io)?;
    if let Some(name) = final_path.file_name() {
        *job.file_name.lock_safe() = name.to_string_lossy().into_owned();
    }
    Ok(final_path)
}

/// 使用者主動停下（不是失敗）—— 呼叫端用它區分「要不要覆寫任務狀態」。
pub fn is_stopped(stop: &Arc<AtomicBool>) -> bool {
    stop.load(Ordering::Relaxed)
}

/// 單段模式的免持久化續傳：`.part` 現在多長就從那裡接（長度本身就是進度）。
/// 回 0 表示從頭來。
///
/// 幾個必要條件：
/// - 伺服器支援 Range 且知道總大小，否則接不回正確位置
/// - **不能是分段模式**：分段會把 `.part` 預配置成完整大小，長度就不再等於進度
/// - `part_len >= total` 一律不信：正常的完整檔早該被 rename 成正式檔名，
///   還留著多半是舊版預配置留下的空殼（後半段全是 0）。信了它就會直接
///   「跳過下載 → 完整性檢查通過 → rename」，交出一個大小正確但內容壞掉的檔案。
fn resume_offset(cfg: &JobConfig, ranged: bool, total: u64, split: bool, part_len: u64) -> u64 {
    if !cfg.resume_from_part_len || !ranged || split || total == 0 {
        return 0;
    }
    if part_len >= total {
        return 0;
    }
    part_len
}

/// 首次請求:確認狀態碼、取檔名(Content-Disposition)、總大小與 Range
/// 支援度,然後預配置 `.part` 檔並切段。
async fn probe_and_prepare(
    client: &reqwest::Client,
    job: &DownloadJob,
    url: &str,
) -> Result<(), EngineError> {
    let resp = client
        .get(url)
        .header(RANGE, "bytes=0-")
        .send()
        .await
        .map_err(EngineError::net)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(map_status_error(status, resp).await);
    }

    if job.cfg.rename_from_headers {
        if let Some(name) = resp
            .headers()
            .get(CONTENT_DISPOSITION)
            .and_then(|v| filename_from_content_disposition(&String::from_utf8_lossy(v.as_bytes())))
        {
            let cleaned = crate::torrent::commands::sanitize_folder_name(&name);
            if !cleaned.is_empty() {
                *job.file_name.lock_safe() = cleaned;
            }
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

    job.range_supported.store(ranged, Ordering::Relaxed);
    job.total_bytes.store(total, Ordering::Relaxed);

    tokio::fs::create_dir_all(&job.dest_dir)
        .await
        .map_err(EngineError::io)?;
    let part_path = job.part_path();

    let split = job.cfg.max_segments > 1 && ranged && total >= job.cfg.min_split_bytes;

    let part_len = tokio::fs::metadata(&part_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let resume_from = resume_offset(&job.cfg, ranged, total, split, part_len);

    let segments: Vec<Arc<Segment>> = if split {
        let n = job.cfg.max_segments;
        let base = total / n;
        (0..n)
            .map(|i| Segment::new(i * base, if i == n - 1 { total } else { (i + 1) * base }, 0))
            .collect()
    } else {
        vec![Segment::new(
            0,
            if total > 0 { total } else { UNBOUNDED },
            resume_from,
        )]
    };

    if resume_from > 0 {
        // 續傳：不能 truncate，既有內容就是進度本身
        tracing::info!("續傳 {:?}：從 {} bytes 接續", part_path, resume_from);
        tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false) // 刻意不清空：既有內容就是續傳進度本身
            .open(&part_path)
            .await
            .map_err(EngineError::io)?;
    } else {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&part_path)
            .await
            .map_err(EngineError::io)?;
        // 只有分段模式需要預配置整個檔案大小（各段 seek 到自己的偏移寫）。
        // 單段是循序 append，預配置反而會毀掉「.part 長度 = 已下載量」這個續傳依據。
        if split && total > 0 {
            file.set_len(total).await.map_err(EngineError::io)?;
        }
    }

    *job.segments.lock_safe() = segments;
    Ok(())
}

/// 一條 worker 的一生：搬完手上這段就去偷下一段，偷不到才收工。
/// 併發連線數等於 worker 數，不會因為偷工作而變多。
#[allow(clippy::too_many_arguments)]
async fn segment_worker(
    client: reqwest::Client,
    url: String,
    part_path: PathBuf,
    seg: Arc<Segment>,
    segments: Segments,
    stop: Arc<AtomicBool>,
    abort: Arc<AtomicBool>,
    ranged: bool,
    steal_enabled: bool,
    limiter: Arc<RateLimiter>,
) -> Result<(), EngineError> {
    let mut seg = seg;
    loop {
        download_segment(
            &client, &url, &part_path, &seg, &stop, &abort, ranged, &limiter,
        )
        .await?;

        if !steal_enabled || stop.load(Ordering::Relaxed) || abort.load(Ordering::Relaxed) {
            return Ok(());
        }
        match steal_work(&segments) {
            Some(next) => {
                tracing::info!(
                    "分段 @{} 收工，接手 [{}, {})",
                    seg.start,
                    next.start,
                    next.end()
                );
                seg = next;
            }
            None => return Ok(()),
        }
    }
}

/// 單一分段:從 start+written 處發 Range 請求,串流寫入 `.part` 對應偏移。
/// 整包資料不落記憶體,逐 chunk 寫盤。
///
/// 一個分段的生命週期裡可以換好幾條連線（回收門檻 / 看門狗 / 伺服器提早關線），
/// 每次都從 `start + written` 接續 —— 位置完全由 `written` 決定，換線不丟進度。
/// `end` 也是每個 chunk 重讀：別的 worker 可能剛把這一段的後半段偷走了。
#[allow(clippy::too_many_arguments)]
async fn download_segment(
    client: &reqwest::Client,
    url: &str,
    part_path: &Path,
    seg: &Arc<Segment>,
    stop: &Arc<AtomicBool>,
    abort: &Arc<AtomicBool>,
    ranged: bool,
    limiter: &Arc<RateLimiter>,
) -> Result<(), EngineError> {
    let stopped = || stop.load(Ordering::Relaxed) || abort.load(Ordering::Relaxed);
    let mut watchdog_used: u32 = 0;
    let mut idle_used: u32 = 0;

    loop {
        if stopped() {
            return Ok(());
        }
        let pos = seg.start + seg.written();
        let end = seg.end();
        if end != UNBOUNDED && pos >= end {
            return Ok(());
        }

        let mut req = client.get(url);
        if ranged {
            let range = if end == UNBOUNDED {
                format!("bytes={pos}-")
            } else {
                format!("bytes={pos}-{}", end - 1)
            };
            req = req.header(RANGE, range);
        }
        let resp = req.send().await.map_err(EngineError::net)?;
        match resp.status() {
            StatusCode::PARTIAL_CONTENT => {}
            StatusCode::OK if !ranged || pos == 0 => {}
            StatusCode::OK => {
                // 要求 Range 卻回整檔 → 伺服器行為變了,續傳資料不可信。
                return Err(EngineError {
                    message: "伺服器不再支援續傳,請刪除任務後重新加入".to_string(),
                    retryable: false,
                    kind: ErrorKind::Other,
                });
            }
            status => return Err(map_status_error(status, resp).await),
        }

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(part_path)
            .await
            .map_err(EngineError::io)?;
        file.seek(std::io::SeekFrom::Start(pos))
            .await
            .map_err(EngineError::io)?;

        let conn_start = Instant::now();
        let mut conn_bytes: u64 = 0;
        let mut peak_bps = 0.0f64;
        let mut degraded_windows: u32 = 0;
        let mut window_start = Instant::now();
        let mut window_bytes: u64 = 0;

        let mut stream = resp.bytes_stream();
        let outcome = loop {
            let Some(chunk) = stream.next().await else {
                // 串流自然結束：該收的都收到就是完成，否則是伺服器提早關線。
                // 不支援 Range 時接不回原位，只能交給 run() 的完整性檢查處理。
                break if seg.is_complete() || !ranged {
                    SegOutcome::Complete
                } else {
                    SegOutcome::Reconnect(Recycle::EarlyClose)
                };
            };
            if stopped() {
                break SegOutcome::Stopped;
            }
            let chunk = chunk.map_err(EngineError::net)?;

            // 這一段的 end 可能剛剛被別的 worker 縮小（work stealing）：
            // 只寫到 end 為止，超出的部分留給偷走它的那條 worker 自己去抓。
            // 不夾的話兩條 worker 會各自把同一段 bytes 計進 written，進度會超過 100%。
            let end_now = seg.end();
            let mut n = chunk.len() as u64;
            if end_now != UNBOUNDED {
                let room = end_now.saturating_sub(seg.start + seg.written());
                if room == 0 {
                    break SegOutcome::Complete;
                }
                n = n.min(room);
            }
            file.write_all(&chunk[..n as usize])
                .await
                .map_err(EngineError::io)?;
            seg.written.fetch_add(n, Ordering::Relaxed);
            conn_bytes += n;
            window_bytes += n;
            // 全域限速：多個分段共用同一顆桶，合計吞吐才是設定值
            if !limiter.acquire(n, stopped).await {
                break SegOutcome::Stopped;
            }
            if seg.is_complete() {
                break SegOutcome::Complete;
            }

            if !ranged {
                continue;
            }
            // 限速開著時既不預防性回收也不看門：低速是設定要的，不是退化
            if limiter.limit().load(Ordering::Relaxed) != 0 {
                continue;
            }
            if recycle_due(conn_bytes, conn_start.elapsed()) {
                break SegOutcome::Reconnect(Recycle::Threshold);
            }
            let elapsed = window_start.elapsed();
            if elapsed >= WATCH_WINDOW && watchdog_used < MAX_WATCHDOG_RECONNECTS {
                let bps = window_bytes as f64 / elapsed.as_secs_f64();
                window_start = Instant::now();
                window_bytes = 0;
                let (peak, degraded, recycle) = watchdog_step(peak_bps, degraded_windows, bps);
                peak_bps = peak;
                degraded_windows = degraded;
                if recycle {
                    break SegOutcome::Reconnect(Recycle::Degraded);
                }
            }
        };
        file.flush().await.map_err(EngineError::io)?;
        drop(file);

        let kind = match outcome {
            SegOutcome::Complete | SegOutcome::Stopped => return Ok(()),
            SegOutcome::Reconnect(kind) => kind,
        };
        if kind == Recycle::Degraded {
            watchdog_used += 1;
        }
        tracing::info!(
            "分段 @{} 換連線（{}）：本條連線搬了 {} bytes，已完成 {} bytes",
            seg.start,
            kind.label(),
            conn_bytes,
            seg.written()
        );
        // 一個 byte 都沒拿到就換線，可能是伺服器直接關線 → 有限次數 + 間隔，別空轉
        if conn_bytes == 0 {
            idle_used += 1;
            if idle_used > MAX_IDLE_RECONNECTS {
                return Err(EngineError::other(
                    format!("連線重建 {MAX_IDLE_RECONNECTS} 次仍無進度（{}）", kind.label()),
                    true,
                ));
            }
            tokio::time::sleep(IDLE_RECONNECT_DELAY).await;
        } else {
            idle_used = 0;
        }
    }
}

/// `filename*=UTF-8''…`(RFC 5987)優先,退回 `filename="…"`。
pub fn filename_from_content_disposition(value: &str) -> Option<String> {
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
pub fn total_from_content_range(value: &str) -> Option<u64> {
    value.rsplit('/').next()?.trim().parse().ok()
}

/// 目標已存在時改用 "name (1).ext"、"name (2).ext"…
pub fn unique_path(path: &Path) -> PathBuf {
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

    fn job_with_suffix(suffix: &str) -> DownloadJob {
        DownloadJob {
            url: Mutex::new(String::new()),
            dest_dir: PathBuf::from("/dl"),
            file_name: Mutex::new("a.zip".into()),
            total_bytes: AtomicU64::new(0),
            range_supported: AtomicBool::new(false),
            segments: Arc::new(Mutex::new(Vec::new())),
            cfg: JobConfig {
                max_segments: 1,
                min_split_bytes: 0,
                rename_from_headers: false,
                part_suffix: suffix.to_string(),
                resume_from_part_len: false,
            },
        }
    }

    /// part_suffix 決定 `.part` 檔名：同名任務靠它區分，不然會互寫同一個檔
    #[test]
    fn part_path_follows_config_suffix() {
        assert_eq!(
            job_with_suffix(".part").part_path(),
            PathBuf::from("/dl/a.zip.part")
        );
        assert_eq!(
            job_with_suffix(".7.part").part_path(),
            PathBuf::from("/dl/a.zip.7.part")
        );
    }

    fn web_cfg() -> JobConfig {
        JobConfig {
            max_segments: 1,
            min_split_bytes: 0,
            rename_from_headers: false,
            part_suffix: ".part".into(),
            resume_from_part_len: true,
        }
    }

    /// 正常續傳：`.part` 有一半就從一半接
    #[test]
    fn resumes_from_part_length() {
        assert_eq!(resume_offset(&web_cfg(), true, 1000, false, 400), 400);
    }

    /// 「暫停後再繼續變成完成」的成因：`.part` 被預配置成完整大小，
    /// 長度不再代表進度。長度 >= total 一律當作不可信、從頭來，
    /// 否則會跳過下載直接 rename，交出一個後半段全是 0 的壞檔。
    #[test]
    fn preallocated_part_is_not_trusted_as_progress() {
        assert_eq!(resume_offset(&web_cfg(), true, 1000, false, 1000), 0);
        assert_eq!(resume_offset(&web_cfg(), true, 1000, false, 4096), 0);
    }

    /// 分段模式的 `.part` 一定是預配置過的，長度絕不能拿來當進度
    /// （分段的續傳偏移由呼叫端持久化）
    #[test]
    fn split_mode_never_resumes_from_length() {
        assert_eq!(resume_offset(&web_cfg(), true, 1000, true, 400), 0);
    }

    /// 沒有 Range 支援或不知道總大小就接不回正確位置
    #[test]
    fn resume_needs_range_and_known_total() {
        assert_eq!(resume_offset(&web_cfg(), false, 1000, false, 400), 0);
        assert_eq!(resume_offset(&web_cfg(), true, 0, false, 400), 0);
    }

    /// 直鏈那邊靠 persisted segments，不吃這條路
    #[test]
    fn disabled_by_config() {
        let mut cfg = web_cfg();
        cfg.resume_from_part_len = false;
        assert_eq!(resume_offset(&cfg, true, 1000, false, 400), 0);
    }

    /// 預防性換線：bytes 或時間任一到門檻就換
    #[test]
    fn recycle_triggers_on_bytes_or_age() {
        assert!(!recycle_due(1024, Duration::from_secs(5)));
        assert!(recycle_due(RECYCLE_BYTES, Duration::from_secs(5)));
        assert!(recycle_due(1024, RECYCLE_INTERVAL));
    }

    /// 速度上升就更新峰值，退化計數歸零
    #[test]
    fn watchdog_learns_peak() {
        let (peak, degraded, recycle) = watchdog_step(0.0, 0, 1_000_000.0);
        assert_eq!(peak, 1_000_000.0);
        assert_eq!(degraded, 0);
        assert!(!recycle);
    }

    /// 連續兩個窗口掉到峰值三成以下才換線（單一窗口抖動不算）
    #[test]
    fn watchdog_needs_consecutive_degraded_windows() {
        let (peak, degraded, recycle) = watchdog_step(1_000_000.0, 0, 100_000.0);
        assert_eq!(peak, 1_000_000.0);
        assert_eq!(degraded, 1);
        assert!(!recycle, "第一個退化窗口不換線");
        let (_, degraded, recycle) = watchdog_step(peak, degraded, 100_000.0);
        assert_eq!(degraded, DEGRADE_WINDOWS);
        assert!(recycle);
    }

    /// 小幅波動（掉到五成）不算退化，計數還要歸零
    #[test]
    fn watchdog_ignores_mild_dips() {
        let (_, degraded, recycle) = watchdog_step(1_000_000.0, 1, 500_000.0);
        assert_eq!(degraded, 0);
        assert!(!recycle);
    }

    /// 404/410 都要歸到 NotFound（web 靠這個標 not_found）、401/403 歸 Auth
    #[test]
    fn status_codes_are_classified() {
        assert_eq!(classify_status(404).unwrap().1, ErrorKind::NotFound);
        assert_eq!(classify_status(410).unwrap().1, ErrorKind::NotFound);
        assert_eq!(classify_status(401).unwrap().1, ErrorKind::Auth);
        assert_eq!(classify_status(403).unwrap().1, ErrorKind::Auth);
        // 其餘要看 body 才知道細節
        assert!(classify_status(500).is_none());
        assert!(classify_status(400).is_none());
    }

    // ---- work stealing ----

    const M: u64 = 1024 * 1024;

    /// 尾段效應的解法：跑完的 worker 把「剩最多」那一段的後半段搶過來，
    /// donor 的 end 跟著縮小，兩段合起來仍然剛好覆蓋原區間（不重不漏）。
    #[test]
    fn steal_splits_the_segment_with_most_left() {
        let done = Segment::new(0, 100 * M, 100 * M);
        let busy = Segment::new(100 * M, 200 * M, 20 * M); // 剩 80M
        done.claim();
        busy.claim();
        let segs = Mutex::new(vec![done, busy.clone()]);

        let stolen = steal_work(&segs).expect("剩 80M 該切得下去");
        assert_eq!(stolen.start, 160 * M);
        assert_eq!(stolen.end(), 200 * M);
        assert_eq!(busy.end(), 160 * M, "donor 的 end 要跟著縮");
        assert_eq!(busy.end(), stolen.start, "兩段要接得上，不重不漏");
        assert_eq!(segs.lock_safe().len(), 3);
    }

    /// 偷完之後 donor 的 written 不能超過它剩下的段長，
    /// 否則同一段 bytes 會被兩個 Segment 各算一次、進度超過 100%
    #[test]
    fn steal_keeps_donor_progress_within_its_new_range() {
        let busy = Segment::new(0, 100 * M, 30 * M);
        busy.claim();
        let segs = Mutex::new(vec![busy.clone()]);

        let stolen = steal_work(&segs).expect("剩 70M 該切得下去");
        assert!(busy.written() <= busy.end() - busy.start);
        // 兩段的長度加起來仍是原本的區間
        assert_eq!(
            (busy.end() - busy.start) + (stolen.end() - stolen.start),
            100 * M
        );
    }

    /// 先撿沒人認領的既有段 —— 重啟續傳時 segments 會多於 worker 數，
    /// 這些段沒人撿就永遠搬不完
    #[test]
    fn steal_prefers_unclaimed_existing_segment() {
        let busy = Segment::new(0, 100 * M, 0);
        busy.claim();
        let orphan = Segment::new(100 * M, 120 * M, 0);
        let segs = Mutex::new(vec![busy, orphan.clone()]);

        let got = steal_work(&segs).expect("該撿到孤兒段");
        assert!(Arc::ptr_eq(&got, &orphan));
        assert_eq!(segs.lock_safe().len(), 2, "撿現成的不該新增段");
    }

    /// 剩太少就不值得再開一條連線
    #[test]
    fn steal_declines_tiny_remainder() {
        let s = Segment::new(0, MIN_STEAL_BYTES - 1, 0);
        s.claim();
        assert!(steal_work(&Mutex::new(vec![s])).is_none());
    }

    /// 總大小未知（UNBOUNDED）沒有中點可切
    #[test]
    fn steal_declines_unbounded_segment() {
        let s = Segment::new(0, UNBOUNDED, 0);
        s.claim();
        assert!(steal_work(&Mutex::new(vec![s])).is_none());
    }

    /// 一段可以被反覆切下去，直到剩的量低於門檻；
    /// 切完的區間必須首尾相接、合起來還是原本那一段
    #[test]
    fn steal_can_split_repeatedly_until_threshold() {
        let busy = Segment::new(0, 64 * M, 0);
        busy.claim();
        let segs = Mutex::new(vec![busy]);

        let mut splits = 0;
        while steal_work(&segs).is_some() {
            splits += 1;
            assert!(splits < 32, "不該無限切下去");
        }
        assert!(splits >= 3, "64M 至少切得出幾塊，實得 {splits}");

        let mut ranges: Vec<(u64, u64)> =
            segs.lock_safe().iter().map(|s| (s.start, s.end())).collect();
        ranges.sort();
        assert_eq!(ranges.first().unwrap().0, 0);
        assert_eq!(ranges.last().unwrap().1, 64 * M);
        for w in ranges.windows(2) {
            assert_eq!(w[0].1, w[1].0, "區間要首尾相接");
        }
    }
}
