import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { addHttpDownload } from "../../lib/httpApi";
import { getAppSettings, updateAppSettings } from "../../lib/settingsApi";

interface Props {
  onClose: () => void;
  onAdded: (existingId: number | null) => void;
}

export function AddHttpDialog({ onClose, onAdded }: Props) {
  const [link, setLink] = useState("");
  const [outDir, setOutDir] = useState("");
  const [saveAsDefault, setSaveAsDefault] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const isHttp = /^https?:\/\//i.test(link.trim());

  // 預設目錄自動帶入(app_settings.json,使用者已改過就不覆蓋)
  useEffect(() => {
    (async () => {
      try {
        let s = await getAppSettings();
        // 舊版 localStorage 值一次性遷移
        const legacy = localStorage.getItem("httpDefaultDir");
        if (legacy !== null) {
          localStorage.removeItem("httpDefaultDir");
          if (legacy && !s.http_default_dir) {
            s = await updateAppSettings((cur) => ({ ...cur, http_default_dir: legacy }));
          }
        }
        if (s.http_default_dir) setOutDir((prev) => prev || s.http_default_dir);
      } catch {}
    })();
  }, []);

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
      // 勾選時記住目錄;留空 = 清除預設,回到系統下載資料夾。存失敗不擋加入
      if (saveAsDefault) {
        try {
          await updateAppSettings((s) => ({ ...s, http_default_dir: outDir.trim() }));
        } catch {}
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
