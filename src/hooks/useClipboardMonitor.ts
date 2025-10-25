// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useCallback } from 'react';
// 移除 invoke 和 isUrlValid (因為驗證工作已移至 Rust)
import { listen, Event } from '@tauri-apps/api/event'; // ✨ 新增事件 API
import { Task } from '../types'; // 假設 Task 在這裡被匯入

// 定義任務列表的結構 (為了讓 TypeScript 編譯通過)
// 確保這個 Task 與 App.tsx 傳入的 tasks 類型一致，且包含 url: string
// type Task = { id: number; url: string; /* ... */ }; 

// 定義 useTaskManager 導出的 addTask 函數類型
type AddTaskFunction = (url: string) => Promise<void>;

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
    tasks: Task[] // ✨ 修正：接受 tasks 列表
): UseClipboardMonitor => {
    const [monitorClipboard, setMonitorClipboardState] = useState(false);
    const [url, setUrl] = useState('');
    // 移除 lastClipboardContent，因為變化檢查由 Rust 處理

    // 處理「監控剪貼簿」勾選框變化的函數 (移除起始 invoke 讀取)
    const setMonitorClipboard = useCallback((enabled: boolean) => {
        setMonitorClipboardState(enabled);
        // Rust Monitor 已經在後台運行，無需手動處理起始值。
    }, []);

    // 💡 核心監控邏輯：使用 useEffect 監聽 Tauri Event
    useEffect(() => {
        let unlisten: (() => void) | undefined;

        if (monitorClipboard) {
            const startListening = async () => {
                // 監聽來自 Rust 的 'new-valid-url' 事件
                // 該事件只有在內容改變且通過 Rust 驗證時才會發送
                unlisten = await listen<string>('new-valid-url', (event: Event<string>) => {
                    const newUrl = event.payload;

                    console.log(`[Event] 剪貼簿偵測到新的有效 URL: ${newUrl}`);

                    // 1. 執行前端檢查，避免重複新增
                    const isAlreadyInList = tasks.some(task => task.url === newUrl);

                    if (!isAlreadyInList) {
                        console.log("URL 不在列表中，自動新增任務。");
                        addTask(newUrl);
                    } else {
                        console.log("URL 已在列表中，跳過新增。");
                    }

                    // 2. 將新的 URL 設置到輸入框狀態 (可選)
                    setUrl(newUrl);
                });
            };

            startListening();
        }

        // 當元件卸載或 monitorClipboard/tasks 改變時，清除監聽器
        return () => {
            if (unlisten) {
                unlisten(); // 執行 unlisten 函數
            }
        };

        // 依賴項：addTask 和 tasks 必須包含在內
    }, [monitorClipboard, addTask, tasks]);

    return {
        monitorClipboard,
        setMonitorClipboard,
        url,
        setUrl,
    };
};