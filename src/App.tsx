// src/App.tsx

import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { useUrlDrop } from './hooks/useUrlDrop';
import { useDownloadTasks } from './hooks/useDownloadTasks';
import { useTorrentStats } from './hooks/useTorrentStats';
import { Toolbar } from './components/Toolbar';
import { TaskListView } from './components/TaskListView';
import { BtView } from './components/bt/BtView';

type Tab = 'web' | 'bt';

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

  // BT stats 訂閱掛 App 層，切分頁不中斷;剪貼簿 magnet 加入時播 ding
  const { stats: btStats, toasts: btToasts } = useTorrentStats(playDing);

  const [tab, setTab] = useState<Tab>(() =>
    (localStorage.getItem("activeTab") as Tab) || "web"
  );
  const switchTab = useCallback((t: Tab) => {
    setTab(t);
    localStorage.setItem("activeTab", t);
  }, []);

  const btActiveCount =
    (btStats?.torrents.filter(t => !t.finished).length ?? 0) +
    (btStats?.pending.length ?? 0);

  const [bandwidthKbps, setBandwidthKbps] = useState<number>(() =>
    Number(localStorage.getItem("bandwidthKbps") || "0")
  );

  const doneCount = downloadTasks.filter(t => t.status === "done").length;
  const pendingCount = downloadTasks.filter(
    t => t.status === "idle" || t.status === "error" || t.status === "paused"
  ).length;

  useEffect(() => {
    invoke("set_bandwidth_limit", { bytesPerSec: bandwidthKbps * 1024 });
  }, []);

  const handleBandwidthChange = useCallback(async (kbps: number) => {
    setBandwidthKbps(kbps);
    localStorage.setItem("bandwidthKbps", String(kbps));
    await invoke("set_bandwidth_limit", { bytesPerSec: kbps * 1024 });
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
      ) : (
        <BtView stats={btStats} />
      )}
      <div className="toast-container">
        {btToasts.map(t => (
          <div key={t.key} className="toast">
            {t.text}
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;
