// src/jin/commands.rs
// 工作需求遊戲設定分頁的 IPC：先 preview（不寫檔）→ 使用者確認 → apply（寫檔）。
// 兩者共用同一個 plan_files，preview 看到什麼 apply 就做什麼。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::jin::core::{
    build_codes, build_new_content, collect_files, detect_state, extract_env, uncomment_content,
    State as FileState,
};
use crate::settings::SettingsState;

#[derive(Debug, Clone, Serialize)]
pub struct JinFilePlan {
    pub path: String,
    pub env: String,
    /// add | add_commented | uncomment | skip | error
    pub action: String,
    pub message: String,
    /// apply 後實際寫檔成功才 true；preview 一律 false
    pub applied: bool,
}

#[derive(Debug, Serialize)]
pub struct JinPreview {
    pub hall: String,
    pub codes: Vec<String>,
    pub roots: Vec<String>,
    pub files: Vec<JinFilePlan>,
}

/// 執行進度（掃描/寫檔逐檔推給前端，UNC 路徑慢時使用者才知道還在跑）
#[derive(Debug, Clone, Serialize)]
pub struct JinProgress {
    /// scan | write | done
    pub phase: String,
    pub done: usize,
    pub total: usize,
    pub path: String,
}

/// WSL UNC 每個檔案 ~23ms 純延遲，序列 70 檔要 1.6s；8 條並行實測降到 ~0.35s。
/// 再往上（16）沒有更快，只多佔連線。
const IO_THREADS: usize = 8;

/// 依序回傳結果的並行 map — 工作用 atomic index 分派（各檔耗時不均也不會有人閒著），
/// 結果寫回自己的 slot，所以輸出順序與輸入相同（清單維持路徑排序）。
fn par_map<T, R, F>(items: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(usize, &T) -> R + Sync,
{
    let slots: Vec<Mutex<Option<R>>> = items.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let threads = IO_THREADS.min(items.len());

    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let r = f(i, &items[i]);
                *slots[i].lock().unwrap() = Some(r);
            });
        }
    });

    slots
        .into_iter()
        .filter_map(|m| m.into_inner().unwrap())
        .collect()
}

fn emit_progress(app: &AppHandle, phase: &str, done: usize, total: usize, path: &str) {
    let _ = app.emit(
        "jin-progress",
        JinProgress {
            phase: phase.into(),
            done,
            total,
            path: path.into(),
        },
    );
}

/// 每個檔案的計畫 + 要寫入的新內容（skip/error 為 None）
fn plan_files(
    app: &AppHandle,
    roots: &[String],
    hall: &str,
    codes: &[String],
    comment_envs: &[String],
) -> Result<Vec<(JinFilePlan, Option<String>)>, String> {
    let files = collect_files(roots).map_err(|e| e.to_string())?;
    let total = files.len();
    let done = AtomicUsize::new(0);

    // 讀檔並行；純文字處理（detect/build）順便在同一條上做，反正瓶頸是 IO
    let out = par_map(&files, |_, path| {
        let env = extract_env(path).unwrap_or_default();
        let should_comment = comment_envs.contains(&env);

        let plan = match std::fs::read_to_string(path) {
            Err(e) => (
                JinFilePlan {
                    path: path.clone(),
                    env,
                    action: "error".into(),
                    message: format!("讀檔失敗：{}", e),
                    applied: false,
                },
                None,
            ),
            Ok(content) => {
                let (action, message, new_content) = match detect_state(&content, hall) {
                    FileState::Active => ("skip", "已啟用".to_string(), None),
                    FileState::Commented => {
                        if should_comment {
                            ("skip", "維持註解狀態".to_string(), None)
                        } else {
                            let c = uncomment_content(&content, hall, codes);
                            ("uncomment", "解除註解".to_string(), Some(c))
                        }
                    }
                    FileState::Missing => {
                        match build_new_content(&content, hall, codes, should_comment) {
                            Ok(c) => {
                                if should_comment {
                                    ("add_commented", "新增（註解狀態）".to_string(), Some(c))
                                } else {
                                    ("add", "新增".to_string(), Some(c))
                                }
                            }
                            Err(e) => ("error", e.to_string(), None),
                        }
                    }
                };
                (
                    JinFilePlan {
                        path: path.clone(),
                        env,
                        action: action.into(),
                        message,
                        applied: false,
                    },
                    new_content,
                )
            }
        };

        // 並行下「當前檔名」只是抽樣，完成數才是準的
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        emit_progress(app, "scan", n, total, path);
        plan
    });

    Ok(out)
}

/// roots 傳空 → 用 app_settings.json 的 jin_roots
fn resolve_roots(settings: &SettingsState, roots: Vec<String>) -> Vec<String> {
    let roots: Vec<String> = roots
        .into_iter()
        .filter(|r| !r.trim().is_empty())
        .collect();
    if roots.is_empty() {
        settings.get().jin_roots
    } else {
        roots
    }
}

fn normalize(hall: &str, suffixes: &[String]) -> Result<(String, Vec<String>), String> {
    let hall = hall.trim().to_uppercase();
    if hall.is_empty() {
        return Err("請輸入 HALL".into());
    }
    let codes = build_codes(&hall, suffixes);
    if codes.is_empty() {
        return Err("請至少輸入一個代碼後綴".into());
    }
    Ok((hall, codes))
}

#[tauri::command]
pub async fn jin_preview(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    roots: Vec<String>,
    hall: String,
    suffixes: Vec<String>,
    comment_envs: Vec<String>,
) -> Result<JinPreview, String> {
    let (hall, codes) = normalize(&hall, &suffixes)?;
    let roots = resolve_roots(&settings, roots);
    let envs: Vec<String> = comment_envs.iter().map(|e| e.trim().to_lowercase()).collect();

    let (h, c, r) = (hall.clone(), codes.clone(), roots.clone());
    let worker_app = app.clone();
    // 掃的是網路磁碟（WSL UNC），丟到 blocking 執行緒別卡住 runtime
    let result =
        tauri::async_runtime::spawn_blocking(move || plan_files(&worker_app, &r, &h, &c, &envs))
            .await
            .map_err(|e| e.to_string())?;
    // 不論成功失敗都收掉前端的進度條
    emit_progress(&app, "done", 0, 0, "");
    let files: Vec<JinFilePlan> = result?.into_iter().map(|(plan, _)| plan).collect();

    Ok(JinPreview {
        hall,
        codes,
        roots,
        files,
    })
}

#[tauri::command]
pub async fn jin_apply(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    roots: Vec<String>,
    hall: String,
    suffixes: Vec<String>,
    comment_envs: Vec<String>,
) -> Result<Vec<JinFilePlan>, String> {
    let (hall, codes) = normalize(&hall, &suffixes)?;
    let roots = resolve_roots(&settings, roots);
    let envs: Vec<String> = comment_envs.iter().map(|e| e.trim().to_lowercase()).collect();

    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let planned = plan_files(&worker_app, &roots, &hall, &codes, &envs)?;
        // 只有真的要寫的檔算進寫檔進度，skip 不佔分母
        let total = planned.iter().filter(|(_, c)| c.is_some()).count();
        let written = AtomicUsize::new(0);

        // 寫檔同樣是逐檔 round trip，並行寫（各檔互不相干，失敗只影響自己那筆）
        let results = par_map(&planned, |_, (plan, new_content)| {
            let mut plan = plan.clone();
            if let Some(content) = new_content {
                match std::fs::write(&plan.path, content) {
                    Ok(()) => plan.applied = true,
                    Err(e) => {
                        plan.action = "error".into();
                        plan.message = format!("寫檔失敗：{}", e);
                    }
                }
                let n = written.fetch_add(1, Ordering::Relaxed) + 1;
                emit_progress(&worker_app, "write", n, total, &plan.path);
            }
            plan
        });
        Ok::<_, String>(results)
    })
    .await
    .map_err(|e| e.to_string())?;
    emit_progress(&app, "done", 0, 0, "");
    result
}
