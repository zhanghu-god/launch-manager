[CmdletBinding()]
param(
  [switch]$SkipBuild,
  [string]$Toolchain = "stable-x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Config = Get-Content -Raw -LiteralPath (Join-Path $Root "src-tauri\tauri.conf.json") | ConvertFrom-Json
$Version = $Config.version
$Exe = Join-Path $Root "src-tauri\target\release\launch-manager.exe"
$ReleaseDir = Join-Path $Root "release"
$Zip = Join-Path $ReleaseDir "launch-manager_${Version}_x64-portable.zip"
$WorkDir = Join-Path $ReleaseDir ("staging-" + [guid]::NewGuid().ToString("N"))
$Stage = Join-Path $WorkDir "Launch Manager"

try {
  if (-not $SkipBuild) {
    rustup run $Toolchain cargo build --release --manifest-path (Join-Path $Root "src-tauri\Cargo.toml")
    if ($LASTEXITCODE -ne 0) { throw "Rust release 构建失败" }
  }
  if (-not (Test-Path -LiteralPath $Exe)) { throw "找不到 release 程序：$Exe" }

  New-Item -ItemType Directory -Force -Path (Join-Path $Stage "data") | Out-Null
  Copy-Item -LiteralPath $Exe -Destination (Join-Path $Stage "launch-manager.exe")
  Copy-Item -LiteralPath (Join-Path $Root "README.md") -Destination (Join-Path $Stage "README.md")

  New-Item -ItemType Directory -Force -Path $ReleaseDir | Out-Null
  Compress-Archive -LiteralPath $Stage -DestinationPath $Zip -Force
  $Hash = Get-FileHash -Algorithm SHA256 -LiteralPath $Zip
  "Package: $Zip"
  "Size: $((Get-Item -LiteralPath $Zip).Length) bytes"
  "SHA256: $($Hash.Hash.ToLowerInvariant())"
}
finally {
  if (Test-Path -LiteralPath $WorkDir) {
    Remove-Item -LiteralPath $WorkDir -Recurse -Force
  }
}
