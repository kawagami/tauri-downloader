// Toolbar.tsx

import React from "react";
import { TaskInputForm } from "./TaskInputForm";

interface ToolbarProps {
    monitorClipboard: boolean;
    onMonitorChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
    onRemoveAll: () => void;
    onDownloadAll: () => void;
    onStopDownload: () => void;
    isBatchDownloading: boolean;
    batchProgress: { current: number; total: number };
    tasksEmpty: boolean;
}

export const Toolbar: React.FC<ToolbarProps> = ({
    monitorClipboard,
    onMonitorChange,
    onRemoveAll,
    onDownloadAll,
    onStopDownload,
    isBatchDownloading,
    batchProgress,
    tasksEmpty,
}) => (
    <div className="sticky-toolbar">
        <TaskInputForm
            monitorClipboard={monitorClipboard}
            onMonitorChange={onMonitorChange}
        />
        <div className="toolbar-actions">
            <button onClick={onRemoveAll}>全部刪除</button>
            {!isBatchDownloading ? (
                <button
                    onClick={onDownloadAll}
                    disabled={tasksEmpty}
                    style={{ marginLeft: "10px" }}
                >
                    全部下載
                </button>
            ) : (
                <button
                    onClick={onStopDownload}
                    style={{ marginLeft: "10px", background: "#f87171", color: "white" }}
                >
                    停止下載 ({batchProgress.current} / {batchProgress.total})
                </button>
            )}
        </div>
    </div>
);
