import { useState } from "react";
import "./App.css";

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

  // 模擬新增任務的函數
  const handleAddTask = () => {
    if (url) {
      const newTask: Task = {
        id: tasks.length + 1,
        name: "新增任務 " + (tasks.length + 1), // 這裡可以根據URL解析
        episode: "1 [1/1]",
        status: "Pending",
        progress: 0,
        path: "D:\\temp\\",
      };
      setTasks([...tasks, newTask]);
      setUrl(""); // 清空輸入框
    }
  };

  return (
    <div className="container">
      {/* 頂部選單 */}
      <div className="navbar">
        <div className="navbar-left">
          <span className="menu-item">選單</span>
          <span className="menu-item">幫助</span>
        </div>
      </div>

      {/* 輸入與控制區塊 */}
      <div className="input-section">
        <input
          type="text"
          className="url-input"
          placeholder="https://www.wnacg.com/photos-index-aid-323188.html"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
        />
        <button className="add-button" onClick={handleAddTask}>
          新增
        </button>
        <div className="checkbox-group">
          <input
            type="checkbox"
            id="monitorClipboard"
            checked={monitorClipboard}
            onChange={(e) => setMonitorClipboard(e.target.checked)}
          />
          <label htmlFor="monitorClipboard">監控剪貼簿</label>
        </div>
      </div>

      {/* 任務列表 */}
      <div className="task-list-container">
        <table className="task-table">
          <thead>
            <tr>
              <th>名稱</th>
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
                    <span>100% [1/1]</span> {/* 這裡可以動態顯示進度 */}
                  </div>
                </td>
                <td>Completed</td> {/* 這裡可以放操作按鈕 */}
                <td>{task.path}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* 下載按鈕區塊 */}
      <div className="action-buttons-container">
        {/* 下載按鈕的 SVG 或圖片 */}
      </div>
    </div>
  );
}

export default App;