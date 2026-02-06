// src/components/TaskInputForm.tsx

import React from 'react';

interface TaskInputFormProps {
    monitorClipboard: boolean;
    onMonitorChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

export const TaskInputForm: React.FC<TaskInputFormProps> = ({
    monitorClipboard,
    onMonitorChange
}) => {

    return (
        <form className="input-section">
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