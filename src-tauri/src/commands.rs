use std::path::Path;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::models::{AppEntry, AppInfo, Settings};
use crate::storage::AppStore;
use crate::update::{self, PreparedUpdate, UpdateInfo};
use crate::{autostart, hotkey, maximize, scanner};
use portable_kit::dataio::{self, ImportReport};
use portable_kit::webdav::{self, BackupReport, RestoreReport, WebDavConfig};

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
    portable_kit::logging::info(&format!(
        "settings saved hotkey={} scan_dir={} autostart={}",
        saved.hotkey, saved.scan_dir, saved.autostart
    ));
    Ok(saved)
}

#[tauri::command]
pub fn scan_apps(store: State<'_, AppStore>) -> Result<Vec<AppEntry>, String> {
    let settings = store.settings();
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径：{e}"))?;
    let self_dir = exe.parent().ok_or_else(|| "程序路径没有父目录".to_string())?;
    let apps = scanner::scan_apps(Path::new(&settings.scan_dir), self_dir);
    match &apps {
        Ok(list) => portable_kit::logging::info(&format!(
            "scan dir={} found={}",
            settings.scan_dir,
            list.len()
        )),
        Err(e) => portable_kit::logging::error(&format!("scan failed: {e}")),
    }
    apps
}

#[tauri::command]
pub fn launch_app(exe_path: String, maximize: bool) -> Result<(), String> {
    let path = Path::new(&exe_path);
    if !path.is_file() {
        return Err("程序文件不存在".into());
    }
    let dir = path.parent().ok_or_else(|| "程序路径没有父目录".to_string())?;
    let mut command = std::process::Command::new(path);
    command.current_dir(dir);
    // 脱离管理器进程树/作业对象：叉掉或退出管理器不会连带关闭已启动的程序。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        command.creation_flags(
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB,
        );
    }
    let child = command.spawn().map_err(|e| format!("启动失败：{e}"))?;
    portable_kit::logging::info(&format!(
        "launch exe={} pid={} maximize={}",
        exe_path,
        child.id(),
        maximize
    ));
    if maximize {
        maximize::maximize_process_window(child.id());
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo { version: update::current_version().to_string(), repo: update::REPO.to_string() }
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
pub fn quit_app(app: AppHandle, store: State<'_, AppStore>) {
    // 退出前 best-effort 云备份，不阻塞退出
    if let Ok(cfg) = webdav_cfg(&store) {
        let settings = store.settings();
        let _ = backup_all_with(&cfg, &settings.scan_dir);
    }
    app.exit(0);
}

// ---------- WebDAV 云备份 / 数据导入导出 ----------

/// 远端同步键 = 主 exe 主名（小写），与各 app 自带同步的固定键一致，
/// 保证管理器备份与 app 自身同步写入同一远端目录，云盘只留一份数据。
fn exe_key(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("app").to_lowercase()
}

fn data_of(dir_path: &str) -> Result<std::path::PathBuf, String> {
    Ok(Path::new(dir_path).join("data"))
}

/// 管理器自身的目录与名称（扫描器排除自身，需单独纳入备份/导入导出）。
fn self_dir_and_name() -> Result<(String, std::path::PathBuf), String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径：{e}"))?;
    let dir = exe.parent().ok_or_else(|| "程序路径没有父目录".to_string())?;
    let name = dir
        .file_name()
        .ok_or_else(|| "无法识别自身目录名".to_string())?
        .to_string_lossy()
        .into_owned();
    Ok((name, dir.to_path_buf()))
}

fn webdav_cfg(store: &AppStore) -> Result<WebDavConfig, String> {
    let w = store.settings().webdav;
    if !w.enabled {
        return Err("未启用 WebDAV 云备份".into());
    }
    if w.url.trim().is_empty() {
        return Err("未配置 WebDAV 地址".into());
    }
    Ok(WebDavConfig { url: w.url, username: w.username, password: w.password, root: w.root })
}

/// 扫描所有 app 并增量备份其 data 目录（供命令/退出/定时线程复用）。
pub fn backup_all_with(cfg: &WebDavConfig, scan_dir: &str) -> Result<Vec<BackupReport>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径：{e}"))?;
    let self_dir = exe.parent().ok_or_else(|| "程序路径没有父目录".to_string())?;
    let apps = scanner::scan_apps(Path::new(scan_dir), self_dir)?;
    let mut reports = Vec::new();
    // 管理器自身配置也需同步（键=自身 exe 主名）
    let (_self_name, self_dir) = self_dir_and_name()?;
    let self_data = self_dir.join("data");
    if self_data.exists() {
        let r = webdav::backup_app(cfg, &exe_key(&exe), &self_data)?;
        portable_kit::logging::info(&format!(
            "webdav backup app={} uploaded={} skipped={} removed={}",
            r.app, r.uploaded, r.skipped, r.removed
        ));
        reports.push(r);
    }
    for app in apps {
        let data = Path::new(&app.dir_path).join("data");
        if data.exists() {
            let r = webdav::backup_app(cfg, &exe_key(Path::new(&app.exe_path)), &data)?;
            portable_kit::logging::info(&format!(
                "webdav backup app={} uploaded={} skipped={} removed={}",
                r.app, r.uploaded, r.skipped, r.removed
            ));
            reports.push(r);
        }
    }
    Ok(reports)
}

#[tauri::command]
pub fn list_data_apps(store: State<'_, AppStore>) -> Result<Vec<AppEntry>, String> {
    let settings = store.settings();
    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径：{e}"))?;
    let self_dir = exe.parent().ok_or_else(|| "程序路径没有父目录".to_string())?;
    let mut apps = scanner::scan_apps(Path::new(&settings.scan_dir), self_dir)?;
    let (name, dir) = self_dir_and_name()?;
    apps.insert(
        0,
        AppEntry {
            id: "self".into(),
            name,
            exe_path: exe.to_string_lossy().into_owned(),
            dir_path: dir.to_string_lossy().into_owned(),
        },
    );
    Ok(apps)
}

#[tauri::command]
pub fn webdav_test(store: State<'_, AppStore>) -> Result<String, String> {
    let cfg = webdav_cfg(&store)?;
    webdav::test_connection(&cfg)?;
    portable_kit::logging::info("webdav connection ok");
    Ok("连接成功".into())
}

#[tauri::command]
pub fn webdav_backup_all(store: State<'_, AppStore>) -> Result<Vec<BackupReport>, String> {
    let cfg = webdav_cfg(&store)?;
    let settings = store.settings();
    backup_all_with(&cfg, &settings.scan_dir)
}

#[tauri::command]
pub fn webdav_restore_app(
    store: State<'_, AppStore>,
    app_key: String,
    dir_path: String,
) -> Result<RestoreReport, String> {
    let cfg = webdav_cfg(&store)?;
    let data = data_of(&dir_path)?;
    let r = webdav::restore_app(&cfg, &app_key, &data)?;
    portable_kit::logging::info(&format!(
        "webdav restore app={} downloaded={}",
        r.app, r.downloaded
    ));
    Ok(r)
}

#[tauri::command]
pub fn export_app_data(app_key: String, dir_path: String, path: String) -> Result<(), String> {
    let data = data_of(&dir_path)?;
    let value = dataio::export_app(&app_key, &data)?;
    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("写入导出文件失败：{e}"))?;
    portable_kit::logging::info(&format!("export app={} to={}", app_key, path));
    Ok(())
}

#[tauri::command]
pub fn import_app_data(app_key: String, dir_path: String, path: String) -> Result<ImportReport, String> {
    let data = data_of(&dir_path)?;
    let value = dataio::read_envelope(Path::new(&path))?;
    let r = dataio::import_app(&value, &app_key, &data)?;
    portable_kit::logging::info(&format!("import app={} written={}", app_key, r.written));
    Ok(r)
}
