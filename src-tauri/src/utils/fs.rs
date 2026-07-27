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

/// 下載中的暫存路徑（`{save_path}.part`）。整段附加在原檔名後，
/// 標題本身含點號也不會被 `with_extension` 吃掉。
pub fn part_path(save_path: &Path) -> PathBuf {
    let mut name = save_path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("download"));
    name.push(PART_SUFFIX);
    save_path.with_file_name(name)
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
            part_path(Path::new(r"C:\dl\a.b.zip")),
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
        std::fs::write(part_path(&first), b"x").unwrap();
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
