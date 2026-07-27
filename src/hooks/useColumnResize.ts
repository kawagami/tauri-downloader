// useColumnResize.ts

import { useCallback, useEffect, useRef, useState } from "react";
import { getJsonPref, setJsonPref } from "../lib/uiPrefs";

export function useColumnResize(storageKey: string, defaultWidths: number[]) {
    const loadWidths = (): number[] => {
        try {
            const parsed = getJsonPref<number[] | null>(storageKey, null);
            if (parsed) {
                if (Array.isArray(parsed) && parsed.length === defaultWidths.length) return parsed;
            }
        } catch {}
        return [...defaultWidths];
    };

    const [colWidths, setColWidths] = useState<number[]>(loadWidths);
    const dragging = useRef<{ colIndex: number; startX: number; startWidth: number } | null>(null);
    // 拖曳期間的最新寬度，同步更新（不等 render）：
    // mouseup 要拿得到最後一次 mousemove 的結果，而副作用（寫 localStorage）
    // 不能塞在 setState updater 裡 —— StrictMode 會雙呼 updater
    //（同 useDownloadTasks.reorderTasks 的理由）
    const widthsRef = useRef(colWidths);

    const onMouseDown = useCallback((colIndex: number, e: React.MouseEvent) => {
        e.preventDefault();
        dragging.current = { colIndex, startX: e.clientX, startWidth: colWidths[colIndex] };
    }, [colWidths]);

    useEffect(() => {
        const onMouseMove = (e: MouseEvent) => {
            if (!dragging.current) return;
            const { colIndex, startX, startWidth } = dragging.current;
            const delta = e.clientX - startX;
            const newWidth = Math.max(50, startWidth - delta);
            const next = [...widthsRef.current];
            next[colIndex] = newWidth;
            widthsRef.current = next;
            setColWidths(next);
        };
        const onMouseUp = () => {
            if (!dragging.current) return;
            dragging.current = null;
            setJsonPref(storageKey, widthsRef.current);
        };
        window.addEventListener("mousemove", onMouseMove);
        window.addEventListener("mouseup", onMouseUp);
        return () => {
            window.removeEventListener("mousemove", onMouseMove);
            window.removeEventListener("mouseup", onMouseUp);
        };
    }, [storageKey]);

    return { colWidths, onMouseDown };
}
