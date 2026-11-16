//! 全局快捷键注册与唤出启动面板。

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::desktop_icons;

pub fn show_launcher(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit("show-view", "launcher");
}

/// 托盘左键 / 快捷键切换：可见则隐藏，隐藏则唤出并重扫。
pub fn toggle_launcher(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            return;
        }
    }
    show_launcher(app);
}

pub fn register(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcut: Shortcut =
        hotkey.trim().parse().map_err(|e| format!("无法解析快捷键 {hotkey}：{e}"))?;
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| format!("清理旧快捷键失败：{e}"))?;
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                show_launcher(app);
            }
        })
        .map_err(|e| format!("注册快捷键失败：{e}"))?;
    portable_kit::logging::info(&format!("global hotkey registered: {hotkey}"));

    // 桌面图标切换快捷键（失败只记录，不影响主快捷键）。
    if let Ok(icons_shortcut) = "Ctrl+Alt+I".parse::<Shortcut>() {
        if let Err(error) = manager.on_shortcut(icons_shortcut, |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                match desktop_icons::toggle_icons() {
                    Ok(hidden) => portable_kit::logging::info(&format!(
                        "desktop icons toggled by hotkey hidden={hidden}"
                    )),
                    Err(reason) => portable_kit::logging::warn(&format!(
                        "desktop icons hotkey toggle failed: {reason}"
                    )),
                }
            }
        }) {
            portable_kit::logging::warn(&format!("桌面图标快捷键 Ctrl+Alt+I 注册失败：{error}"));
        }
    }
    Ok(())
}
