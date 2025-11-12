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
    const {
        tasks: downloadTasks,
        handleDownload,
        handleDownloadAllSequentially,
        stopBatchDownload, // ✅ 新增這個
        isBatchDownloading,
    } = useDownloadTasks(tasks, onRemoveTask);

    return (
        <TaskListView
            tasks={downloadTasks}
            onRemoveTask={onRemoveTask}
            onRemoveAll={onRemoveAll}
            onDownload={handleDownload}
            onDownloadAll={handleDownloadAllSequentially}
            onStopDownloadAll={stopBatchDownload} // ✅ 新增這個 prop
            isBatchDownloading={isBatchDownloading}
        />
    );
};
