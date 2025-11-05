import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface DownloadPageProps {
    url?: string;   // 外部傳入下載 URL
    title?: string; // 外部傳入檔案標題 / 名稱
}

export const DownloadPage: React.FC<DownloadPageProps> = ({ url, title }) => {
    const [progress, setProgress] = useState(0);
    const [savePath, setSavePath] = useState("");

    useEffect(() => {
        // 監聽 Rust emit 的下載進度事件
        const unlisten = listen<number>("download_progress", (event) => {
            setProgress(event.payload);
        });

        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    const handleDownload = async () => {
        if (!url) return; // 沒有 URL 就不執行

        try {
            const path = await invoke<string>("download_with_progress", {
                url,
                title,
            });
            setSavePath(path);
        } catch (error) {
            console.error("下載失敗:", error);
        }
    };

    // 如果 url 改變自動開始下載（可選）
    useEffect(() => {
        if (url) handleDownload();
    }, [url]);

    return (
        <div className="p-4">
            <button
                onClick={handleDownload}
                className="bg-blue-500 text-white px-4 py-2 rounded"
                disabled={!url} // 沒有 URL 按鈕不可按
            >
                開始下載
            </button>

            <div className="mt-4">
                <div>進度：{progress.toFixed(2)}%</div>
                <div className="w-full bg-gray-200 rounded h-2 mt-2">
                    <div
                        className="bg-green-500 h-2 rounded"
                        style={{ width: `${progress}%` }}
                    />
                </div>

                {savePath && (
                    <p className="mt-3 text-sm text-gray-600">
                        ✅ 下載完成：{savePath}
                    </p>
                )}
            </div>
        </div>
    );
};
