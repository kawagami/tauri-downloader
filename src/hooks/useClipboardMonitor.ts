// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useCallback, useRef } from 'react';
import { listen, Event } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { ClipboardPayload, Task } from '../types';

type AddTaskFunction = (payload: ClipboardPayload) => Promise<void>;

interface UseClipboardMonitor {
    monitorClipboard: boolean;
    setMonitorClipboard: (enabled: boolean) => Promise<void>;
}

export const useClipboardMonitor = (
    addTask: AddTaskFunction,
    tasks: Task[]
): UseClipboardMonitor => {
    const [monitorClipboard, setMonitorClipboardState] = useState(true);
    const tasksRef = useRef(tasks);

    useEffect(() => {
        tasksRef.current = tasks;
    }, [tasks]);

    const setMonitorClipboard = useCallback(async (enabled: boolean) => {
        setMonitorClipboardState(enabled);
        await invoke('set_monitor_paused', { paused: !enabled });
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
                fn();
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