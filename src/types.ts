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

/** 後端 `ingest::IngestSummary` — 一次加入（剪貼簿/拖曳）的統計；合集才有 series_title */
export interface IngestSummary {
    series_title: string | null;
    added: number;
    existed: number;   // 清單裡已經有了
    skipped: number;   // 下載目錄已有同名檔（只有剪貼簿監控會檢查）
    failed: number;    // 合集個別章節抓取失敗
}

/** `series-start` 事件 */
export interface SeriesStartEvent {
    title: string;
    total: number;
}
