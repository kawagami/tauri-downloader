// src/logging.rs
// 日誌初始化 — 沒有 subscriber 的話全專案的 tracing::error!/warn! 都是丟進黑洞，
// 出事時完全沒線索（monitor 抓取失敗、DB 錯誤、BT 啟動細節都只走 tracing）。
//
// 輸出兩路：app_data_dir/logs/app.log（每日輪替、保留 7 天）＋ stderr（dev 時看得到）。
// 預設過濾自家 crate info、其餘 warn — librqbit/hyper 的 debug 量大到會蓋掉自己的訊息。

use std::path::Path;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

const DEFAULT_FILTER: &str = "warn,tauri_downloader_lib=info";
/// 覆寫過濾用的環境變數（值同 RUST_LOG 語法，例：`tauri_downloader_lib=debug`）
const FILTER_ENV: &str = "TAURI_DOWNLOADER_LOG";
/// 保留幾天份的日誌檔
const MAX_LOG_FILES: usize = 7;

fn filter() -> EnvFilter {
    EnvFilter::try_from_env(FILTER_ENV).unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER))
}

/// 建 rolling appender；建不起來（磁碟權限等）就只留 stderr，不擋 app 啟動。
fn file_appender(app_data_dir: &Path) -> Option<RollingFileAppender> {
    let logs_dir = app_data_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&logs_dir) {
        eprintln!("建立日誌目錄失敗 {:?}: {}", logs_dir, e);
        return None;
    }
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("app")
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(&logs_dir)
        .map_err(|e| eprintln!("建立日誌檔失敗 {:?}: {}", logs_dir, e))
        .ok()
}

/// 重複呼叫安全（try_init 失敗即忽略）。app_data_dir 須已存在或可建立。
pub fn init(app_data_dir: &Path) {
    let file_layer = file_appender(app_data_dir).map(|w| {
        fmt::layer()
            .with_ansi(false)
            .with_writer(w)
            .with_filter(filter())
    });

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(fmt::layer().with_writer(std::io::stderr).with_filter(filter()))
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// appender 真的會在 logs/ 下建出檔案（只驗 IO，不碰全域 subscriber）
    #[test]
    fn file_appender_writes_into_logs_dir() {
        let dir = std::env::temp_dir().join(format!("log-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut appender = file_appender(&dir).expect("appender 應該建得起來");
        writeln!(appender, "hello").unwrap();
        appender.flush().unwrap();

        let logs: Vec<_> = std::fs::read_dir(dir.join("logs"))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(logs.len(), 1, "應該只有一個當日日誌檔");
        let name = logs[0].file_name().to_string_lossy().into_owned();
        assert!(name.starts_with("app.") && name.ends_with(".log"), "{name}");
        assert!(std::fs::read_to_string(logs[0].path()).unwrap().contains("hello"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 預設過濾字串本身要合法（打錯會靜默退成空過濾器，等於沒日誌）
    #[test]
    fn default_filter_is_valid() {
        assert!(EnvFilter::try_new(DEFAULT_FILTER).is_ok());
    }
}
