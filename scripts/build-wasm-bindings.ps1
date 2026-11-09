param([string]$Toolchain = 'nightly-2026-08-01')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $root
try {
    $metadata = cargo "+$Toolchain" metadata --no-deps --format-version=1
    if ($LASTEXITCODE) { throw 'Cargo metadata failed' }
    $target = ($metadata | ConvertFrom-Json).target_directory
    cargo "+$Toolchain" build -p ospf-wasm --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE) { throw 'WASM build failed' }
    $output = Join-Path $target 'npm/ospf-wasm-bindings'
    New-Item -ItemType Directory -Force -Path $output | Out-Null
    wasm-bindgen (Join-Path $target 'wasm32-unknown-unknown/release/ospf_wasm.wasm') --target web --typescript --out-name index --out-dir $output
    if ($LASTEXITCODE) { throw 'wasm-bindgen failed (install CLI version 0.2.129)' }
    $declarations = Join-Path $output 'index.d.ts'
    $generated = [System.IO.File]::ReadAllText($declarations)
    [System.IO.File]::WriteAllText($declarations, "/// <reference lib=`"esnext.disposable`" />`n$generated", [System.Text.UTF8Encoding]::new($false))
    cargo "+$Toolchain" run -p ospf-rust-bridge --bin export-types -- (Join-Path $output 'protocol.d.ts')
    if ($LASTEXITCODE) { throw 'Protocol declaration generation failed' }
    @{
        name = 'ospf-wasm-bindings'; version = '0.1.0'; type = 'module'; license = 'MIT'
        files = @('index.js', 'index.d.ts', 'index_bg.wasm', 'index_bg.wasm.d.ts', 'protocol.d.ts', 'LICENSE')
        exports = @{
            '.' = @{ types = './index.d.ts'; default = './index.js' }
            './protocol' = @{ types = './protocol.d.ts' }
            './index_bg.wasm' = './index_bg.wasm'
        }
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'package.json') -Encoding utf8
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $output
    npm pack $output --pack-destination (Join-Path $target 'npm')
    if ($LASTEXITCODE) { throw 'npm pack failed' }
} finally { Pop-Location }
