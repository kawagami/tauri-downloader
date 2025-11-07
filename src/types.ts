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

export interface DownloadableTask extends Task {
    progress?: number;
    status?: "idle" | "downloading" | "done" | "error";
    savePath?: string;
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
     * 新增任務。
     * @param payload 包含 URL、title、image 等資訊的單一物件。
     */
    addTask: (payload: ClipboardPayload) => Promise<void>;

    /**
     * 刪除單個任務。
     * @param url 要刪除的任務 URL。
     */
    removeTask: (url: string) => Promise<void>;

    /**
     * 刪除全部任務。
     */
    removeAllTasks: () => Promise<void>;
}
