// TaskListView.tsx

import React from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
    DndContext,
    closestCenter,
    PointerSensor,
    useSensor,
    useSensors,
    DragEndEvent,
} from "@dnd-kit/core";
import {
    SortableContext,
    verticalListSortingStrategy,
    useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
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

const formatBytes = (bytes: number) => {
    if (bytes < 0) return "未知";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

const COL_NAMES = ["標題", "預覽圖", "新增時間", "大小", "進度", "操作"];
const DEFAULT_WIDTHS = [300, 80, 140, 90, 120, 130];

interface SortableRowProps {
    task: DownloadableTask;
    onRemoveTask: (url: string) => void;
    onDownload: (task: DownloadableTask) => void;
    isBatchDownloading: boolean;
}

const SortableRow: React.FC<SortableRowProps> = ({ task, onRemoveTask, onDownload, isBatchDownloading }) => {
    const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: task.url });

    const rowStyle: React.CSSProperties = {
        transform: CSS.Translate.toString(transform),
        transition,
        opacity: isDragging ? 0.5 : 1,
    };

    return (
        <tr ref={setNodeRef} style={rowStyle}>
            <td {...attributes} {...listeners} className="drag-handle">
                ⠿
            </td>
            <td style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                <a
                    href={task.url}
                    target="_blank"
                    rel="noreferrer"
                    className="task-title-link"
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

            <td className="meta-text">
                {task.created_at
                    ? new Date(task.created_at * 1000).toLocaleString()
                    : "-"}
            </td>
            <td className="meta-text">{formatBytes(task.file_size ?? -1)}</td>
            <td>
                {task.status === "downloading" ? (
                    <>
                        {(task.progress ?? 0) < 0 ? (
                            <div className="progress-indeterminate" />
                        ) : (
                            <div className="progress-track">
                                <div className="progress-fill" style={{ width: `${task.progress ?? 0}%` }} />
                            </div>
                        )}
                        <div className="progress-meta">
                            <div>{(task.progress ?? 0) < 0 ? "計算中" : `${(task.progress ?? 0).toFixed(1)}%`}</div>
                            {task.speed != null && task.speed > 0 && <div>{formatSpeed(task.speed)}</div>}
                            {task.timeRemaining != null && task.timeRemaining > 0 && <div>{formatTime(task.timeRemaining)}</div>}
                        </div>
                    </>
                ) : task.status === "done" ? (
                    <span className="status-badge status-done">完成 ✅</span>
                ) : task.status === "error" ? (
                    <div>
                        <span className="status-badge status-error">錯誤 ❌</span>
                        {task.errorMessage && (
                            <div className="status-msg" style={{ color: "var(--danger)" }} title={task.errorMessage}>
                                {task.errorMessage}
                            </div>
                        )}
                    </div>
                ) : task.status === "not_found" ? (
                    <div>
                        <span className="status-badge status-not-found">找不到 🚫</span>
                        {task.errorMessage && (
                            <div className="status-msg" style={{ color: "#b91c1c" }} title={task.errorMessage}>
                                {task.errorMessage}
                            </div>
                        )}
                    </div>
                ) : task.status === "paused" ? (
                    <span className="status-badge status-paused">已暫停 ⏸</span>
                ) : (
                    <span className="status-badge status-idle">待下載</span>
                )}
            </td>
            <td>
                <div style={{ display: "flex", gap: 6 }}>
                    {/* 下載中禁止刪除：任務移除後後端串流仍會繼續寫檔且無法單獨取消 */}
                    <button
                        className="btn-sm btn-danger"
                        onClick={() => onRemoveTask(task.url)}
                        disabled={task.status === "downloading"}
                    >
                        刪除
                    </button>
                    {/* 批次進行中禁止單筆下載：並行下載共用全域取消旗標會互相干擾 */}
                    <button
                        className="btn-sm btn-primary"
                        onClick={() => onDownload(task)}
                        disabled={task.status === "downloading" || isBatchDownloading}
                    >
                        {task.status === "downloading" ? "下載中..." : "下載"}
                    </button>
                    {task.status === "done" && task.savePath && (
                        <button className="btn-sm" onClick={() => revealItemInDir(task.savePath!)}>
                            開啟
                        </button>
                    )}
                </div>
            </td>
        </tr>
    );
};

interface TaskListViewProps {
    tasks: DownloadableTask[];
    onRemoveTask: (url: string) => void;
    onDownload: (task: DownloadableTask) => void;
    onReorder: (activeUrl: string, overUrl: string) => void;
    isBatchDownloading: boolean;
}

export const TaskListView: React.FC<TaskListViewProps> = ({
    tasks,
    onRemoveTask,
    onDownload,
    onReorder,
    isBatchDownloading,
}) => {
    const { colWidths, onMouseDown } = useColumnResize("task-table-col-widths", DEFAULT_WIDTHS);

    const sensors = useSensors(useSensor(PointerSensor));

    const handleDragEnd = (event: DragEndEvent) => {
        const { active, over } = event;
        if (over && active.id !== over.id) {
            onReorder(active.id as string, over.id as string);
        }
    };

    return (
        <div className="task-list-container">
            <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
                <table className="task-table" style={{ tableLayout: "fixed", width: "100%" }}>
                    <colgroup>
                        <col style={{ width: 20 }} />
                        <col />
                        {colWidths.slice(1).map((w, i) => <col key={i + 1} style={{ width: w }} />)}
                    </colgroup>
                    <thead>
                        <tr>
                            <th style={{ width: 20 }}></th>
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
                    <SortableContext items={tasks.map(t => t.url)} strategy={verticalListSortingStrategy}>
                        <tbody>
                            {tasks.length === 0 ? (
                                <tr>
                                    <td colSpan={COL_NAMES.length + 1} className="empty-state">
                                        尚無任務 — 開啟「監控剪貼簿」並複製連結，或直接拖入連結
                                    </td>
                                </tr>
                            ) : (
                                tasks.map((task) => (
                                    <SortableRow
                                        key={task.url}
                                        task={task}
                                        onRemoveTask={onRemoveTask}
                                        onDownload={onDownload}
                                        isBatchDownloading={isBatchDownloading}
                                    />
                                ))
                            )}
                        </tbody>
                    </SortableContext>
                </table>
            </DndContext>
        </div>
    );
};
