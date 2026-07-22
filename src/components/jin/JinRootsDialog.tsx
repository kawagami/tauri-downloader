// 工作需求遊戲設定 — 掃描根目錄設定（存 app_settings.json 的 jin_roots）

import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getAppSettings, updateAppSettings } from "../../lib/settingsApi";

interface Props {
  onClose: () => void;
  onSaved: () => void;
}

export function JinRootsDialog({ onClose, onSaved }: Props) {
  const [roots, setRoots] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getAppSettings()
      .then((s) => setRoots(s.jin_roots.length > 0 ? s.jin_roots : [""]))
      .catch((e) => setError(String(e)));
  }, []);

  function setAt(i: number, val: string) {
    setRoots((prev) => prev.map((r, idx) => (idx === i ? val : r)));
  }

  async function pickAt(i: number) {
    const dir = await open({ directory: true, defaultPath: roots[i] || undefined });
    if (typeof dir === "string") setAt(i, dir);
  }

  async function save() {
    setBusy(true);
    setError(null);
    try {
      const cleaned = roots.map((r) => r.trim()).filter(Boolean);
      await updateAppSettings((s) => ({ ...s, jin_roots: cleaned }));
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>掃描根目錄</h2>
        <p className="hint">每個根目錄會遞迴掃出底下所有 code.*.php；也可以直接填單一檔案路徑。</p>
        {roots.map((r, i) => (
          <label key={i}>
            根目錄 {i + 1}
            <div className="dir-picker">
              <input
                type="text"
                value={r}
                placeholder="\\wsl.localhost\..."
                onChange={(e) => setAt(i, e.target.value)}
              />
              <button type="button" onClick={() => pickAt(i)}>
                瀏覽…
              </button>
              <button
                type="button"
                onClick={() => setRoots((prev) => prev.filter((_, idx) => idx !== i))}
              >
                移除
              </button>
            </div>
          </label>
        ))}
        <button type="button" className="btn-sm" onClick={() => setRoots((prev) => [...prev, ""])}>
          ＋ 新增根目錄
        </button>
        {error && <p className="error-text">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn-primary" disabled={busy} onClick={save}>
            {busy ? "儲存中…" : "儲存"}
          </button>
        </div>
      </div>
    </div>
  );
}
