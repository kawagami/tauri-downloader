import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Task, DownloadableTask } from "../types";

export function useDownloadTasks(baseTasks: Task[]) {
    const [tasks, setTasks] = useState<DownloadableTask[]>(
        baseTasks.map((t) => ({ ...t, status: "idle", progress: 0 }))
    );

    useEffect(() => {
        setTasks(baseTasks.map((t) => ({ ...t, status: "idle", progress: 0 })));
    }, [baseTasks]);

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

    return { tasks, setTasks, handleDownload };
}
