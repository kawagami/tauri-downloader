// 分頁操作錯誤橫幅 — 設了之後自動消失（BT/直鏈/遊戲設定共用）

import { useEffect, useState } from "react";

export function useAutoClearError(ms = 6000) {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!error) return;
    const t = setTimeout(() => setError(null), ms);
    return () => clearTimeout(t);
  }, [error, ms]);

  return [error, setError] as const;
}
