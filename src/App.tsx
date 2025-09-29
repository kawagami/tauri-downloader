import { useState } from "react";
import "./App.css";
import downloadIcon from "./assets/react.svg"; // 請自行準備或使用 SVG
import { invoke } from "@tauri-apps/api/core";

// 假設的任務資料結構
interface Task {
  id: number;
  name: string;
  episode: string;
  status: "Completed" | "Downloading" | "Pending";
  progress: number;
  path: string;
}

function App() {
  const [url, setUrl] = useState("");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [monitorClipboard, setMonitorClipboard] = useState(false);

  // 處理 URL 輸入框的變化
  const handleUrlChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setUrl(e.target.value);
  };

  // 處理「新增」按鈕點擊事件

  // 編輯 handleAddTask 函數
  const handleAddTask = async (e: React.FormEvent) => {
    e.preventDefault();
    if (url.trim() === "") {
      return;
    }

    try {
      // 使用 invoke 呼叫後端的 download_url 指令
      const result = await invoke("download_url", { url });
      console.log("從後端接收到的回傳訊息：", result);

      // 這裡我們修正了 newTask 的 status 屬性
      const newTask: Task = {
        id: Date.now(),
        name: url,
        episode: "...",
        status: "Pending", // 這裡將 status 設為 "Pending"
        progress: 0,
        path: "D:\\temp\\",
      };

      setTasks((prevTasks) => [...prevTasks, newTask]);
      setUrl("");
    } catch (error) {
      console.error("呼叫後端指令時發生錯誤：", error);
    }
  };

  // 處理「監控剪貼簿」勾選框的變化
  const handleMonitorChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setMonitorClipboard(e.target.checked);
    console.log("監控剪貼簿狀態：", e.target.checked);
  };

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
        {/* URL 輸入與控制區塊 */}
        <form className="input-section" onSubmit={handleAddTask}>
          <div className="input-group">
            <input
              type="text"
              placeholder="https://www.wnacg.com/photos-index-aid-323188.html"
              value={url}
              onChange={handleUrlChange}
              className="url-input"
            />
            <button type="submit" className="add-button">
              新增
            </button>
          </div>
          <div className="checkbox-group">
            <input
              type="checkbox"
              id="monitorClipboard"
              checked={monitorClipboard}
              onChange={handleMonitorChange}
            />
            <label htmlFor="monitorClipboard">監控剪貼簿</label>
          </div>
        </form>

        {/* 任務列表 */}
        <div className="task-list-container">
          <table className="task-table">
            <thead>
              <tr>
                <th>名箱</th>
                <th>話(集)數</th>
                <th>狀態</th>
                <th>指令</th>
                <th>路徑</th>
              </tr>
            </thead>
            <tbody>
              {tasks.map((task) => (
                <tr key={task.id}>
                  <td>{task.name}</td>
                  <td>{task.episode}</td>
                  <td>
                    <div className={`status-bar status-${task.status.toLowerCase()}`}>
                      {task.progress}% [{task.episode}]
                    </div>
                  </td>
                  <td>{task.status}</td>
                  <td>{task.path}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </main>

      {/* 右側下載按鈕 */}
      <div className="side-action-button">
        <img src={downloadIcon} alt="Download" />
      </div>
    </div>
  );
}

export default App;