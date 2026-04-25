// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useCallback, useRef } from 'react';
import { listen, Event } from '@tauri-apps/api/event';
import { ClipboardPayload, Task } from '../types';

type AddTaskFunction = (payload: ClipboardPayload) => Promise<void>;

interface UseClipboardMonitor {
    monitorClipboard: boolean;
    setMonitorClipboard: (enabled: boolean) => void;
}

/**
 * 處理剪貼簿監控的 Side Effect 邏輯。
 * @param addTask 來自 useTaskManager 的新增任務函數。
 * @param tasks 當前的任務列表，用於判斷是否重複。
 */
export const useClipboardMonitor = (
    addTask: AddTaskFunction,
    tasks: Task[]
): UseClipboardMonitor => {
    const [monitorClipboard, setMonitorClipboardState] = useState(true);
    const tasksRef = useRef(tasks);

    useEffect(() => {
        tasksRef.current = tasks;
    }, [tasks]);

    const setMonitorClipboard = useCallback((enabled: boolean) => {
        setMonitorClipboardState(enabled);
    }, []);

    useEffect(() => {
        if (!monitorClipboard) return;

        let unlisten: (() => void) | undefined;
        let mounted = true;

        const startListening = async () => {
            const fn = await listen<ClipboardPayload>('new-valid-url-payload', (event: Event<ClipboardPayload>) => {
                if (!mounted) return;
                const payload = event.payload;
                if (!tasksRef.current.some(task => task.url === payload.url)) {
                    addTask(payload);
                }
            });

            if (!mounted) {
                fn(); // cleanup 已跑，立即 unlisten
            } else {
                unlisten = fn;
            }
        };

        startListening();

        return () => {
            mounted = false;
            unlisten?.();
        };
    }, [monitorClipboard, addTask]);

    return {
        monitorClipboard,
        setMonitorClipboard,
    };
};