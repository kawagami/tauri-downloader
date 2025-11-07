import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Task, DownloadableTask } from "../types";

interface TaskListProps {
    tasks: Task[];
    onRemoveTask: (url: string) => void;
    onRemoveAll: () => void;
}

export const TaskList: React.FC<TaskListProps> = ({
    tasks: baseTasks,
    onRemoveTask,
    onRemoveAll,
}) => {
    // 本地維護「含下載狀態」的 task 狀態
    const [tasks, setTasks] = useState<DownloadableTask[]>(
        baseTasks.map((t) => ({ ...t, status: "idle", progress: 0 }))
    );

    useEffect(() => {
        // 若外部 tasks 更新（例如重新載入列表），也同步更新本地狀態
        setTasks(baseTasks.map((t) => ({ ...t, status: "idle", progress: 0 })));
    }, [baseTasks]);

    // 監聽進度事件
    useEffect(() => {
        let unlisten: (() => void) | undefined;

        const setup = async () => {
            unlisten = await listen<{ url: string; progress: number }>(
                "download_progress",
                (event) => {
                    // console.log("🔥 received progress:", event.payload);
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

        return () => {
            if (unlisten) unlisten();
        };
    }, []);

    const handleDownload = async (task: DownloadableTask) => {
        setTasks((prev) =>
            prev.map((t) =>
                t.url === task.url
                    ? { ...t, status: "downloading", progress: 0 }
                    : t
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
            console.error("下載失敗:", err);
            setTasks((prev) =>
                prev.map((t) =>
                    t.url === task.url ? { ...t, status: "error" } : t
                )
            );
        }
    };

    return (
        <div className="task-list-container">
            <div style={{ marginBottom: "10px" }}>
                <button onClick={onRemoveAll}>全部刪除</button>
            </div>

            <table className="task-table">
                <thead>
                    <tr>
                        <th>標題 (Name)</th>
                        <th>連結 (URL)</th>
                        <th>預覽圖</th>
                        <th>進度</th>
                        <th>操作</th>
                    </tr>
                </thead>
                <tbody>
                    {tasks.map((task) => (
                        <tr key={task.url}>
                            <td>{task.title}</td>
                            <td>
                                <a href={task.download_page_href} target="_blank">
                                    {task.url.length > 30
                                        ? task.url.substring(0, 30) + "..."
                                        : task.url}
                                </a>
                            </td>
                            <td>
                                {task.image ? (
                                    <img
                                        src={task.image}
                                        alt={task.title}
                                        className="thumbnail"
                                    />
                                ) : (
                                    <span>無圖片</span>
                                )}
                            </td>
                            <td>
                                {task.status === "downloading" ? (
                                    <>
                                        <div className="w-32 bg-gray-200 h-2 rounded">
                                            <div
                                                className="bg-green-500 h-2 rounded"
                                                style={{
                                                    width: `${task.progress ?? 0}%`,
                                                }}
                                            />
                                        </div>
                                        <div className="text-xs text-gray-500 mt-1">
                                            {(task.progress ?? 0).toFixed(1)}%
                                        </div>
                                    </>
                                ) : task.status === "done" ? (
                                    <span className="text-green-600">完成 ✅</span>
                                ) : task.status === "error" ? (
                                    <span className="text-red-500">錯誤 ❌</span>
                                ) : (
                                    <span>-</span>
                                )}
                            </td>
                            <td>
                                <button onClick={() => onRemoveTask(task.url)}>
                                    刪除
                                </button>
                                <button
                                    onClick={() => handleDownload(task)}
                                    style={{ marginLeft: "5px" }}
                                    disabled={task.status === "downloading"}
                                >
                                    {task.status === "downloading"
                                        ? "下載中..."
                                        : "下載"}
                                </button>
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
};
