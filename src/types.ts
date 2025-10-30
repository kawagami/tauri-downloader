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
    url: string;
    title: string;
    image: string;
    download_page_href: string;
}

export interface ClipboardPayload {
    url: string;
    title: string;
    image: string;
    download_page_href: string;
}

export type AddTaskFunction = (payload: ClipboardPayload) => Promise<void>;

export interface UseTaskManager {
    tasks: Task[];
    /**
     * 處理新增任務的邏輯。
     * @param payload 包含 URL、title 和 image 的單一物件。
     */
    addTask: (payload: ClipboardPayload) => Promise<void>;
}
