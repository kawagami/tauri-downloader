// src/App.tsx

import React, { useCallback } from "react";
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
    handleDownloadAllSequentially,
    stopBatchDownload,
    isBatchDownloading,
    batchProgress,
  } = useDownloadTasks(tasks, removeTask);

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
        onDownloadAll={handleDownloadAllSequentially}
        onStopDownload={stopBatchDownload}
        isBatchDownloading={isBatchDownloading}
        batchProgress={batchProgress}
        tasksEmpty={downloadTasks.length === 0}
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
