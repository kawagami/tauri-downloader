// useDownloadTasks.ts

import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Task, DownloadableTask } from "../types";

export function useDownloadTasks(baseTasks: Task[], onRemoveTask: (url: string) => void) {
    const [tasks, setTasks] = useState<DownloadableTask[]>(
        baseTasks.map((t) => ({ ...t, status: "idle", progress: 0 }))
    );
    const [isBatchDownloading, setIsBatchDownloading] = useState(false);
    const [batchProgress, setBatchProgress] = useState({ current: 0, total: 0 });
    const shouldStop = useRef(false);

    useEffect(() => {
        setTasks(prev => {
            const prevMap = new Map(prev.map(t => [t.url, t]));
            return baseTasks.map(t => prevMap.get(t.url) ?? { ...t, status: "idle", progress: 0 });
        });
    }, [baseTasks]);

    // --- 監聽 tauri 傳來的進度事件 ---
    useEffect(() => {
        let unlisten: (() => void) | undefined;
        const setup = async () => {
            unlisten = await listen<{
                url: string;
                progress: number;
                speed_bytes_per_sec: number;
                time_remaining_secs: number;
            }>(
                "download_progress",
                (event) => {
                    const { url, progress, speed_bytes_per_sec, time_remaining_secs } = event.payload;
                    setTasks((prev) =>
                        prev.map((t) =>
                            t.download_page_href === url
                                ? { ...t, progress, speed: speed_bytes_per_sec, timeRemaining: time_remaining_secs }
                                : t
                        )
                    );
                }
            );
        };
        setup();
        return () => unlisten?.();
    }, []);

    // --- 單一下載 ---
    const handleDownload = async (task: DownloadableTask) => {
        setTasks((prev) =>
            prev.map((t) =>
                t.url === task.url ? { ...t, status: "downloading", progress: 0 } : t
            )
        );
        try {
            const savePath = await invoke<string>("download_with_progress", {
                url: task.download_page_href,
                title: task.title,
            });
            setTasks((prev) =>
                prev.map((t) =>
                    t.url === task.url
                        ? { ...t, status: "done", progress: 100, savePath }
                        : t
                )
            );
        } catch (err) {
            setTasks((prev) =>
                prev.map((t) =>
                    t.url === task.url ? { ...t, status: "error", errorMessage: String(err) } : t
                )
            );
        }
    };

    // --- 批次下載 ---
    const handleDownloadAllSequentially = async () => {
        if (isBatchDownloading) return;
        shouldStop.current = false;
        setIsBatchDownloading(true);

        const pending = tasks.filter(t => t.status === "idle" || t.status === "error");
        setBatchProgress({ current: 0, total: pending.length });
        let current = 0;

        for (const task of tasks) {
            if (shouldStop.current) break;

            if (task.status !== "idle" && task.status !== "error") continue;

            current += 1;
            setBatchProgress(prev => ({ ...prev, current }));

            setTasks((prev) =>
                prev.map((t) =>
                    t.url === task.url ? { ...t, status: "downloading", progress: 0 } : t
                )
            );

            try {
                console.log(`[Batch] 開始下載: ${task.title}`);
                await invoke<string>("download_with_progress", {
                    url: task.download_page_href,
                    title: task.title,
                });

                console.log(`[Batch] 完成: ${task.title}`);
                setTasks((prev) =>
                    prev.map((t) =>
                        t.url === task.url
                            ? { ...t, status: "done", progress: 100 }
                            : t
                    )
                );

                onRemoveTask(task.url);
                await new Promise((resolve) => setTimeout(resolve, 1000));
            } catch (err) {
                setTasks((prev) =>
                    prev.map((t) =>
                        t.url === task.url ? { ...t, status: "error", errorMessage: String(err) } : t
                    )
                );
            }
        }

        setIsBatchDownloading(false);
    };

    // 🔸 外部可呼叫的停止方法
    const stopBatchDownload = () => {
        shouldStop.current = true;
        invoke("cancel_download");
    };

    return {
        tasks,
        setTasks,
        handleDownload,
        handleDownloadAllSequentially,
        stopBatchDownload,
        isBatchDownloading,
        batchProgress,
    };
}
