use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use anyhow::Context;
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
        self.inner.read().unwrap().clone().ok_or_else(|| {
            match self.last_error.read().unwrap().clone() {
                Some(e) => format!("BT 引擎未啟動:{}", e),
                None => "BT 引擎啟動中,請稍候".to_string(),
            }
        })
    }

    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "ready": self.inner.read().unwrap().is_some(),
            "error": self.last_error.read().unwrap().clone(),
        })
    }
}

/// 背景初始化 BT session，結果寫回 BtEngine 並 emit "bt-engine-status"。
/// 已就緒或初始化中則直接返回，可重複呼叫（retry）。
pub fn spawn_init(app: AppHandle) {
    {
        let engine = app.state::<BtEngine>();
        if engine.inner.read().unwrap().is_some() {
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
                *engine.inner.write().unwrap() = Some(Arc::new(ts));
                *engine.last_error.write().unwrap() = None;
                let _ = app.emit("bt-engine-status", engine.status());
            }
            Err(e) => {
                let msg = format!("{:#}", e);
                tracing::error!("BT 引擎啟動失敗: {}", msg);
                *engine.last_error.write().unwrap() = Some(msg);
                let _ = app.emit("bt-engine-status", engine.status());
            }
        }
        engine.initializing.store(false, Ordering::SeqCst);
    });
}

pub async fn init(app_data_dir: PathBuf, settings: BtSettings) -> anyhow::Result<TorrentState> {
    let session_dir = app_data_dir.join("bt-session");
    let download_dir = PathBuf::from(&settings.default_download_dir);

    // 固定 port 有設就用，否則 rqbit CLI 同款預設 range
    let listen_port_range = match settings.listen_port {
        Some(p) => p..p.saturating_add(1),
        None => 4240..4260,
    };

    let session = Session::new_with_opts(
        download_dir,
        SessionOptions {
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(session_dir),
            }),
            fastresume: true,
            listen_port_range: Some(listen_port_range),
            enable_upnp_port_forwarding: true,
            ratelimits: LimitsConfig {
                upload_bps: settings.upload_limit_bps.and_then(NonZeroU32::new),
                download_bps: settings.download_limit_bps.and_then(NonZeroU32::new),
            },
            ..Default::default()
        },
    )
    .await
    .context("failed to create librqbit session")?;

    let api = Api::new(session.clone(), None);

    Ok(TorrentState {
        api,
        session,
        pending: Mutex::new(HashMap::new()),
        pending_seq: AtomicU64::new(0),
    })
}
