// TaskListView.tsx

import React, { useCallback, useEffect, useRef, useState } from "react";
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

const STORAGE_KEY = "task-table-col-widths";
const DEFAULT_WIDTHS = [300, 80, 140, 120, 130];
const COL_NAMES = ["標題", "預覽圖", "新增時間", "進度", "操作"];

function loadWidths(): number[] {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (raw) {
            const parsed = JSON.parse(raw);
            if (Array.isArray(parsed) && parsed.length === DEFAULT_WIDTHS.length) return parsed;
        }
    } catch {}
    return [...DEFAULT_WIDTHS];
}

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
    const [colWidths, setColWidths] = useState<number[]>(loadWidths);
    const dragging = useRef<{ colIndex: number; startX: number; startWidth: number } | null>(null);

    const onMouseDown = useCallback((colIndex: number, e: React.MouseEvent) => {
        e.preventDefault();
        dragging.current = { colIndex, startX: e.clientX, startWidth: colWidths[colIndex] };
    }, [colWidths]);

    useEffect(() => {
        const onMouseMove = (e: MouseEvent) => {
            if (!dragging.current) return;
            const { colIndex, startX, startWidth } = dragging.current;
            const delta = e.clientX - startX;
            const newWidth = Math.max(50, startWidth - delta);
            setColWidths(prev => {
                const next = [...prev];
                next[colIndex] = newWidth;
                return next;
            });
        };
        const onMouseUp = () => {
            if (!dragging.current) return;
            dragging.current = null;
            setColWidths(prev => {
                localStorage.setItem(STORAGE_KEY, JSON.stringify(prev));
                return prev;
            });
        };
        window.addEventListener("mousemove", onMouseMove);
        window.addEventListener("mouseup", onMouseUp);
        return () => {
            window.removeEventListener("mousemove", onMouseMove);
            window.removeEventListener("mouseup", onMouseUp);
        };
    }, []);

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
