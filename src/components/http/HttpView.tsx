// 直鏈下載分頁 — 工具列(新增、清除完成、總速度) + 任務清單 + 新增 dialog

import { useEffect, useState } from "react";
import { deleteHttpDownload, type HttpStatsEvent } from "../../lib/httpApi";
import { formatSpeed } from "../../lib/format";
import { HttpRow } from "./HttpRow";
import { AddHttpDialog } from "./AddHttpDialog";

interface Props {
  stats: HttpStatsEvent | null;
}

export function HttpView({ stats }: Props) {
  const [showAdd, setShowAdd] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    if (!actionError) return;
    const t = setTimeout(() => setActionError(null), 6000);
    return () => clearTimeout(t);
  }, [actionError]);

  const tasks = stats?.tasks ?? [];
  const finished = tasks.filter((t) => t.state === "finished");

  async function clearFinished() {
    if (finished.length === 0) return;
    if (!window.confirm(`清除 ${finished.length} 個已完成任務?已下載檔案會保留。`)) return;
    try {
      await Promise.all(finished.map((t) => deleteHttpDownload(t.id, false)));
    } catch (e) {
      setActionError(String(e));
    }
  }

  return (
    <>
      <div className="sticky-toolbar bt-toolbar">
        <div className="toolbar-actions">
          <button type="button" className="btn-primary" onClick={() => setShowAdd(true)}>
            ＋ 新增直鏈
          </button>
          <button type="button" disabled={finished.length === 0} onClick={clearFinished}>
            清除完成{finished.length > 0 ? ` (${finished.length})` : ""}
          </button>
        </div>
        <div className="toolbar-summary">
          {stats && <>↓ {formatSpeed(stats.total_down_bps)}</>}
        </div>
      </div>

      <main className="main-content">
        {actionError && <div className="bt-banner-error">{actionError}</div>}

        {tasks.length === 0 ? (
          <div className="empty-hint">
            尚無直鏈任務。點「＋ 新增直鏈」貼上 HTTP 下載連結。支援分段下載與斷點續傳,連結過期可換新連結接續。
          </div>
        ) : (
          <div className="torrent-list">
            {tasks.map((t) => (
              <HttpRow key={t.id} t={t} onActionError={setActionError} />
            ))}
          </div>
        )}
      </main>

      {showAdd && <AddHttpDialog onClose={() => setShowAdd(false)} onAdded={() => {}} />}
    </>
  );
}
