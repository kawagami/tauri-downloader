use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Context;
use librqbit::limits::LimitsConfig;
use librqbit::{Api, Session, SessionOptions, SessionPersistenceConfig};

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

/// BT 引擎狀態，獨立 manage，不混入主 AppState
pub struct TorrentState {
    pub api: Api,
    pub session: Arc<Session>,
    pub settings: Mutex<BtSettings>,
    pub settings_path: PathBuf,
    pub pending: Mutex<HashMap<u64, PendingAdd>>,
    pub pending_seq: AtomicU64,
}

pub async fn init(app_data_dir: PathBuf) -> anyhow::Result<TorrentState> {
    let settings_path = app_data_dir.join("bt_settings.json");
    let settings = BtSettings::load(&settings_path);

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
        settings: Mutex::new(settings),
        settings_path,
        pending: Mutex::new(HashMap::new()),
        pending_seq: AtomicU64::new(0),
    })
}
