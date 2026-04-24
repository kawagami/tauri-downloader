// src/App.tsx

import React, { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { TaskInputForm } from './components/TaskInputForm';
import { TaskList } from './components/TaskList';


function App() {

  const { tasks, addTask, removeTask, removeAllTasks, reloadTasks } = useTaskManager();
  const {
    monitorClipboard,
    setMonitorClipboard,
  } = useClipboardMonitor(addTask, tasks);

  const handleMonitorChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const enabled = e.target.checked;
    setMonitorClipboard(enabled);
    await invoke("set_monitor_paused", { paused: !enabled });
    if (enabled) {
      await reloadTasks(); // 重新載入，補上暫停期間的任務
    }
  }, [setMonitorClipboard, reloadTasks]);


  return (
    <div className="container">
      {/* 頂部導覽列 */}
      <header className="header">
        <div className="menu">
          <span>選單</span>
          <span>幫助</span>
        </div>
      </header>

      {/* 主要內容區塊 */}
      <main className="main-content">

        <TaskInputForm
          monitorClipboard={monitorClipboard}
          onMonitorChange={handleMonitorChange}
        />

        <TaskList
          tasks={tasks}
          onRemoveTask={(url) => removeTask(url)}
          onRemoveAll={() => removeAllTasks()}
        />

      </main>

    </div>
  );
}

export default App;