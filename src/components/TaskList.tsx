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
                        {/* ✨ 變更為顯示標題 */}
                        <th>標題 (Name)</th>
                        <th>狀態</th>
                        {/* 考慮將 URL 放在單獨的欄位或工具提示中，這裡先顯示在 cell 中 */}
                        <th>連結 (URL)</th>
                        {/* ✨ 變更為顯示圖片 */}
                        <th>預覽圖</th>
                    </tr>
                </thead>
                <tbody>
                    {tasks.map((task) => (
                        <tr key={task.id}>
                            {/* ✨ 使用 task.title 來顯示標題 */}
                            <td title={task.name}>{task.title}</td>
                            <td>{task.status}</td>

                            {/* 顯示 URL，過長時建議省略或使用 title 提示 */}
                            <td title={task.url} className="url-cell">
                                <a href={task.url} target="_blank" rel="noopener noreferrer">
                                    {/* 顯示 URL 的一部分，防止過長 */}
                                    {task.url.length > 30 ? task.url.substring(0, 30) + '...' : task.url}
                                </a>
                            </td>

                            {/* ✨ 核心變動：使用 <img> 標籤顯示 task.image */}
                            <td>
                                {task.image && task.image !== "placeholder.png" ? (
                                    <img
                                        src={task.image} // 使用圖片路徑作為 src
                                        alt={`預覽圖: ${task.title}`}
                                        style={{ width: '100px', height: 'auto', objectFit: 'contain' }} // 設定圖片大小
                                    />
                                ) : (
                                    <span>無圖片</span>
                                )}
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
};