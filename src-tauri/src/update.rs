//! 便携版自动更新：检查 GitHub Release、下载校验 ZIP、暂存新程序并重启替换。
//!
//! 更新只替换 exe 与 README，不覆盖用户 data 中的配置。

use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::{PreparedUpdate, UpdateInfo};

pub const REPO: &str = "zhanghu-god/launch-manager";
const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/zhanghu-god/launch-manager/releases/latest";
const USER_AGENT: &str = "launch-manager-updater";
const PORTABLE_SUFFIX: &str = "_x64-portable.zip";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagedMeta {
    version: String,
    asset_name: String,
    asset_digest: String,
    staged_exe: String,
    staged_readme: Option<String>,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub async fn check_update() -> Result<UpdateInfo, String> {
    let client = http_client(Duration::from_secs(30))?;
    let release: GithubRelease = client
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("检查更新失败：{e}"))?
        .error_for_status()
        .map_err(|e| format!("检查更新失败：{e}"))?
        .json()
        .await
        .map_err(|e| format!("解析更新信息失败：{e}"))?;

    let current = parse_version(current_version())?;
    let latest = parse_version(&release.tag_name)?;
    let asset = select_asset(&release.assets)?;
    let digest = asset
        .digest
        .clone()
        .ok_or_else(|| "Release 附件缺少 SHA-256 校验值".to_string())?;

    Ok(UpdateInfo {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        has_update: latest > current,
        release_url: release.html_url,
        asset_name: asset.name.clone(),
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
        asset_digest: digest,
    })
}

pub async fn prepare_update(data_dir: &Path, info: &UpdateInfo) -> Result<PreparedUpdate, String> {
    if !info.has_update {
        return Err("当前已是最新版本".into());
    }

    let update_dir = data_dir.join("update");
    let staged_dir = update_dir.join("staged");
    let meta_path = update_dir.join("update.json");
    fs::create_dir_all(&staged_dir).map_err(|e| format!("创建更新目录失败：{e}"))?;

    if let Ok(text) = fs::read_to_string(&meta_path) {
        if let Ok(meta) = serde_json::from_str::<StagedMeta>(&text) {
            let staged_exe = PathBuf::from(&meta.staged_exe);
            if meta.version == info.latest_version
                && meta.asset_digest == info.asset_digest
                && staged_exe.exists()
            {
                return Ok(PreparedUpdate {
                    version: meta.version,
                    asset_name: meta.asset_name,
                    already_staged: true,
                });
            }
        }
    }

    let client = http_client(Duration::from_secs(300))?;
    let bytes = client
        .get(&info.asset_url)
        .send()
        .await
        .map_err(|e| format!("下载更新失败：{e}"))?
        .error_for_status()
        .map_err(|e| format!("下载更新失败：{e}"))?
        .bytes()
        .await
        .map_err(|e| format!("下载更新失败：{e}"))?;

    if info.asset_size > 0 && bytes.len() as u64 != info.asset_size {
        return Err("更新包大小与 Release 不一致".into());
    }
    verify_digest(&bytes, &info.asset_digest)?;

    if staged_dir.exists() {
        fs::remove_dir_all(&staged_dir).map_err(|e| format!("清理旧更新文件失败：{e}"))?;
    }
    fs::create_dir_all(&staged_dir).map_err(|e| format!("创建更新目录失败：{e}"))?;

    let staged_readme = extract_update_files(&bytes, &staged_dir)?;
    let staged_exe = staged_dir.join("launch-manager.exe");
    if !staged_exe.exists() {
        return Err("更新包中未找到 launch-manager.exe".into());
    }

    let meta = StagedMeta {
        version: info.latest_version.clone(),
        asset_name: info.asset_name.clone(),
        asset_digest: info.asset_digest.clone(),
        staged_exe: staged_exe.to_string_lossy().into_owned(),
        staged_readme: staged_readme.map(|p| p.to_string_lossy().into_owned()),
    };
    let text = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    fs::write(&meta_path, text).map_err(|e| format!("写入更新元数据失败：{e}"))?;

    Ok(PreparedUpdate {
        version: meta.version,
        asset_name: meta.asset_name,
        already_staged: false,
    })
}

pub fn apply_update_and_restart(data_dir: &Path) -> Result<(), String> {
    let update_dir = data_dir.join("update");
    let meta_path = update_dir.join("update.json");
    let text = fs::read_to_string(&meta_path).map_err(|_| "没有已准备好的更新".to_string())?;
    let meta: StagedMeta = serde_json::from_str(&text).map_err(|e| format!("更新元数据损坏：{e}"))?;
    let staged_exe = PathBuf::from(&meta.staged_exe);
    if !staged_exe.exists() {
        return Err("暂存的新程序不存在，请重新下载更新".into());
    }

    let target_exe = std::env::current_exe().map_err(|e| format!("无法定位当前程序：{e}"))?;
    let target_dir = target_exe
        .parent()
        .ok_or_else(|| "程序路径没有父目录".to_string())?;
    let target_readme = target_dir.join("README.md");
    let staged_readme = meta.staged_readme.clone().unwrap_or_default();
    let script_path = update_dir.join("apply-update.ps1");
    fs::write(&script_path, UPDATE_HELPER_SCRIPT)
        .map_err(|e| format!("写入更新脚本失败：{e}"))?;

    spawn_helper(
        &script_path,
        std::process::id(),
        &staged_exe,
        &target_exe,
        &staged_readme,
        &target_readme,
        &update_dir,
    )?;
    Ok(())
}

/// 启动时清理上次更新遗留的脚本目录；仍有 update.json 时表示用户选择了稍后更新。
pub fn cleanup_leftover(data_dir: &Path) {
    let update_dir = data_dir.join("update");
    if update_dir.exists() && !update_dir.join("update.json").exists() {
        let _ = fs::remove_dir_all(update_dir);
    }
}

fn http_client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .build()
        .map_err(|e| format!("创建网络客户端失败：{e}"))
}

fn parse_version(tag: &str) -> Result<Version, String> {
    let normalized = tag.trim().trim_start_matches('v');
    Version::parse(normalized).map_err(|e| format!("无法解析版本号 {tag}：{e}"))
}

fn select_asset(assets: &[GithubAsset]) -> Result<&GithubAsset, String> {
    assets
        .iter()
        .find(|asset| asset.name.ends_with(PORTABLE_SUFFIX))
        .or_else(|| {
            assets
                .iter()
                .find(|asset| asset.name.contains("portable") && asset.name.ends_with(".zip"))
        })
        .ok_or_else(|| "最新 Release 中没有便携 ZIP 附件".to_string())
}

fn verify_digest(bytes: &[u8], digest: &str) -> Result<(), String> {
    let expected = digest
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(digest.trim())
        .to_ascii_lowercase();
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err("更新包 SHA-256 校验失败".into());
    }
    Ok(())
}

fn extract_update_files(bytes: &[u8], staged_dir: &Path) -> Result<Option<PathBuf>, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("打开更新包失败：{e}"))?;
    let mut exe_index = None;
    let mut readme_index = None;

    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|e| format!("读取更新包失败：{e}"))?;
        let name = file.name().replace('\\', "/");
        if name.ends_with("/launch-manager.exe") || name == "launch-manager.exe" {
            exe_index = Some(index);
        } else if name.ends_with("/README.md") || name == "README.md" {
            readme_index = Some(index);
        }
    }

    let exe_index = exe_index.ok_or_else(|| "更新包中未找到 launch-manager.exe".to_string())?;
    extract_one(&mut archive, exe_index, &staged_dir.join("launch-manager.exe"))?;

    let readme_path = match readme_index {
        Some(index) => {
            let path = staged_dir.join("README.md");
            extract_one(&mut archive, index, &path)?;
            Some(path)
        }
        None => None,
    };
    Ok(readme_path)
}

fn extract_one(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    index: usize,
    target: &Path,
) -> Result<(), String> {
    let mut file = archive
        .by_index(index)
        .map_err(|e| format!("读取更新包失败：{e}"))?;
    let mut out = fs::File::create(target).map_err(|e| format!("创建暂存文件失败：{e}"))?;
    io::copy(&mut file, &mut out).map_err(|e| format!("解压更新文件失败：{e}"))?;
    out.flush().map_err(|e| format!("写入暂存文件失败：{e}"))?;
    Ok(())
}

#[cfg(windows)]
fn spawn_helper(
    script: &Path,
    pid: u32,
    staged_exe: &Path,
    target_exe: &Path,
    staged_readme: &str,
    target_readme: &Path,
    update_dir: &Path,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut command = std::process::Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-File",
        ])
        .arg(script)
        .args(["-ProcessId", &pid.to_string()])
        .arg("-StagedExe")
        .arg(staged_exe)
        .arg("-TargetExe")
        .arg(target_exe)
        .arg("-StagedReadme")
        .arg(staged_readme)
        .arg("-TargetReadme")
        .arg(target_readme)
        .arg("-UpdateDir")
        .arg(update_dir)
        .creation_flags(CREATE_NO_WINDOW);
    command
        .spawn()
        .map_err(|e| format!("启动更新脚本失败：{e}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn spawn_helper(
    _script: &Path,
    _pid: u32,
    _staged_exe: &Path,
    _target_exe: &Path,
    _staged_readme: &str,
    _target_readme: &Path,
    _update_dir: &Path,
) -> Result<(), String> {
    Err("自动更新目前仅支持 Windows".into())
}

const UPDATE_HELPER_SCRIPT: &str = r#"param(
  [int]$ProcessId,
  [string]$StagedExe,
  [string]$TargetExe,
  [string]$StagedReadme,
  [string]$TargetReadme,
  [string]$UpdateDir
)
$ErrorActionPreference = 'Stop'
while (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue) {
  Start-Sleep -Milliseconds 200
}
Copy-Item -LiteralPath $StagedExe -Destination $TargetExe -Force
if ($StagedReadme -and (Test-Path -LiteralPath $StagedReadme)) {
  Copy-Item -LiteralPath $StagedReadme -Destination $TargetReadme -Force
}
Start-Process -FilePath $TargetExe -WorkingDirectory (Split-Path -Parent $TargetExe)
$staged = Join-Path $UpdateDir 'staged'
if (Test-Path -LiteralPath $staged) {
  Remove-Item -LiteralPath $staged -Recurse -Force -ErrorAction SilentlyContinue
}
$meta = Join-Path $UpdateDir 'update.json'
if (Test-Path -LiteralPath $meta) {
  Remove-Item -LiteralPath $meta -Force -ErrorAction SilentlyContinue
}
"#;
