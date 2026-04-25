use crate::{providers::ClipboardPayload, state::AppState};

use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 取得 SQLite 檔案路徑
pub fn get_db_path(app_handle: &AppHandle) -> PathBuf {
    let mut path = app_handle
        .path()
        .app_data_dir()
        .expect("無法取得 app data 資料夾");
    std::fs::create_dir_all(&path).expect("建立資料夾失敗");
    path.push("tasks.db");
    path
}

/// 建立並初始化資料庫
pub fn init_db(app_handle: &AppHandle) -> Result<Connection> {
    let db_path = get_db_path(app_handle);
    let conn = Connection::open(db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT UNIQUE NOT NULL,
            title TEXT,
            image TEXT,
            download_page_href TEXT,
            created_at INTEGER DEFAULT 0,
            db_status TEXT NOT NULL DEFAULT 'idle'
        )",
        [],
    )?;

    conn.execute("ALTER TABLE tasks ADD COLUMN created_at INTEGER DEFAULT 0", []).ok();
    conn.execute("ALTER TABLE tasks ADD COLUMN db_status TEXT NOT NULL DEFAULT 'idle'", []).ok();

    Ok(conn)
}

/// 新增任務資料，回傳是否實際插入（false = 已存在略過）
pub fn insert_task(app_handle: &AppHandle, payload: &ClipboardPayload) -> Result<bool> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();

    let affected = conn.execute(
        "INSERT OR IGNORE INTO tasks (url, title, image, download_page_href, created_at, db_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            payload.url,
            payload.title,
            payload.image,
            payload.download_page_href,
            payload.created_at,
            payload.db_status,
        ],
    )?;

    Ok(affected > 0)
}

/// 取得所有任務資料
pub fn get_all_tasks(app_handle: &AppHandle) -> Result<Vec<ClipboardPayload>> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();

    let mut stmt = conn.prepare(
        "SELECT url, title, image, download_page_href, created_at, db_status FROM tasks ORDER BY created_at ASC",
    )?;

    let task_iter = stmt.query_map([], |row| {
        Ok(ClipboardPayload {
            url: row.get(0)?,
            title: row.get(1)?,
            image: row.get(2)?,
            download_page_href: row.get(3)?,
            created_at: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            db_status: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "idle".to_string()),
        })
    })?;

    let mut tasks = Vec::new();
    for task in task_iter {
        tasks.push(task?);
    }

    Ok(tasks)
}

/// 刪除指定 URL 的任務
pub fn delete_task_by_url(app_handle: &AppHandle, url: &str) -> Result<()> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();
    conn.execute("DELETE FROM tasks WHERE url = ?1", params![url])?;
    Ok(())
}

/// 更新任務的持久化狀態
pub fn update_task_status(app_handle: &AppHandle, url: &str, status: &str) -> Result<()> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();
    conn.execute("UPDATE tasks SET db_status = ?1 WHERE url = ?2", params![status, url])?;
    Ok(())
}

/// 清空所有任務
pub fn clear_all_tasks(app_handle: &AppHandle) -> Result<()> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();
    conn.execute("DELETE FROM tasks", [])?;
    Ok(())
}
