import { useState } from "react";
import "./App.css";
import downloadIcon from "./assets/react.svg"; // 請自行準備或使用 SVG

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
  const handleAddTask = (e: React.FormEvent) => {
    e.preventDefault();
    if (url.trim() === "") {
      return; // 如果 URL 為空，則不執行任何操作
    }

    // 模擬新增一個任務，實際情況會根據 URL 產生不同資訊
    const newTask: Task = {
      id: Date.now(),
      name: `新增任務 ${tasks.length + 1}`,
      episode: "1 [1/1]",
      status: "Completed", // 這裡預設為已完成，之後會連接後端
      progress: 100,
      path: "D:\\temp\\",
    };

    // 將新任務添加到任務列表中
    setTasks((prevTasks) => [...prevTasks, newTask]);
    // 清空輸入框
    setUrl("");
    console.log("新增任務成功：", newTask);
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