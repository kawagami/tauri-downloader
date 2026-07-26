// Toolbar.tsx

import React from "react";

interface ToolbarProps {
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
}

export const Toolbar: React.FC<ToolbarProps> = ({
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
}) => (
    // 頻寬限制與預設目錄已搬進統一設定 dialog（tab bar ⚙），這裡只留清單操作
    <div className="sticky-toolbar" style={{ flexDirection: "column", alignItems: "flex-start", gap: "6px" }}>
        {totalCount > 0 && (
            <div className="toolbar-summary">
                共 {totalCount} 筆 · {doneCount} 完成 · {pendingCount} 待下載
            </div>
        )}
        <div className="toolbar-actions" style={{ flexWrap: "wrap" }}>
            <button
                onClick={() => {
                    if (window.confirm(`確定刪除全部 ${totalCount} 筆任務？`)) onRemoveAll();
                }}
                // 批次下載中清掉清單會讓進行中的下載失去取消入口，先停再刪
                disabled={totalCount === 0 || isBatchDownloading}
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
