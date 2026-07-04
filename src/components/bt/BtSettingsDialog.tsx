import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getAppSettings, updateAppSettings, type BtSettings } from "../../lib/settingsApi";

interface Props {
  onClose: () => void;
  onSaved: (s: BtSettings) => void;
}

export function BtSettingsDialog({ onClose, onSaved }: Props) {
  const [settings, setSettings] = useState<BtSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAppSettings()
      .then((s) => setSettings(s.bt))
      .catch((e) => setError(String(e)));
  }, []);

  if (!settings) {
    return (
      <div className="modal-backdrop" onClick={onClose}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          {error ? <p className="error-text">{error}</p> : <p>載入中…</p>}
        </div>
      </div>
    );
  }

  async function pickFolder() {
    const dir = await open({ directory: true, defaultPath: settings!.default_download_dir });
    if (typeof dir === "string") setSettings({ ...settings!, default_download_dir: dir });
  }

  async function save() {
    setError(null);
    try {
      await updateAppSettings((s) => ({ ...s, bt: settings! }));
      onSaved(settings!);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  }

  const numField = (v: number | null) => (v === null ? "" : String(v));
  const parseNum = (s: string): number | null => {
    const n = parseInt(s, 10);
    return Number.isFinite(n) && n > 0 ? n : null;
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>BT 設定</h2>
        <label>
          預設下載目錄
          <div className="dir-picker">
            <input
              type="text"
              value={settings.default_download_dir}
              onChange={(e) => setSettings({ ...settings, default_download_dir: e.target.value })}
            />
            <button type="button" onClick={pickFolder}>
              瀏覽…
            </button>
          </div>
        </label>
        <label>
          BT 監聽 port(留空 = 自動)
          <input
            type="number"
            min={1}
            max={65535}
            value={numField(settings.listen_port)}
            onChange={(e) => setSettings({ ...settings, listen_port: parseNum(e.target.value) })}
          />
        </label>
        <label>
          下載限速 bytes/s(留空 = 不限)
          <input
            type="number"
            min={1}
            value={numField(settings.download_limit_bps)}
            onChange={(e) =>
              setSettings({ ...settings, download_limit_bps: parseNum(e.target.value) })
            }
          />
        </label>
        <label>
          上傳限速 bytes/s(留空 = 不限)
          <input
            type="number"
            min={1}
            value={numField(settings.upload_limit_bps)}
            onChange={(e) =>
              setSettings({ ...settings, upload_limit_bps: parseNum(e.target.value) })
            }
          />
        </label>
        <p className="hint">port 與限速重啟 app 後生效。剪貼簿偵測到 magnet 會自動下載到預設目錄。</p>
        {error && <p className="error-text">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn-primary" onClick={save}>
            儲存
          </button>
        </div>
      </div>
    </div>
  );
}
