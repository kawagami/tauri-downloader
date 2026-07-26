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
            setColWidths(prev => {
                const next = [...prev];
                next[colIndex] = newWidth;
                return next;
            });
        };
        const onMouseUp = () => {
            if (!dragging.current) return;
            dragging.current = null;
            setColWidths(prev => {
                setJsonPref(storageKey, prev);
                return prev;
            });
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
