// 直鏈下載 stats 事件唯一訂閱點 — 掛 App 層（不隨分頁卸載）。
// 完成通知推 App 的共用 toast 佇列（useToasts）。

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { HttpFinishedEvent, HttpStatsEvent } from "../lib/httpApi";

export function useHttpStats(pushToast: (text: string) => void) {
  const [stats, setStats] = useState<HttpStatsEvent | null>(null);

  useEffect(() => {
    let cancelled = false;

    const unlistenStats = listen<HttpStatsEvent>("http-stats", (e) => {
      if (!cancelled) setStats(e.payload);
    });
    const unlistenFinished = listen<HttpFinishedEvent>("http-finished", (e) => {
      if (cancelled) return;
      pushToast(`下載完成:${e.payload.name}`);
    });

    return () => {
      cancelled = true;
      unlistenStats.then((fn) => fn());
      unlistenFinished.then((fn) => fn());
    };
  }, [pushToast]);

  return { stats };
}
