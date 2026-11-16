# Launch Manager

一个便携软件启动管理器。默认托盘后台运行，通过全局快捷键唤出本地便携软件列表，支持编号或方向键启动。

## 功能

- 默认后台运行，只显示托盘图标
- 单实例，重复启动会唤出已有面板
- 全局快捷键唤出启动面板，默认 `Ctrl+Shift+X`
- 扫描软件目录下的一级子目录，自动识别主 exe
- 数字键 `1–9` 快速启动
- `↑` / `↓` 选择，`Enter` 启动，`Esc` 隐藏
- 鼠标点击启动
- 启动软件时可选最大化窗口（默认开），避免小屏下窗口超出屏幕
- 设置页：软件目录、快捷键、开机自启、启动后隐藏、最大化、主题
- 内置自动更新：启动与设置页检查 GitHub Release，下载校验后重启替换
- 开机自启仅在当前用户启动目录创建快捷方式，不写注册表
- 运行日志写入自身 `data/logs`（按天分文件），并每小时清理自身与被管理应用超过 7 天的日志
- WebDAV 云备份：统一配置一套服务器/账号，每小时 + 退出时自动增量备份所有被管理软件及**管理器自身**的 `data/`，可测试连接/立即备份/按软件云恢复
- 数据导入/导出：每个软件可导出为单个 JSON 文件、从 JSON 导入；导入/恢复前自动备份现有数据
- 桌面图标：托盘菜单或 `Ctrl+Alt+I` 显示/隐藏；设置页可选“双击桌面空白处切换”（全局鼠标钩子，默认关）
- 配置保存在 exe 同级 `data` 目录

## 部署建议

将本程序放在便携软件根目录下，例如：

```text
C:\Users\zhang\launch\
├─ Launch Manager\
│  ├─ launch-manager.exe
│  └─ data\
├─ DDL Board\
├─ push-wp\
└─ ...
```

默认扫描 exe 所在目录的上一级，因此放在 `launch\Launch Manager` 时会自动扫描 `launch`。

## 使用

1. 解压便携 ZIP。
2. 双击 `launch-manager.exe`，程序进入托盘后台。
3. 按 `Ctrl+Shift+X` 唤出启动面板。
4. 按编号或方向键选择软件，回车启动。
5. 托盘菜单可打开面板、设置或退出。

## 开机自启

在设置中开启后，会在当前用户启动目录创建 `launch-manager.lnk`。关闭后删除。程序目录移动后，再次保存设置或启动时会尝试修复快捷方式。

## 构建

需要 Rust MSVC 工具链、Node.js、WebView2 Runtime。

```powershell
$env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-msvc"
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
node --check src/main.js
node scripts/check-frontend.mjs
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-portable.ps1
```

## 版本

当前版本：`0.2.8`。版本号同步维护在 `src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json`。
