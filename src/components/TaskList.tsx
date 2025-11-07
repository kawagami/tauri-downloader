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
    const { tasks: downloadTasks, handleDownload } = useDownloadTasks(tasks);

    return (
        <TaskListView
            tasks={downloadTasks}
            onRemoveTask={onRemoveTask}
            onRemoveAll={onRemoveAll}
            onDownload={handleDownload}
        />
    );
};
