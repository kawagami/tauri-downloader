import React from 'react';
import { Task } from '../types';

interface TaskListProps {
    tasks: Task[];
    onRemoveTask: (url: string) => void;
    onRemoveAll: () => void;
}

export const TaskList: React.FC<TaskListProps> = ({ tasks, onRemoveTask, onRemoveAll }) => {
    return (
        <div className="task-list-container">
            <div style={{ marginBottom: '10px' }}>
                <button onClick={onRemoveAll}>全部刪除</button>
            </div>

            <table className="task-table">
                <thead>
                    <tr>
                        <th>標題 (Name)</th>
                        <th>連結 (URL)</th>
                        <th>預覽圖</th>
                        <th>操作</th>
                    </tr>
                </thead>
                <tbody>
                    {tasks.map((task) => (
                        <tr key={task.url}>
                            <td title={task.title}>{task.title}</td>
                            <td title={task.url} className="url-cell">
                                <a href={task.url} target="_blank" rel="noopener noreferrer">
                                    {task.url.length > 30 ? task.url.substring(0, 30) + '...' : task.url}
                                </a>
                            </td>
                            <td>
                                {task.image && task.image !== "placeholder.png" ? (
                                    <div className="image-container">
                                        <img
                                            src={task.image}
                                            alt={`預覽圖: ${task.title}`}
                                            className="thumbnail"
                                        />
                                        <div className="image-preview">
                                            <img src={task.image} alt={`預覽圖: ${task.title}`} />
                                        </div>
                                    </div>
                                ) : (
                                    <span>無圖片</span>
                                )}
                            </td>
                            <td>
                                <button onClick={() => onRemoveTask(task.url)}>刪除</button>
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
};
