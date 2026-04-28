// useDownloadTasks.ts

import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { arrayMove } from "@dnd-kit/sortable";
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
    const tasksRef = useRef<DownloadableTask[]>([]);

    useEffect(() => {
        setTasks(prev => {
            const prevMap = new Map(prev.map(t => [t.url, t]));
            const merged = baseTasks.map(t =>
                prevMap.get(t.url) ?? { ...t, status: dbStatusToUi(t.db_status), progress: 0 }
            );
            return sortTasks(merged);
        });
    }, [baseTasks]);

    useEffect(() => { tasksRef.current = tasks; }, [tasks]);

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
            await persistStatus(task.url, "done");
        } catch (err) {
            await applyResult(task.url, err);
        }
    };

    // --- 清除已完成 ---
    const handleClearDone = useCallback(async () => {
        await Promise.all(
            tasks.filter(t => t.status === "done").map(t => onRemoveTask(t.url))
        );
    }, [tasks, onRemoveTask]);

    // --- 批次下載 ---
    const handleDownloadAllSequentially = async () => {
        if (isBatchDownloading) return;
        shouldStop.current = false;
        setIsBatchDownloading(true);
        let completed = 0;

        const isPending = (t: DownloadableTask) =>
            t.status === "idle" || t.status === "error" || t.status === "paused";

        while (true) {
            if (shouldStop.current) break;

            const next = tasksRef.current.find(isPending);
            if (!next) break;

            const remaining = tasksRef.current.filter(isPending).length;
            setBatchProgress({ current: completed, total: completed + remaining });

            setTasks(prev => prev.map(t =>
                t.url === next.url ? { ...t, status: "downloading", progress: 0 } : t
            ));

            try {
                const savePath = await invoke<string>("download_with_progress", {
                    url: next.download_page_href,
                    title: next.title,
                });
                completed++;
                setBatchProgress(prev => ({ ...prev, current: completed }));
                setTasks(prev => prev.map(t =>
                    t.url === next.url ? { ...t, status: "done", progress: 100, savePath } : t
                ));
                await persistStatus(next.url, "done");
            } catch (err) {
                await applyResult(next.url, err);
                if (String(err).includes("已取消")) break;
            }
        }

        setIsBatchDownloading(false);
    };

    const stopBatchDownload = () => {
        shouldStop.current = true;
        invoke("cancel_download");
    };

    const reorderTasks = useCallback((activeUrl: string, overUrl: string) => {
        setTasks(prev => {
            const oldIndex = prev.findIndex(t => t.url === activeUrl);
            const newIndex = prev.findIndex(t => t.url === overUrl);
            if (oldIndex === -1 || newIndex === -1) return prev;
            const next = arrayMove(prev, oldIndex, newIndex);
            invoke("reorder_tasks", { urls: next.map(t => t.url) }).catch(() => {});
            return next;
        });
    }, []);

    return {
        tasks,
        setTasks,
        handleDownload,
        handleClearDone,
        handleDownloadAllSequentially,
        stopBatchDownload,
        isBatchDownloading,
        batchProgress,
        reorderTasks,
    };
}
