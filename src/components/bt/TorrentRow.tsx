import React, { useEffect, useRef, useState } from "react";
import {
  deleteTorrent,
  pauseTorrent,
  resumeTorrent,
  torrentDetails,
  type TorrentDetails,
  type TorrentStatsItem,
} from "../../lib/btApi";
import { formatBytes, formatEta, formatSpeed } from "../../lib/format";

interface Props {
  t: TorrentStatsItem;
  highlighted: boolean;
  onActionError: (msg: string) => void;
}

function stateLabel(t: TorrentStatsItem): string {
  if (t.error) return "錯誤";
  if (t.finished) return "完成";
  switch (t.state) {
    case "initializing":
      return "檢查中";
    case "live":
      return t.name === null || t.total_bytes === 0 ? "抓取 metadata" : "下載中";
    case "paused":
      return "已暫停";
    case "error":
      return "錯誤";
  }
}

function badgeClass(t: TorrentStatsItem): string {
  if (t.error || t.state === "error") return "status-error";
  if (t.finished) return "status-done";
  if (t.state === "paused") return "status-paused";
  if (t.state === "initializing") return "status-idle";
  return "status-live";
}

export const TorrentRow = React.memo(function TorrentRow({
  t,
  highlighted,
  onActionError,
}: Props) {
  const [expanded, setExpanded] = useState(false);
  const [details, setDetails] = useState<TorrentDetails | null>(null);
  // 等待秒數靠每秒一次的 stats event 觸發 re-render，不需自己開 timer
  const firstSeen = useRef(Date.now());
  const fetchingMeta = t.state === "live" && (t.name === null || t.total_bytes === 0);

  useEffect(() => {
    if (expanded && !details) {
      torrentDetails(t.id).then(setDetails).catch(() => setDetails(null));
    }
  }, [expanded, details, t.id]);

  async function run(action: () => Promise<void>) {
    try {
      await action();
    } catch (e) {
      onActionError(String(e));
    }
  }

  function onDelete() {
    const withFiles = window.confirm(
      `刪除「${t.name ?? `任務 #${t.id}`}」。\n\n按「確定」同時刪除已下載檔案;按「取消」回到列表。`
    );
    if (withFiles) run(() => deleteTorrent(t.id, true));
  }

  function onRemove() {
    if (window.confirm(`移除任務「${t.name ?? `#${t.id}`}」?已下載檔案會保留。`)) {
      run(() => deleteTorrent(t.id, false));
    }
  }

  const elapsedSec = Math.floor((Date.now() - firstSeen.current) / 1000);

  return (
    <div
      id={`torrent-row-${t.id}`}
      className={`torrent-row ${t.error ? "has-error" : ""} ${highlighted ? "highlighted" : ""}`}
    >
      <div className="row-main" onClick={() => setExpanded((v) => !v)}>
        <div className="row-title">
          <span className="name">{t.name ?? "（抓取 metadata 中…）"}</span>
          <span className={`status-badge ${badgeClass(t)}`}>{stateLabel(t)}</span>
        </div>
        <div className="progress-track">
          <div
            className="progress-fill"
            style={{ width: `${Math.min(100, t.progress_percent).toFixed(2)}%` }}
          />
        </div>
        <div className="row-stats">
          {fetchingMeta ? (
            <span>已等待 {elapsedSec}s — 冷門種子可能要很久,可手動刪除</span>
          ) : (
            <>
              <span>{t.progress_percent.toFixed(1)}%</span>
              <span>
                {formatBytes(t.downloaded_bytes)} / {formatBytes(t.total_bytes)}
              </span>
              <span>↓ {formatSpeed(t.down_speed_bps)}</span>
              <span>↑ {formatSpeed(t.up_speed_bps)}</span>
              <span>peers {t.peers_live}</span>
              {!t.finished && t.state === "live" && (
                <span>剩 {formatEta(t.downloaded_bytes, t.total_bytes, t.down_speed_bps)}</span>
              )}
            </>
          )}
        </div>
        {t.error && <div className="row-error">{t.error}</div>}
      </div>
      <div className="row-actions">
        {t.state === "paused" ? (
          <button type="button" className="btn-sm" onClick={() => run(() => resumeTorrent(t.id))}>
            恢復
          </button>
        ) : (
          <button
            type="button"
            className="btn-sm"
            disabled={t.state === "initializing"}
            onClick={() => run(() => pauseTorrent(t.id))}
          >
            暫停
          </button>
        )}
        <button type="button" className="btn-sm" onClick={onRemove}>
          移除
        </button>
        <button type="button" className="btn-danger btn-sm" onClick={onDelete}>
          刪除
        </button>
      </div>
      {expanded && details?.files && (
        <ul className="file-list">
          {details.files.map((f, i) => (
            <li key={i}>
              <span className="file-name">{f.components.join("/")}</span>
              <span className="file-size">{formatBytes(f.length)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
});
