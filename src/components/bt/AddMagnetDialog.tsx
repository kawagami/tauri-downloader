import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { addMagnet, getBtSettings, saveBtSettings } from "../../lib/btApi";

interface Props {
  defaultDir: string;
  onClose: () => void;
  onAdded: (existingId: number | null) => void;
  onDefaultDirSaved: (dir: string) => void;
}

export function AddMagnetDialog({ defaultDir, onClose, onAdded, onDefaultDirSaved }: Props) {
  const [link, setLink] = useState("");
  const [outDir, setOutDir] = useState(defaultDir);
  const [startNow, setStartNow] = useState(true);
  const [saveAsDefault, setSaveAsDefault] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const isMagnet = link.trim().startsWith("magnet:");

  // 剪貼簿是 magnet 連結就自動帶入
  useEffect(() => {
    navigator.clipboard
      ?.readText?.()
      .then((text) => {
        const t = text.trim();
        if (t.startsWith("magnet:")) setLink((prev) => prev || t);
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
      const result = await addMagnet(link, outDir || undefined, !startNow);
      // 存預設目錄失敗不擋加入
      if (saveAsDefault && outDir && outDir !== defaultDir) {
        try {
          const s = await getBtSettings();
          await saveBtSettings({ ...s, default_download_dir: outDir });
          onDefaultDirSaved(outDir);
        } catch {}
      }
      onAdded(result.already_exists ? (result.id ?? null) : null);
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
        <h2>新增磁力任務</h2>
        <label>
          磁力連結
          <textarea
            rows={3}
            placeholder="magnet:?xt=urn:btih:..."
            value={link}
            onChange={(e) => setLink(e.target.value)}
            autoFocus
          />
        </label>
        <label>
          下載到
          <div className="dir-picker">
            <input type="text" value={outDir} onChange={(e) => setOutDir(e.target.value)} />
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
          將此目錄設為預設下載目錄
        </label>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={startNow}
            onChange={(e) => setStartNow(e.target.checked)}
          />
          立即開始下載(取消勾選 = 只加入清單,之後手動「恢復」開始)
        </label>
        {error && <p className="error-text">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="btn-primary"
            disabled={busy || !isMagnet}
            onClick={submit}
          >
            {busy ? "加入中…" : startNow ? "開始下載" : "加入清單"}
          </button>
        </div>
      </div>
    </div>
  );
}
