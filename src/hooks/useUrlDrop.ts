// src/hooks/useUrlDrop.ts

import { useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { IngestSummary } from '../types';
import { addMagnet } from '../lib/btApi';

interface UseUrlDrop {
    isDragging: boolean;
    onDragEnter: (e: React.DragEvent) => void;
    onDragOver: (e: React.DragEvent) => void;
    onDragLeave: (e: React.DragEvent) => void;
    onDrop: (e: React.DragEvent) => void;
}

/**
 * useUrlDrop
 * - 接收從瀏覽器拖入的連結（HTML5 DnD，需 tauri.conf.json dragDropEnabled:false）
 * - 站台 URL → add_url_manually（與剪貼簿共用後端 ingest）；任務經事件進清單（useTaskEvents），
 *   合集在後端展開成每話一筆
 * - magnet 連結 → add_magnet（BT 分頁），與剪貼簿監控行為一致
 * - 獨立於剪貼簿監控開關
 * - 通知走共用 toast 佇列：同一件事（如「磁力任務已存在」）從剪貼簿或拖曳進來
 *   以前會長成兩種樣子（toast vs 專屬橫幅），現在一致
 */
export const useUrlDrop = (
    pushToast: (text: string) => void,
    onMagnetAdded?: () => void,
): UseUrlDrop => {
    const [isDragging, setIsDragging] = useState(false);
    // dragenter/dragleave 會在子元素間反覆觸發，用計數器避免閃爍
    const dragDepth = useRef(0);

    const onDragEnter = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        dragDepth.current += 1;
        setIsDragging(true);
    }, []);

    // dragover 必須 preventDefault，drop 才會觸發；不動計數器（持續連發）
    const onDragOver = useCallback((e: React.DragEvent) => {
        e.preventDefault();
    }, []);

    const onDragLeave = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        dragDepth.current = Math.max(0, dragDepth.current - 1);
        if (dragDepth.current === 0) setIsDragging(false);
    }, []);

    const onDrop = useCallback(async (e: React.DragEvent) => {
        e.preventDefault();
        dragDepth.current = 0;
        setIsDragging(false);

        const raw =
            e.dataTransfer.getData('text/uri-list') ||
            e.dataTransfer.getData('text/plain');
        // text/uri-list 可能多行且含 # 註解行，取第一個有效 URL
        const url = raw
            .split(/\r?\n/)
            .map(l => l.trim())
            .find(l => l && !l.startsWith('#'));

        if (!url) {
            pushToast('拖入內容沒有有效連結');
            return;
        }

        try {
            if (url.startsWith('magnet:')) {
                const result = await addMagnet(url);
                if (result.already_exists) {
                    pushToast('磁力任務已存在');
                } else {
                    onMagnetAdded?.();
                }
                return;
            }
            const summary = await invoke<IngestSummary>('add_url_manually', { url });
            // 合集的統計由 series-done 事件出 toast；一般作品重複加入要給回饋，不然看起來像沒反應
            if (!summary.series_title && summary.added === 0 && summary.existed > 0) {
                pushToast('任務已存在');
            }
        } catch (err) {
            pushToast(String(err));
        }
    }, [pushToast, onMagnetAdded]);

    return { isDragging, onDragEnter, onDragOver, onDragLeave, onDrop };
};
