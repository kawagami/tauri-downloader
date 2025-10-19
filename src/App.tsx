// src/App.tsx

import React, { useCallback } from "react";
import "./App.css";
import downloadIcon from "./assets/react.svg"; // 確保這個路徑正確

// 引入自訂 Hooks 和元件
import { useTaskManager } from './hooks/useTaskManager';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { TaskInputForm } from './components/TaskInputForm';
import { TaskList } from './components/TaskList';


function App() {
  // 1. 呼叫核心邏輯 Hooks
  const { tasks, addTask } = useTaskManager();
  // 將 addTask 傳入 useClipboardMonitor
  const {
    monitorClipboard,
    setMonitorClipboard,
    url,
    setUrl
  } = useClipboardMonitor(addTask);

  // 2. 處理表單提交 (TaskInputForm 的 onSubmit)
  // 這裡我們建立一個處理函數來橋接 TaskInputForm 的 event 和 addTask 的 string
  const handleFormSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    // 呼叫 addTask，它會使用 url 狀態
    addTask(url);
  }, [addTask, url]);

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
          url={url}
          setUrl={setUrl}
          onSubmit={handleFormSubmit}
          monitorClipboard={monitorClipboard}
          onMonitorChange={handleMonitorChange}
        />

        <TaskList tasks={tasks} />

      </main>

      {/* 右側下載按鈕 */}
      <div className="side-action-button">
        <img src={downloadIcon} alt="Download" />
      </div>
    </div>
  );
}

export default App;