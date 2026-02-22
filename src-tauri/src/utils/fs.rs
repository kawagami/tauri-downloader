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
