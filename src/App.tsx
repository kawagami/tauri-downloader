// src/App.tsx

import React, { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { useDownloadTasks } from './hooks/useDownloadTasks';
import { TaskInputForm } from './components/TaskInputForm';
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
      <div className="sticky-toolbar">
        <TaskInputForm
          monitorClipboard={monitorClipboard}
          onMonitorChange={handleMonitorChange}
        />
        <div className="toolbar-actions">
          <button onClick={() => removeAllTasks()}>全部刪除</button>
          {!isBatchDownloading ? (
            <button
              onClick={handleDownloadAllSequentially}
              disabled={downloadTasks.length === 0}
              style={{ marginLeft: "10px" }}
            >
              全部下載
            </button>
          ) : (
            <button
              onClick={stopBatchDownload}
              style={{ marginLeft: "10px", background: "#f87171", color: "white" }}
            >
              停止下載 ({batchProgress.current} / {batchProgress.total})
            </button>
          )}
        </div>
      </div>

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
