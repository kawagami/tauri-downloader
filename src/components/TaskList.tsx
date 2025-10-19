// src/components/TaskList.tsx

import React from 'react';
import { Task } from '../types';

interface TaskListProps {
    tasks: Task[];
}

export const TaskList: React.FC<TaskListProps> = ({ tasks }) => {
    return (
        <div className="task-list-container">
            <table className="task-table">
                <thead>
                    <tr>
                        <th>名箱</th>
                        <th>話(集)數</th>
                        <th>狀態</th>
                        <th>指令</th>
                        <th>路徑</th>
                    </tr>
                </thead>
                <tbody>
                    {tasks.map((task) => (
                        <tr key={task.id}>
                            <td>{task.name}</td>
                            <td>{task.episode}</td>
                            <td>
                                <div className={`status-bar status-${task.status.toLowerCase()}`}>
                                    {task.progress}% [{task.episode}]
                                </div>
                            </td>
                            <td>{task.status}</td>
                            <td>{task.path}</td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
};