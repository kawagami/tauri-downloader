// src/hooks/useTaskManager.ts

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Task, isUrlValid, createNewTaskFromPayload, ClipboardPayload, UseTaskManager } from '../types';

// 2. ⚠️ 修改 UseTaskManager 介面和 addTask 的簽名


export const useTaskManager = (): UseTaskManager => {
    const [tasks, setTasks] = useState<Task[]>([]);

    /**
     * 處理新增任務的邏輯，會呼叫後端指令。
     */
    // 3. ⚠️ 更新 addTask 函數簽名，接受單一 payload 物件
    const addTask = useCallback(async (payload: ClipboardPayload) => {

        // 4. 從 payload 中解構出屬性
        const { url, title, image, download_page_href } = payload;
        const taskUrl = url.trim();

        // 驗證邏輯不變
        if (!taskUrl || !isUrlValid(taskUrl)) {
            console.warn("URL 無效或為空，無法新增。");
            return;
        }

        // 構建任務時，使用傳入的 payload 資訊
        const newTask = createNewTaskFromPayload({
            url: taskUrl,
            // 剪貼簿傳入的值可能為空，但手動輸入時 title/image 確定為空字串
            // createNewTaskFromPayload 應該處理提供預設值 'Fetching Title...' 的邏輯
            title: title || 'Fetching Title...',
            image: image || '0',
            download_page_href: download_page_href || '0',
        });

        // 檢查任務是否已存在 
        if (tasks.some(task => task.url === newTask.url)) {
            console.warn(`任務已存在: ${newTask.url}`);
            return;
        }

        try {
            // 呼叫後端指令，只傳遞 URL (假設後端只處理 URL)
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