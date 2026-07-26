// 統一設定 dialog — 原本散在四處的設定（tab bar 音量、網站分頁頻寬、BT 設定、jin 根目錄）
// 全部收在這裡。分區只是分頁籤,存檔一次寫回 app_settings.json。
// 主題/音量是純 UI 偏好(localStorage),改了立即生效不進 draft;
// monitor_clipboard 也走既有的即時開關,存檔用 patch 合併避免蓋掉。

import { useEffect, useState } from "react";
import {
  bpsToKbps,
  getAppSettings,
  kbpsToBps,
  updateAppSettings,
  type AppSettings,
} from "../lib/settingsApi";
import { pickDir } from "../lib/pickDir";

export type SettingsSection = "general" | "web" | "bt" | "http" | "jin";

const SECTIONS: { id: SettingsSection; label: string }[] = [
  { id: "general", label: "一般" },
  { id: "web", label: "網站下載" },
  { id: "bt", label: "磁力下載" },
  { id: "http", label: "直鏈下載" },
  { id: "jin", label: "遊戲設定" },
];

interface Props {
  section?: SettingsSection;
  onClose: () => void;
  onSaved: (s: AppSettings) => void;
  volume: number;
  onVolumeChange: (v: number) => void;
  theme: "light" | "dark";
  onToggleTheme: () => void;
  monitorClipboard: boolean;
  onMonitorChange: (enabled: boolean) => void;
}

/** 目錄輸入 + 瀏覽按鈕（各分頁的預設目錄長得一樣） */
function DirField({
  value,
  placeholder,
  onChange,
  onRemove,
}: {
  value: string;
  placeholder?: string;
  onChange: (v: string) => void;
  onRemove?: () => void;
}) {
  return (
    <div className="dir-picker">
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
      <button
        type="button"
        onClick={async () => {
          const dir = await pickDir(value);
          if (dir) onChange(dir);
        }}
      >
        瀏覽…
      </button>
      {onRemove && (
        <button type="button" onClick={onRemove}>
          移除
        </button>
      )}
    </div>
  );
}

/** 限速輸入：UI 一律 KB/s，存檔換算 bytes/s */
function LimitField({
  label,
  bps,
  onChange,
}: {
  label: string;
  bps: number;
  onChange: (bps: number) => void;
}) {
  const kbps = bpsToKbps(bps);
  return (
    <label>
      {label}
      <div className="toolbar-field">
        <input
          type="number"
          min={0}
          step={1}
          value={kbps === 0 ? "" : kbps}
          placeholder="無限制"
          onChange={(e) => onChange(kbpsToBps(parseInt(e.target.value, 10)))}
          style={{ width: "110px" }}
        />
        <span>KB/s</span>
      </div>
    </label>
  );
}

export function SettingsDialog({
  section = "general",
  onClose,
  onSaved,
  volume,
  onVolumeChange,
  theme,
  onToggleTheme,
  monitorClipboard,
  onMonitorChange,
}: Props) {
  const [active, setActive] = useState<SettingsSection>(section);
  const [draft, setDraft] = useState<AppSettings | null>(null);
  const [roots, setRoots] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getAppSettings()
      .then((s) => {
        setDraft(s);
        setRoots(s.jin_roots.length > 0 ? s.jin_roots : [""]);
      })
      .catch((e) => setError(String(e)));
  }, []);

  async function save() {
    if (!draft) return;
    setBusy(true);
    setError(null);
    try {
      // patch 合併：monitor_clipboard 由 tab bar 即時開關維護，不被舊 draft 蓋掉
      const next = await updateAppSettings((cur) => ({
        ...cur,
        web: draft.web,
        http: draft.http,
        bt: draft.bt,
        jin_roots: roots.map((r) => r.trim()).filter(Boolean),
      }));
      onSaved(next);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal modal-wide" onClick={(e) => e.stopPropagation()}>
        <h2>設定</h2>
        <div className="settings-body">
          <nav className="settings-nav">
            {SECTIONS.map((s) => (
              <button
                key={s.id}
                type="button"
                className={`settings-nav-btn ${active === s.id ? "active" : ""}`}
                onClick={() => setActive(s.id)}
              >
                {s.label}
              </button>
            ))}
          </nav>

          <div className="settings-pane">
            {!draft ? (
              <p>載入中…</p>
            ) : active === "general" ? (
              <>
                <label className="checkbox-label">
                  <input
                    type="checkbox"
                    checked={monitorClipboard}
                    onChange={(e) => onMonitorChange(e.target.checked)}
                  />
                  監控剪貼簿（複製站台連結/magnet 自動加入）
                </label>
                <label>
                  通知音量
                  <div className="toolbar-field">
                    <input
                      type="range"
                      min="0"
                      max="3"
                      step="0.05"
                      value={volume}
                      onChange={(e) => onVolumeChange(Number(e.target.value))}
                      style={{ width: "160px" }}
                    />
                    <span>{Math.round(volume * 100)}%</span>
                  </div>
                </label>
                <label>
                  外觀
                  <div className="toolbar-field">
                    <button type="button" className="btn-sm" onClick={onToggleTheme}>
                      {theme === "light" ? "🌙 切換深色" : "☀️ 切換淺色"}
                    </button>
                  </div>
                </label>
                <p className="hint">音量與主題是本機偏好,不寫入設定檔;按「儲存」只影響其他分區。</p>
              </>
            ) : active === "web" ? (
              <>
                <label>
                  預設下載目錄（留空 = 系統下載資料夾）
                  <DirField
                    value={draft.web.default_dir}
                    placeholder="預設:下載資料夾"
                    onChange={(v) => setDraft({ ...draft, web: { ...draft.web, default_dir: v } })}
                  />
                </label>
                <LimitField
                  label="下載限速（留空 = 不限）"
                  bps={draft.web.limit_bps}
                  onChange={(bps) => setDraft({ ...draft, web: { ...draft.web, limit_bps: bps } })}
                />
                <p className="hint">
                  剪貼簿監控的「同名檔案已存在就略過」也是看這個目錄。限速即時生效。
                </p>
              </>
            ) : active === "bt" ? (
              <>
                <label>
                  預設下載目錄（留空 = 系統下載資料夾）
                  <DirField
                    value={draft.bt.default_dir}
                    placeholder="預設:下載資料夾"
                    onChange={(v) => setDraft({ ...draft, bt: { ...draft.bt, default_dir: v } })}
                  />
                </label>
                <label>
                  BT 監聽 port（留空 = 自動）
                  <input
                    type="number"
                    min={1}
                    max={65535}
                    value={draft.bt.listen_port ?? ""}
                    onChange={(e) => {
                      const n = parseInt(e.target.value, 10);
                      setDraft({
                        ...draft,
                        bt: {
                          ...draft.bt,
                          listen_port: Number.isFinite(n) && n > 0 ? n : null,
                        },
                      });
                    }}
                  />
                </label>
                <LimitField
                  label="下載限速（留空 = 不限）"
                  bps={draft.bt.download_limit_bps}
                  onChange={(bps) =>
                    setDraft({ ...draft, bt: { ...draft.bt, download_limit_bps: bps } })
                  }
                />
                <LimitField
                  label="上傳限速（留空 = 不限）"
                  bps={draft.bt.upload_limit_bps}
                  onChange={(bps) =>
                    setDraft({ ...draft, bt: { ...draft.bt, upload_limit_bps: bps } })
                  }
                />
                <p className="hint">port 與限速重啟 app 後生效；預設目錄即時生效。</p>
              </>
            ) : active === "http" ? (
              <>
                <label>
                  預設下載目錄（留空 = 系統下載資料夾）
                  <DirField
                    value={draft.http.default_dir}
                    placeholder="預設:下載資料夾"
                    onChange={(v) => setDraft({ ...draft, http: { ...draft.http, default_dir: v } })}
                  />
                </label>
                <LimitField
                  label="下載限速（留空 = 不限）"
                  bps={draft.http.limit_bps}
                  onChange={(bps) => setDraft({ ...draft, http: { ...draft.http, limit_bps: bps } })}
                />
                <p className="hint">限速是所有直鏈任務的總和（分段共用同一顆桶），即時生效。</p>
              </>
            ) : (
              <>
                <p className="hint">
                  每個根目錄會遞迴掃出底下所有 code.*.php；也可以直接填單一檔案路徑。
                </p>
                {roots.map((r, i) => (
                  <label key={i}>
                    根目錄 {i + 1}
                    <DirField
                      value={r}
                      placeholder="\\wsl.localhost\..."
                      onChange={(v) => setRoots((prev) => prev.map((x, idx) => (idx === i ? v : x)))}
                      onRemove={() => setRoots((prev) => prev.filter((_, idx) => idx !== i))}
                    />
                  </label>
                ))}
                <button
                  type="button"
                  className="btn-sm"
                  onClick={() => setRoots((prev) => [...prev, ""])}
                >
                  ＋ 新增根目錄
                </button>
              </>
            )}
          </div>
        </div>

        {error && <p className="error-text">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn-primary" disabled={busy || !draft} onClick={save}>
            {busy ? "儲存中…" : "儲存"}
          </button>
        </div>
      </div>
    </div>
  );
}
