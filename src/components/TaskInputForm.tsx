// src/components/TaskInputForm.tsx

import React from 'react';

interface TaskInputFormProps {
    url: string;
    setUrl: (url: string) => void;
    onSubmit: (e: React.FormEvent) => void;
    monitorClipboard: boolean;
    onMonitorChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

export const TaskInputForm: React.FC<TaskInputFormProps> = ({
    url,
    setUrl,
    onSubmit,
    monitorClipboard,
    onMonitorChange
}) => {

    // 處理 URL 輸入框的變化
    const handleUrlChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        setUrl(e.currentTarget.value);
    };

    return (
        <form className="input-section" onSubmit={onSubmit}>
            <div className="input-group">
                <input
                    type="text"
                    placeholder="target url"
                    value={url}
                    onChange={handleUrlChange}
                    className="url-input"
                />
                <button type="submit" className="add-button">
                    新增
                </button>
            </div>
            <div className="checkbox-group">
                <input
                    type="checkbox"
                    id="monitorClipboard"
                    checked={monitorClipboard}
                    onChange={onMonitorChange}
                />
                <label htmlFor="monitorClipboard">監控剪貼簿</label>
            </div>
        </form>
    );
};