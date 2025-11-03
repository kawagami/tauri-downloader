use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::{monitor::ClipboardPayload, state::AppState};

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
            download_page_href TEXT
        )",
        [],
    )?;

    Ok(conn)
}

/// 新增任務資料
pub fn insert_task(app_handle: &AppHandle, payload: &ClipboardPayload) -> Result<()> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();

    conn.execute(
        "INSERT OR IGNORE INTO tasks (url, title, image, download_page_href)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            payload.url,
            payload.title,
            payload.image,
            payload.download_page_href
        ],
    )?;

    Ok(())
}

/// 取得所有任務資料
pub fn get_all_tasks(app_handle: &AppHandle) -> Result<Vec<ClipboardPayload>> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();

    let mut stmt =
        conn.prepare("SELECT url, title, image, download_page_href FROM tasks ORDER BY id ASC")?;

    let task_iter = stmt.query_map([], |row| {
        Ok(ClipboardPayload {
            url: row.get(0)?,
            title: row.get(1)?,
            image: row.get(2)?,
            download_page_href: row.get(3)?,
        })
    })?;

    let mut tasks = Vec::new();
    for task in task_iter {
        tasks.push(task?);
    }

    Ok(tasks)
}

/// 取得指定 URL 的任務
pub fn get_task_by_url(app_handle: &AppHandle, url: &str) -> Result<Option<ClipboardPayload>> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();

    let mut stmt =
        conn.prepare("SELECT url, title, image, download_page_href FROM tasks WHERE url = ?1")?;

    let mut rows = stmt.query(params![url])?;

    if let Some(row) = rows.next()? {
        Ok(Some(ClipboardPayload {
            url: row.get(0)?,
            title: row.get(1)?,
            image: row.get(2)?,
            download_page_href: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

/// 刪除指定 URL 的任務
pub fn delete_task_by_url(app_handle: &AppHandle, url: &str) -> Result<()> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();
    conn.execute("DELETE FROM tasks WHERE url = ?1", params![url])?;
    Ok(())
}

/// 清空所有任務
pub fn clear_all_tasks(app_handle: &AppHandle) -> Result<()> {
    let state = app_handle.state::<AppState>();
    let conn = state.db.lock().unwrap();
    conn.execute("DELETE FROM tasks", [])?;
    Ok(())
}
