// TaskList.tsx

import { useDownloadTasks } from "../hooks/useDownloadTasks";
import { TaskListView } from "./TaskListView";
import { Task } from "../types";

export const TaskList = ({
    tasks,
    onRemoveTask,
    onRemoveAll,
}: {
    tasks: Task[];
    onRemoveTask: (url: string) => void;
    onRemoveAll: () => void;
}) => {
    // ✅ 將 onRemoveTask 傳入 useDownloadTasks（用於下載完自動刪除）
    const {
        tasks: downloadTasks,
        handleDownload,
        handleDownloadAllSequentially,
        isBatchDownloading,
    } = useDownloadTasks(tasks, onRemoveTask);

    return (
        <TaskListView
            tasks={downloadTasks}
            onRemoveTask={onRemoveTask}
            onRemoveAll={onRemoveAll}
            onDownload={handleDownload}
            // ✅ 新增兩個 props 給 TaskListView
            onDownloadAll={handleDownloadAllSequentially}
            isBatchDownloading={isBatchDownloading}
        />
    );
};
