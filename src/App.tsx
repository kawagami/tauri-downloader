// src/App.tsx

import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { useUrlDrop } from './hooks/useUrlDrop';
import { useDownloadTasks } from './hooks/useDownloadTasks';
import { Toolbar } from './components/Toolbar';
import { TaskListView } from './components/TaskListView';


function App() {
  const { tasks, addTask, removeTask, removeAllTasks, volume, setVolume } = useTaskManager();
  const { monitorClipboard, setMonitorClipboard } = useClipboardMonitor(addTask, tasks);
  const { isDragging, dropError, onDragEnter, onDragOver, onDragLeave, onDrop } = useUrlDrop(addTask);
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
      <Toolbar
        monitorClipboard={monitorClipboard}
        onMonitorChange={handleMonitorChange}
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
        dingVolume={volume}
        onDingVolumeChange={setVolume}
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
    </div>
  );
}

export default App;
