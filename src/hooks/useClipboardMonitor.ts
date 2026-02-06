// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useCallback } from 'react';
import { listen, Event } from '@tauri-apps/api/event';
import { ClipboardPayload, Task, AddTaskFunction } from '../types';

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
    const [monitorClipboard, setMonitorClipboardState] = useState(false);

    const setMonitorClipboard = useCallback((enabled: boolean) => {
        setMonitorClipboardState(enabled);
    }, []);

    // 💡 核心監控邏輯：使用 useEffect 監聽 Tauri Event
    useEffect(() => {
        let unlisten: (() => void) | undefined;

        if (monitorClipboard) {
            const startListening = async () => {
                // 2 & 3. ✨ 更改監聽事件名稱和類型
                unlisten = await listen<ClipboardPayload>('new-valid-url-payload', (event: Event<ClipboardPayload>) => {
                    const payload = event.payload;

                    console.log(`[Event] 剪貼簿偵測到新的有效 URL: ${payload.url}`);
                    console.log(`[Event] 額外資訊: 標題="${payload.title}", 圖片 URL=${payload.image}`);

                    // 1. 執行前端檢查，避免重複新增
                    // 注意：這裡的 tasks 依賴是 useEffect 的閉包值，
                    // 雖然 React 會在 tasks 變化時重新執行 useEffect，但在極端情況下仍可能重複。
                    // 但以 React Hooks 標準，這樣處理是常見且合理的。
                    const isAlreadyInList = tasks.some(task => task.url === payload.url);

                    if (!isAlreadyInList) {
                        console.log("URL 不在列表中，自動新增任務。");

                        // ⚠️ 修改這裡：直接傳遞整個 payload 物件
                        addTask(payload);
                    } else {
                        console.log("URL 已在列表中，跳過新增。");
                    }
                });
            };

            startListening();
        }

        return () => {
            if (unlisten) {
                unlisten(); // 執行 unlisten 函數
            }
        };

        // 依賴項：addTask 和 tasks 必須包含在內
        // tasks 應該是依賴項，因為 tasks 變化時，我們需要更新監聽器閉包內的 isAlreadyInList 檢查。
    }, [monitorClipboard, addTask, tasks]);

    return {
        monitorClipboard,
        setMonitorClipboard,
    };
};