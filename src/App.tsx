import { useState, useEffect, useRef } from "react";
import "./App.css";
import downloadIcon from "./assets/react.svg";
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

// 定義一個簡單的 URL 驗證函數
const isUrlValid = (text: string): boolean => {
  return text.startsWith("http://") || text.startsWith("https://");
};

// 💡 輔助函數：將 URL 轉換為一個新的 Task 對象
const createNewTaskFromUrl = (url: string): Task => {
  const displayName = url.length > 50 ? url.substring(0, 50) + "..." : url;
  return {
    id: Date.now(),
    name: displayName,
    episode: "待解析",
    status: "Pending",
    progress: 0,
    path: "D:\\temp\\",
  };
};


function App() {
  const [url, setUrl] = useState("");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [monitorClipboard, setMonitorClipboard] = useState(false);
  const lastClipboardContent = useRef<string | null>(null);

  // 💡 重構後的 handleAddTask：能處理兩種來源 (表單/剪貼簿)
  const handleAddTask = async (e?: React.FormEvent, inputUrl?: string) => {
    // 來自表單提交時，阻止預設行為
    if (e) {
      e.preventDefault();
    }

    // 決定要處理哪個 URL：優先使用傳入的 inputUrl，否則使用狀態中的 url
    const taskUrl = inputUrl || url;

    // 檢查是否為空或無效
    if (taskUrl.trim() === "" || !isUrlValid(taskUrl)) {
      return;
    }

    // 檢查任務是否已在清單中 (簡單檢查)
    if (tasks.some(task => task.name === createNewTaskFromUrl(taskUrl).name)) {
      console.log("任務已存在，不重複新增。");
      if (!inputUrl) {
        setUrl(""); // 手動新增時清空輸入框
      }
      return;
    }

    try {
      // 呼叫後端指令
      await invoke("download_url", { url: taskUrl });

      // 建立新任務並更新清單
      const newTask = createNewTaskFromUrl(taskUrl);
      // 使用函數式更新確保拿到最新的 tasks 狀態
      setTasks((prevTasks) => [...prevTasks, newTask]);

      console.log(`成功新增任務: ${newTask.name}`);

      // 只有當任務是從輸入框手動新增時，才清空輸入框狀態
      if (!inputUrl) {
        setUrl("");
      }
    } catch (error) {
      console.error("呼叫後端指令時發生錯誤：", error);
    }
  };

  // 💡 監控剪貼簿邏輯 (已修復，移除 setUrl，改呼叫 handleAddTask)
  useEffect(() => {
    let intervalId: number | undefined;

    if (monitorClipboard) {
      intervalId = setInterval(async () => {
        try {
          const currentContent = await invoke("read_clipboard") as string;

          if (currentContent && currentContent !== lastClipboardContent.current) {

            // 檢查內容是否為有效的 URL
            if (isUrlValid(currentContent)) {
              // 🚨 修正處：呼叫 handleAddTask 並傳入 URL，自動加入清單
              handleAddTask(undefined, currentContent);
            }

            // 更新上一次的內容
            lastClipboardContent.current = currentContent;
          }
        } catch (error) {
          console.error("無法讀取剪貼簿：", error);
        }
      }, 1000);
    }

    return () => {
      if (intervalId) {
        clearInterval(intervalId);
      }
    };
    // 為了讓 useEffect 內部的 handleAddTask 能夠正確存取到最新的 tasks 狀態，
    // 我們需要將 tasks.length 加入到依賴項中。
  }, [monitorClipboard, tasks.length, handleAddTask]); // 加入 handleAddTask 以符合 React Hooks 規範

  // 處理 URL 輸入框的變化
  const handleUrlChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setUrl(e.target.value);
  };

  // 處理「監控剪貼簿」勾選框的變化
  const handleMonitorChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setMonitorClipboard(e.target.checked);

    if (e.target.checked) {
      // 啟動時讀取一次，避免一開始就觸發檢查
      invoke("read_clipboard").then((content) => {
        lastClipboardContent.current = content as string;
      }).catch((error) => {
        console.error("剪貼簿起始讀取錯誤:", error);
      });
    }
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
        {/* 表單提交時，只傳入 event */}
        <form className="input-section" onSubmit={(e) => handleAddTask(e)}>
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