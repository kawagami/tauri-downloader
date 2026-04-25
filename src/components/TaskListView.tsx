// TaskListView.tsx

import React from "react";
import { DownloadableTask } from "../types";
import { useColumnResize } from "../hooks/useColumnResize";

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

const COL_NAMES = ["標題", "預覽圖", "新增時間", "進度", "操作"];
const DEFAULT_WIDTHS = [300, 80, 140, 120, 130];

interface TaskListViewProps {
    tasks: DownloadableTask[];
    onRemoveTask: (url: string) => void;
    onDownload: (task: DownloadableTask) => void;
}

export const TaskListView: React.FC<TaskListViewProps> = ({
    tasks,
    onRemoveTask,
    onDownload,
}) => {
    const { colWidths, onMouseDown } = useColumnResize("task-table-col-widths", DEFAULT_WIDTHS);

    return (
        <div className="task-list-container">
            <table className="task-table" style={{ tableLayout: "fixed", width: "100%" }}>
                <colgroup>
                    <col />
                    {colWidths.slice(1).map((w, i) => <col key={i + 1} style={{ width: w }} />)}
                </colgroup>
                <thead>
                    <tr>
                        {COL_NAMES.map((name, i) => (
                            <th key={i} style={{ position: "relative", userSelect: "none", overflow: "hidden" }}>
                                {name}
                                {i > 0 && (
                                    <div
                                        onMouseDown={(e) => onMouseDown(i, e)}
                                        style={{
                                            position: "absolute",
                                            left: 0,
                                            top: 0,
                                            bottom: 0,
                                            width: 6,
                                            cursor: "col-resize",
                                            background: "transparent",
                                        }}
                                    />
                                )}
                            </th>
                        ))}
                    </tr>
                </thead>
                <tbody>
                    {tasks.map((task) => (
                        <tr key={task.url}>
                            <td style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
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

                            <td style={{ fontSize: "0.75rem", color: "#6b7280", whiteSpace: "nowrap", overflow: "hidden" }}>
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
                                ) : task.status === "not_found" ? (
                                    <span className="text-red-400" title={task.errorMessage}>找不到 🚫</span>
                                ) : task.status === "paused" ? (
                                    <span style={{ color: "#f59e0b" }}>已暫停 ⏸</span>
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
};
