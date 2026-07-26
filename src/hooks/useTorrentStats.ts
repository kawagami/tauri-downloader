// BT stats 事件唯一訂閱點 — 掛在 App 層（不隨分頁卸載）。
// 通知走 App 的共用 toast 佇列（useToasts），這裡只負責把事件轉成文字。

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { TorrentFinishedEvent, TorrentStatsEvent } from "../lib/btApi";

export function useTorrentStats(
  pushToast: (text: string) => void,
  onMagnetAdded?: () => void,
) {
  const [stats, setStats] = useState<TorrentStatsEvent | null>(null);

  useEffect(() => {
    let cancelled = false;

    const unlistenStats = listen<TorrentStatsEvent>("torrent-stats", (e) => {
      if (!cancelled) setStats(e.payload);
    });
    const unlistenFinished = listen<TorrentFinishedEvent>("torrent-finished", (e) => {
      if (cancelled) return;
      pushToast(`下載完成:${e.payload.name ?? `任務 #${e.payload.id}`}`);
    });
    // 剪貼簿監控偵測到 magnet 並成功加入時
    const unlistenAdded = listen<string | null>("new-magnet-added", (e) => {
      if (cancelled) return;
      pushToast(`已加入磁力任務:${e.payload ?? "(無名稱 magnet)"}`);
      onMagnetAdded?.();
    });
    // 剪貼簿 magnet 加入失敗（如 BT 引擎啟動中）
    const unlistenAddError = listen<string>("magnet-add-error", (e) => {
      if (cancelled) return;
      pushToast(`磁力任務加入失敗:${e.payload}`);
    });

    return () => {
      cancelled = true;
      unlistenStats.then((fn) => fn());
      unlistenFinished.then((fn) => fn());
      unlistenAdded.then((fn) => fn());
      unlistenAddError.then((fn) => fn());
    };
  }, [pushToast, onMagnetAdded]);

  return { stats };
}
