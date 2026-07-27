// src/download_core.rs
// 網站下載的速度/ETA 計算。直鏈下載走 http_dl/events.rs 的 tick 差值，
// 這裡是同一套想法的單任務版本。

use std::time::{Duration, Instant};

/// 未知值哨兵（總大小拿不到時，percentage 與 ETA 都無從算起）。
/// 不用 f64::INFINITY：serde_json 會把非有限浮點序列化成 null，
/// 但前端型別宣告是 number，等於埋一個假型別。
const UNKNOWN: f64 = -1.0;

/// 兩次取樣的最小間隔。比這短就沿用上次的速度，避免除以趨近 0 的時間差
/// 炸出離譜數字（下載結束時會緊接著補發一次最終進度）。
const MIN_SAMPLE: Duration = Duration::from_millis(100);

/// 下載進度詳細信息
#[derive(Debug, Clone)]
pub struct ProgressMetrics {
    pub total_size: u64,          // 總文件大小 (Bytes)
    pub downloaded_size: u64,     // 已下載大小 (Bytes)
    pub percentage: f64,          // 進度百分比 (0.0 - 100.0)，-1 = 未知
    pub speed_bytes_per_sec: f64, // 當前下載速度 (Bytes/sec)
    pub time_remaining_secs: f64, // 預計剩餘時間 (秒)，-1 = 未知
}

/// 核心下載管理器
pub struct DownloadManager {
    /// 起始時間 — 只用在第一次取樣（還沒有前一個樣本可以比）
    start_time: Option<Instant>,
    /// 上次取樣的（時間, 已下載 bytes）
    last_sample: Option<(Instant, u64)>,
    /// 上次算出的速度，取樣間隔太短時沿用
    last_speed: f64,
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadManager {
    pub fn new() -> Self {
        DownloadManager {
            start_time: None,
            last_sample: None,
            last_speed: 0.0,
        }
    }

    pub fn start_download(&mut self, total_size: u64) {
        self.start_time = Some(Instant::now());
        self.last_sample = None;
        self.last_speed = 0.0;
        tracing::debug!("Core: 下載啟動，總大小: {} Bytes", total_size);
    }

    /// 根據當前數據和時間計算最新的 ProgressMetrics。
    /// 速度取「距上次取樣的差值」而非整段平均 —— 平均值會讓網速變化後的
    /// ETA 長時間失真（前段慢後段快時尤其明顯）。
    pub fn calculate_metrics(&mut self, downloaded: u64, total: u64) -> ProgressMetrics {
        let now = Instant::now();

        let speed = match self.last_sample {
            Some((t, bytes)) => {
                let elapsed = now.duration_since(t);
                if elapsed >= MIN_SAMPLE {
                    let s = downloaded.saturating_sub(bytes) as f64 / elapsed.as_secs_f64();
                    self.last_sample = Some((now, downloaded));
                    self.last_speed = s;
                    s
                } else {
                    self.last_speed
                }
            }
            // 第一次取樣沒有前一個樣本，用「開始到現在」的平均墊著
            None => {
                let elapsed = self
                    .start_time
                    .map_or(0.0, |start| now.duration_since(start).as_secs_f64());
                let s = if elapsed > 0.0 {
                    downloaded as f64 / elapsed
                } else {
                    0.0
                };
                self.last_sample = Some((now, downloaded));
                self.last_speed = s;
                s
            }
        };

        // total == 0 表示沒有 Content-Length，進度與 ETA 都無從計算
        let percentage = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            UNKNOWN
        };

        let time_remaining_secs = if total == 0 || speed <= 0.0 {
            UNKNOWN
        } else {
            total.saturating_sub(downloaded) as f64 / speed
        };

        ProgressMetrics {
            total_size: total,
            downloaded_size: downloaded,
            percentage,
            speed_bytes_per_sec: speed,
            time_remaining_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_total_reports_sentinels_not_infinity() {
        let mut m = DownloadManager::new();
        m.start_download(0);
        let metrics = m.calculate_metrics(1024, 0);
        assert_eq!(metrics.percentage, UNKNOWN);
        assert_eq!(metrics.time_remaining_secs, UNKNOWN);
        // 非有限浮點會被 serde_json 序列化成 null，前端型別會對不上
        assert!(metrics.time_remaining_secs.is_finite());
    }

    /// 速度看的是「距上次取樣」的差值：後半段停住就該掉下來，不是被前半段的平均拖著
    #[test]
    fn speed_uses_recent_window_not_lifetime_average() {
        let mut m = DownloadManager::new();
        m.start_download(1000);

        std::thread::sleep(Duration::from_millis(120));
        let fast = m.calculate_metrics(600, 1000);
        assert!(fast.speed_bytes_per_sec > 0.0);

        // 這段完全沒有進展 → 即時速度應該掉到 0（整段平均仍會是正的）
        std::thread::sleep(Duration::from_millis(120));
        let stalled = m.calculate_metrics(600, 1000);
        assert_eq!(stalled.speed_bytes_per_sec, 0.0);
        assert_eq!(stalled.time_remaining_secs, UNKNOWN);
    }

    /// 取樣間隔過短時沿用上次速度，不會除以趨近 0 的時間差
    #[test]
    fn reuses_last_speed_for_rapid_samples() {
        let mut m = DownloadManager::new();
        m.start_download(1000);
        std::thread::sleep(Duration::from_millis(120));
        let first = m.calculate_metrics(500, 1000);
        let immediate = m.calculate_metrics(500, 1000);
        assert_eq!(first.speed_bytes_per_sec, immediate.speed_bytes_per_sec);
    }
}
