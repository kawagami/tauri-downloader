// HTTP 直鏈下載 IPC 封裝 + TS 契約（後端 http_dl/ 模組）

import { invoke } from "@tauri-apps/api/core";

// ---- Event 契約（後端 http_dl/events.rs） ----

export type HttpTaskState = "running" | "paused" | "finished" | "error";

export interface HttpTaskItem {
  id: number;
  name: string;
  state: HttpTaskState;
  progress_percent: number; // 0-100；total 未知時為 0
  downloaded_bytes: number;
  total_bytes: number; // 0 = 未知
  down_speed_bps: number;
  error: string | null;
  retryable: boolean; // 網路類錯誤可直接重試；否則需貼新連結
}

export interface HttpStatsEvent {
  tasks: HttpTaskItem[];
  total_down_bps: number;
}

export interface HttpFinishedEvent {
  id: number;
  name: string;
}

// ---- Command wrappers ----

export interface AddHttpResult {
  already_exists?: boolean;
  id: number;
}

export function addHttpDownload(url: string, outDir?: string): Promise<AddHttpResult> {
  return invoke("add_http_download", { url, outDir: outDir ?? null });
}

export function pauseHttpDownload(id: number): Promise<void> {
  return invoke("pause_http_download", { id });
}

export function resumeHttpDownload(id: number): Promise<void> {
  return invoke("resume_http_download", { id });
}

/** token 過期後貼上指向同一檔案的新連結,接著續傳。 */
export function updateHttpUrl(id: number, url: string): Promise<void> {
  return invoke("update_http_url", { id, url });
}

export function deleteHttpDownload(id: number, deleteFiles: boolean): Promise<void> {
  return invoke("delete_http_download", { id, deleteFiles });
}
