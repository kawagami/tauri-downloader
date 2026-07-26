// 共用 HTTP client 建構 — 網站下載與直鏈下載用同一組逾時設定。

use std::time::Duration;

/// 連線建立逾時。
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// 單次 read 間隔逾時：server 停止傳輸時串流才不會永久卡住。
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .expect("failed to build reqwest client")
}
