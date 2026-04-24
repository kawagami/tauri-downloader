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
    created_at: number; // Unix timestamp (seconds)
}

export interface DownloadableTask extends Task {
    progress?: number;
    speed?: number;        // bytes/sec
    timeRemaining?: number; // seconds, Infinity = calculating
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
    addTask: (payload: ClipboardPayload) => Promise<void>;
    removeTask: (url: string) => Promise<void>;
    removeAllTasks: () => Promise<void>;
    reloadTasks: () => Promise<void>;
}
