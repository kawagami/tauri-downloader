use regex::Regex;
use sanitize_filename::sanitize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// 網站下載的存檔副檔名（wnacg 是 zip 打包）
const ARCHIVE_EXT: &str = "zip";
/// 下載中的暫存副檔名 — 完成才 rename 成正式檔名
const PART_SUFFIX: &str = ".part";

/// 標題 → 檔名基底。命名規則的唯一來源：`{base}.zip`、碰撞時 `{base}_N.zip`。
fn base_name(title: &str) -> String {
    sanitize(title)
}

pub fn get_unique_save_path(dir: PathBuf, title: &str) -> PathBuf {
    let base_name = base_name(title);
    let mut path = dir.join(format!("{}.{}", base_name, ARCHIVE_EXT));
    let mut counter = 1;

    while path.exists() {
        path.set_file_name(format!("{}_{}.{}", base_name, counter, ARCHIVE_EXT));
        counter += 1;
    }
    path
}

/// 下載中的暫存路徑（`{save_path}{suffix}`）。整段附加在原檔名後，
/// 標題本身含點號也不會被 `with_extension` 吃掉。
pub fn part_path(save_path: &Path, suffix: &str) -> PathBuf {
    let mut name = save_path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("download"));
    name.push(suffix);
    save_path.with_file_name(name)
}

/// FNV-1a —— 只要「同一個 URL 每次都算出同一個值」，跨程序、跨版本都不能變。
/// `DefaultHasher` 不行：std 明說它的輸出不保證跨版本穩定，而這個值會寫進
/// 檔名，變了就等於續傳進度整個消失。
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 網站下載的 `.part` 後綴：`.{url 雜湊}.part`。
///
/// 光用 `{title}.part` 會出事：`get_unique_save_path` 只跟已完成的 `.zip` 比對、
/// 刻意不認 `.part`，所以「同標題、不同 URL」的兩個任務會算出同一個 `.part`。
/// 前一個任務暫停留下的半截檔，會被後一個任務當成自己的續傳起點接下去寫，
/// 最後 rename 成一個兩部作品拼起來、大小卻剛好對得上的壞檔 —— 全程無錯誤訊息。
/// 直鏈那邊摻的是 task id，web 沒有 id 可用，就拿來源 URL 當識別。
pub fn web_part_suffix(url: &str) -> String {
    format!(".{:016x}{}", fnv1a(url), PART_SUFFIX)
}

/// 舊版（無雜湊）的 `{name}.part` 搬成新命名，續傳進度不丟。
/// 與 http_dl 載入時做的 `.part` 遷移同一個用意。
pub fn migrate_legacy_part(save_path: &Path, suffix: &str) {
    let old = part_path(save_path, PART_SUFFIX);
    let new = part_path(save_path, suffix);
    if old != new && old.exists() && !new.exists() {
        match std::fs::rename(&old, &new) {
            Ok(()) => tracing::info!("舊 .part 已改名: {:?} -> {:?}", old, new),
            Err(e) => tracing::warn!("舊 .part 改名失敗 {:?}: {}", old, e),
        }
    }
}

/// 這個標題是否已經下載完成過（`{base}.zip` 或 `{base}_N.zip`）。
/// 與 `get_unique_save_path` 綁在同一套命名規則上，改一處兩邊同步 —
/// monitor 的「已存在就略過」與實際存檔不會漂掉。
/// 下載中的 `.part` 不算數，所以半途中斷不會讓該作品永遠被略過。
pub fn already_downloaded(dir: &Path, title: &str) -> bool {
    let base = base_name(title);
    let exact = format!("{}.{}", base, ARCHIVE_EXT);
    // 精確比對 {base}_N.zip，避免誤擋標題互為前綴的不同作品
    let numbered = Regex::new(&format!(
        r"^{}_\d+\.{}$",
        regex::escape(&base),
        regex::escape(ARCHIVE_EXT)
    ))
    .ok();

    std::fs::read_dir(dir)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name == exact.as_str() || numbered.as_ref().is_some_and(|re| re.is_match(&name))
            })
        })
        .unwrap_or(false)
}

/// 設定裡的下載目錄 → 實際路徑。三個分頁共用同一套語意：
/// 空字串 = 系統下載資料夾，取不到時退回工作目錄。
pub fn resolve_dir(configured: &str) -> PathBuf {
    let trimmed = configured.trim();
    if !trimmed.is_empty() {
        return PathBuf::from(trimmed);
    }
    dirs::download_dir().unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_dir_uses_configured_path() {
        assert_eq!(resolve_dir(r"C:\tmp"), PathBuf::from(r"C:\tmp"));
        assert_eq!(resolve_dir("  C:\\tmp  "), PathBuf::from(r"C:\tmp"));
    }

    #[test]
    fn resolve_dir_falls_back_when_empty() {
        assert_eq!(resolve_dir("   "), resolve_dir(""));
        assert!(!resolve_dir("").as_os_str().is_empty());
    }

    #[test]
    fn part_path_appends_suffix() {
        assert_eq!(
            part_path(Path::new(r"C:\dl\a.b.zip"), ".part"),
            PathBuf::from(r"C:\dl\a.b.zip.part")
        );
    }

    /// 命名規則的兩端要對齊：get_unique_save_path 產出的檔名，
    /// already_downloaded 必須認得（含 _N 變體），.part 則不能認。
    #[test]
    fn already_downloaded_matches_save_path_naming() {
        let dir = std::env::temp_dir().join(format!("dl-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let title = "某作品 [tag]";

        assert!(!already_downloaded(&dir, title));

        // 下載中的 .part 不算完成
        let first = get_unique_save_path(dir.clone(), title);
        std::fs::write(part_path(&first, ".part"), b"x").unwrap();
        assert!(!already_downloaded(&dir, title));

        std::fs::write(&first, b"x").unwrap();
        assert!(already_downloaded(&dir, title));

        // 第二次會拿到 _1 變體，同樣要被認出來
        let second = get_unique_save_path(dir.clone(), title);
        assert_ne!(first, second);
        std::fs::remove_file(&first).unwrap();
        std::fs::write(&second, b"x").unwrap();
        assert!(already_downloaded(&dir, title));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 同標題不同 URL 必須拿到不同的 `.part`，否則後一個任務會把前一個
    /// 暫停留下的半截檔當成自己的續傳起點，拼出一個大小對得上的壞檔
    #[test]
    fn web_part_suffix_separates_same_title_tasks() {
        let a = web_part_suffix("https://www.wnacg.com/photos-index-aid-1.html");
        let b = web_part_suffix("https://www.wnacg.com/photos-index-aid-2.html");
        assert_ne!(a, b);
        assert!(a.ends_with(".part"));
        // 同一個 URL 每次都要算出同一個後綴，不然重開 app 就接不回進度
        assert_eq!(a, web_part_suffix("https://www.wnacg.com/photos-index-aid-1.html"));
        assert_ne!(
            part_path(Path::new(r"C:\dl\same.zip"), &a),
            part_path(Path::new(r"C:\dl\same.zip"), &b)
        );
    }

    /// 舊版的 `{name}.part` 要能搬到新命名，續傳進度不丟
    #[test]
    fn legacy_part_is_migrated_to_hashed_name() {
        let dir = std::env::temp_dir().join(format!("dl-migrate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let save = dir.join("a.zip");
        let suffix = web_part_suffix("https://example.com/a");

        std::fs::write(part_path(&save, ".part"), b"half").unwrap();
        migrate_legacy_part(&save, &suffix);
        assert!(!part_path(&save, ".part").exists());
        assert_eq!(std::fs::read(part_path(&save, &suffix)).unwrap(), b"half");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 標題互為前綴的不同作品不能互相誤擋
    #[test]
    fn already_downloaded_ignores_prefix_collision() {
        let dir = std::env::temp_dir().join(format!("dl-prefix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("abc_extra.zip"), b"x").unwrap();
        assert!(!already_downloaded(&dir, "abc"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
