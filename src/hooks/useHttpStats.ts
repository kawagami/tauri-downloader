// 直鏈下載 stats 事件唯一訂閱點 — 掛 App 層（不隨分頁卸載），
// 完成通知轉 toast。

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { HttpFinishedEvent, HttpStatsEvent } from "../lib/httpApi";
import type { Toast } from "./useTorrentStats";

export function useHttpStats() {
  const [stats, setStats] = useState<HttpStatsEvent | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);

  useEffect(() => {
    let cancelled = false;

    const unlistenStats = listen<HttpStatsEvent>("http-stats", (e) => {
      if (!cancelled) setStats(e.payload);
    });
    const unlistenFinished = listen<HttpFinishedEvent>("http-finished", (e) => {
      if (cancelled) return;
      const key = Date.now() + Math.random();
      const text = `下載完成:${e.payload.name}`;
      setToasts((t) => [...t, { key, text }]);
      setTimeout(() => {
        setToasts((t) => t.filter((x) => x.key !== key));
      }, 6000);
    });

    return () => {
      cancelled = true;
      unlistenStats.then((fn) => fn());
      unlistenFinished.then((fn) => fn());
    };
  }, []);

  return { stats, toasts };
}
