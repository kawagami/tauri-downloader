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
    totalCount: number;
    doneCount: number;
    pendingCount: number;
    hasDownloadable: boolean;
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
    totalCount,
    doneCount,
    pendingCount,
    hasDownloadable,
    hasDoneTasks,
    bandwidthKbps,
    onBandwidthChange,
    dingVolume,
    onDingVolumeChange,
}) => (
    <div className="sticky-toolbar" style={{ flexDirection: "column", alignItems: "flex-start", gap: "6px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "16px", flexWrap: "wrap" }}>
            <div className="checkbox-group">
                <input
                    type="checkbox"
                    id="monitorClipboard"
                    checked={monitorClipboard}
                    onChange={onMonitorChange}
                />
                <label htmlFor="monitorClipboard">監控剪貼簿</label>
            </div>
            <div className="toolbar-field">
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
                <span>KB/s</span>
            </div>
            <div className="toolbar-field">
                <span>通知音量</span>
                <input
                    type="range"
                    min="0"
                    max="3"
                    step="0.05"
                    value={dingVolume}
                    onChange={e => onDingVolumeChange(Number(e.target.value))}
                    style={{ width: "80px" }}
                />
                <span style={{ width: "28px" }}>{Math.round(dingVolume * 100)}%</span>
            </div>
            {totalCount > 0 && (
                <div className="toolbar-summary">
                    共 {totalCount} 筆 · {doneCount} 完成 · {pendingCount} 待下載
                </div>
            )}
        </div>
        <div className="toolbar-actions" style={{ flexWrap: "wrap" }}>
            <button
                onClick={() => {
                    if (window.confirm(`確定刪除全部 ${totalCount} 筆任務？`)) onRemoveAll();
                }}
                disabled={totalCount === 0}
            >
                全部刪除
            </button>
            <button onClick={onClearDone} disabled={!hasDoneTasks}>
                清除已完成
            </button>
            {!isBatchDownloading ? (
                <button className="btn-primary" onClick={onDownloadAll} disabled={!hasDownloadable}>
                    全部下載
                </button>
            ) : (
                <button className="btn-danger" onClick={onStopDownload}>
                    停止下載 ({batchProgress.current} / {batchProgress.total})
                </button>
            )}
        </div>
    </div>
);
