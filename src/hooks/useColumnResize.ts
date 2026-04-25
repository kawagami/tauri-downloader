// useColumnResize.ts

import { useCallback, useEffect, useRef, useState } from "react";

export function useColumnResize(storageKey: string, defaultWidths: number[]) {
    const loadWidths = (): number[] => {
        try {
            const raw = localStorage.getItem(storageKey);
            if (raw) {
                const parsed = JSON.parse(raw);
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
                localStorage.setItem(storageKey, JSON.stringify(prev));
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
