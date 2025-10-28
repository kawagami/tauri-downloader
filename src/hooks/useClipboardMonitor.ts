// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useCallback } from 'react';
import { listen, Event } from '@tauri-apps/api/event';
import { Task } from '../types'; // 假設 Task 在這裡被匯入

// 1. ✨ 定義 ClipboardPayload 的 TypeScript 介面
// 必須與 Rust 中的 ClipboardPayload 結構一致 (注意：image 在 Rust 中是 u64，這裡用 number)
interface ClipboardPayload {
    url: string;
    title: string;
    image: string; // 假設 u64 對應到 JS 的 number
}

// 定義 useTaskManager 導出的 addTask 函數類型
// 由於 Rust 現在提供更多資訊，建議調整 addTask 以接收完整的 Payload
// 如果 addTask 只能接收 url，則保持原樣，只傳遞 url。
type AddTaskFunction = (
    url: string,
    title?: string,
    image?: string
) => Promise<void>;


interface UseClipboardMonitor {
    monitorClipboard: boolean;
    setMonitorClipboard: (enabled: boolean) => void;
    url: string; // 用於管理輸入框的狀態
    setUrl: (url: string) => void;
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
    const [url, setUrl] = useState('');

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
                    const newUrl = payload.url; // 4. 從 payload 中提取 URL

                    console.log(`[Event] 剪貼簿偵測到新的有效 URL: ${newUrl}`);
                    console.log(`[Event] 額外資訊: 標題="${payload.title}", 圖片 URL=${payload.image}`);

                    // 1. 執行前端檢查，避免重複新增
                    // 注意：這裡的 tasks 依賴是 useEffect 的閉包值，
                    // 雖然 React 會在 tasks 變化時重新執行 useEffect，但在極端情況下仍可能重複。
                    // 但以 React Hooks 標準，這樣處理是常見且合理的。
                    const isAlreadyInList = tasks.some(task => task.url === newUrl);

                    if (!isAlreadyInList) {
                        console.log("URL 不在列表中，自動新增任務。");

                        // 4. ✨ 呼叫 addTask，傳遞 URL (以及額外資訊，如果 addTask 支援)
                        // 這裡假設您的 addTask 函數已經更新以接收 title 和 image
                        addTask(newUrl, payload.title, payload.image);

                    } else {
                        console.log("URL 已在列表中，跳過新增。");
                    }

                    // 2. 將新的 URL 設置到輸入框狀態 (可選)
                    setUrl(newUrl);
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
        url,
        setUrl,
    };
};