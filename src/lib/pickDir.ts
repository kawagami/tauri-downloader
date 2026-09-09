// 選資料夾 — 三處共用同一個入口：AddMagnetDialog、AddHttpDialog、
// SettingsDialog 的 DirField（jin 根目錄也走 DirField，沒有自己的 dialog）

import { open } from "@tauri-apps/plugin-dialog";

/** 開資料夾選擇器，回傳選中的路徑；使用者取消回 null */
export async function pickDir(current?: string): Promise<string | null> {
  const dir = await open({ directory: true, defaultPath: current || undefined });
  return typeof dir === "string" ? dir : null;
}
