//! Launch Manager —— 便携软件启动管理器。

mod autostart;
mod commands;
mod hotkey;
mod maximize;
mod models;
mod scanner;
mod storage;
mod update;

use storage::AppStore;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = storage::portable_data_dir().expect("无法定位程序目录");
    storage::configure_webview_data_dir(&data_dir).expect("无法创建便携缓存目录");
    let log_dir = portable_kit::logging::init("launch-manager").ok();
    portable_kit::logging::install_panic_hook();
    portable_kit::logging::info(&format!(
        "startup version={} exe={:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::current_exe().ok()
    ));
    update::cleanup_leftover(&data_dir);
    let store = AppStore::load(data_dir.clone()).expect("无法初始化本地配置");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            hotkey::show_launcher(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(store)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::scan_apps,
            commands::launch_app,
            commands::get_autostart,
            commands::get_app_info,
            commands::check_update,
            commands::prepare_update,
            commands::apply_update_and_restart,
            commands::show_view,
            commands::hide_window,
            commands::quit_app,
            commands::webdav_test,
            commands::webdav_backup_all,
            commands::webdav_restore_app,
            commands::export_app_data,
            commands::import_app_data,
        ])
        .setup(move |app| {
            let open = MenuItem::with_id(app, "open", "打开启动面板", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &settings, &quit])?;
            let icon =
                app.default_window_icon().cloned().ok_or_else(|| "缺少应用图标".to_string())?;
            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Launch Manager（左键切换面板，右键菜单）")
                .on_tray_icon_event(|app, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        hotkey::toggle_launcher(app.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => hotkey::show_launcher(app),
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        let _ = app.emit("show-view", "settings");
                    }
                    "quit" => commands::quit_app(app.clone(), app.state::<AppStore>()),
                    _ => {}
                })
                .build(app)?;

            let store = app.state::<AppStore>();
            let settings = store.settings();
            if settings.autostart {
                let _ = autostart::set_enabled(true, store.data_dir());
            }
            if let Err(error) = hotkey::register(app.handle(), &settings.hotkey) {
                portable_kit::logging::error(&format!("快捷键注册失败：{error}"));
                eprintln!("快捷键注册失败：{error}");
            }
            // 常驻定时任务：每小时清理过期日志 + （启用时）WebDAV 增量云备份
            let scan_dir = settings.scan_dir.clone();
            let wd = settings.webdav.clone();
            let own_log = log_dir.clone();
            std::thread::spawn(move || loop {
                let mut removed = 0usize;
                if let Some(dir) = &own_log {
                    removed += portable_kit::logging::cleanup_old_logs(dir, 7).unwrap_or(0);
                }
                if let Ok(entries) = std::fs::read_dir(&scan_dir) {
                    for entry in entries.flatten() {
                        let logs = entry.path().join("data").join("logs");
                        removed += portable_kit::logging::cleanup_old_logs(&logs, 7).unwrap_or(0);
                    }
                }
                if removed > 0 {
                    portable_kit::logging::info(&format!("hourly log cleanup removed={removed}"));
                }
                if wd.enabled && !wd.url.trim().is_empty() {
                    let cfg = portable_kit::webdav::WebDavConfig {
                        url: wd.url.clone(),
                        username: wd.username.clone(),
                        password: wd.password.clone(),
                        root: wd.root.clone(),
                    };
                    match commands::backup_all_with(&cfg, &scan_dir) {
                        Ok(reports) => portable_kit::logging::info(&format!(
                            "hourly webdav backup apps={}",
                            reports.len()
                        )),
                        Err(e) => portable_kit::logging::warn(&format!(
                            "hourly webdav backup failed: {e}"
                        )),
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(3600));
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
