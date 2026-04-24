// TaskListView.tsx

import React from "react";
import { DownloadableTask } from "../types";

const formatSpeed = (bps: number) => {
    if (bps < 1024) return `${bps.toFixed(0)} B/s`;
    if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
    return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
};

const formatTime = (secs: number) => {
    if (!isFinite(secs) || secs <= 0) return "計算中";
    if (secs < 60) return `${Math.ceil(secs)}s`;
    return `${Math.floor(secs / 60)}m ${Math.ceil(secs % 60)}s`;
};

interface TaskListViewProps {
    tasks: DownloadableTask[];
    onRemoveTask: (url: string) => void;
    onRemoveAll: () => void;
    onDownload: (task: DownloadableTask) => void;
    onDownloadAll: () => void;
    onStopDownloadAll: () => void;
    isBatchDownloading: boolean;
    batchProgress: { current: number; total: number };
}

export const TaskListView: React.FC<TaskListViewProps> = ({
    tasks,
    onRemoveTask,
    onRemoveAll,
    onDownload,
    onDownloadAll,
    onStopDownloadAll,
    isBatchDownloading,
    batchProgress,
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
                    停止下載 ({batchProgress.current} / {batchProgress.total})
                </button>
            )}
        </div>

        <table className="task-table">
            <thead>
                <tr>
                    <th>標題</th>
                    <th>預覽圖</th>
                    <th>新增時間</th>
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

                        <td style={{ fontSize: "0.75rem", color: "#6b7280", whiteSpace: "nowrap" }}>
                            {task.created_at
                                ? new Date(task.created_at * 1000).toLocaleString()
                                : "-"}
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
                                    <div className="text-xs text-gray-500 mt-1" style={{ lineHeight: "1.6" }}>
                                        <div>{(task.progress ?? 0).toFixed(1)}%</div>
                                        {task.speed != null && <div>{formatSpeed(task.speed)}</div>}
                                        {task.timeRemaining != null && <div>{formatTime(task.timeRemaining)}</div>}
                                    </div>
                                </>
                            ) : task.status === "done" ? (
                                <span className="text-green-600">完成 ✅</span>
                            ) : task.status === "error" ? (
                                <span className="text-red-500" title={task.errorMessage}>錯誤 ❌</span>
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
