// src/hooks/useTaskManager.ts

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Task, ClipboardPayload, UseTaskManager } from '../types';

// 2. ⚠️ 修改 UseTaskManager 介面和 addTask 的簽名


export const useTaskManager = (): UseTaskManager => {
    const [tasks, setTasks] = useState<Task[]>([]);

    /**
     * 處理新增任務的邏輯，會呼叫後端指令。
     */
    // 3. ⚠️ 更新 addTask 函數簽名，接受單一 payload 物件
    const addTask = useCallback(async (payload: ClipboardPayload) => {

        // 檢查任務是否已存在 
        if (tasks.some(task => task.url === payload.url)) {
            console.warn(`任務已存在: ${payload.url}`);
            return;
        }

        try {
            // 呼叫後端指令，只傳遞 URL (假設後端只處理 URL)
            await invoke("download_url", { url: payload.url });

            // 更新前端的任務清單
            setTasks(prevTasks => [...prevTasks, payload]);

            console.log(`成功提交新任務: ${payload.title}`);

        } catch (error) {
            console.error("呼叫後端指令 [download_url] 時發生錯誤：", error);
        }
    }, [tasks]); // 依賴於 tasks

    return {
        tasks,
        addTask,
    };
};