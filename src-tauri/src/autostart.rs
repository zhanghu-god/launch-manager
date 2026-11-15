//! 开机自启：仅在当前用户启动目录创建/删除快捷方式，不写注册表。

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub fn startup_shortcut_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA").map_err(|_| "无法获取 APPDATA 目录".to_string())?;
    Ok(Path::new(&appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join("launch-manager.lnk"))
}

pub fn is_enabled() -> Result<bool, String> {
    Ok(startup_shortcut_path()?.exists())
}

pub fn set_enabled(enable: bool, data_dir: &Path) -> Result<(), String> {
    let shortcut = startup_shortcut_path()?;
    if !enable {
        if shortcut.exists() {
            fs::remove_file(&shortcut).map_err(|e| format!("删除自启快捷方式失败：{e}"))?;
        }
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| format!("无法获取程序路径：{e}"))?;
    let exe_dir = exe.parent().ok_or_else(|| "程序路径没有父目录".to_string())?;
    if let Some(parent) = shortcut.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建启动目录失败：{e}"))?;
    }

    let cache_dir = data_dir.join("cache");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("创建缓存目录失败：{e}"))?;
    let script = cache_dir.join("set-autostart.ps1");
    fs::write(&script, AUTOSTART_SCRIPT).map_err(|e| format!("写入自启脚本失败：{e}"))?;

    let mut command = std::process::Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
        .arg(&script)
        .arg("-ShortcutPath")
        .arg(&shortcut)
        .arg("-TargetExe")
        .arg(&exe)
        .arg("-WorkingDir")
        .arg(exe_dir);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);

    let status = command.status().map_err(|e| format!("执行自启脚本失败：{e}"))?;
    if !status.success() {
        return Err("创建自启快捷方式失败".into());
    }
    Ok(())
}

const AUTOSTART_SCRIPT: &str = r#"param(
  [string]$ShortcutPath,
  [string]$TargetExe,
  [string]$WorkingDir
)
$ErrorActionPreference = 'Stop'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($ShortcutPath)
$shortcut.TargetPath = $TargetExe
$shortcut.WorkingDirectory = $WorkingDir
$shortcut.Description = 'Launch Manager'
$shortcut.Save()
"#;
