// src/state.rs
use reqwest::Client;
use rusqlite::Connection;
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc, Mutex,
};

use crate::utils::{net::build_client, ratelimit::RateLimiter};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub client: Client,
    pub monitor_running: Arc<AtomicBool>,
    pub monitor_paused: Arc<AtomicBool>,
    pub download_cancelled: Arc<AtomicBool>,
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
            download_cancelled: Arc::new(AtomicBool::new(false)),
            limiter: RateLimiter::new(Arc::new(AtomicU64::new(0))),
        }
    }
}
