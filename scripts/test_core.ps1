# 运行核心单测；传参数 clippy 时额外执行 clippy（-D warnings）。
# 用法: .\test_core.ps1 [clippy]
param([string]$Arg)
Set-Location "$PSScriptRoot\..\core"
cargo test -p nexdrop-core
if ($Arg -eq 'clippy') {
  cargo clippy -p nexdrop-core -- -D warnings
}
