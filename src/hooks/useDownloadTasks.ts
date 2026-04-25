// useDownloadTasks.ts

import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Task, DownloadableTask } from "../types";

type UiStatus = NonNullable<DownloadableTask["status"]>;

function dbStatusToUi(dbStatus: string): UiStatus {
    switch (dbStatus) {
        case "not_found": return "not_found";
        case "paused": return "paused";
        case "done": return "done";
        default: return "idle";
    }
}

function sortTasks(arr: DownloadableTask[]): DownloadableTask[] {
    return arr.slice().sort((a, b) =>
        (a.status === "not_found" ? 1 : 0) - (b.status === "not_found" ? 1 : 0)
    );
}

async function persistStatus(url: string, status: string) {
    await invoke("update_task_status", { url, status }).catch(() => {});
}

export function useDownloadTasks(baseTasks: Task[], onRemoveTask: (url: string) => void) {
    const [tasks, setTasks] = useState<DownloadableTask[]>(() =>
        sortTasks(baseTasks.map(t => ({ ...t, status: dbStatusToUi(t.db_status), progress: 0 })))
    );
    const [isBatchDownloading, setIsBatchDownloading] = useState(false);
    const [batchProgress, setBatchProgress] = useState({ current: 0, total: 0 });
    const shouldStop = useRef(false);

    useEffect(() => {
        setTasks(prev => {
            const prevMap = new Map(prev.map(t => [t.url, t]));
            const merged = baseTasks.map(t =>
                prevMap.get(t.url) ?? { ...t, status: dbStatusToUi(t.db_status), progress: 0 }
            );
            return sortTasks(merged);
        });
    }, [baseTasks]);

    // --- 監聽進度事件 ---
    useEffect(() => {
        let unlisten: (() => void) | undefined;
        const setup = async () => {
            unlisten = await listen<{
                url: string;
                progress: number;
                speed_bytes_per_sec: number;
                time_remaining_secs: number;
            }>("download_progress", (event) => {
                const { url, progress, speed_bytes_per_sec, time_remaining_secs } = event.payload;
                setTasks(prev =>
                    prev.map(t =>
                        t.download_page_href === url
                            ? { ...t, progress, speed: speed_bytes_per_sec, timeRemaining: time_remaining_secs }
                            : t
                    )
                );
            });
        };
        setup();
        return () => unlisten?.();
    }, []);

    // --- 處理下載結果（共用邏輯）---
    async function applyResult(taskUrl: string, err?: unknown) {
        if (!err) return; // success handled at call site

        const errStr = String(err);
        if (errStr.includes("已取消")) {
            setTasks(prev => prev.map(t =>
                t.url === taskUrl ? { ...t, status: "paused", progress: 0 } : t
            ));
            await persistStatus(taskUrl, "paused");
        } else if (errStr.includes("NOT_FOUND")) {
            await persistStatus(taskUrl, "not_found");
            setTasks(prev =>
                sortTasks(prev.map(t =>
                    t.url === taskUrl
                        ? { ...t, status: "not_found", errorMessage: "找不到檔案 (404)" }
                        : t
                ))
            );
        } else {
            setTasks(prev => prev.map(t =>
                t.url === taskUrl ? { ...t, status: "error", errorMessage: errStr } : t
            ));
        }
    }

    // --- 單一下載 ---
    const handleDownload = async (task: DownloadableTask) => {
        setTasks(prev => prev.map(t =>
            t.url === task.url ? { ...t, status: "downloading", progress: 0 } : t
        ));
        try {
            const savePath = await invoke<string>("download_with_progress", {
                url: task.download_page_href,
                title: task.title,
            });
            setTasks(prev => prev.map(t =>
                t.url === task.url ? { ...t, status: "done", progress: 100, savePath } : t
            ));
        } catch (err) {
            await applyResult(task.url, err);
        }
    };

    // --- 批次下載 ---
    const handleDownloadAllSequentially = async () => {
        if (isBatchDownloading) return;
        shouldStop.current = false;
        setIsBatchDownloading(true);

        const pending = tasks.filter(t =>
            t.status === "idle" || t.status === "error" || t.status === "paused"
        );
        setBatchProgress({ current: 0, total: pending.length });
        let current = 0;

        for (const task of tasks) {
            if (shouldStop.current) break;
            if (task.status !== "idle" && task.status !== "error" && task.status !== "paused") continue;

            current += 1;
            setBatchProgress(prev => ({ ...prev, current }));

            setTasks(prev => prev.map(t =>
                t.url === task.url ? { ...t, status: "downloading", progress: 0 } : t
            ));

            try {
                await invoke<string>("download_with_progress", {
                    url: task.download_page_href,
                    title: task.title,
                });

                setTasks(prev => prev.map(t =>
                    t.url === task.url ? { ...t, status: "done", progress: 100 } : t
                ));

                onRemoveTask(task.url);
                await new Promise(resolve => setTimeout(resolve, 1000));
            } catch (err) {
                await applyResult(task.url, err);
                if (String(err).includes("已取消")) break;
            }
        }

        setIsBatchDownloading(false);
    };

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
