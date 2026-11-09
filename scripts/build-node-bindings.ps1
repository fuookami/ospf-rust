param(
    [string]$Toolchain = 'nightly-2026-08-01',
    [ValidateSet('scip', 'scip-bundled', 'scip-from-source', 'scip-quadratic', 'gurobi10', 'gurobi11', 'gurobi12')]
    [string[]]$Features = @()
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$nodeRoot = Join-Path $root 'ospf-rust-node'
$metadata = cargo "+$Toolchain" metadata --no-deps --format-version=1 --manifest-path (Join-Path $root 'Cargo.toml')
if ($LASTEXITCODE) { throw 'Cargo metadata failed' }
$target = ($metadata | ConvertFrom-Json).target_directory
$output = Join-Path $target 'npm/ospf-node-bindings'
$destination = [System.IO.Path]::GetRelativePath($nodeRoot, $output)
# napi-rs 2.x 生成声明时将输出目录拼接到当前目录，因此跨盘路径需要暂存。
# napi-rs 2.x joins its destination to cwd when generating declarations, so cross-drive paths need staging.
if ([System.IO.Path]::IsPathRooted($destination)) {
    $staging = Join-Path $root 'target/npm/ospf-node-bindings'
    $destination = [System.IO.Path]::GetRelativePath($nodeRoot, $staging)
} else {
    $staging = $output
}
$previousToolchain = $env:RUSTUP_TOOLCHAIN
$previousTarget = $env:CARGO_TARGET_DIR
New-Item -ItemType Directory -Force -Path $output | Out-Null
New-Item -ItemType Directory -Force -Path $staging | Out-Null
Push-Location $nodeRoot
try {
    $env:RUSTUP_TOOLCHAIN = $Toolchain
    $env:CARGO_TARGET_DIR = $target
    $arguments = @('--yes', '@napi-rs/cli@2.18.4', 'build', $destination, '--platform', '--target', 'x86_64-pc-windows-msvc', '--dts', 'index.d.ts', '--js', 'index.js')
    if ($Features.Count) { $arguments += @('--features', ($Features -join ',')) }
    npx @arguments
    if ($LASTEXITCODE) { throw 'N-API build failed' }
    foreach ($file in @('index.js', 'index.d.ts', 'ospf.win32-x64-msvc.node')) {
        $source = Join-Path $staging $file
        if (!(Test-Path -LiteralPath $source)) { throw "Missing generated $file" }
        if ($staging -ne $output) { Move-Item -LiteralPath $source -Destination (Join-Path $output $file) -Force }
    }
    Copy-Item -LiteralPath 'package.json' -Destination $output
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $output
    npm pack $output --pack-destination (Join-Path $target 'npm')
    if ($LASTEXITCODE) { throw 'npm pack failed' }
} finally {
    $env:RUSTUP_TOOLCHAIN = $previousToolchain
    $env:CARGO_TARGET_DIR = $previousTarget
    Pop-Location
}
