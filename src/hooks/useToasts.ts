// 共用 toast 佇列 — BT 與直鏈的完成/加入通知都推這裡（App 層統一渲染）

import { useCallback, useState } from "react";

export interface Toast {
  key: number;
  text: string;
}

const TOAST_MS = 6000;

export function useToasts() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const pushToast = useCallback((text: string) => {
    const key = Date.now() + Math.random();
    setToasts((t) => [...t, { key, text }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.key !== key)), TOAST_MS);
  }, []);

  return { toasts, pushToast };
}
