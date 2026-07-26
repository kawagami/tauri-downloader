// 磁力/直鏈分頁共用工具列 — 新增按鈕 + 清除完成（含確認）+ 右側速度摘要

import type { ReactNode } from "react";

interface Props {
  addLabel: string;
  onAdd: () => void;
  addDisabled?: boolean;
  finishedCount: number;
  /** 確認後才呼叫；失敗訊息由呼叫端接 */
  onClearFinished: () => void | Promise<void>;
  extra?: ReactNode;
  summary?: ReactNode;
}

export function ListToolbar({
  addLabel,
  onAdd,
  addDisabled,
  finishedCount,
  onClearFinished,
  extra,
  summary,
}: Props) {
  function clearFinished() {
    if (finishedCount === 0) return;
    if (!window.confirm(`清除 ${finishedCount} 個已完成任務?已下載檔案會保留。`)) return;
    void onClearFinished();
  }

  return (
    <div className="sticky-toolbar bt-toolbar">
      <div className="toolbar-actions">
        <button type="button" className="btn-primary" disabled={addDisabled} onClick={onAdd}>
          {addLabel}
        </button>
        <button type="button" disabled={finishedCount === 0} onClick={clearFinished}>
          清除完成{finishedCount > 0 ? ` (${finishedCount})` : ""}
        </button>
        {extra}
      </div>
      <div className="toolbar-summary">{summary}</div>
    </div>
  );
}
