// 讀剪貼簿 — 一律走後端 read_clipboard 指令。
// webview 的 navigator.clipboard.readText() 需要 focus/權限，被擋時是靜默失敗
//（dialog 的「自動帶入」就會莫名其妙沒作用），後端沒這個限制。

import { invoke } from "@tauri-apps/api/core";

/** 讀不到（權限/系統問題）回空字串，呼叫端當作「剪貼簿沒東西」處理即可 */
export async function readClipboard(): Promise<string> {
  try {
    return await invoke<string>("read_clipboard");
  } catch {
    return "";
  }
}
