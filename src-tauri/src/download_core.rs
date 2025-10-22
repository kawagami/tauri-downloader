// src/download_core.rs

// 引入計時相關
use std::time::{Duration, Instant};

/// 下載進度詳細信息
#[derive(Debug, Clone)]
pub struct ProgressMetrics {
    pub total_size: u64,          // 總文件大小 (Bytes)
    pub downloaded_size: u64,     // 已下載大小 (Bytes)
    pub percentage: f64,          // 進度百分比 (0.0 - 100.0)
    pub speed_bytes_per_sec: f64, // 當前下載速度 (Bytes/sec)
    pub time_remaining_secs: f64, // 預計剩餘時間 (秒)
}

/// 下載狀態的列舉
#[derive(Debug, Clone)]
pub enum DownloadState {
    Pending,                      // 等待中/初始化
    Downloading(ProgressMetrics), // 正在下載，附帶進度數據
    Completed,                    // 完成
    Failed(String),               // 失敗，附帶錯誤信息
}

/// 核心下載管理器
pub struct DownloadManager {
    // 追蹤下載的起始時間，用於計算速度和 ETR
    start_time: Option<Instant>,
    // 這裡可以存放更多下載相關的狀態或配置
}

impl DownloadManager {
    pub fn new() -> Self {
        DownloadManager { start_time: None }
    }

    /// 初始化並啟動下載（模擬邏輯）
    pub fn start_download(&mut self, total_size: u64) {
        // 真正開始下載時，記錄時間
        self.start_time = Some(Instant::now());

        // 實際的下載啟動邏輯 (例如：建立網路連線)
        println!("Core: 下載啟動，總大小: {} Bytes", total_size);
    }

    /// 根據當前數據和時間計算最新的 ProgressMetrics
    pub fn calculate_metrics(&self, downloaded: u64, total: u64) -> ProgressMetrics {
        let percentage = (downloaded as f64 / total as f64) * 100.0;

        // --- 計算速度和剩餘時間 ---
        let elapsed = self
            .start_time
            .map_or(Duration::ZERO, |start| start.elapsed());

        let elapsed_secs = elapsed.as_secs_f64();

        let speed = if elapsed_secs > 0.0 {
            downloaded as f64 / elapsed_secs
        } else {
            0.0
        };

        let remaining_bytes = total.saturating_sub(downloaded) as f64;

        let time_remaining_secs = if speed > 0.0 {
            remaining_bytes / speed
        } else {
            f64::INFINITY // 速度為零，視為無限剩餘時間
        };
        // -------------------------

        ProgressMetrics {
            total_size: total,
            downloaded_size: downloaded,
            percentage,
            speed_bytes_per_sec: speed,
            time_remaining_secs,
        }
    }
}
