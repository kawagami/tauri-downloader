// BT 磁力下載分頁 — 工具列(速度/新增/清除完成/設定) + 任務清單 + dialogs

import { useEffect, useState } from "react";
import { deleteTorrent, getBtSettings, type TorrentStatsEvent } from "../../lib/btApi";
import { formatSpeed } from "../../lib/format";
import { TorrentRow } from "./TorrentRow";
import { PendingRow } from "./PendingRow";
import { AddMagnetDialog } from "./AddMagnetDialog";
import { BtSettingsDialog } from "./BtSettingsDialog";

interface Props {
  stats: TorrentStatsEvent | null;
}

export function BtView({ stats }: Props) {
  const [showAdd, setShowAdd] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [defaultDir, setDefaultDir] = useState("");
  const [highlightId, setHighlightId] = useState<number | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    getBtSettings()
      .then((s) => setDefaultDir(s.default_download_dir))
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!actionError) return;
    const t = setTimeout(() => setActionError(null), 6000);
    return () => clearTimeout(t);
  }, [actionError]);

  // 重複加入 → highlight 既有任務並捲過去
  function onAdded(existingId: number | null) {
    if (existingId !== null) {
      setHighlightId(existingId);
      setTimeout(() => {
        document
          .getElementById(`torrent-row-${existingId}`)
          ?.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 50);
      setTimeout(() => setHighlightId(null), 3000);
    }
  }

  const torrents = stats?.torrents ?? [];
  const pending = stats?.pending ?? [];
  const finished = torrents.filter((t) => t.finished);

  async function clearFinished() {
    if (finished.length === 0) return;
    if (!window.confirm(`清除 ${finished.length} 個已完成任務?已下載檔案會保留。`)) return;
    try {
      await Promise.all(finished.map((t) => deleteTorrent(t.id, false)));
    } catch (e) {
      setActionError(String(e));
    }
  }

  return (
    <>
      <div className="sticky-toolbar bt-toolbar">
        <div className="toolbar-actions">
          <button type="button" className="btn-primary" onClick={() => setShowAdd(true)}>
            ＋ 新增磁力
          </button>
          <button type="button" disabled={finished.length === 0} onClick={clearFinished}>
            清除完成{finished.length > 0 ? ` (${finished.length})` : ""}
          </button>
          <button type="button" onClick={() => setShowSettings(true)}>
            BT 設定
          </button>
        </div>
        <div className="toolbar-summary">
          {stats && (
            <>
              ↓ {formatSpeed(stats.session.total_down_bps)}　↑ {formatSpeed(stats.session.total_up_bps)}
            </>
          )}
        </div>
      </div>

      <main className="main-content">
        {actionError && <div className="bt-banner-error">{actionError}</div>}

        {torrents.length === 0 && pending.length === 0 ? (
          <div className="empty-hint">
            尚無磁力任務。點「＋ 新增磁力」貼上 magnet 連結,或直接複製 magnet 連結(剪貼簿監控開啟時自動加入)。
          </div>
        ) : (
          <div className="torrent-list">
            {pending.map((p) => (
              <PendingRow key={`pending-${p.key}`} p={p} />
            ))}
            {torrents.map((t) => (
              <TorrentRow
                key={t.id}
                t={t}
                highlighted={highlightId === t.id}
                onActionError={setActionError}
              />
            ))}
          </div>
        )}
      </main>

      {showAdd && (
        <AddMagnetDialog
          defaultDir={defaultDir}
          onClose={() => setShowAdd(false)}
          onAdded={onAdded}
        />
      )}
      {showSettings && (
        <BtSettingsDialog
          onClose={() => setShowSettings(false)}
          onSaved={(s) => setDefaultDir(s.default_download_dir)}
        />
      )}
    </>
  );
}
