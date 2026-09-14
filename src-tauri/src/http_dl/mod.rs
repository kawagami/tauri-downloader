// HTTP 直鏈下載模組 — 自 magnet-downloader 專案移植。
// 與 BT 引擎獨立；位元組搬運與網站下載共用 crate::dl 引擎，這裡只管任務層:
// 任意直鏈、Range 分段並行、斷點續傳、token 過期換連結接續。持久化存 app_data_dir/http_tasks.json。
pub mod commands;
pub mod events;
pub mod manager;
