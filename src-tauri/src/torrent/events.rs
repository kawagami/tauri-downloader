use std::collections::HashMap;
use std::time::Duration;

use librqbit::TorrentStatsState;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use super::state::TorrentState;

const MIB: f64 = 1024.0 * 1024.0;

/// 每秒收集所有 torrent 統計，推一個 "torrent-stats" event。
/// finished false → true 轉換時額外推 "torrent-finished"
///（首個 tick 不發，避免重啟後恢復的已完成任務誤報）。
pub fn spawn_stats_task(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut was_finished: HashMap<usize, bool> = HashMap::new();
        let mut first_tick = true;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let state = app.state::<TorrentState>();
            let list = state.api.api_torrent_list();
            let mut torrents = Vec::with_capacity(list.torrents.len());

            for d in &list.torrents {
                let Some(id) = d.id else { continue };
                let Ok(stats) = state.api.api_stats_v1(id.into()) else {
                    continue;
                };

                let progress = if stats.total_bytes > 0 {
                    stats.progress_bytes as f64 / stats.total_bytes as f64 * 100.0
                } else {
                    0.0
                };
                let (down_bps, up_bps, peers_live) = stats
                    .live
                    .as_ref()
                    .map(|l| {
                        (
                            (l.download_speed.mbps * MIB) as u64,
                            (l.upload_speed.mbps * MIB) as u64,
                            l.snapshot.peer_stats.live,
                        )
                    })
                    .unwrap_or((0, 0, 0));
                let state_str = match stats.state {
                    TorrentStatsState::Initializing => "initializing",
                    TorrentStatsState::Live => "live",
                    TorrentStatsState::Paused => "paused",
                    TorrentStatsState::Error => "error",
                };

                let prev = was_finished.insert(id, stats.finished).unwrap_or(false);
                if !first_tick && !prev && stats.finished {
                    let _ = app.emit("torrent-finished", json!({ "id": id, "name": d.name }));
                }

                torrents.push(json!({
                    "id": id,
                    "name": d.name,
                    "state": state_str,
                    "finished": stats.finished,
                    "progress_percent": progress,
                    "downloaded_bytes": stats.progress_bytes,
                    "total_bytes": stats.total_bytes,
                    "down_speed_bps": down_bps,
                    "up_speed_bps": up_bps,
                    "peers_live": peers_live,
                    "error": stats.error,
                }));
            }

            was_finished.retain(|k, _| list.torrents.iter().any(|d| d.id == Some(*k)));

            first_tick = false;

            let pending: Vec<_> = state
                .pending
                .lock()
                .unwrap()
                .iter()
                .map(|(key, p)| {
                    json!({
                        "key": key,
                        "name": p.name,
                        "elapsed_s": p.added_at.elapsed().as_secs(),
                        "error": p.error,
                    })
                })
                .collect();

            let ss = state.api.api_session_stats();
            let payload = json!({
                "torrents": torrents,
                "pending": pending,
                "session": {
                    "total_down_bps": (ss.download_speed.mbps * MIB) as u64,
                    "total_up_bps": (ss.upload_speed.mbps * MIB) as u64,
                }
            });
            let _ = app.emit("torrent-stats", payload);
        }
    });
}
