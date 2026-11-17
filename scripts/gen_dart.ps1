# 重新生成 Dart 绑定：core/src/api → app/lib/src/rust（flutter_rust_bridge）。
# 修改 Rust API 或桥接类型后必须重跑。
Set-Location "$PSScriptRoot\..\app"
flutter_rust_bridge_codegen generate
Write-Host "dart bindings regenerated: app\lib\src\rust\"
