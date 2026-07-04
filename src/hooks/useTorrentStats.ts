// BT stats 事件唯一訂閱點 — 掛在 App 層（不隨分頁卸載），
// 完成通知與剪貼簿 magnet 加入通知都在這裡轉成 toast。

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { TorrentFinishedEvent, TorrentStatsEvent } from "../lib/btApi";

export interface Toast {
  key: number;
  text: string;
}

export function useTorrentStats(onMagnetAdded?: () => void) {
  const [stats, setStats] = useState<TorrentStatsEvent | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);

  useEffect(() => {
    let cancelled = false;

    const pushToast = (text: string) => {
      const key = Date.now() + Math.random();
      setToasts((t) => [...t, { key, text }]);
      setTimeout(() => {
        setToasts((t) => t.filter((x) => x.key !== key));
      }, 6000);
    };

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

    return () => {
      cancelled = true;
      unlistenStats.then((fn) => fn());
      unlistenFinished.then((fn) => fn());
      unlistenAdded.then((fn) => fn());
    };
  }, [onMagnetAdded]);

  return { stats, toasts };
}
