// src/App.tsx

import React, { useCallback } from "react";
import "./App.css";

// 引入自訂 Hooks 和元件
import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { TaskInputForm } from './components/TaskInputForm';
import { TaskList } from './components/TaskList';


function App() {

  // 1. 呼叫核心邏輯 Hooks
  const { tasks, addTask, removeTask, removeAllTasks } = useTaskManager();
  // 將 addTask 傳入 useClipboardMonitor
  const {
    monitorClipboard,
    setMonitorClipboard,
  } = useClipboardMonitor(addTask, tasks);

  // 3. 處理監控切換 (TaskInputForm 的 onMonitorChange)
  const handleMonitorChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setMonitorClipboard(e.target.checked);
  }, [setMonitorClipboard]);


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