// HTTP 直鏈下載模組 — 自 magnet-downloader 專案移植。
// 與 BT 引擎、網站下載完全獨立:任意直鏈、Range 分段並行、斷點續傳、
// token 過期換連結接續。持久化存 app_data_dir/http_tasks.json。
pub mod commands;
pub mod events;
pub mod manager;
