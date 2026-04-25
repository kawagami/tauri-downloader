// TaskList.tsx — kept for backwards compat, delegates to TaskListView

import { useDownloadTasks } from "../hooks/useDownloadTasks";
import { TaskListView } from "./TaskListView";
import { Task } from "../types";

export const TaskList = ({
    tasks,
    onRemoveTask,
}: {
    tasks: Task[];
    onRemoveTask: (url: string) => void;
}) => {
    const {
        tasks: downloadTasks,
        handleDownload,
    } = useDownloadTasks(tasks, onRemoveTask);

    return (
        <TaskListView
            tasks={downloadTasks}
            onRemoveTask={onRemoveTask}
            onDownload={handleDownload}
        />
    );
};
