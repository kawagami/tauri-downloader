// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { isUrlValid } from '../types'; // 引入 URL 驗證輔助函數

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
 * @param addTask 來自 useTaskManager 的新增任務函數，用於自動新增偵測到的 URL。
 */
export const useClipboardMonitor = (addTask: AddTaskFunction): UseClipboardMonitor => {
    const [monitorClipboard, setMonitorClipboardState] = useState(false);
    const [url, setUrl] = useState('');
    const lastClipboardContent = useRef<string | null>(null);

    // 處理「監控剪貼簿」勾選框變化的函數
    const setMonitorClipboard = useCallback((enabled: boolean) => {
        setMonitorClipboardState(enabled);

        // 當開啟監控時，先讀取一次當前剪貼簿的內容作為起始值
        if (enabled) {
            invoke("read_clipboard")
                .then((content) => {
                    lastClipboardContent.current = content as string;
                })
                .catch((error) => {
                    console.error("剪貼簿起始讀取錯誤:", error);
                });
        }
    }, []);

    // 💡 核心監控邏輯：使用 useEffect 和 setInterval
    useEffect(() => {
        let intervalId: number | undefined;

        if (monitorClipboard) {
            // 每 1000 毫秒 (1 秒) 執行一次檢查
            intervalId = setInterval(async () => {
                try {
                    const currentContent = await invoke("read_clipboard") as string;

                    if (currentContent && currentContent !== lastClipboardContent.current) {

                        // 檢查內容是否為有效的 URL
                        if (isUrlValid(currentContent)) {
                            console.log("剪貼簿偵測到新的有效 URL，自動新增任務。");
                            // 呼叫外部傳入的 addTask 函數
                            addTask(currentContent);

                            const parseContent = await invoke("process_clipboard_url") as string;
                            console.log("解析取得 title", parseContent);

                        }

                        // 更新上一次的內容
                        lastClipboardContent.current = currentContent;
                    }
                } catch (error) {
                    // 處理錯誤，例如剪貼簿無法存取
                    console.error("無法讀取剪貼簿：", error);
                }
            }, 1000); // 1 秒檢查一次
        }

        // 當元件卸載或 monitorClipboard 改變時，清除定時器
        return () => {
            if (intervalId) {
                clearInterval(intervalId);
            }
        };
        // 依賴項：addTask 必須包含在內，以確保 setInterval 內部使用的是最新的函數定義
        // 雖然 addTask 是用 useCallback 包裝的，但這樣寫是符合 Hooks 規範的。
    }, [monitorClipboard, addTask]);

    return {
        monitorClipboard,
        setMonitorClipboard,
        url,
        setUrl,
    };
};