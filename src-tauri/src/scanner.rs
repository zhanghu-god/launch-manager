//! 扫描本地便携软件目录，找出每个子目录的主 exe。

use std::fs;
use std::path::{Path, PathBuf};

use crate::models::AppEntry;

const EXCLUDED_KEYWORDS: [&str; 6] = [
    "uninstall",
    "unins",
    "updater",
    "update",
    "setup",
    "install",
];

pub fn scan_apps(scan_dir: &Path, self_dir: &Path) -> Result<Vec<AppEntry>, String> {
    if !scan_dir.exists() {
        return Err(format!("软件目录不存在：{}", scan_dir.display()));
    }
    let mut apps = Vec::new();
    let entries = fs::read_dir(scan_dir).map_err(|e| format!("读取软件目录失败：{e}"))?;
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        if same_dir(&dir, self_dir) {
            continue;
        }
        let Some(folder_name) = dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(exe) = find_main_exe(&dir, folder_name)? {
            apps.push(AppEntry {
                id: folder_name.to_lowercase(),
                name: folder_name.to_string(),
                exe_path: exe.to_string_lossy().into_owned(),
                dir_path: dir.to_string_lossy().into_owned(),
            });
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

fn find_main_exe(dir: &Path, folder_name: &str) -> Result<Option<PathBuf>, String> {
    let mut candidates = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("读取 {} 失败：{e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("exe") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let lower = stem.to_lowercase();
        if EXCLUDED_KEYWORDS.iter().any(|key| lower.contains(key)) {
            continue;
        }
        candidates.push(path);
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    let folder_lower = folder_name.to_lowercase();
    if let Some(preferred) = candidates
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) == Some(folder_lower.clone()))
    {
        return Ok(Some(preferred.clone()));
    }
    candidates.sort();
    Ok(candidates.into_iter().next())
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("lm-scan-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"exe").unwrap();
    }

    #[test]
    fn scans_folders_and_prefers_matching_exe() {
        let root = TestDir::new();
        let self_dir = root.0.join("launch-manager");
        touch(&self_dir.join("launch-manager.exe"));
        touch(&root.0.join("DDL Board").join("ddl-desktop.exe"));
        touch(&root.0.join("push-wp").join("push-wp.exe"));
        touch(&root.0.join("Tool").join("Tool.exe"));
        touch(&root.0.join("Tool").join("uninstall.exe"));
        touch(&root.0.join("Empty").join("readme.txt"));

        let apps = scan_apps(&root.0, &self_dir).unwrap();
        let names: Vec<&str> = apps.iter().map(|app| app.name.as_str()).collect();
        assert_eq!(names, vec!["DDL Board", "push-wp", "Tool"]);
        assert!(apps.iter().all(|app| !app.exe_path.contains("uninstall")));
    }
}
