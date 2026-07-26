use sanitize_filename::sanitize;
use std::path::PathBuf;

pub fn get_unique_save_path(dir: PathBuf, title: &str) -> PathBuf {
    let base_name = sanitize(title);
    let mut path = dir.join(format!("{}.zip", base_name));
    let mut counter = 1;

    while path.exists() {
        path.set_file_name(format!("{}_{}.zip", base_name, counter));
        counter += 1;
    }
    path
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
}
