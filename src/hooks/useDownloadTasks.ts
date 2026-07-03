// useDownloadTasks.ts

import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { arrayMove } from "@dnd-kit/sortable";
import { Task, DownloadableTask } from "../types";

type UiStatus = NonNullable<DownloadableTask["status"]>;

// download_with_progress 失敗時後端回傳 { code, message }（見 src-tauri/src/error.rs），
// 比對 code 而非錯誤訊息子字串
type BackendError = { code?: string; message?: string };

function errCode(err: unknown): string {
    const code = (err as BackendError)?.code;
    return typeof code === "string" ? code : "OTHER";
}

function errMessage(err: unknown): string {
    const msg = (err as BackendError)?.message;
    return typeof msg === "string" ? msg : String(err);
}

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
            // 以 prev 順序為準（保留拖曳排序結果），新任務 append 到最後；
            // 若照 baseTasks 順序重建，reorder 後一新增任務排序就會跳回舊順序
            const baseUrls = new Set(baseTasks.map(t => t.url));
            const kept = prev.filter(t => baseUrls.has(t.url));
            const keptUrls = new Set(kept.map(t => t.url));
            const added = baseTasks
                .filter(t => !keptUrls.has(t.url))
                .map(t => ({ ...t, status: dbStatusToUi(t.db_status), progress: 0 }));
            return sortTasks([...kept, ...added]);
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

    // --- 處理下載失敗（共用邏輯），回傳是否為使用者取消 ---
    async function applyResult(taskUrl: string, err: unknown): Promise<boolean> {
        const code = errCode(err);
        if (code === "CANCELLED") {
            setTasks(prev => prev.map(t =>
                t.url === taskUrl ? { ...t, status: "paused", progress: 0 } : t
            ));
            await persistStatus(taskUrl, "paused");
            return true;
        }
        if (code === "NOT_FOUND") {
            await persistStatus(taskUrl, "not_found");
            setTasks(prev =>
                sortTasks(prev.map(t =>
                    t.url === taskUrl
                        ? { ...t, status: "not_found", errorMessage: "找不到檔案 (404)" }
                        : t
                ))
            );
            return false;
        }
        setTasks(prev => prev.map(t =>
            t.url === taskUrl ? { ...t, status: "error", errorMessage: errMessage(err) } : t
        ));
        return false;
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
                fileUrl: task.file_url ?? "",
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

        // 每輪批次中每個任務只嘗試一次：失敗任務維持 error 狀態但不再重選，
        // 否則持久性錯誤（如伺服器一直 500）會無限重試
        const attempted = new Set<string>();
        const isPending = (t: DownloadableTask) =>
            (t.status === "idle" || t.status === "error" || t.status === "paused") &&
            !attempted.has(t.url);

        while (true) {
            if (shouldStop.current) break;

            const next = tasksRef.current.find(isPending);
            if (!next) break;
            attempted.add(next.url);

            const remaining = tasksRef.current.filter(isPending).length + 1;
            setBatchProgress({ current: completed, total: completed + remaining });

            setTasks(prev => prev.map(t =>
                t.url === next.url ? { ...t, status: "downloading", progress: 0 } : t
            ));

            try {
                const savePath = await invoke<string>("download_with_progress", {
                    url: next.download_page_href,
                    title: next.title,
                    fileUrl: next.file_url ?? "",
                });
                completed++;
                setBatchProgress(prev => ({ ...prev, current: completed }));
                setTasks(prev => prev.map(t =>
                    t.url === next.url ? { ...t, status: "done", progress: 100, savePath } : t
                ));
                await persistStatus(next.url, "done");
            } catch (err) {
                const cancelled = await applyResult(next.url, err);
                if (cancelled) break;
            }
        }

        setIsBatchDownloading(false);
    };

    const stopBatchDownload = () => {
        shouldStop.current = true;
        invoke("cancel_download");
    };

    const reorderTasks = useCallback((activeUrl: string, overUrl: string) => {
        // 先算結果再 setState，invoke 不放進 updater（StrictMode 會雙呼 updater）
        const prev = tasksRef.current;
        const oldIndex = prev.findIndex(t => t.url === activeUrl);
        const newIndex = prev.findIndex(t => t.url === overUrl);
        if (oldIndex === -1 || newIndex === -1) return;
        const next = arrayMove(prev, oldIndex, newIndex);
        setTasks(next);
        invoke("reorder_tasks", { urls: next.map(t => t.url) }).catch(() => {});
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
