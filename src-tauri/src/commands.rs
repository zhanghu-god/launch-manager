use std::path::Path;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::models::{AppEntry, AppInfo, PreparedUpdate, Settings, UpdateInfo};
use crate::storage::AppStore;
use crate::{autostart, hotkey, maximize, scanner, update};

#[tauri::command]
pub fn get_settings(store: State<'_, AppStore>) -> Settings {
    store.settings()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    settings: Settings,
    store: State<'_, AppStore>,
) -> Result<Settings, String> {
    let old = store.settings();
    hotkey::register(&app, &settings.hotkey)?;
    let saved = store.update_settings(settings.clone())?;
    if saved.autostart != old.autostart || saved.autostart {
        autostart::set_enabled(saved.autostart, store.data_dir())?;
    }
    Ok(saved)
}

#[tauri::command]
pub fn scan_apps(store: State<'_, AppStore>) -> Result<Vec<AppEntry>, String> {
    let settings = store.settings();
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径：{e}"))?;
    let self_dir = exe
        .parent()
        .ok_or_else(|| "程序路径没有父目录".to_string())?;
    scanner::scan_apps(Path::new(&settings.scan_dir), self_dir)
}

#[tauri::command]
pub fn launch_app(exe_path: String, maximize: bool) -> Result<(), String> {
    let path = Path::new(&exe_path);
    if !path.is_file() {
        return Err("程序文件不存在".into());
    }
    let dir = path
        .parent()
        .ok_or_else(|| "程序路径没有父目录".to_string())?;
    let child = std::process::Command::new(path)
        .current_dir(dir)
        .spawn()
        .map_err(|e| format!("启动失败：{e}"))?;
    if maximize {
        maximize::maximize_process_window(child.id());
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        version: update::current_version().to_string(),
        repo: update::REPO.to_string(),
    }
}

#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    update::check_update().await
}

#[tauri::command]
pub async fn prepare_update(
    info: UpdateInfo,
    store: State<'_, AppStore>,
) -> Result<PreparedUpdate, String> {
    let data_dir = store.data_dir().to_path_buf();
    update::prepare_update(&data_dir, &info).await
}

#[tauri::command]
pub fn apply_update_and_restart(app: AppHandle, store: State<'_, AppStore>) -> Result<(), String> {
    update::apply_update_and_restart(store.data_dir())?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn get_autostart() -> Result<bool, String> {
    autostart::is_enabled()
}

#[tauri::command]
pub fn show_view(app: AppHandle, view: String) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit("show-view", view);
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
