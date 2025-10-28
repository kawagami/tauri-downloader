// src/hooks/useTaskManager.ts

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
// 假設從 types.ts 引入了 Task 介面、isUrlValid 函數
// 注意：我們將不再需要 createNewTaskFromUrl，或者需要大幅修改它。
import { Task, isUrlValid, createNewTaskFromPayload } from '../types';

// 1. ✨ 修改 UseTaskManager 介面和 addTask 的簽名
interface UseTaskManager {
    tasks: Task[];
    /**
     * @param url 要新增的下載 URL
     * @param title 可選：從剪貼簿 payload 取得的標題
     * @param image 可選：從剪貼簿 payload 取得的圖片ID
     */
    addTask: (url: string, title?: string, image?: string) => Promise<void>;
}

export const useTaskManager = (): UseTaskManager => {
    const [tasks, setTasks] = useState<Task[]>([]);

    /**
     * 處理新增任務的邏輯，會呼叫後端指令。
     */
    // 2. ✨ 更新 addTask 函數簽名
    const addTask = useCallback(async (url: string, title?: string, image?: string) => {
        const taskUrl = url.trim();

        // 驗證邏輯不變
        // 注意：isUrlValid 應該只檢查格式，不依賴 title/image 的存在
        if (!taskUrl || !isUrlValid(taskUrl)) {
            console.warn("URL 無效或為空，無法新增。");
            return;
        }

        // 3. ✨ 使用新的輔助函數來構建任務
        // 我們將所有資訊傳入，讓輔助函數來處理缺少的欄位（例如手動輸入時缺少 title/image）
        const newTask = createNewTaskFromPayload({
            url: taskUrl,
            title: title || 'Fetching Title...', // 提供預設值或佔位符
            image: image || '0', // 提供預設值或佔位符
        });

        // 檢查任務是否已存在 (建議用 url 或 image 欄位來檢查重複性，而不是 name)
        // 由於 title 和 image 可能會變動，我們改用 url 或您認為最穩定的 ID 來檢查
        if (tasks.some(task => task.url === newTask.url)) {
            console.warn(`任務已存在: ${newTask.url}`);
            return;
        }

        try {
            // 呼叫後端指令，通知 Rust 準備開始下載
            await invoke("download_url", { url: taskUrl });

            // 更新前端的任務清單
            setTasks(prevTasks => [...prevTasks, newTask]);

            console.log(`成功提交新任務: ${newTask.name}`);

        } catch (error) {
            console.error("呼叫後端指令 [download_url] 時發生錯誤：", error);
        }
    }, [tasks]); // 依賴於 tasks

    return {
        tasks,
        addTask,
    };
};