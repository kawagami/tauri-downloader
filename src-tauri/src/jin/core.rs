// src/jin/core.rs
// 自 tools/src/bin/jin_game_add.rs + tools/src/lib.rs 移植的純邏輯（無 IPC、無 UI）。
// 職責：掃 code.*.php、判斷現況（已啟用/已註解/未加入）、產生新內容。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::{bail, Result};

/// 目錄走訪的並行度（和 commands.rs 的 IO_THREADS 同一個理由：9P 延遲綁定）
const WALK_THREADS: usize = 8;

/// 找到 `key` 區段，把 `new_entries` 插在區段結尾 `    ],`（4 空格縮排）之前。
/// 同時支援 `'Key' => [` 與對齊過的 `'Key'   => [`。
pub fn insert_before_section_end(content: &str, key: &str, new_entries: &str) -> Result<String> {
    let key_pat = format!("    '{}'", key);
    let start = content
        .find(&key_pat)
        .ok_or_else(|| anyhow::anyhow!("Section not found: '{}'", key))?;

    let after_key = start + key_pat.len();
    let bracket_rel = content[after_key..]
        .find('[')
        .ok_or_else(|| anyhow::anyhow!("Cannot find '[' for section: '{}'", key))?;
    let after_bracket = after_key + bracket_rel + 1;

    let closing = "\n    ],";
    let rel_end = content[after_bracket..]
        .find(closing)
        .ok_or_else(|| anyhow::anyhow!("Cannot find closing of: '{}'", key))?;

    // +1 跳過 \n，讓 content[..insert_pos] 以 \n 結尾
    let insert_pos = after_bracket + rel_end + 1;

    let prefix = &content[..insert_pos];
    let last_nonws = prefix.trim_end();
    let needs_comma = !last_nonws.ends_with(',');

    let mut result = String::with_capacity(content.len() + new_entries.len() + 2);
    if needs_comma {
        result.push_str(last_nonws);
        result.push_str(",\n");
    } else {
        result.push_str(prefix);
    }
    result.push_str(new_entries);
    result.push_str(&content[insert_pos..]);

    Ok(result)
}

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Active,
    Commented,
    Missing,
}

pub fn detect_state(content: &str, hall: &str) -> State {
    let active_pat = format!("\n        '{}'", hall);
    let comment_pat = format!("//        '{}'", hall);
    if content.contains(&active_pat) {
        State::Active
    } else if content.contains(&comment_pat) {
        State::Commented
    } else {
        State::Missing
    }
}

/// k8s overlays：env 取 `overlays/<env>/` 那層（local/ 底下檔名仍是 code.dev.php，
/// 用檔名會判錯）。compose 佈局沒有 overlays 目錄，退回檔名 `code.<env>.php`。
pub fn extract_env(file_path: &str) -> Option<String> {
    let path = Path::new(file_path);
    let parts: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    if let Some(i) = parts.iter().position(|c| c == "overlays") {
        if let Some(env) = parts.get(i + 1) {
            return Some(env.clone());
        }
    }

    let name = path.file_name()?.to_str()?;
    let without_ext = name.strip_suffix(".php")?;
    Some(without_ext.strip_prefix("code.")?.to_lowercase())
}

fn is_code_php(p: &Path) -> bool {
    let name = p.file_name().unwrap_or_default().to_string_lossy();
    name.starts_with("code.") && name.ends_with(".php")
}

/// 共用佇列的並行目錄走訪 — WSL UNC 每個 read_dir 都是一次 9P round trip（371 個目錄
/// 序列走要 1.7 秒），多條一起撈。用 `entry.file_type()` 而非 `path.is_dir()`：
/// 前者讀目錄項目自帶的資訊，後者每個項目要再一次 stat。
fn walk_code_php_parallel(start: Vec<PathBuf>, threads: usize) -> Result<Vec<String>> {
    let queue = Mutex::new(start);
    let active = AtomicUsize::new(0);
    let out: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let first_err: Mutex<Option<String>> = Mutex::new(None);

    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let dir = queue.lock().unwrap().pop();
                let Some(dir) = dir else {
                    // 佇列空但還有人在讀目錄 → 可能馬上有新子目錄進來
                    if active.load(Ordering::SeqCst) == 0 {
                        break;
                    }
                    std::thread::yield_now();
                    continue;
                };

                active.fetch_add(1, Ordering::SeqCst);
                match std::fs::read_dir(&dir) {
                    Ok(entries) => {
                        let mut subdirs = Vec::new();
                        let mut found = Vec::new();
                        for e in entries.filter_map(|e| e.ok()) {
                            match e.file_type() {
                                Ok(ft) if ft.is_dir() => subdirs.push(e.path()),
                                Ok(_) => {
                                    let p = e.path();
                                    if is_code_php(&p) {
                                        found.push(p.to_string_lossy().into_owned());
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        if !subdirs.is_empty() {
                            queue.lock().unwrap().extend(subdirs);
                        }
                        if !found.is_empty() {
                            out.lock().unwrap().extend(found);
                        }
                    }
                    Err(e) => {
                        let mut fe = first_err.lock().unwrap();
                        if fe.is_none() {
                            *fe = Some(format!("{}：{}", dir.display(), e));
                        }
                    }
                }
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });

    if let Some(e) = first_err.into_inner().unwrap() {
        bail!("讀取目錄失敗 {}", e);
    }
    Ok(out.into_inner().unwrap())
}

/// 掃多個根目錄底下所有 code.*.php；單一檔案路徑直接收下。
/// 根目錄不存在只警告不中斷（WSL 路徑可能沒掛上）。
pub fn collect_files(roots: &[String]) -> Result<Vec<String>> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for root in roots.iter().filter(|r| !r.trim().is_empty()) {
        let p = Path::new(root.trim());
        if p.is_dir() {
            dirs.push(p.to_path_buf());
        } else if p.is_file() {
            files.push(p.to_string_lossy().into_owned());
        } else {
            skipped.push(root.clone());
        }
    }

    if !dirs.is_empty() {
        // 各根目錄共用同一個佇列，樹的大小不均也不會有執行緒閒著
        files.extend(walk_code_php_parallel(dirs, WALK_THREADS)?);
    }

    if files.is_empty() {
        if skipped.is_empty() {
            bail!("找不到任何 code.*.php");
        }
        bail!("找不到任何 code.*.php（無法存取：{}）", skipped.join("、"));
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// hall/correspond/type/code 四種區塊文字
pub fn build_blocks(hall: &str, codes: &[String]) -> (String, String, String, String) {
    let hall_block = {
        let code_lines: String = codes
            .iter()
            .map(|c| format!("            '{}',\n", c))
            .collect();
        format!("        '{}' => [\n{}        ],\n", hall, code_lines)
    };
    let correspond_block: String = codes
        .iter()
        .map(|c| format!("        '{}' => '{}',\n", c, hall))
        .collect();
    let type_block = format!("        '{}',\n", hall);
    let code_block: String = codes
        .iter()
        .map(|c| format!("        '{}',\n", c))
        .collect();
    (hall_block, correspond_block, type_block, code_block)
}

pub fn comment_block(entries: &str) -> String {
    entries
        .lines()
        .map(|line| format!("//{}", line))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn build_new_content(
    content: &str,
    hall: &str,
    codes: &[String],
    commented: bool,
) -> Result<String> {
    let (hall_block, correspond_block, type_block, code_block) = build_blocks(hall, codes);

    let (hb, cb, tb, gb) = if commented {
        (
            comment_block(&hall_block),
            comment_block(&correspond_block),
            comment_block(&type_block),
            comment_block(&code_block),
        )
    } else {
        (hall_block, correspond_block, type_block, code_block)
    };

    let mut result = content.to_string();
    result = insert_before_section_end(&result, "Game", &hb)?;
    result = insert_before_section_end(&result, "GameCorrespond", &cb)?;
    result = insert_before_section_end(&result, "GameType", &tb)?;
    result = insert_before_section_end(&result, "GameCode", &gb)?;
    result = insert_before_section_end(&result, "RebateGameCode", &gb)?;
    result = insert_before_section_end(&result, "RebateGame", &hb)?;

    Ok(result)
}

pub fn uncomment_content(content: &str, hall: &str, codes: &[String]) -> String {
    let (hall_block, correspond_block, type_block, code_block) = build_blocks(hall, codes);
    let mut result = content.to_string();
    // replace 同時涵蓋 Game+RebateGame（hall_block）與 GameCode+RebateGameCode（code_block）
    result = result.replace(&comment_block(&hall_block), &hall_block);
    result = result.replace(&comment_block(&correspond_block), &correspond_block);
    result = result.replace(&comment_block(&type_block), &type_block);
    result = result.replace(&comment_block(&code_block), &code_block);
    result
}

/// 把使用者輸入的 hall + suffix 清單轉成完整 code（`HALL_SUFFIX`，全大寫）
pub fn build_codes(hall: &str, suffixes: &[String]) -> Vec<String> {
    suffixes
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("{}_{}", hall, s.to_uppercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<?php\nreturn [\n    'Game' => [\n        'AAA' => [\n            'AAA_E',\n        ],\n    ],\n    'GameCorrespond' => [\n        'AAA_E' => 'AAA',\n    ],\n    'GameType' => [\n        'AAA',\n    ],\n    'GameCode' => [\n        'AAA_E',\n    ],\n    'RebateGameCode' => [\n        'AAA_E',\n    ],\n    'RebateGame' => [\n        'AAA' => [\n            'AAA_E',\n        ],\n    ],\n];\n";

    #[test]
    fn insert_then_detect_active() {
        let codes = build_codes("BBB", &["e".into(), "f".into()]);
        assert_eq!(codes, vec!["BBB_E", "BBB_F"]);
        let out = build_new_content(SAMPLE, "BBB", &codes, false).unwrap();
        assert_eq!(detect_state(&out, "BBB"), State::Active);
        assert!(out.contains("        'BBB_E' => 'BBB',\n"));
    }

    #[test]
    fn commented_then_uncomment_roundtrip() {
        let codes = build_codes("BBB", &["e".into()]);
        let commented = build_new_content(SAMPLE, "BBB", &codes, true).unwrap();
        assert_eq!(detect_state(&commented, "BBB"), State::Commented);
        let restored = uncomment_content(&commented, "BBB", &codes);
        assert_eq!(detect_state(&restored, "BBB"), State::Active);
        assert_eq!(restored, build_new_content(SAMPLE, "BBB", &codes, false).unwrap());
    }

    #[test]
    fn missing_section_is_error() {
        let err = build_new_content("<?php\nreturn [\n];\n", "BBB", &["BBB_E".into()], false);
        assert!(err.is_err());
    }

    /// 並行走訪要撈齊巢狀目錄的檔案，且輸出經排序後與序列版一致
    #[test]
    fn parallel_walk_finds_nested_files() {
        let base = std::env::temp_dir().join(format!("jin_walk_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mut expected = Vec::new();
        for i in 0..4 {
            let dir = base.join(format!("env{}", i)).join("nested");
            std::fs::create_dir_all(&dir).unwrap();
            for name in ["code.dev.php", "code.prod.php", "other.php"] {
                let p = dir.join(name);
                std::fs::write(&p, "x").unwrap();
                if name.starts_with("code.") {
                    expected.push(p.to_string_lossy().into_owned());
                }
            }
        }
        expected.sort();

        let mut got = collect_files(&[base.to_string_lossy().into_owned()]).unwrap();
        got.sort();
        assert_eq!(got, expected);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn env_from_overlays_beats_filename() {
        assert_eq!(
            extract_env(r"C:\x\kustomize\overlays\prod\local\code.dev.php").as_deref(),
            Some("prod")
        );
        assert_eq!(
            extract_env(r"C:\x\sites\code.staging.php").as_deref(),
            Some("staging")
        );
    }
}
