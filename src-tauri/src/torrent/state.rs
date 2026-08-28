use crate::utils::lock::RwLockExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use anyhow::Context;
use librqbit::dht::PersistentDhtConfig;
use librqbit::limits::LimitsConfig;
use librqbit::{Api, Session, SessionOptions, SessionPersistenceConfig};
use tauri::{AppHandle, Emitter, Manager};

use super::settings::BtSettings;

/// 背景解析 metadata 中的 magnet add。librqbit 的 add_torrent 要等
/// metadata 抓完才返回（冷門種子可能永遠等不到），所以 add 丟到背景跑，
/// 完成前由這裡追蹤給 UI 顯示。
pub struct PendingAdd {
    pub name: Option<String>,
    pub info_hash: Option<String>,
    pub added_at: Instant,
    pub error: Option<String>,
    pub handle: Option<tauri::async_runtime::JoinHandle<()>>,
}

/// BT 引擎狀態，獨立 manage，不混入主 AppState。
/// BT 設定在 SettingsState（settings.rs），不在這 — 引擎掛掉設定照常可用。
pub struct TorrentState {
    pub api: Api,
    pub session: Arc<Session>,
    pub pending: Mutex<HashMap<u64, PendingAdd>>,
    pub pending_seq: AtomicU64,
}

/// BT 引擎外殼 — session 建立失敗（如 port 被舊 magnet-downloader 佔走）
/// 只讓 BT 分頁失效，不拖垮整個 app。init 在背景跑，可 retry。
#[derive(Default)]
pub struct BtEngine {
    pub inner: RwLock<Option<Arc<TorrentState>>>,
    pub last_error: RwLock<Option<String>>,
    initializing: AtomicBool,
}

impl BtEngine {
    /// 取引擎，未就緒回錯誤字串（直接可當 command 錯誤回前端）
    pub fn get(&self) -> Result<Arc<TorrentState>, String> {
        self.inner.read_safe().clone().ok_or_else(|| {
            match self.last_error.read_safe().clone() {
                Some(e) => format!("BT 引擎未啟動:{}", e),
                None => "BT 引擎啟動中,請稍候".to_string(),
            }
        })
    }

    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "ready": self.inner.read_safe().is_some(),
            "error": self.last_error.read_safe().clone(),
        })
    }
}

/// 背景初始化 BT session，結果寫回 BtEngine 並 emit "bt-engine-status"。
/// 已就緒或初始化中則直接返回，可重複呼叫（retry）。
pub fn spawn_init(app: AppHandle) {
    {
        let engine = app.state::<BtEngine>();
        if engine.inner.read_safe().is_some() {
            return;
        }
        if engine.initializing.swap(true, Ordering::SeqCst) {
            return;
        }
    }
    tauri::async_runtime::spawn(async move {
        let result = async {
            let dir = app.path().app_data_dir().context("無法取得 app data 目錄")?;
            std::fs::create_dir_all(&dir)?;
            let bt_settings = app.state::<crate::settings::SettingsState>().get().bt;
            init(dir, bt_settings).await
        }
        .await;

        let engine = app.state::<BtEngine>();
        match result {
            Ok(ts) => {
                *engine.inner.write_safe() = Some(Arc::new(ts));
                *engine.last_error.write_safe() = None;
                let _ = app.emit("bt-engine-status", engine.status());
            }
            Err(e) => {
                let msg = format!("{:#}", e);
                tracing::error!("BT 引擎啟動失敗: {}", msg);
                *engine.last_error.write_safe() = Some(msg);
                let _ = app.emit("bt-engine-status", engine.status());
            }
        }
        engine.initializing.store(false, Ordering::SeqCst);
    });
}

/// 0 = 不限 → None；其餘夾到 u32 上限
fn nz_u32(bps: u64) -> Option<NonZeroU32> {
    NonZeroU32::new(bps.min(u32::MAX as u64) as u32)
}

/// DHT 綁 port 失敗時的重試次數（每次 librqbit 會重挑一個隨機 port）
const DHT_BIND_ATTEMPTS: u32 = 4;

/// librqbit 的 DHT 會把上次用的 UDP port 存進 dht.json 下次沿用。
/// Windows 上 Hyper-V/WSL 會保留成段的 ephemeral port（`netsh interface ipv4
/// show excludedportrange protocol=udp`），一旦存到保留範圍內的 port，之後
/// 每次啟動都 os error 10013（存取被拒）而不是「port 被佔用」，永遠好不了。
/// 所以：先預檢綁得起來嗎，綁不起來就砍掉檔案讓它重挑；啟動失敗也同樣處理後重試。
fn dht_persisted_addr(path: &std::path::Path) -> Option<SocketAddr> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value.get("addr")?.as_str()?.parse().ok()
}

fn drop_dht_state(path: &std::path::Path, reason: &str) {
    if path.exists() {
        match std::fs::remove_file(path) {
            Ok(()) => tracing::warn!("重設 DHT 狀態（{}）: {:?}", reason, path),
            Err(e) => tracing::error!("刪除 DHT 狀態失敗 {:?}: {}", path, e),
        }
    }
}

/// 只針對「綁 socket 被拒」重試；其他錯誤（如設定錯誤）直接往上丟
fn is_bind_error(err: &anyhow::Error) -> bool {
    let msg = format!("{:#}", err);
    msg.contains("error binding socket") || msg.contains("os error 10013")
}

pub async fn init(app_data_dir: PathBuf, settings: BtSettings) -> anyhow::Result<TorrentState> {
    let session_dir = app_data_dir.join("bt-session");
    // DHT 狀態放自己的 session 目錄：librqbit 預設是 %LOCALAPPDATA%\rqbit\dht\cache\dht.json，
    // 與舊 magnet-downloader 共用同一份（連 DHT 身分與 port 都共用）
    let dht_path = session_dir.join("dht.json");
    let download_dir = crate::utils::fs::resolve_dir(&settings.default_dir);

    // 固定 port 有設就用，否則 rqbit CLI 同款預設 range
    let listen_port_range = match settings.listen_port {
        Some(p) => p..p.saturating_add(1),
        None => 4240..4260,
    };

    // 預檢：上次存的 DHT port 現在綁不起來就先砍掉，不必等 session 建到一半才失敗
    if let Some(addr) = dht_persisted_addr(&dht_path) {
        if std::net::UdpSocket::bind(addr).is_err() {
            drop_dht_state(&dht_path, &format!("{} 綁不起來", addr));
        }
    }

    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=DHT_BIND_ATTEMPTS {
        let opts = SessionOptions {
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(session_dir.clone()),
            }),
            fastresume: true,
            listen_port_range: Some(listen_port_range.clone()),
            enable_upnp_port_forwarding: true,
            dht_config: Some(PersistentDhtConfig {
                config_filename: Some(dht_path.clone()),
                ..Default::default()
            }),
            ratelimits: LimitsConfig {
                // 設定統一用 u64 bytes/s、0 = 不限；librqbit 要 NonZeroU32
                upload_bps: nz_u32(settings.upload_limit_bps),
                download_bps: nz_u32(settings.download_limit_bps),
            },
            ..Default::default()
        };

        match Session::new_with_opts(download_dir.clone(), opts)
            .await
            .context("failed to create librqbit session")
        {
            Ok(session) => {
                let api = Api::new(session.clone(), None);
                return Ok(TorrentState {
                    api,
                    session,
                    pending: Mutex::new(HashMap::new()),
                    pending_seq: AtomicU64::new(0),
                });
            }
            Err(e) if is_bind_error(&e) && attempt < DHT_BIND_ATTEMPTS => {
                tracing::warn!("BT session 綁 port 失敗（第 {} 次），換 port 重試: {:#}", attempt, e);
                drop_dht_state(&dht_path, "綁 port 被拒");
                last_err = Some(e);
            }
            Err(e) => return Err(e),
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("failed to create librqbit session")))
}
