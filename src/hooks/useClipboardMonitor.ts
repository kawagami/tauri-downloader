// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useCallback, useRef } from 'react';
import { listen, Event } from '@tauri-apps/api/event';
import { getAppSettings, updateAppSettings } from '../lib/settingsApi';
import { ClipboardPayload, Task } from '../types';

type AddTaskFunction = (payload: ClipboardPayload) => Promise<void>;

interface UseClipboardMonitor {
    monitorClipboard: boolean;
    setMonitorClipboard: (enabled: boolean) => Promise<void>;
}

export const useClipboardMonitor = (
    addTask: AddTaskFunction,
    tasks: Task[],
    pushToast: (text: string) => void
): UseClipboardMonitor => {
    const [monitorClipboard, setMonitorClipboardState] = useState(true);
    const tasksRef = useRef(tasks);

    useEffect(() => {
        tasksRef.current = tasks;
    }, [tasks]);

    // 開關狀態持久化在 app_settings.json;後端啟動已自行套用,這裡只同步 UI
    useEffect(() => {
        getAppSettings()
            .then(s => setMonitorClipboardState(s.monitor_clipboard))
            .catch(() => {});
    }, []);

    const setMonitorClipboard = useCallback(async (enabled: boolean) => {
        setMonitorClipboardState(enabled);
        // save_app_settings 會即時套用 monitor_paused
        await updateAppSettings(s => ({ ...s, monitor_clipboard: enabled }));
    }, []);

    useEffect(() => {
        if (!monitorClipboard) return;

        let mounted = true;
        const unlisteners: (() => void)[] = [];
        const track = (p: Promise<() => void>) =>
            p.then(fn => (mounted ? unlisteners.push(fn) : fn()));

        track(listen<ClipboardPayload>('new-valid-url-payload', (event: Event<ClipboardPayload>) => {
            if (!mounted) return;
            const payload = event.payload;
            if (!tasksRef.current.some(task => task.url === payload.url)) {
                addTask(payload);
            }
        }));

        // 抓取失敗（網路掛了/站台改版/provider 未實作）也要讓使用者知道，
        // 否則畫面上跟「沒複製到」完全一樣。與 magnet-add-error 對稱。
        track(listen<string>('url-fetch-error', (event: Event<string>) => {
            if (mounted) pushToast(`連結處理失敗:${event.payload}`);
        }));

        return () => {
            mounted = false;
            unlisteners.forEach(fn => fn());
        };
    }, [monitorClipboard, addTask, pushToast]);

    return {
        monitorClipboard,
        setMonitorClipboard,
    };
};