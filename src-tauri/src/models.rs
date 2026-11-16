use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSettings {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub root: String,
}

impl Default for WebDavSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            root: "rust-apps".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub scan_dir: String,
    pub hotkey: String,
    pub autostart: bool,
    pub hide_after_launch: bool,
    pub maximize_on_launch: bool,
    pub theme: String,
    #[serde(default)]
    pub desktop_double_click_icons: bool,
    #[serde(default)]
    pub webdav: WebDavSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scan_dir: String::new(),
            hotkey: "Ctrl+Shift+X".into(),
            autostart: false,
            hide_after_launch: true,
            maximize_on_launch: true,
            theme: "system".into(),
            desktop_double_click_icons: false,
            webdav: WebDavSettings::default(),
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
