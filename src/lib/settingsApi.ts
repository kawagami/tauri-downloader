// 統一 app 設定 IPC 封裝 + TS 契約（後端 settings.rs）
// 後端會用到的設定走這裡；純 UI 偏好（theme/activeTab/音量/欄寬）走 uiPrefs.ts。
// 限速一律 bytes/s、0 = 不限；目錄空字串 = 系統下載資料夾（三個分頁同語意）。

import { invoke } from "@tauri-apps/api/core";

export interface DownloadSettings {
  default_dir: string;
  limit_bps: number;
}

export interface BtSettings {
  default_dir: string;
  listen_port: number | null;
  upload_limit_bps: number;
  download_limit_bps: number;
}

export interface AppSettings {
  monitor_clipboard: boolean;
  /** 網站下載（站台爬取） */
  web: DownloadSettings;
  /** 直鏈下載 */
  http: DownloadSettings;
  bt: BtSettings;
  /** 工作需求遊戲設定分頁掃描的根目錄 */
  jin_roots: string[];
}

export function getAppSettings(): Promise<AppSettings> {
  return invoke("get_app_settings");
}

export function saveAppSettings(settings: AppSettings): Promise<void> {
  return invoke("save_app_settings", { settings });
}

/** get → 改 → 存:各元件不持有設定複本,避免互相蓋掉 */
export async function updateAppSettings(
  patch: (s: AppSettings) => AppSettings,
): Promise<AppSettings> {
  const next = patch(await getAppSettings());
  await saveAppSettings(next);
  return next;
}

/** 限速輸入一律以 KB/s 呈現，存檔轉 bytes/s */
export const kbpsToBps = (kbps: number): number =>
  Number.isFinite(kbps) && kbps > 0 ? Math.round(kbps * 1024) : 0;
export const bpsToKbps = (bps: number): number => (bps > 0 ? Math.round(bps / 1024) : 0);
