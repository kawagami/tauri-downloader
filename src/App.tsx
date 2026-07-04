// src/App.tsx

import React, { useCallback, useEffect, useState } from "react";
import "./App.css";

import { getAppSettings, updateAppSettings } from './lib/settingsApi';

import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { useUrlDrop } from './hooks/useUrlDrop';
import { useDownloadTasks } from './hooks/useDownloadTasks';
import { useTorrentStats } from './hooks/useTorrentStats';
import { useHttpStats } from './hooks/useHttpStats';
import { Toolbar } from './components/Toolbar';
import { TaskListView } from './components/TaskListView';
import { BtView } from './components/bt/BtView';
import { HttpView } from './components/http/HttpView';

type Tab = 'web' | 'bt' | 'http';
type Theme = 'light' | 'dark';

// 首次渲染前就決定主題,避免閃白;無記錄時跟隨系統
function initTheme(): Theme {
  const saved = localStorage.getItem("theme") as Theme | null;
  const theme = saved ?? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  document.documentElement.dataset.theme = theme;
  return theme;
}

function App() {
  const { tasks, addTask, removeTask, removeAllTasks, volume, setVolume, playDing } = useTaskManager();
  const { monitorClipboard, setMonitorClipboard } = useClipboardMonitor(addTask, tasks);
  const { isDragging, dropError, onDragEnter, onDragOver, onDragLeave, onDrop } = useUrlDrop(addTask, playDing);
  const {
    tasks: downloadTasks,
    handleDownload,
    handleClearDone,
    handleDownloadAllSequentially,
    stopBatchDownload,
    isBatchDownloading,
    batchProgress,
    reorderTasks,
  } = useDownloadTasks(tasks, removeTask);

  // BT / 直鏈 stats 訂閱掛 App 層，切分頁不中斷;剪貼簿 magnet 加入時播 ding
  const { stats: btStats, toasts: btToasts } = useTorrentStats(playDing);
  const { stats: httpStats, toasts: httpToasts } = useHttpStats();

  const [tab, setTab] = useState<Tab>(() =>
    (localStorage.getItem("activeTab") as Tab) || "web"
  );
  const switchTab = useCallback((t: Tab) => {
    setTab(t);
    localStorage.setItem("activeTab", t);
  }, []);

  const [theme, setTheme] = useState<Theme>(initTheme);
  const toggleTheme = useCallback(() => {
    setTheme(prev => {
      const next = prev === "light" ? "dark" : "light";
      document.documentElement.dataset.theme = next;
      localStorage.setItem("theme", next);
      return next;
    });
  }, []);

  const btActiveCount =
    (btStats?.torrents.filter(t => !t.finished).length ?? 0) +
    (btStats?.pending.length ?? 0);
  const httpActiveCount =
    httpStats?.tasks.filter(t => t.state !== "finished").length ?? 0;

  // 頻寬限制持久化在 app_settings.json;後端啟動已自行套用,mount 只同步 UI
  const [bandwidthKbps, setBandwidthKbps] = useState<number>(0);
  useEffect(() => {
    (async () => {
      try {
        let s = await getAppSettings();
        // 舊版 localStorage 值一次性遷移
        const legacy = localStorage.getItem("bandwidthKbps");
        if (legacy !== null) {
          localStorage.removeItem("bandwidthKbps");
          const kbps = Number(legacy) || 0;
          if (kbps > 0 && s.bandwidth_limit_kbps === 0) {
            s = await updateAppSettings(cur => ({ ...cur, bandwidth_limit_kbps: kbps }));
          }
        }
        setBandwidthKbps(s.bandwidth_limit_kbps);
      } catch {}
    })();
  }, []);

  const doneCount = downloadTasks.filter(t => t.status === "done").length;
  const pendingCount = downloadTasks.filter(
    t => t.status === "idle" || t.status === "error" || t.status === "paused"
  ).length;

  const handleBandwidthChange = useCallback(async (kbps: number) => {
    setBandwidthKbps(kbps);
    // save_app_settings 會即時套用頻寬限制
    await updateAppSettings(s => ({ ...s, bandwidth_limit_kbps: kbps }));
  }, []);

  const handleMonitorChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    await setMonitorClipboard(e.target.checked);
  }, [setMonitorClipboard]);

  return (
    <div
      className="container"
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      {isDragging && (
        <div className="drop-overlay">
          <div className="drop-overlay-box">拖入連結即可新增任務</div>
        </div>
      )}
      {dropError && <div className="drop-error">{dropError}</div>}
      <nav className="tab-bar">
        <button
          type="button"
          className={`tab-btn ${tab === "web" ? "active" : ""}`}
          onClick={() => switchTab("web")}
        >
          網站下載
        </button>
        <button
          type="button"
          className={`tab-btn ${tab === "bt" ? "active" : ""}`}
          onClick={() => switchTab("bt")}
        >
          磁力下載
          {btActiveCount > 0 && <span className="tab-badge">{btActiveCount}</span>}
        </button>
        <button
          type="button"
          className={`tab-btn ${tab === "http" ? "active" : ""}`}
          onClick={() => switchTab("http")}
        >
          直鏈下載
          {httpActiveCount > 0 && <span className="tab-badge">{httpActiveCount}</span>}
        </button>
        <div className="tab-bar-controls">
          <div className="checkbox-group">
            <input
              type="checkbox"
              id="monitorClipboard"
              checked={monitorClipboard}
              onChange={handleMonitorChange}
            />
            <label htmlFor="monitorClipboard">監控剪貼簿</label>
          </div>
          <div className="toolbar-field">
            <span>通知音量</span>
            <input
              type="range"
              min="0"
              max="3"
              step="0.05"
              value={volume}
              onChange={e => setVolume(Number(e.target.value))}
              style={{ width: "80px" }}
            />
            <span style={{ width: "28px" }}>{Math.round(volume * 100)}%</span>
          </div>
          <button
            type="button"
            className="btn-sm theme-toggle"
            onClick={toggleTheme}
            title={theme === "light" ? "切換深色模式" : "切換淺色模式"}
          >
            {theme === "light" ? "🌙" : "☀️"}
          </button>
        </div>
      </nav>
      {tab === "web" ? (
        <>
          <Toolbar
            onRemoveAll={removeAllTasks}
            onClearDone={handleClearDone}
            onDownloadAll={handleDownloadAllSequentially}
            onStopDownload={stopBatchDownload}
            isBatchDownloading={isBatchDownloading}
            batchProgress={batchProgress}
            totalCount={downloadTasks.length}
            doneCount={doneCount}
            pendingCount={pendingCount}
            hasDownloadable={pendingCount > 0}
            hasDoneTasks={downloadTasks.some(t => t.status === "done")}
            bandwidthKbps={bandwidthKbps}
            onBandwidthChange={handleBandwidthChange}
          />
          <main className="main-content">
            <TaskListView
              tasks={downloadTasks}
              onRemoveTask={removeTask}
              onDownload={handleDownload}
              onReorder={reorderTasks}
              isBatchDownloading={isBatchDownloading}
            />
          </main>
        </>
      ) : tab === "bt" ? (
        <BtView stats={btStats} />
      ) : (
        <HttpView stats={httpStats} />
      )}
      <div className="toast-container">
        {[...btToasts, ...httpToasts].map(t => (
          <div key={t.key} className="toast">
            {t.text}
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;
