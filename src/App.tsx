// src/App.tsx

import React, { useCallback, useEffect, useState } from "react";
import "./App.css";

import { getPref, setPref, PREF_KEYS, migrateLegacyPrefs } from './lib/uiPrefs';

import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { useUrlDrop } from './hooks/useUrlDrop';
import { useDownloadTasks } from './hooks/useDownloadTasks';
import { useToasts } from './hooks/useToasts';
import { useTorrentStats } from './hooks/useTorrentStats';
import { useHttpStats } from './hooks/useHttpStats';
import { Toolbar } from './components/Toolbar';
import { TaskListView } from './components/TaskListView';
import { SettingsDialog, type SettingsSection } from './components/SettingsDialog';
import { BtView } from './components/bt/BtView';
import { HttpView } from './components/http/HttpView';
import { JinView } from './components/jin/JinView';

type Tab = 'web' | 'bt' | 'http' | 'jin';
type Theme = 'light' | 'dark';

// 首次渲染前就決定主題,避免閃白;無記錄時跟隨系統
function initTheme(): Theme {
  const saved = localStorage.getItem(PREF_KEYS.theme) as Theme | null;
  const theme = saved ?? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  document.documentElement.dataset.theme = theme;
  return theme;
}

function App() {
  const { tasks, addTask, removeTask, removeAllTasks, volume, setVolume, playDing } = useTaskManager();
  // toast 佇列要先建立：剪貼簿/拖曳/BT/直鏈的通知全推同一條
  const { toasts, pushToast } = useToasts();
  const { monitorClipboard, setMonitorClipboard } = useClipboardMonitor(addTask, tasks, pushToast);
  const { isDragging, onDragEnter, onDragOver, onDragLeave, onDrop } = useUrlDrop(addTask, pushToast, playDing);
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

  // BT / 直鏈 stats 訂閱掛 App 層，切分頁不中斷；兩者共用同一條 toast 佇列
  const { stats: btStats } = useTorrentStats(pushToast, playDing);
  const { stats: httpStats } = useHttpStats(pushToast);

  const [tab, setTab] = useState<Tab>(() => (getPref(PREF_KEYS.activeTab, "web") as Tab));
  const switchTab = useCallback((t: Tab) => {
    setTab(t);
    setPref(PREF_KEYS.activeTab, t);
  }, []);

  const [theme, setTheme] = useState<Theme>(initTheme);
  const toggleTheme = useCallback(() => {
    setTheme(prev => {
      const next = prev === "light" ? "dark" : "light";
      document.documentElement.dataset.theme = next;
      setPref(PREF_KEYS.theme, next);
      return next;
    });
  }, []);

  // 統一設定 dialog：所有後端設定的唯一入口，各分頁只是開到對應分區
  const [settingsSection, setSettingsSection] = useState<SettingsSection | null>(null);
  // 存檔後遞增 → 各分頁重讀自己關心的設定
  const [settingsRev, setSettingsRev] = useState(0);

  // 舊版散在 localStorage 的後端設定一次性搬進 app_settings.json
  useEffect(() => {
    migrateLegacyPrefs().then(() => setSettingsRev(r => r + 1));
  }, []);

  const btActiveCount =
    (btStats?.torrents.filter(t => !t.finished).length ?? 0) +
    (btStats?.pending.length ?? 0);
  const httpActiveCount =
    httpStats?.tasks.filter(t => t.state !== "finished").length ?? 0;

  const doneCount = downloadTasks.filter(t => t.status === "done").length;
  const pendingCount = downloadTasks.filter(
    t => t.status === "idle" || t.status === "error" || t.status === "paused"
  ).length;

  const handleMonitorChange = useCallback(async (enabled: boolean) => {
    await setMonitorClipboard(enabled);
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
        <button
          type="button"
          className={`tab-btn ${tab === "jin" ? "active" : ""}`}
          onClick={() => switchTab("jin")}
        >
          遊戲設定
        </button>
        <div className="tab-bar-controls">
          <div className="checkbox-group">
            <input
              type="checkbox"
              id="monitorClipboard"
              checked={monitorClipboard}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => handleMonitorChange(e.target.checked)}
            />
            <label htmlFor="monitorClipboard">監控剪貼簿</label>
          </div>
          <button
            type="button"
            className="btn-sm"
            onClick={() => setSettingsSection("general")}
            title="設定"
          >
            ⚙ 設定
          </button>
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
        <BtView
          stats={btStats}
          settingsRev={settingsRev}
          onOpenSettings={() => setSettingsSection("bt")}
        />
      ) : tab === "http" ? (
        <HttpView stats={httpStats} />
      ) : (
        <JinView
          settingsRev={settingsRev}
          onOpenSettings={() => setSettingsSection("jin")}
        />
      )}
      {settingsSection && (
        <SettingsDialog
          section={settingsSection}
          onClose={() => setSettingsSection(null)}
          onSaved={() => setSettingsRev(r => r + 1)}
          volume={volume}
          onVolumeChange={setVolume}
          theme={theme}
          onToggleTheme={toggleTheme}
          monitorClipboard={monitorClipboard}
          onMonitorChange={handleMonitorChange}
        />
      )}
      <div className="toast-container">
        {toasts.map(t => (
          <div key={t.key} className="toast">
            {t.text}
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;
