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
    created_at: number;
    db_status: string; // DB 持久化狀態: idle | paused | not_found | done
}

export interface DownloadableTask extends Task {
    progress?: number;
    speed?: number;
    timeRemaining?: number;
    status?: "idle" | "downloading" | "done" | "error" | "paused" | "not_found";
    savePath?: string;
    errorMessage?: string;
}

export interface ClipboardPayload {
    url: string;
    title: string;
    image: string;
    download_page_href: string;
    created_at: number;
    db_status: string;
}

export type AddTaskFunction = (payload: ClipboardPayload) => Promise<void>;

export interface UseTaskManager {
    tasks: Task[];
    addTask: (payload: ClipboardPayload) => Promise<void>;
    removeTask: (url: string) => Promise<void>;
    removeAllTasks: () => Promise<void>;
    reloadTasks: () => Promise<void>;
}
