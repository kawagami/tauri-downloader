// src/hooks/useClipboardMonitor.ts

import { useState, useEffect, useCallback } from 'react';
import { getAppSettings, updateAppSettings } from '../lib/settingsApi';

interface UseClipboardMonitor {
    monitorClipboard: boolean;
    setMonitorClipboard: (enabled: boolean) => Promise<void>;
}

/**
 * 只管監控開關。任務事件（新增/合集/抓取失敗）在 useTaskEvents，
 * 剪貼簿與拖曳共用、不受這個開關控制。
 */
export const useClipboardMonitor = (): UseClipboardMonitor => {
    const [monitorClipboard, setMonitorClipboardState] = useState(true);

    // 開關狀態持久化在 app_settings.json;後端啟動已自行套用,這裡只同步 UI
    useEffect(() => {
        getAppSettings()
            .then(s => setMonitorClipboardState(s.monitor_clipboard))
            .catch(() => {});
    }, []);

    const setMonitorClipboard = useCallback(async (enabled: boolean) => {
        setMonitorClipboardState(enabled);
        // save_app_settings 會即時套用 monitor_paused
        await updateAppSettings(s => ({ ...s, monitor_clipboard: enabled }));
    }, []);

    return {
        monitorClipboard,
        setMonitorClipboard,
    };
};
