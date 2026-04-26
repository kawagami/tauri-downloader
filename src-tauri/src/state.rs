// src/state.rs
use reqwest::Client;
use rusqlite::Connection;
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc, Mutex,
};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub client: Client,
    pub monitor_running: Arc<AtomicBool>,
    pub monitor_paused: Arc<AtomicBool>,
    pub download_cancelled: Arc<AtomicBool>,
    pub bandwidth_limit_bps: Arc<AtomicU64>, // 0 = 無限制
}

impl AppState {
    pub fn new(db: Connection, monitor_running: Arc<AtomicBool>) -> Self {
        Self {
            db: Mutex::new(db),
            client: Client::new(),
            monitor_running,
            monitor_paused: Arc::new(AtomicBool::new(false)),
            download_cancelled: Arc::new(AtomicBool::new(false)),
            bandwidth_limit_bps: Arc::new(AtomicU64::new(0)),
        }
    }
}
