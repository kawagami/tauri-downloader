// src/types.ts

// 任務的狀態必須是這三種字串之一
export type TaskStatus = "Completed" | "Downloading" | "Pending";

// 任務資料結構
export interface Task {
    id: number;
    url: string;
    name: string;
    episode: string;
    status: TaskStatus;
    progress: number; // 0 到 100
    path: string;
}

// 輔助函數：將 URL 轉換為一個新的 Task 對象
export const createNewTaskFromUrl = (url: string): Task => {
    // 限制顯示的名稱長度
    const displayName = url.length > 50 ? url.substring(0, 50) + "..." : url;
    return {
        id: Date.now(), // 使用時間戳作為唯一 ID
        url: displayName,
        name: displayName,
        episode: "待解析", // 初始狀態
        status: "Pending",
        progress: 0,
        path: "D:\\temp\\", // 預設路徑
    };
};

// 簡單的 URL 驗證函數
export const isUrlValid = (text: string): boolean => {
    return text.startsWith("http://") || text.startsWith("https://");
};