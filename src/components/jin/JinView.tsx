// 工作需求遊戲設定分頁 — 輸入 HALL/後綴 → 預覽每個 code.*.php 會怎麼改 → 確認後套用
// 對應原本的 CLI：tools/src/bin/jin_game_add.rs

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ACTION_LABEL,
  jinApply,
  jinPreview,
  listenJinProgress,
  parseSuffixes,
  type JinAction,
  type JinFilePlan,
  type JinPreview,
  type JinProgress,
} from "../../lib/jinApi";
import { JinRootsDialog } from "./JinRootsDialog";

const ACTION_CLASS: Record<JinAction, string> = {
  add: "status-done",
  add_commented: "status-paused",
  uncomment: "status-done",
  skip: "status-idle",
  error: "status-error",
};

export function JinView() {
  const [hall, setHall] = useState("");
  const [suffixRaw, setSuffixRaw] = useState("");
  const [commentEnvs, setCommentEnvs] = useState<string[]>([]);
  const [preview, setPreview] = useState<JinPreview | null>(null);
  const [results, setResults] = useState<JinFilePlan[] | null>(null);
  const [busy, setBusy] = useState<null | "preview" | "apply">(null);
  const [error, setError] = useState<string | null>(null);
  const [showRoots, setShowRoots] = useState(false);
  const [progress, setProgress] = useState<JinProgress | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const startedAt = useRef(0);

  // 後端逐檔推進度(掃 WSL UNC 可能好幾秒,要讓使用者知道還在跑)
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    listenJinProgress((p) => {
      setProgress(p.phase === "done" ? null : p);
    }).then((f) => {
      if (alive) unlisten = f;
      else f();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  // 執行中每 0.1 秒更新經過時間,慢也看得出來沒卡死
  useEffect(() => {
    if (!busy) {
      setElapsed(0);
      return;
    }
    startedAt.current = Date.now();
    const id = setInterval(() => setElapsed((Date.now() - startedAt.current) / 1000), 100);
    return () => clearInterval(id);
  }, [busy]);

  const suffixes = useMemo(() => parseSuffixes(suffixRaw), [suffixRaw]);
  const canPreview = hall.trim().length > 0 && suffixes.length > 0;

  // 掃到的環境（k8s overlays 那層或 code.<env>.php 檔名）— 勾選代表該環境寫成註解狀態
  const envs = useMemo(() => {
    const set = new Set((preview?.files ?? []).map((f) => f.env).filter(Boolean));
    return [...set].sort();
  }, [preview]);

  const rows = results ?? preview?.files ?? [];
  const counts = useMemo(() => {
    const c: Record<string, number> = {};
    for (const r of rows) c[r.action] = (c[r.action] ?? 0) + 1;
    return c;
  }, [rows]);
  const changeCount = (counts.add ?? 0) + (counts.add_commented ?? 0) + (counts.uncomment ?? 0);

  const run = useCallback(
    async (envsOverride?: string[]) => {
      setBusy("preview");
      setError(null);
      try {
        const p = await jinPreview({
          roots: [],
          hall: hall.trim(),
          suffixes,
          commentEnvs: envsOverride ?? commentEnvs,
        });
        setPreview(p);
        setResults(null);
      } catch (e) {
        setError(String(e));
        setPreview(null);
        setResults(null);
      } finally {
        setBusy(null);
        setProgress(null);
      }
    },
    [hall, suffixes, commentEnvs],
  );

  const toggleEnv = useCallback(
    (env: string) => {
      const next = commentEnvs.includes(env)
        ? commentEnvs.filter((e) => e !== env)
        : [...commentEnvs, env];
      setCommentEnvs(next);
      if (preview) void run(next);
    },
    [commentEnvs, preview, run],
  );

  async function apply() {
    if (!preview || changeCount === 0) return;
    if (!window.confirm(`將改寫 ${changeCount} 個檔案（不備份，改壞請用 git 還原）。確定套用？`)) return;
    setBusy("apply");
    setError(null);
    try {
      const r = await jinApply({
        roots: [],
        hall: preview.hall,
        suffixes,
        commentEnvs,
      });
      setResults(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
      setProgress(null);
    }
  }

  return (
    <>
      <div className="sticky-toolbar bt-toolbar">
        <div className="toolbar-actions">
          <div className="toolbar-field">
            <span>HALL</span>
            <input
              type="text"
              value={hall}
              placeholder="NEWGAME"
              disabled={busy !== null}
              onChange={(e) => setHall(e.target.value)}
              style={{ width: "130px", textTransform: "uppercase" }}
            />
          </div>
          <div className="toolbar-field">
            <span>代碼後綴</span>
            <input
              type="text"
              value={suffixRaw}
              placeholder="E F C"
              disabled={busy !== null}
              onChange={(e) => setSuffixRaw(e.target.value)}
              style={{ width: "160px", textTransform: "uppercase" }}
            />
          </div>
          <button
            type="button"
            className="btn-primary"
            disabled={!canPreview || busy !== null}
            onClick={() => run()}
          >
            {busy === "preview" ? "掃描中…" : "預覽"}
          </button>
          <button
            type="button"
            className="btn-danger"
            disabled={busy !== null || !preview || changeCount === 0 || results !== null}
            onClick={apply}
          >
            {busy === "apply" ? "套用中…" : `套用${changeCount > 0 ? ` (${changeCount})` : ""}`}
          </button>
          <button type="button" disabled={busy !== null} onClick={() => setShowRoots(true)}>
            根目錄設定
          </button>
        </div>
        <div className="toolbar-summary">
          {busy && <span className="status-live">執行中 {elapsed.toFixed(1)}s</span>}
          {!busy && preview && (
            <>
              {preview.codes.join("、")}｜共 {rows.length} 檔
              {changeCount > 0 && <>｜異動 {changeCount}</>}
              {counts.skip ? <>｜略過 {counts.skip}</> : null}
              {counts.error ? <>｜錯誤 {counts.error}</> : null}
            </>
          )}
        </div>
      </div>

      <main className="main-content">
        {error && <div className="bt-banner-error">{error}</div>}

        {busy && (
          <div className="jin-progress">
            <div className="jin-progress-head">
              <strong>
                {busy === "apply"
                  ? progress?.phase === "write"
                    ? "寫入檔案中…"
                    : "重新掃描中…"
                  : "掃描檔案中…"}
              </strong>
              <span className="meta-text">
                {progress && progress.total > 0
                  ? `${progress.done} / ${progress.total}`
                  : "準備中"}
                ｜{elapsed.toFixed(1)}s
              </span>
            </div>
            {progress && progress.total > 0 ? (
              <div className="progress-track">
                <div
                  className="progress-fill"
                  style={{ width: `${Math.round((progress.done / progress.total) * 100)}%` }}
                />
              </div>
            ) : (
              <div className="progress-indeterminate" />
            )}
            <div className="meta-text jin-progress-path">{progress?.path ?? ""}</div>
          </div>
        )}

        {envs.length > 0 && (
          <div className="sticky-toolbar" style={{ top: "auto", flexWrap: "wrap" }}>
            <span style={{ fontSize: "13px", color: "var(--text-muted)" }}>
              寫成註解狀態的環境：
            </span>
            {envs.map((env) => (
              <div className="checkbox-group" key={env}>
                <input
                  type="checkbox"
                  id={`jin-env-${env}`}
                  checked={commentEnvs.includes(env)}
                  disabled={busy !== null}
                  onChange={() => toggleEnv(env)}
                />
                <label htmlFor={`jin-env-${env}`}>{env}</label>
              </div>
            ))}
          </div>
        )}

        {rows.length === 0 ? (
          <div className="empty-hint">
            輸入 HALL 與代碼後綴（空白或逗號分隔，例：E F C）後按「預覽」，會掃描根目錄下所有
            code.*.php，列出每個檔案會做的動作。確認無誤再按「套用」寫檔（不會備份）。
          </div>
        ) : (
          <div className={`task-list-container ${busy ? "is-busy" : ""}`}>
            <table className="task-table">
              <thead>
                <tr>
                  <th style={{ width: "120px" }}>動作</th>
                  <th style={{ width: "110px" }}>環境</th>
                  <th>檔案</th>
                  <th style={{ width: "180px" }}>說明</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((f) => (
                  <tr key={f.path}>
                    <td>
                      <span className={`status-badge ${ACTION_CLASS[f.action]}`}>
                        {results !== null && f.applied ? "✓ " : ""}
                        {ACTION_LABEL[f.action]}
                      </span>
                    </td>
                    <td className="meta-text">{f.env || "-"}</td>
                    <td className="meta-text" style={{ wordBreak: "break-all" }}>
                      {f.path}
                    </td>
                    <td className="meta-text">{f.message}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </main>

      {showRoots && (
        <JinRootsDialog
          onClose={() => setShowRoots(false)}
          onSaved={() => {
            setShowRoots(false);
            if (preview) void run();
          }}
        />
      )}
    </>
  );
}
