// Toolbar.tsx

import React from "react";

interface ToolbarProps {
    monitorClipboard: boolean;
    onMonitorChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
    onRemoveAll: () => void;
    onClearDone: () => Promise<void>;
    onDownloadAll: () => void;
    onStopDownload: () => void;
    isBatchDownloading: boolean;
    batchProgress: { current: number; total: number };
    tasksEmpty: boolean;
    hasDoneTasks: boolean;
    bandwidthKbps: number;
    onBandwidthChange: (kbps: number) => void;
    dingVolume: number;
    onDingVolumeChange: (v: number) => void;
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
    dingVolume,
    onDingVolumeChange,
}) => (
    <div className="sticky-toolbar" style={{ flexDirection: "column", alignItems: "flex-start", gap: "6px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
            <div className="checkbox-group">
                <input
                    type="checkbox"
                    id="monitorClipboard"
                    checked={monitorClipboard}
                    onChange={onMonitorChange}
                />
                <label htmlFor="monitorClipboard">監控剪貼簿</label>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
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
                    style={{ width: "80px" }}
                />
                <span style={{ fontSize: "12px" }}>KB/s</span>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
                <span style={{ fontSize: "12px" }}>通知音量</span>
                <input
                    type="range"
                    min="0"
                    max="3"
                    step="0.05"
                    value={dingVolume}
                    onChange={e => onDingVolumeChange(Number(e.target.value))}
                    style={{ width: "80px" }}
                />
                <span style={{ fontSize: "12px", width: "28px" }}>{Math.round(dingVolume * 100)}%</span>
            </div>
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
        </div>
    </div>
);
