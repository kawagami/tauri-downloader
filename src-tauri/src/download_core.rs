// src/download_core.rs
// 網站下載的速度/ETA 計算。直鏈下載走 http_dl/events.rs 的每秒 tick 差值，
// 這裡是同一套想法的單任務版本，但取樣間隔更短所以要另外平滑。

use std::time::{Duration, Instant};

/// 未知值哨兵（總大小拿不到時，percentage 與 ETA 都無從算起）。
/// 不用 f64::INFINITY：serde_json 會把非有限浮點序列化成 null，
/// 但前端型別宣告是 number，等於埋一個假型別。
const UNKNOWN: f64 = -1.0;

/// 兩次取樣的最小間隔。呼叫端每 250ms 問一次，但窗口這麼短時 chunk 到達是突發的
/// （某個窗口 0 bytes、下個窗口一次來一大包），算出來的瞬時速度會在 0 和爆量之間跳。
const MIN_SAMPLE: Duration = Duration::from_millis(500);

/// 指數移動平均權重：新樣本佔 30%，時間常數約 1.7 秒。
/// 純瞬時值太跳、純累計平均又反應不了網速變化，取中間。
const EMA_ALPHA: f64 = 0.3;

/// 下載進度詳細信息
#[derive(Debug, Clone)]
pub struct ProgressMetrics {
    pub total_size: u64,          // 總文件大小 (Bytes)
    pub downloaded_size: u64,     // 已下載大小 (Bytes)
    pub percentage: f64,          // 進度百分比 (0.0 - 100.0)，-1 = 未知
    pub speed_bytes_per_sec: f64, // 平滑後的下載速度 (Bytes/sec)
    pub time_remaining_secs: f64, // 預計剩餘時間 (秒)，-1 = 未知
}

/// 核心下載管理器
pub struct DownloadManager {
    /// 上次取樣的（時間, 已下載 bytes）
    last_sample: Option<(Instant, u64)>,
    /// 平滑後的速度，取樣間隔還沒到就沿用
    speed: f64,
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadManager {
    pub fn new() -> Self {
        DownloadManager {
            last_sample: None,
            speed: 0.0,
        }
    }

    pub fn start_download(&mut self, total_size: u64) {
        self.last_sample = None;
        self.speed = 0.0;
        tracing::debug!("Core: 下載啟動，總大小: {} Bytes", total_size);
    }

    /// 根據當前數據和時間計算最新的 ProgressMetrics。
    ///
    /// 速度取「距上次取樣的差值」再過一層 EMA：
    /// - 整段平均會讓網速變化後的 ETA 長時間失真
    /// - 純瞬時差值在 250ms 這種短窗口下會劇烈跳動（UI 讀起來就是一直閃）
    pub fn calculate_metrics(&mut self, downloaded: u64, total: u64) -> ProgressMetrics {
        let now = Instant::now();

        match self.last_sample {
            // 第一次只記基準點、速度先當 0。
            // 不能用「downloaded / 從開始到現在」墊：續傳時 downloaded 一開始就是
            // 已完成的那幾 MB，除以 0.25 秒會噴出離譜的速度。
            None => {
                self.last_sample = Some((now, downloaded));
                self.speed = 0.0;
            }
            Some((t, bytes)) => {
                let elapsed = now.duration_since(t);
                if elapsed >= MIN_SAMPLE {
                    let sample = downloaded.saturating_sub(bytes) as f64 / elapsed.as_secs_f64();
                    // 第一個真正的樣本直接採用，不然畫面會從 0 慢慢爬上來
                    self.speed = if self.speed == 0.0 {
                        sample
                    } else {
                        EMA_ALPHA * sample + (1.0 - EMA_ALPHA) * self.speed
                    };
                    self.last_sample = Some((now, downloaded));
                }
                // 間隔沒到就沿用上次的平滑值
            }
        }

        // total == 0 表示沒有 Content-Length，進度與 ETA 都無從計算
        let percentage = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            UNKNOWN
        };

        let time_remaining_secs = if total == 0 || self.speed <= 0.0 {
            UNKNOWN
        } else {
            total.saturating_sub(downloaded) as f64 / self.speed
        };

        ProgressMetrics {
            total_size: total,
            downloaded_size: downloaded,
            percentage,
            speed_bytes_per_sec: self.speed,
            time_remaining_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 取樣間隔的兩倍，確保跨過 MIN_SAMPLE
    const TICK: Duration = Duration::from_millis(600);

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

    /// 續傳時第一次取樣不能拿「已完成的量 / 剛過的 0.25 秒」當速度
    #[test]
    fn first_sample_does_not_spike_on_resume() {
        let mut m = DownloadManager::new();
        m.start_download(100_000_000);
        // 續傳：一開始 downloaded 就已經是 50MB
        let first = m.calculate_metrics(50_000_000, 100_000_000);
        assert_eq!(first.speed_bytes_per_sec, 0.0, "第一次取樣只該記基準點");
    }

    /// 速度看的是「距上次取樣」的差值：後半段停住就該掉下來，
    /// 但經過 EMA 平滑，是漸降而不是瞬間歸零（歸零會讓 UI 元素閃掉）
    #[test]
    fn speed_decays_when_transfer_stalls() {
        let mut m = DownloadManager::new();
        m.start_download(1000);

        m.calculate_metrics(0, 1000); // 基準點
        std::thread::sleep(TICK);
        let fast = m.calculate_metrics(600, 1000);
        assert!(fast.speed_bytes_per_sec > 0.0);

        // 這段完全沒有進展 → 速度下降但不會直接變 0
        std::thread::sleep(TICK);
        let stalled = m.calculate_metrics(600, 1000);
        assert!(stalled.speed_bytes_per_sec < fast.speed_bytes_per_sec);
        assert!(stalled.speed_bytes_per_sec > 0.0, "EMA 不該一步歸零");
    }

    /// 取樣間隔沒到就沿用上次的平滑值，不會除以趨近 0 的時間差
    #[test]
    fn reuses_last_speed_for_rapid_samples() {
        let mut m = DownloadManager::new();
        m.start_download(1000);
        m.calculate_metrics(0, 1000);
        std::thread::sleep(TICK);
        let first = m.calculate_metrics(500, 1000);
        let immediate = m.calculate_metrics(500, 1000);
        assert_eq!(first.speed_bytes_per_sec, immediate.speed_bytes_per_sec);
    }
}
