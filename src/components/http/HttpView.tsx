// 直鏈下載分頁 — 共用工具列(新增、清除完成、總速度) + 任務清單 + 新增 dialog

import { useState } from "react";
import { deleteHttpDownload, type HttpStatsEvent } from "../../lib/httpApi";
import { formatSpeed } from "../../lib/format";
import { useAutoClearError } from "../../hooks/useAutoClearError";
import { ListToolbar } from "../common/ListToolbar";
import { HttpRow } from "./HttpRow";
import { AddHttpDialog } from "./AddHttpDialog";

interface Props {
  stats: HttpStatsEvent | null;
}

// 預設目錄由 AddHttpDialog 開啟時自己讀（每次開啟都是新 mount），不需要 settingsRev
export function HttpView({ stats }: Props) {
  const [showAdd, setShowAdd] = useState(false);
  const [actionError, setActionError] = useAutoClearError();

  const tasks = stats?.tasks ?? [];
  const finished = tasks.filter((t) => t.state === "finished");

  async function clearFinished() {
    try {
      await Promise.all(finished.map((t) => deleteHttpDownload(t.id, false)));
    } catch (e) {
      setActionError(String(e));
    }
  }

  return (
    <>
      <ListToolbar
        addLabel="＋ 新增直鏈"
        onAdd={() => setShowAdd(true)}
        finishedCount={finished.length}
        onClearFinished={clearFinished}
        summary={stats && <>↓ {formatSpeed(stats.total_down_bps)}</>}
      />

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
