// BT 磁力下載分頁 — 共用工具列(新增/清除完成/速度) + 任務清單 + 新增 dialog
// BT 設定已併入統一設定 dialog（App 層），這裡只留一顆按鈕開到 BT 分區

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  deleteTorrent,
  getBtEngineStatus,
  retryBtInit,
  type BtEngineStatus,
  type TorrentStatsEvent,
} from "../../lib/btApi";
import { getAppSettings } from "../../lib/settingsApi";
import { formatSpeed } from "../../lib/format";
import { useAutoClearError } from "../../hooks/useAutoClearError";
import { ListToolbar } from "../common/ListToolbar";
import { TorrentRow } from "./TorrentRow";
import { PendingRow } from "./PendingRow";
import { AddMagnetDialog } from "./AddMagnetDialog";

interface Props {
  stats: TorrentStatsEvent | null;
  /** 設定存檔後遞增,用來重讀預設目錄 */
  settingsRev: number;
  onOpenSettings: () => void;
}

export function BtView({ stats, settingsRev, onOpenSettings }: Props) {
  const [showAdd, setShowAdd] = useState(false);
  const [defaultDir, setDefaultDir] = useState("");
  const [highlightId, setHighlightId] = useState<number | null>(null);
  const [actionError, setActionError] = useAutoClearError();
  const [engine, setEngine] = useState<BtEngineStatus | null>(null);

  // 引擎狀態:掛載時查一次 + 訂閱背景 init 結果
  useEffect(() => {
    getBtEngineStatus().then(setEngine).catch(() => {});
    let cancelled = false;
    const unlisten = listen<BtEngineStatus>("bt-engine-status", (e) => {
      if (!cancelled) setEngine(e.payload);
    });
    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  }, []);

  // 設定已獨立於 BT 引擎(SettingsState),不用等引擎就緒
  useEffect(() => {
    getAppSettings()
      .then((s) => setDefaultDir(s.bt.default_dir))
      .catch(() => {});
  }, [settingsRev]);

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
    try {
      await Promise.all(finished.map((t) => deleteTorrent(t.id, false)));
    } catch (e) {
      setActionError(String(e));
    }
  }

  return (
    <>
      <ListToolbar
        addLabel="＋ 新增磁力"
        onAdd={() => setShowAdd(true)}
        addDisabled={!engine?.ready}
        finishedCount={finished.length}
        onClearFinished={clearFinished}
        extra={
          <button type="button" onClick={onOpenSettings}>
            BT 設定
          </button>
        }
        summary={
          stats && (
            <>
              ↓ {formatSpeed(stats.session.total_down_bps)}　↑{" "}
              {formatSpeed(stats.session.total_up_bps)}
            </>
          )
        }
      />

      <main className="main-content">
        {actionError && <div className="bt-banner-error">{actionError}</div>}

        {engine && !engine.ready && engine.error && (
          <div className="bt-banner-error">
            BT 引擎啟動失敗:{engine.error}
            <div style={{ marginTop: 8 }}>
              常見原因:其他 BT 程式開著搶 BT port(關掉後重試);
              或 Windows 保留了 DHT 用的 UDP port(Hyper-V/WSL,os error 10013)——
              後者 app 已會自動換 port 重試,連續失敗可執行
              <code> netsh interface ipv4 show excludedportrange protocol=udp </code>
              確認保留範圍。
              <button
                type="button"
                className="btn-sm"
                style={{ marginLeft: 8 }}
                onClick={() => retryBtInit().catch(() => {})}
              >
                重試
              </button>
            </div>
          </div>
        )}
        {engine && !engine.ready && !engine.error && (
          <div className="empty-hint">BT 引擎啟動中…</div>
        )}

        {engine?.ready && torrents.length === 0 && pending.length === 0 ? (
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
          onDefaultDirSaved={setDefaultDir}
        />
      )}
    </>
  );
}
