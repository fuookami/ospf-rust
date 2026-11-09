# Heuristic Plugin

:us: English | :cn: [简体中文](README_ch.md)

`ospf-rust-core-plugin-heuristic` carries the metaheuristic framework and the selection
hyper-heuristic, mirroring Kotlin's `ospf-kotlin-core-plugin-heuristic`. Kotlin keeps the same two
packages inside this plugin, so the Rust module paths keep their original shape:

| Module | Contents |
| --- | --- |
| `heuristic` | Lifecycle skeletons, 18 operator families, composition factories, algorithm entries, metered sessions, and native constraints. |
| `hyper_heuristic` | Two-level adaptive credit, operator registry and portfolio, HV rewards, JSON/YAML/TOML configuration, persistent sessions, and schedule replay. |

Both modules only consume `ospf-rust-core` model/variable/callback types; no solver adapter is
involved, so this plugin stays usable without any native solver installation.

```rust,ignore
use ospf_rust_core_plugin_heuristic::heuristic::HeuristicSolver;
use ospf_rust_core_plugin_heuristic::hyper_heuristic::HyperHeuristicSolver;
```

Module-level documentation stays next to the code:
[`src/heuristic/README.md`](src/heuristic/README.md) and
[`src/hyper_heuristic/README.md`](src/hyper_heuristic/README.md).

## Validation

```bash
cargo test -p ospf-rust-core-plugin-heuristic --lib
```

Runnable demos remain in the core crate's `examples/`: `heuristic_demo` and `hyper_heuristic_demo`.
