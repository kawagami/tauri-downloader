// src/state.rs
use reqwest::Client;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use crate::utils::{net::build_client, ratelimit::RateLimiter};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub client: Client,
    pub monitor_running: Arc<AtomicBool>,
    pub monitor_paused: Arc<AtomicBool>,
    /// 進行中的網站下載 → 各自的取消旗標（key = 任務 url）。
    /// 以前是單一全域 `AtomicBool`，一旦並行就會互相取消，所以 UI 得靠 disable
    /// 按鈕封住並行入口；改成 per-task 之後就沒有這個限制了。
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// 網站下載限速器（bytes/s，0 = 無限制），限速值即時可調
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(db: Connection, monitor_running: Arc<AtomicBool>) -> Self {
        Self {
            db: Mutex::new(db),
            client: build_client(),
            monitor_running,
            monitor_paused: Arc::new(AtomicBool::new(false)),
            cancels: Mutex::new(HashMap::new()),
            limiter: RateLimiter::new(Arc::new(AtomicU64::new(0))),
        }
    }

    /// 登記一個下載中的任務，回傳它專屬的取消旗標。
    /// 同一個 url 重複登記會把舊的換掉（舊的那條無論如何都該停）。
    pub fn register_cancel(&self, url: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Some(old) = self
            .cancels
            .lock()
            .unwrap()
            .insert(url.to_string(), flag.clone())
        {
            old.store(true, Ordering::Relaxed);
        }
        flag
    }

    /// 下載結束後清掉登記（只在還是自己那顆旗標時才移除，
    /// 避免把後來重新開始的那一條誤刪）
    pub fn unregister_cancel(&self, url: &str, flag: &Arc<AtomicBool>) {
        let mut map = self.cancels.lock().unwrap();
        if map.get(url).is_some_and(|f| Arc::ptr_eq(f, flag)) {
            map.remove(url);
        }
    }

    /// 取消全部進行中的網站下載（工具列「停止下載」）
    pub fn cancel_all_downloads(&self) {
        for flag in self.cancels.lock().unwrap().values() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}
