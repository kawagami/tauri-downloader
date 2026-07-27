// src/types.ts

export interface Task {
    url: string;
    title: string;
    image: string;
    download_page_href: string;
    file_url: string;
    file_size: number; // 位元組，-1 = 未知
    created_at: number;
    db_status: string;
}

export interface DownloadableTask extends Task {
    progress?: number;
    speed?: number;
    timeRemaining?: number;
    status?: "idle" | "downloading" | "done" | "error" | "paused" | "not_found";
    savePath?: string;
    errorMessage?: string;
}

/**
 * 後端 ClipboardPayload（providers/mod.rs）與清單裡的 Task 是同一組欄位 —
 * 以前是兩份逐字相同的 interface，改欄位得記得改兩邊。別名讓它們不可能漂掉。
 */
export type ClipboardPayload = Task;
