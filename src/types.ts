// src/types.ts

export interface Task {
    url: string;
    title: string;
    image: string;
    download_page_href: string;
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

export interface ClipboardPayload {
    url: string;
    title: string;
    image: string;
    download_page_href: string;
    created_at: number;
    db_status: string;
}
