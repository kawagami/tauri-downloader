// 選資料夾 — 四個 dialog（BT/直鏈/jin/統一設定）共用同一個入口

import { open } from "@tauri-apps/plugin-dialog";

/** 開資料夾選擇器，回傳選中的路徑；使用者取消回 null */
export async function pickDir(current?: string): Promise<string | null> {
  const dir = await open({ directory: true, defaultPath: current || undefined });
  return typeof dir === "string" ? dir : null;
}
