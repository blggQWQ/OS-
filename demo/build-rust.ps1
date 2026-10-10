$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskCompiler = Join-Path $taskRoot 'tmp/rust-runtime/toolchains/stable-x86_64-pc-windows-msvc/bin/rustc.exe'
$taskLinker = Join-Path $taskRoot 'tmp/native-build/w64devkit/bin/gcc.exe'
if (!(Test-Path -LiteralPath $taskCompiler) -or !(Test-Path -LiteralPath $taskLinker)) {
    throw 'For this local environment, the isolated compiler and linker are required; see README.'
}
& $taskCompiler --edition=2021 --test (Join-Path $taskRoot 'crates/dm/src/protection.rs') --target x86_64-pc-windows-gnu -C "linker=$taskLinker" -o (Join-Path $taskRoot 'tmp/protection-tests.exe')
if ($LASTEXITCODE -ne 0) { throw 'Rust test build failed' }
& (Join-Path $taskRoot 'tmp/protection-tests.exe') --nocapture
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }
& $taskCompiler --edition=2021 (Join-Path $taskRoot 'crates/dm/src/bin/protection_demo.rs') --target x86_64-pc-windows-gnu -C "linker=$taskLinker" -o (Join-Path $taskRoot 'demo/protection-demo.exe')
if ($LASTEXITCODE -ne 0) { throw 'Rust bridge build failed' }
