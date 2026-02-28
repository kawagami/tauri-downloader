// TaskListView.tsx

import React from "react";
import { DownloadableTask } from "../types";

interface TaskListViewProps {
    tasks: DownloadableTask[];
    onRemoveTask: (url: string) => void;
    onRemoveAll: () => void;
    onDownload: (task: DownloadableTask) => void;
    onDownloadAll: () => void;
    onStopDownloadAll: () => void;
    isBatchDownloading: boolean;
}

export const TaskListView: React.FC<TaskListViewProps> = ({
    tasks,
    onRemoveTask,
    onRemoveAll,
    onDownload,
    onDownloadAll,
    onStopDownloadAll,
    isBatchDownloading,
}) => (
    <div className="task-list-container">
        <div style={{ marginBottom: "10px" }}>
            <button onClick={onRemoveAll}>全部刪除</button>

            {!isBatchDownloading ? (
                <button
                    onClick={onDownloadAll}
                    disabled={tasks.length === 0}
                    style={{ marginLeft: "10px" }}
                >
                    全部下載
                </button>
            ) : (
                <button
                    onClick={onStopDownloadAll}
                    style={{ marginLeft: "10px", background: "#f87171", color: "white" }}
                >
                    停止下載
                </button>
            )}
        </div>

        <table className="task-table">
            <thead>
                <tr>
                    <th>標題</th>
                    <th>預覽圖</th>
                    <th>進度</th>
                    <th>操作</th>
                </tr>
            </thead>
            <tbody>
                {tasks.map((task) => (
                    <tr key={task.url}>
                        <td>
                            <a
                                href={task.url}
                                target="_blank"
                                rel="noreferrer"
                                style={{ textDecoration: 'none', color: '#3b82f6' }}
                                title={task.url}
                            >
                                {task.title}
                            </a>
                        </td>

                        <td>
                            {task.image ? (
                                <div className="image-container">
                                    <img src={task.image} alt={task.title} className="thumbnail" />
                                    <div className="image-preview">
                                        <img src={task.image} alt={task.title} />
                                    </div>
                                </div>
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
                                            style={{ width: `${task.progress ?? 0}%` }}
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
                            <button onClick={() => onRemoveTask(task.url)}>刪除</button>
                            <button
                                onClick={() => onDownload(task)}
                                disabled={task.status === "downloading"}
                                style={{ marginLeft: "5px" }}
                            >
                                {task.status === "downloading" ? "下載中..." : "下載"}
                            </button>
                        </td>
                    </tr>
                ))}
            </tbody>
        </table>
    </div>
);
