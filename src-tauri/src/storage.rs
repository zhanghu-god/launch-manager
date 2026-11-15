//! 便携配置存储：所有配置位于 exe 同级 data 目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::models::Settings;

// 便携数据隔离复用公共 crate，避免各项目重复实现。
pub use portable_kit::portable::{configure_webview_data_dir, portable_data_dir};

/// 默认扫描 exe 所在目录的上一级；部署在 launch/launch-manager 时即 launch 目录。
pub fn default_scan_dir() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(launch_dir) = exe.parent().and_then(|dir| dir.parent()) {
            return launch_dir.to_string_lossy().into_owned();
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return Path::new(&profile).join("launch").to_string_lossy().into_owned();
    }
    String::from(".")
}

pub struct AppStore {
    data_dir: PathBuf,
    settings_path: PathBuf,
    settings: Mutex<Settings>,
}

impl AppStore {
    pub fn load(data_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&data_dir).map_err(|e| format!("无法创建数据目录：{e}"))?;
        let settings_path = data_dir.join("config.json");
        let existed = settings_path.exists();
        let mut settings: Settings = read_json_or_default(&settings_path)?;
        if settings.scan_dir.trim().is_empty() {
            settings.scan_dir = default_scan_dir();
        }
        // 旧默认热键迁移到新默认（用户自定义过的不覆盖）
        let mut dirty = !existed;
        if settings.hotkey == "Ctrl+Alt+Space" {
            settings.hotkey = "Ctrl+Shift+X".into();
            dirty = true;
        }
        validate_settings(&settings)?;
        if dirty {
            write_json(&settings_path, &settings)?;
        }
        Ok(Self { data_dir, settings_path, settings: Mutex::new(settings) })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    pub fn update_settings(&self, settings: Settings) -> Result<Settings, String> {
        validate_settings(&settings)?;
        write_json(&self.settings_path, &settings)?;
        *self.settings.lock().map_err(|_| "配置数据锁已损坏".to_string())? = settings.clone();
        Ok(settings)
    }
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if settings.scan_dir.trim().is_empty() {
        return Err("软件目录不能为空".into());
    }
    if settings.hotkey.trim().is_empty() {
        return Err("快捷键不能为空".into());
    }
    if !matches!(settings.theme.as_str(), "system" | "light" | "dark") {
        return Err("无效的主题设置".into());
    }
    Ok(())
}

fn read_json_or_default<T>(path: &Path) -> Result<T, String>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let text =
        fs::read_to_string(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} 格式错误：{e}", path.display()))
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: Serialize + ?Sized,
{
    let text = serde_json::to_string_pretty(value).map_err(|e| format!("序列化配置失败：{e}"))?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, text).map_err(|e| format!("写入 {} 失败：{e}", temp_path.display()))?;
    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(first_error) if path.exists() => {
            fs::remove_file(path).map_err(|e| format!("替换 {} 失败：{e}", path.display()))?;
            fs::rename(&temp_path, path).map_err(|e| {
                format!("替换 {} 失败：{e}（首次错误：{first_error}）", path.display())
            })
        }
        Err(e) => Err(format!("保存 {} 失败：{e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_and_persist() {
        let dir = std::env::temp_dir().join(format!("lm-store-{}", uuid::Uuid::new_v4()));
        let store = AppStore::load(dir.clone()).expect("加载配置");
        assert!(!store.settings().scan_dir.is_empty());
        assert_eq!(store.settings().hotkey, "Ctrl+Shift+X");
        let mut settings = store.settings();
        settings.theme = "dark".into();
        store.update_settings(settings.clone()).expect("保存配置");
        let reloaded = AppStore::load(dir.clone()).expect("重新加载");
        assert_eq!(reloaded.settings(), settings);
        let _ = fs::remove_dir_all(dir);
    }
}
