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
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = storage::portable_data_dir().expect("无法定位程序目录");
    storage::configure_webview_data_dir(&data_dir).expect("无法创建便携缓存目录");
    update::cleanup_leftover(&data_dir);
    let store = AppStore::load(data_dir.clone()).expect("无法初始化本地配置");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            hotkey::show_launcher(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
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
        ])
        .setup(|app| {
            let open = MenuItem::with_id(app, "open", "打开启动面板", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &settings, &quit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| "缺少应用图标".to_string())?;
            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("Launch Manager")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => hotkey::show_launcher(app),
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        let _ = app.emit("show-view", "settings");
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            let store = app.state::<AppStore>();
            let settings = store.settings();
            if settings.autostart {
                let _ = autostart::set_enabled(true, store.data_dir());
            }
            if let Err(error) = hotkey::register(app.handle(), &settings.hotkey) {
                eprintln!("快捷键注册失败：{error}");
            }
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
