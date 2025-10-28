// src/types.ts

// 任務的狀態必須是這三種字串之一
export enum TaskStatus {
    Completed = 'Completed',
    Downloading = 'Downloading',
    Pending = 'Pending',
    // ... 其他狀態
}

// 任務資料結構
export interface Task {
    id: number;
    url: string;
    name: string;
    title: string;     // ✨ 新增：從 Rust/reqwest 取得的標題
    image: string;     // ✨ 新增：從 Rust/reqwest 取得的圖片
    status: TaskStatus;
}

// 定義一個 Payload 結構來傳遞數據
export interface TaskPayload {
    url: string;
    title: string;
    image: string;
}

// 輔助函數：根據所有可用資訊建立新任務
export const createNewTaskFromPayload = (payload: TaskPayload): Task => {
    // 這裡的邏輯需要確保 name 欄位可以被適當填充
    const taskName = payload.title || `Task_${Date.now()}`; // 如果沒有 title，使用 fallback

    return {
        id: Date.now(), // 使用時間戳作為簡單 ID
        url: payload.url,
        name: taskName,
        title: payload.title,
        image: payload.image,
        status: TaskStatus.Pending,
    };
};

// 保持 isUrlValid 函數不變
export const isUrlValid = (url: string): boolean => {
    // 您的 URL 驗證邏輯
    return url.startsWith('http') || url.startsWith('https');
};