// 純 UI 偏好（主題/分頁/音量/欄寬）集中在這裡 —
// 後端用得到的設定一律走 settingsApi（app_settings.json），這裡只放前端自己的東西。
// 舊版曾把後端設定也塞 localStorage，migrateLegacyPrefs() 負責一次性搬進 app_settings.json。

import { updateAppSettings } from "./settingsApi";

export const PREF_KEYS = {
  theme: "theme",
  activeTab: "activeTab",
  volume: "dingVolume",
  taskColumns: "task-table-col-widths",
} as const;

export function getPref(key: string, fallback: string): string {
  return localStorage.getItem(key) ?? fallback;
}

export function setPref(key: string, value: string): void {
  localStorage.setItem(key, value);
}

export function getJsonPref<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

export function setJsonPref<T>(key: string, value: T): void {
  localStorage.setItem(key, JSON.stringify(value));
}

/** 舊 key(後端設定誤放 localStorage) → app_settings.json，只搬一次 */
const LEGACY = { bandwidthKbps: "bandwidthKbps", httpDefaultDir: "httpDefaultDir" } as const;

export async function migrateLegacyPrefs(): Promise<void> {
  const kbpsRaw = localStorage.getItem(LEGACY.bandwidthKbps);
  const httpDir = localStorage.getItem(LEGACY.httpDefaultDir);
  if (kbpsRaw === null && httpDir === null) return;

  localStorage.removeItem(LEGACY.bandwidthKbps);
  localStorage.removeItem(LEGACY.httpDefaultDir);
  try {
    await updateAppSettings((s) => {
      const kbps = Number(kbpsRaw) || 0;
      return {
        ...s,
        // 舊值是 KB/s,新設定一律 bytes/s;後端已有值就不覆蓋
        web: kbps > 0 && s.web.limit_bps === 0 ? { ...s.web, limit_bps: kbps * 1024 } : s.web,
        http: httpDir && !s.http.default_dir ? { ...s.http, default_dir: httpDir } : s.http,
      };
    });
  } catch {}
}
