// Toolbar.tsx

import React from "react";

interface ToolbarProps {
    monitorClipboard: boolean;
    onMonitorChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
    onRemoveAll: () => void;
    onClearDone: () => void;
    onDownloadAll: () => void;
    onStopDownload: () => void;
    isBatchDownloading: boolean;
    batchProgress: { current: number; total: number };
    tasksEmpty: boolean;
    hasDoneTasks: boolean;
    bandwidthKbps: number;
    onBandwidthChange: (kbps: number) => void;
}

export const Toolbar: React.FC<ToolbarProps> = ({
    monitorClipboard,
    onMonitorChange,
    onRemoveAll,
    onClearDone,
    onDownloadAll,
    onStopDownload,
    isBatchDownloading,
    batchProgress,
    tasksEmpty,
    hasDoneTasks,
    bandwidthKbps,
    onBandwidthChange,
}) => (
    <div className="sticky-toolbar">
        <div className="checkbox-group">
            <input
                type="checkbox"
                id="monitorClipboard"
                checked={monitorClipboard}
                onChange={onMonitorChange}
            />
            <label htmlFor="monitorClipboard">監控剪貼簿</label>
        </div>
        <div className="toolbar-actions">
            <button onClick={onRemoveAll}>全部刪除</button>
            <button
                onClick={onClearDone}
                disabled={!hasDoneTasks}
                style={{ marginLeft: "10px" }}
            >
                清除已完成
            </button>
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
            <input
                type="number"
                min="0"
                step="1"
                value={bandwidthKbps === 0 ? "" : bandwidthKbps}
                placeholder="無限制"
                onChange={e => {
                    const val = parseInt(e.target.value, 10);
                    onBandwidthChange(isNaN(val) || val < 0 ? 0 : val);
                }}
                style={{ width: "80px", marginLeft: "10px" }}
            />
            <span style={{ marginLeft: "4px", fontSize: "12px" }}>KB/s</span>
        </div>
    </div>
);
