use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub scan_dir: String,
    pub hotkey: String,
    pub autostart: bool,
    pub hide_after_launch: bool,
    pub maximize_on_launch: bool,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scan_dir: String::new(),
            hotkey: "Ctrl+Alt+Space".into(),
            autostart: false,
            hide_after_launch: true,
            maximize_on_launch: true,
            theme: "system".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub exe_path: String,
    pub dir_path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub repo: String,
}
