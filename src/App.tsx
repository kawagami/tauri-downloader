// src/App.tsx

import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { useDownloadTasks } from './hooks/useDownloadTasks';
import { Toolbar } from './components/Toolbar';
import { TaskListView } from './components/TaskListView';


function App() {
  const { tasks, addTask, removeTask, removeAllTasks, reloadTasks } = useTaskManager();
  const { monitorClipboard, setMonitorClipboard } = useClipboardMonitor(addTask, tasks);
  const {
    tasks: downloadTasks,
    handleDownload,
    handleClearDone,
    handleDownloadAllSequentially,
    stopBatchDownload,
    isBatchDownloading,
    batchProgress,
  } = useDownloadTasks(tasks, removeTask);

  const [bandwidthKbps, setBandwidthKbps] = useState<number>(() =>
    Number(localStorage.getItem("bandwidthKbps") || "0")
  );

  useEffect(() => {
    invoke("set_bandwidth_limit", { bytesPerSec: bandwidthKbps * 1024 });
  }, []);

  const handleBandwidthChange = useCallback(async (kbps: number) => {
    setBandwidthKbps(kbps);
    localStorage.setItem("bandwidthKbps", String(kbps));
    await invoke("set_bandwidth_limit", { bytesPerSec: kbps * 1024 });
  }, []);

  const handleMonitorChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const enabled = e.target.checked;
    setMonitorClipboard(enabled);
    await invoke("set_monitor_paused", { paused: !enabled });
    if (enabled) {
      await reloadTasks();
    }
  }, [setMonitorClipboard, reloadTasks]);

  return (
    <div className="container">
      <Toolbar
        monitorClipboard={monitorClipboard}
        onMonitorChange={handleMonitorChange}
        onRemoveAll={removeAllTasks}
        onClearDone={handleClearDone}
        onDownloadAll={handleDownloadAllSequentially}
        onStopDownload={stopBatchDownload}
        isBatchDownloading={isBatchDownloading}
        batchProgress={batchProgress}
        tasksEmpty={downloadTasks.length === 0}
        hasDoneTasks={downloadTasks.some(t => t.status === "done")}
        bandwidthKbps={bandwidthKbps}
        onBandwidthChange={handleBandwidthChange}
      />
      <main className="main-content">
        <TaskListView
          tasks={downloadTasks}
          onRemoveTask={removeTask}
          onDownload={handleDownload}
        />
      </main>
    </div>
  );
}

export default App;
