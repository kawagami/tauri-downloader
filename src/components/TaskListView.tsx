import React from "react";
import { DownloadableTask } from "../types";

interface TaskListViewProps {
    tasks: DownloadableTask[];
    onRemoveTask: (url: string) => void;
    onRemoveAll: () => void;
    onDownload: (task: DownloadableTask) => void;
    onDownloadAll: () => void; // ✅ 新增
    isBatchDownloading: boolean; // ✅ 新增
}

export const TaskListView: React.FC<TaskListViewProps> = ({
    tasks,
    onRemoveTask,
    onRemoveAll,
    onDownload,
    onDownloadAll,
    isBatchDownloading,
}) => (
    <div className="task-list-container">
        <div style={{ marginBottom: "10px" }}>
            <button onClick={onRemoveAll}>全部刪除</button>
            <button
                onClick={onDownloadAll}
                disabled={isBatchDownloading || tasks.length === 0}
                style={{ marginLeft: "10px" }}
            >
                {isBatchDownloading ? "批次下載中..." : "全部下載"}
            </button>
        </div>
        <table className="task-table">
            <thead>
                <tr>
                    <th>標題</th>
                    <th>連結</th>
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
