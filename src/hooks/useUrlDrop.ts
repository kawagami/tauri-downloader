// src/hooks/useUrlDrop.ts

import { useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ClipboardPayload } from '../types';

type AddTaskFunction = (payload: ClipboardPayload) => Promise<void>;

interface UseUrlDrop {
    isDragging: boolean;
    dropError: string | null;
    onDragEnter: (e: React.DragEvent) => void;
    onDragOver: (e: React.DragEvent) => void;
    onDragLeave: (e: React.DragEvent) => void;
    onDrop: (e: React.DragEvent) => void;
}

/**
 * useUrlDrop
 * - 接收從瀏覽器拖入的連結（HTML5 DnD，需 tauri.conf.json dragDropEnabled:false）
 * - 從 dataTransfer 取 URL，呼叫 add_url_manually（複用剪貼簿同一條後端 pipeline）
 * - 回傳 payload 後直接 addTask，獨立於剪貼簿監控開關
 */
export const useUrlDrop = (addTask: AddTaskFunction): UseUrlDrop => {
    const [isDragging, setIsDragging] = useState(false);
    const [dropError, setDropError] = useState<string | null>(null);
    // dragenter/dragleave 會在子元素間反覆觸發，用計數器避免閃爍
    const dragDepth = useRef(0);
    const errorTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    // 設錯誤訊息並 4 秒後自動清除
    const flashError = useCallback((msg: string | null) => {
        if (errorTimer.current) clearTimeout(errorTimer.current);
        setDropError(msg);
        if (msg) {
            errorTimer.current = setTimeout(() => setDropError(null), 4000);
        }
    }, []);

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
            flashError('拖入內容沒有有效連結');
            return;
        }

        try {
            const payload = await invoke<ClipboardPayload>('add_url_manually', { url });
            await addTask(payload);
            flashError(null);
        } catch (err) {
            flashError(String(err));
        }
    }, [addTask, flashError]);

    return { isDragging, dropError, onDragEnter, onDragOver, onDragLeave, onDrop };
};
