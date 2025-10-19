// src/hooks/useTaskManager.ts

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Task, createNewTaskFromUrl, isUrlValid } from '../types'; // 從 types.ts 引入介面和輔助函數

interface UseTaskManager {
    tasks: Task[];
    addTask: (url: string) => Promise<void>;
}

export const useTaskManager = (): UseTaskManager => {
    const [tasks, setTasks] = useState<Task[]>([]);

    /**
     * 處理新增任務的邏輯，會呼叫後端指令。
     * 這個函數會被 TaskInputForm (手動輸入) 和 useClipboardMonitor (自動新增) 呼叫。
     * @param url 要新增的下載 URL
     */
    const addTask = useCallback(async (url: string) => {
        const taskUrl = url.trim();

        if (!taskUrl || !isUrlValid(taskUrl)) {
            console.warn("URL 無效或為空，無法新增。");
            return;
        }

        const newTask = createNewTaskFromUrl(taskUrl);

        // 檢查任務是否已存在
        if (tasks.some(task => task.name === newTask.name)) {
            console.warn(`任務已存在: ${newTask.name}`);
            return;
        }

        try {
            // 呼叫後端指令，通知 Rust 準備開始下載
            // 注意：這裡只呼叫指令，實際進度回饋將在之後透過事件系統處理
            await invoke("download_url", { url: taskUrl });

            // 更新前端的任務清單，狀態設為 Pending
            // 使用函數式更新確保我們在正確的狀態上進行添加
            setTasks(prevTasks => [...prevTasks, newTask]);

            console.log(`成功提交新任務: ${newTask.name}`);

        } catch (error) {
            console.error("呼叫後端指令 [download_url] 時發生錯誤：", error);
            // 實際應用中，這裡應該提示使用者錯誤
        }
    }, [tasks]); // 依賴於 tasks，確保 tasks.some 檢查重複任務時使用的是最新清單

    return {
        tasks,
        addTask,
    };
};