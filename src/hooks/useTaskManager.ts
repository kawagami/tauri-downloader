import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Task, ClipboardPayload, UseTaskManager } from '../types';

/**
 * useTaskManager
 * - 管理任務列表 state
 * - 啟動時從 SQLite 載入任務
 * - 提供 addTask 函數給其他 hook 或 UI 使用
 */
export const useTaskManager = (): UseTaskManager => {
    const [tasks, setTasks] = useState<Task[]>([]);

    // 🔹 使用 useRef 儲存音效實例，避免每次 render 重新創建
    const audioRef = useRef<HTMLAudioElement | null>(null);

    // 🔹 1️⃣ 啟動時從 SQLite 載入所有任務
    useEffect(() => {// 初始化音效
        audioRef.current = new Audio('./ding.mp3');

        const loadTasks = async () => {
            try {
                const result = await invoke<Task[]>("load_all_tasks");
                setTasks(result);
                console.log(`[TaskManager] 載入 ${result.length} 個任務`);
            } catch (err) {
                console.error("[TaskManager] 讀取任務失敗", err);
            }
        };

        loadTasks();
    }, []);


    // 🔹 2️⃣ 新增任務函數
    const addTask = useCallback(async (payload: ClipboardPayload) => {
        // 避免重複任務
        if (tasks.some(task => task.url === payload.url)) {
            console.warn(`[TaskManager] 任務已存在: ${payload.url}`);
            return;
        }

        try {
            // // 可呼叫後端指令，例如下載 URL
            // await invoke("download_with_progress", { url: payload.download_page_href, title: payload.title });

            // 同步更新前端 state
            setTasks(prevTasks => [...prevTasks, payload]);

            // 🔹 播放音效
            if (audioRef.current) {
                audioRef.current.currentTime = 0; // 強制回到開頭，避免連續觸發時沒聲音
                audioRef.current
                    .play()
                    .catch(err => {
                        // 瀏覽器可能會攔截未經使用者互動的自動播放
                        console.warn("[TaskManager] 音效播放被攔截:", err);
                    });
            }

            console.log(`[TaskManager] 成功新增任務: ${payload.title}`);
        } catch (error) {
            console.error("[TaskManager] 呼叫後端 download_with_progress 發生錯誤:", error);
        }
    }, [tasks]);

    const removeTask = useCallback(async (url: string) => {
        try {
            await invoke("remove_task", { url });
            setTasks(prev => prev.filter(task => task.url !== url));
        } catch (err) {
            console.error("刪除任務失敗", err);
        }
    }, []);

    const removeAllTasks = useCallback(async () => {
        try {
            await invoke("remove_all_tasks");
            setTasks([]); // 清空前端 state
        } catch (err) {
            console.error("刪除全部任務失敗", err);
        }
    }, []);

    return {
        tasks,
        addTask,
        removeTask,
        removeAllTasks,
    };
};
