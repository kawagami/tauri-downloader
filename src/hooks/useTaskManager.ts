import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Task, ClipboardPayload } from '../types';

export interface UseTaskManager {
    tasks: Task[];
    addTask: (payload: ClipboardPayload) => Promise<void>;
    removeTask: (url: string) => Promise<void>;
    removeAllTasks: () => Promise<void>;
    reloadTasks: () => Promise<void>;
    volume: number;
    setVolume: (v: number) => void;
}

/**
 * useTaskManager
 * - 管理任務列表 state
 * - 啟動時從 SQLite 載入任務
 * - 提供 addTask 函數給其他 hook 或 UI 使用
 */
export const useTaskManager = (): UseTaskManager => {
    const [tasks, setTasks] = useState<Task[]>([]);
    const [volume, setVolumeState] = useState<number>(() =>
        Number(localStorage.getItem("dingVolume") ?? "1")
    );

    const volumeRef = useRef(volume);
    const gainNodeRef = useRef<GainNode | null>(null);
    const setVolume = useCallback((v: number) => {
        volumeRef.current = v;
        if (gainNodeRef.current) gainNodeRef.current.gain.value = v;
        setVolumeState(v);
        localStorage.setItem("dingVolume", String(v));
    }, []);

    const audioRef = useRef<HTMLAudioElement | null>(null);

    const reloadTasks = useCallback(async () => {
        try {
            const result = await invoke<Task[]>("load_all_tasks");
            setTasks(result);
        } catch (err) {
            console.error("[TaskManager] 讀取任務失敗", err);
        }
    }, []);

    // 🔹 1️⃣ 啟動時從 SQLite 載入所有任務
    useEffect(() => {
        fetch('/ding.mp3')
            .then(r => r.blob())
            .then(blob => {
                const audio = new Audio(URL.createObjectURL(blob));
                const ctx = new AudioContext();
                const source = ctx.createMediaElementSource(audio);
                const gain = ctx.createGain();
                gain.gain.value = volumeRef.current;
                source.connect(gain);
                gain.connect(ctx.destination);
                audioRef.current = audio;
                gainNodeRef.current = gain;
            })
            .catch(() => {});
        reloadTasks();
    }, []);


    const tasksRef = useRef<Task[]>([]);
    useEffect(() => {
        tasksRef.current = tasks;
    }, [tasks]);

    // 🔹 2️⃣ 新增任務函數（stable reference，不依賴 tasks）
    const addTask = useCallback(async (payload: ClipboardPayload) => {
        if (tasksRef.current.some(task => task.url === payload.url)) {
            return;
        }

        setTasks(prev => [...prev, payload]);

        if (audioRef.current) {
            if (gainNodeRef.current) gainNodeRef.current.gain.value = volumeRef.current;
            audioRef.current.currentTime = 0;
            audioRef.current.play().catch(() => {});
        }
    }, []);

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
        reloadTasks,
        volume,
        setVolume,
    };
};
