// src/state.rs
use reqwest::Client;
use rusqlite::Connection;
use std::sync::{
    atomic::AtomicBool,
    Arc, Mutex,
};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub client: Client,
    pub monitor_running: Arc<AtomicBool>,
    pub download_cancelled: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(db: Connection, monitor_running: Arc<AtomicBool>) -> Self {
        Self {
            db: Mutex::new(db),
            client: Client::new(),
            monitor_running,
            download_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}
