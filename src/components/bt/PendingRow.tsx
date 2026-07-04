import { removePending, type PendingItem } from "../../lib/btApi";

interface Props {
  p: PendingItem;
}

export function PendingRow({ p }: Props) {
  return (
    <div className={`torrent-row ${p.error ? "has-error" : ""}`}>
      <div className="row-main">
        <div className="row-title">
          <span className="name">{p.name ?? "(無名稱 magnet)"}</span>
          <span className={`status-badge ${p.error ? "status-error" : "status-live"}`}>
            {p.error ? "加入失敗" : "抓取 metadata"}
          </span>
        </div>
        <div className="row-stats">
          {p.error ? (
            <span className="row-error">{p.error}</span>
          ) : (
            <span>已等待 {p.elapsed_s}s — 冷門種子可能要很久,可取消</span>
          )}
        </div>
      </div>
      <div className="row-actions">
        <button type="button" className="btn-danger btn-sm" onClick={() => removePending(p.key)}>
          {p.error ? "移除" : "取消"}
        </button>
      </div>
    </div>
  );
}
