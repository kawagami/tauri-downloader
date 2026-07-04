import React from "react";
import {
  deleteHttpDownload,
  pauseHttpDownload,
  resumeHttpDownload,
  updateHttpUrl,
  type HttpTaskItem,
} from "../../lib/httpApi";
import { formatBytes, formatEta, formatSpeed } from "../../lib/format";

interface Props {
  t: HttpTaskItem;
  onActionError: (msg: string) => void;
}

function stateLabel(t: HttpTaskItem): string {
  switch (t.state) {
    case "error":
      return "錯誤";
    case "finished":
      return "完成";
    case "running":
      return "下載中";
    case "paused":
      return "已暫停";
  }
}

function badgeClass(t: HttpTaskItem): string {
  switch (t.state) {
    case "error":
      return "status-error";
    case "finished":
      return "status-done";
    case "running":
      return "status-live";
    case "paused":
      return "status-paused";
  }
}

export const HttpRow = React.memo(function HttpRow({ t, onActionError }: Props) {
  async function run(action: () => Promise<void>) {
    try {
      await action();
    } catch (e) {
      onActionError(String(e));
    }
  }

  function onUpdateUrl() {
    const url = window.prompt("連結已失效。請重新複製同一檔案的下載連結並貼上:");
    if (url?.trim()) run(() => updateHttpUrl(t.id, url.trim()));
  }

  function onDelete() {
    const withFiles = window.confirm(
      `刪除「${t.name}」。\n\n按「確定」同時刪除已下載檔案;按「取消」回到列表。`,
    );
    if (withFiles) run(() => deleteHttpDownload(t.id, true));
  }

  function onRemove() {
    if (window.confirm(`移除任務「${t.name}」?已下載檔案會保留。`)) {
      run(() => deleteHttpDownload(t.id, false));
    }
  }

  return (
    <div id={`http-row-${t.id}`} className={`torrent-row ${t.error ? "has-error" : ""}`}>
      <div className="row-main">
        <div className="row-title">
          <span className="name">{t.name}</span>
          <span className={`status-badge ${badgeClass(t)}`}>{stateLabel(t)}</span>
        </div>
        <div className="progress-track">
          <div
            className="progress-fill"
            style={{ width: `${Math.min(100, t.progress_percent).toFixed(2)}%` }}
          />
        </div>
        <div className="row-stats">
          <span>{t.progress_percent.toFixed(1)}%</span>
          <span>
            {formatBytes(t.downloaded_bytes)}
            {t.total_bytes > 0 && ` / ${formatBytes(t.total_bytes)}`}
          </span>
          <span>↓ {formatSpeed(t.down_speed_bps)}</span>
          {t.state === "running" && t.total_bytes > 0 && (
            <span>剩 {formatEta(t.downloaded_bytes, t.total_bytes, t.down_speed_bps)}</span>
          )}
        </div>
        {t.error && <div className="row-error">{t.error}</div>}
      </div>
      <div className="row-actions">
        {t.state === "running" && (
          <button type="button" className="btn-sm" onClick={() => run(() => pauseHttpDownload(t.id))}>
            暫停
          </button>
        )}
        {t.state === "paused" && (
          <button type="button" className="btn-sm" onClick={() => run(() => resumeHttpDownload(t.id))}>
            恢復
          </button>
        )}
        {t.state === "error" && t.retryable && (
          <button type="button" className="btn-sm" onClick={() => run(() => resumeHttpDownload(t.id))}>
            重試
          </button>
        )}
        {t.state === "error" && !t.retryable && (
          <button type="button" className="btn-sm" onClick={onUpdateUrl}>
            更新連結
          </button>
        )}
        <button type="button" className="btn-sm" onClick={onRemove}>
          移除
        </button>
        <button type="button" className="btn-danger btn-sm" onClick={onDelete}>
          刪除
        </button>
      </div>
    </div>
  );
});
