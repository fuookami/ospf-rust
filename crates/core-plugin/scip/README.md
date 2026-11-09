# SCIP Solver Notes

:us: English | :cn: [简体中文](README_ch.md)

## Prerequisites

`ospf-rust-core-plugin-scip` uses `russcip/scip-sys` directly. You must make SCIP headers and libraries discoverable before building or running tests.
This workspace pins `russcip` to `0.9.1`, which is the compatibility path for SCIP 9.x.
Use `scip-bundled` or `scip-from-source` only when the selected `russcip` feature provides
the matching SCIP build.

## CP Boundary

The Gantt task-compilation component owns CP snapshot construction. SCIP consumes the
feature-gated MIP-backed `ExactLowering` facade for the declared exact subset; it is not exposed as
a native CP backend. Optional/variable-duration interval bindings remain `Unsupported`, and
Cumulative raw-handler work remains `Conditional` until its safety contract is independently met.

## Environment Setup

### Windows (PowerShell)

```powershell
$env:SCIPOPTDIR = "C:\path\to\scip"
```

`SCIPOPTDIR` should point to your SCIP installation root.

If `bindgen` cannot find libclang, install LLVM and set:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

### Linux/macOS

```bash
export SCIPOPTDIR=/path/to/scip
export LIBCLANG_PATH=/path/to/llvm/lib
```

## Minimal Validation Commands

From workspace root:

```powershell
# Windows PowerShell
.\crates\core-plugin\scip\scripts\check_non_bundled_env.ps1
.\crates\core-plugin\scip\scripts\run_non_bundled_regression.ps1
```

```bash
# Linux/macOS
bash ./crates/core-plugin/scip/scripts/check_non_bundled_env.sh
bash ./crates/core-plugin/scip/scripts/run_non_bundled_regression.sh
```

Or run commands manually:

```bash
cargo test -p ospf-rust-core-plugin-scip --features scip -- --nocapture
cargo test -p ospf-rust-core scip:: --features scip -- --nocapture
cargo test -p ospf-rust-framework scip_native_callback --features scip -- --nocapture
```

Use bundled SCIP when local SCIP is unavailable:

```bash
cargo test -p ospf-rust-core-plugin-scip --features scip-bundled -- --nocapture
cargo test -p ospf-rust-core scip:: --features scip-bundled -- --nocapture
cargo test -p ospf-rust-framework scip_native_callback --features scip-bundled -- --nocapture
```

## Typical Failure

If you see `Could not find SCIP installation` from `scip-sys`, verify:

1. `SCIPOPTDIR` is set in the current shell.
2. SCIP headers/libs exist under that path.
3. You are running with the `scip` (or `scip-bundled`) feature enabled.

If you see `Unable to find libclang`, set `LIBCLANG_PATH` to LLVM `bin`/`lib`.

## Native Callback Quick Start

```rust,ignore
use std::sync::Arc;
use ospf_rust_core_plugin_scip::{
    SCIPConfig, SCIPNativeControl, SCIPNativeObserver, SCIPNativeWhere, SCIPSolver,
};

let observer: SCIPNativeObserver = Arc::new(|snapshot| {
    if snapshot.where_point == SCIPNativeWhere::Node {
        // custom logic
    }
    if snapshot.matches_mask_bits(0x001000000) {
        // LP event mask matched
    }
    Ok(SCIPNativeControl::Continue)
});

let solver = SCIPSolver::with_config(
    SCIPConfig::new().add_native_observer(observer)
);
```

Note: in framework `ColumnGenerationSolver`, `UserInterrupt` is surfaced as an error result.
