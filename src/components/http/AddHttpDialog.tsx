import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { addHttpDownload } from "../../lib/httpApi";

interface Props {
  onClose: () => void;
  onAdded: (existingId: number | null) => void;
}

const DEFAULT_DIR_KEY = "httpDefaultDir";

export function AddHttpDialog({ onClose, onAdded }: Props) {
  const [link, setLink] = useState("");
  const [outDir, setOutDir] = useState(() => localStorage.getItem(DEFAULT_DIR_KEY) || "");
  const [saveAsDefault, setSaveAsDefault] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const isHttp = /^https?:\/\//i.test(link.trim());

  // 剪貼簿是 http(s) 連結就自動帶入
  useEffect(() => {
    navigator.clipboard
      ?.readText?.()
      .then((text) => {
        const t = text.trim();
        if (/^https?:\/\//i.test(t)) setLink((prev) => prev || t);
      })
      .catch(() => {});
  }, []);

  async function pickFolder() {
    const dir = await open({ directory: true, defaultPath: outDir || undefined });
    if (typeof dir === "string") setOutDir(dir);
  }

  async function submit() {
    setError(null);
    setBusy(true);
    try {
      const result = await addHttpDownload(link.trim(), outDir || undefined);
      // 勾選時記住目錄;留空 = 清除預設,回到系統下載資料夾
      if (saveAsDefault) {
        if (outDir.trim()) localStorage.setItem(DEFAULT_DIR_KEY, outDir.trim());
        else localStorage.removeItem(DEFAULT_DIR_KEY);
      }
      onAdded(result.already_exists ? result.id : null);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>新增直鏈下載</h2>
        <label>
          HTTP 下載連結
          <textarea
            rows={3}
            placeholder="https://..."
            value={link}
            onChange={(e) => setLink(e.target.value)}
            autoFocus
          />
        </label>
        <label>
          下載到(留空 = 系統下載資料夾)
          <div className="dir-picker">
            <input
              type="text"
              value={outDir}
              placeholder="預設:下載資料夾"
              onChange={(e) => setOutDir(e.target.value)}
            />
            <button type="button" onClick={pickFolder}>
              瀏覽…
            </button>
          </div>
        </label>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={saveAsDefault}
            onChange={(e) => setSaveAsDefault(e.target.checked)}
          />
          將此目錄設為預設下載目錄(留空 = 恢復系統下載資料夾)
        </label>
        {error && <p className="error-text">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn-primary" disabled={busy || !isHttp} onClick={submit}>
            {busy ? "加入中…" : "開始下載"}
          </button>
        </div>
      </div>
    </div>
  );
}
