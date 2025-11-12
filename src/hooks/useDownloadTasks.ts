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
    const shouldStop = useRef(false); // 🔹 用 useRef 存放停止旗標

    useEffect(() => {
        setTasks(baseTasks.map((t) => ({ ...t, status: "idle", progress: 0 })));
    }, [baseTasks]);

    // --- 監聽 tauri 傳來的進度事件 ---
    useEffect(() => {
        let unlisten: (() => void) | undefined;
        const setup = async () => {
            unlisten = await listen<{ url: string; progress: number }>(
                "download_progress",
                (event) => {
                    const { url, progress } = event.payload;
                    setTasks((prev) =>
                        prev.map((t) =>
                            t.download_page_href === url ? { ...t, progress } : t
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
        } catch {
            setTasks((prev) =>
                prev.map((t) =>
                    t.url === task.url ? { ...t, status: "error" } : t
                )
            );
        }
    };

    // --- 批次下載 ---
    const handleDownloadAllSequentially = async () => {
        if (isBatchDownloading) return;
        shouldStop.current = false; // 重設停止旗標
        setIsBatchDownloading(true);

        for (const task of tasks) {
            if (shouldStop.current) {
                console.log("🟥 已手動停止批次下載");
                break;
            }

            if (task.status !== "idle" && task.status !== "error") continue;

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
                console.error(`[Batch] 錯誤: ${task.title}`, err);
                setTasks((prev) =>
                    prev.map((t) =>
                        t.url === task.url ? { ...t, status: "error" } : t
                    )
                );
            }
        }

        setIsBatchDownloading(false);
    };

    // 🔸 外部可呼叫的停止方法
    const stopBatchDownload = () => {
        shouldStop.current = true;
    };

    return {
        tasks,
        setTasks,
        handleDownload,
        handleDownloadAllSequentially,
        stopBatchDownload,  // 👈 新增這個
        isBatchDownloading
    };
}
