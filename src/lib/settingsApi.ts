// 統一 app 設定 IPC 封裝 + TS 契約（後端 settings.rs）
// 後端會用到的設定走這裡；純 UI 偏好（theme/activeTab/音量/欄寬）留 localStorage。

import { invoke } from "@tauri-apps/api/core";

export interface BtSettings {
  default_download_dir: string;
  listen_port: number | null;
  upload_limit_bps: number | null;
  download_limit_bps: number | null;
}

export interface AppSettings {
  monitor_clipboard: boolean;
  bandwidth_limit_kbps: number;
  http_default_dir: string;
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
