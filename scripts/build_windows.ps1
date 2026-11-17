# 构建 Windows 应用；CMake 自动 cargo 编译核心并打包 nexdrop_core.dll。
# 用法: .\build_windows.ps1 [debug|release]，默认 debug。
param(
  [ValidateSet('debug', 'release')]
  [string]$Mode = 'debug'
)
Set-Location "$PSScriptRoot\..\app"
if ($Mode -eq 'release') {
  flutter build windows --release
} else {
  flutter build windows --debug
}
Write-Host "built: app\build\windows\x64\runner\$Mode\nexdrop.exe"
