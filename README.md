# ospf-rust

TypeScript bindings, native execution and current coverage are documented in the [TypeScript bridge](docs/typescript-bridge.md). The independent `ospf-typescript` repository owns the public npm SDK, `ospf-wasm`.

:us: English | :cn: [简体中文](README_ch.md)

## Introduction

`ospf-rust` is the Rust implementation and migration workspace for OSPF. It provides base utilities, mathematical foundations, physical quantities, optimization core modeling, framework-level solver orchestration, and domain frameworks for BPP3D, CSP1D, and Gantt scheduling.

For the broader OSPF project and published documentation, see:

- ospf: <https://github.com/fuookami/ospf>
- documentation: <https://fuookami.github.io/ospf/>

## Scope

This workspace owns reusable Rust crates and framework migrations. It keeps domain-independent infrastructure, optimization modeling primitives, and reusable domain frameworks in the repository.

Explicit non-goals:

1. Business-specific request protocols, tenant context, formula languages, and runtime deployment adapters.
2. External renderer implementations.
3. Solver installation and license management beyond feature-gated adapter documentation.

## Module Structure

| Rust crate | Kotlin boundary | Responsibility |
| --- | --- | --- |
| [`ospf-rust-base`](crates/base/README.md) | `ospf-kotlin-utils` foundation | Error handling, indexed types, collections, containers, iterators, and cloneable function helpers. |
| [`ospf-rust-multiarray`](crates/multiarray/README.md) | `ospf-kotlin-multiarray` | Generic multi-dimensional arrays, shapes, views, storage order, and block arrays. |
| [`ospf-rust-math`](crates/math/README.md) | `ospf-kotlin-math` | Algebra, geometry, ordinary math, operators, chaotic systems, fractals, combinatorics, and symbolic computation. |
| [`ospf-rust-quantities`](crates/quantities/README.md) | `ospf-kotlin-quantities` | Physical dimensions, units, compile-time/runtime quantities, and quantity-aware arithmetic. |
| [`ospf-rust-core`](crates/core/README.md) | `ospf-kotlin-core` | Variables, tokens, symbols, `MetaModel`, flattening, solver traits, solver output, IIS, and backend adapters. |
| [`ospf-rust-framework`](crates/framework/README.md) | `ospf-kotlin-framework` | Pipeline modeling, shadow prices, column generation, Benders, combinatorial solvers, persistence contracts, remote solver client, and heartbeat utilities. |
| [`ospf-rust-core-plugin-gurobi`](crates/core-plugin/gurobi/README.md) | `ospf-kotlin-core-plugin-gurobi` | Gurobi core adapter, Gurobi-backed IIS algorithms, and framework Gurobi extension solvers. |
| [`ospf-rust-core-plugin-heuristic`](crates/core-plugin/heuristic/README.md) | `ospf-kotlin-core-plugin-heuristic` | Metaheuristic framework (lifecycle skeletons, operator families, composition) and the selection hyper-heuristic. |
| [`ospf-rust-core-plugin-scip`](crates/core-plugin/scip/README.md) | `ospf-kotlin-core-plugin-scip` | SCIP core adapter, constraint-programming facade, and framework SCIP extension solvers. |
| [`ospf-rust-framework-geometry2d`](crates/frameworks/geometry2d/README.md) | `ospf-kotlin-framework-geometry2d` | Shared 2D quantity geometry kernel: quantity arithmetic, points and shapes, placements, bounding-box overlap, sheet geometry and kerf, used by both BPP2D and CSP2D. |
| [`ospf-rust-framework-bpp`](crates/frameworks/bpp/README.md) | `ospf-kotlin-framework-bpp` | Shared sparse discrete loading, finite-capacity occupancy, edge-preserving conflicts, explicit layouts, native-value validation and geometry adapters. |
| [`ospf-rust-framework-bpp1d`](crates/frameworks/bpp1d/README.md) | `ospf-kotlin-framework-bpp1d` | Quantity-safe exact-demand packing, bounded patterns, shared MILP/LP/final registration, finite-pool column activation, strict CSV, tail optimization and 1D discrete loading. |
| [`ospf-rust-framework-bpp2d`](crates/frameworks/bpp2d/README.md) | `ospf-kotlin-framework-bpp2d` | Reusable 2D packing framework with item/sheet/layer models, BLA and block loading, candidate generation, layer assignment, packing with remnants, and the column-generation flow. |
| [`ospf-rust-framework-bpp3d`](crates/frameworks/bpp3d/README.md) | `ospf-kotlin-framework-bpp3d` | Reusable 3D bin-packing framework with BPP3D contexts, layer generation/assignment, packing, CSV fixtures, and renderer DTOs. |
| [`ospf-rust-framework-csp1d`](crates/frameworks/csp1d/README.md) | `ospf-kotlin-framework-csp1d` | Reusable one-dimensional cutting-stock framework with material, generation, produce, yield, waste, length, and application flows. |
| [`ospf-rust-framework-csp2d`](crates/frameworks/csp2d/README.md) | `ospf-kotlin-framework-csp2d` | Reusable 2D cutting-stock framework with dual-mode materials, guillotine cut trees, pattern generation, the master problem, waste/yield analysis and both solve paths. |
| [`ospf-rust-framework-gantt-scheduling`](crates/frameworks/gantt-scheduling/README.md) | `ospf-kotlin-framework-gantt-scheduling` | Reusable Gantt scheduling framework with task, bunch, capacity, resource, produce, and branch-and-price flows. |
| [`ospf-rust-framework-network-scheduling`](crates/frameworks/network-scheduling/README.md) | `ospf-kotlin-framework-network-scheduling` | Multigraph/resource paths, flow, design/resilience, allocation, full-path compilation, rooted structures, generalized flow and VRPTW pricing. |
| [`ospf-rust-framework-gantt-network`](crates/frameworks/gantt-network/README.md) | Gantt / Network bridge | Optional shared-path, full-state and column-batch bridge, without owning a master or starting another global solve. |
| [`ospf-rust-example`](apps/example/README.md) | `ospf-kotlin-example` | Runnable examples and migration compatibility demos. |

## Architecture Overview

The workspace follows a layered shape:

1. `base`, `multiarray`, `math`, and `quantities` provide reusable foundations.
2. `core` owns optimization modeling primitives and solver-facing model conversion.
3. `framework` adds solver orchestration, pipeline abstractions, shadow prices, persistence contracts, and remote solving.
4. domain framework crates assemble reusable business-domain modeling contexts around `MetaModel`.
5. `example` demonstrates current public flows and migration compatibility paths.

The workspace also keeps `ospf-kotlin`'s two plugin families apart from the domain frameworks:

| Directory | Contents | Kotlin counterpart |
| --- | --- | --- |
| `crates/base` … `crates/framework` | Foundation, core, framework | `ospf-kotlin-utils` … `ospf-kotlin-framework` |
| `crates/core-plugin/*` | Core plugins (`scip`, `gurobi`, `heuristic`) | `ospf-kotlin-core-plugin-*` |
| `crates/framework-plugin/*` | Framework plugins (`persistence`) | `ospf-kotlin-framework-plugin-*` |
| `crates/frameworks/*` | Domain frameworks (`bpp`, `bpp1d`, …, `network-scheduling`) | top-level `ospf-kotlin-framework-*` modules |

Kotlin keeps the domain frameworks at module top level and reserves `*-plugin` for optional integration
modules. Rust keeps the same distinction but groups the domain frameworks under `crates/frameworks` so that
`*-plugin` directories never mix with frameworks. Persistence stays one crate with per-backend features
instead of Kotlin's `persistence-<backend>` modules: Cargo features express the same optional boundary without
splitting a translator family across nine crates.

Framework domain crates should keep optimization semantics in context / aggregation / model component / pipeline layers. Application services coordinate solver selection, lifecycle, trace/KPI/render assembly, and recovery boundaries.

## Constraint Programming Boundary

`ospf-rust-core` exposes an exact `i64` constraint-programming model with immutable snapshots,
stable IDs, source verification, and a unified `SolveReport<i64>`. The generic MIP lowerer uses
checked `i128` internally and only crosses the existing `f64` solver boundary when every integer
coefficient, bound, and generated Big-M is exactly representable; the current gate is `2^53`.

The SCIP CP entry point is a feature-gated, strict finite MIP-backed facade. It is not a native
SCIP/CIP CP backend. The declared CP capability scope is complete: generic MIP lowering returns
verified `ExactLowering` for the supported finite subset and structured `Unsupported` for
`Cumulative`, `Circuit`, `Automaton`, and `Reservoir`; raw cumulative FFI is a `Conditional`
research probe, and true incremental CP sessions remain `Unsupported`. Snapshot-rebuild sessions
are correct but must not be described as native incremental resume. The fake CP solver is for
contract tests and small exhaustive oracles, not production search.

## Documentation Templates

New or refreshed README files should follow:

- [English template](docs/README_TEMPLATE.md)
- [中文模板](docs/README_TEMPLATE_ch.md)

Crate-level README files use the full template. Internal `src/domain`, `src/application`, and `src/infrastructure` README files may use the shorter skeleton: responsibilities, file layout, public API, extension points, lifecycle/data flow, validation, and related modules.

## Usage

Add workspace crates as path dependencies while developing inside this repository. Paths below are relative to a plugin crate under `crates/frameworks/<name>`:

```toml
[dependencies]
ospf-rust-core = { path = "../../core" }
ospf-rust-framework = { path = "../../framework" }
```

Domain frameworks can be enabled directly:

```toml
[dependencies]
ospf-rust-framework-geometry2d = { path = "../geometry2d" }
ospf-rust-framework-bpp2d = { path = "../bpp2d" }
ospf-rust-framework-csp1d = { path = "../csp1d" }
ospf-rust-framework-csp2d = { path = "../csp2d" }
ospf-rust-framework-gantt-scheduling = { path = "../gantt-scheduling" }
ospf-rust-framework-network-scheduling = { path = "../network-scheduling" }
```

## Local Validation

```powershell
cargo check --workspace
cargo test --workspace --no-run
```

For focused development, prefer package-level checks such as:

```powershell
cargo check -p ospf-rust-core
cargo test -p ospf-rust-framework-csp1d
```

Solver-backed tests require the corresponding Cargo feature and local solver installation or bundled support. See the solver notes for [Gurobi](crates/core-plugin/gurobi/README.md) and [SCIP](crates/core-plugin/scip/README.md).

### Feature gating and test coverage

Every crate declares `default = []`, and a large share of the suite sits behind non-default features:

- `ospf-rust-math`'s `scalar_parser` and expression-parsing tests need `parser`; likewise `serde`.
- `serde`-dependent tests in `ospf-rust-core` need `serde`. **The whole CP checkpoint module in
  `ospf-rust-core` (`solver::constraint_programming::checkpoint`) sits behind
  `#[cfg(feature = "serde")]`**, so a plain `cargo test -p ospf-rust-core` **silently skips every
  one of its tests** (including `complete_hint_*` and several cancellation-chain tests). Add
  `--features serde` when checking CP checkpoint coverage.
- `ospf-rust-framework`'s `remote` module (including `RemoteSolverFailureDetail`) needs `remote-solver`;
  the checkpoint envelope wire-contract test `tests/checkpoint_wire_contract.rs` is also compiled and
  run only under `--features remote-solver`.

A plain `cargo test --workspace` therefore **silently skips** those tests. Enable them explicitly when measuring coverage:

```powershell
# Workspace-wide, including remote, excluding async
cargo test --workspace --features "ospf-rust-math/parser ospf-rust-math/serde ospf-rust-core/serde ospf-rust-framework/remote-solver"
```

`async` switches the solver traits to a future-returning shape. `remote-solver` does **not** imply
`async`, so remote tests can run alongside the whole workspace.

To enable `async` you must turn on **each** downstream crate's own `async` at the same time.
Enabling it only on `ospf-rust-framework` leaves `bpp3d` and `gantt` — which still use the synchronous
signature — failing to compile (a half-open state caused by feature unification):

```powershell
cargo test --workspace --features "ospf-rust-framework/async ospf-rust-framework-bpp3d/async ospf-rust-framework-gantt-scheduling/async"
```

`solver-backed` tests additionally require the Gurobi/SCIP features plus local libraries and licenses.

**Note**: `gurobi10`, `gurobi11`, and `gurobi12` are mutually exclusive — do **not** use `--all-features`; use the explicit feature list above.

## Current Boundaries

This repository is actively migrating Kotlin framework capabilities into Rust. Some domain crates expose Kotlin-aligned public surfaces while still using Rust-side deterministic, fake, or feature-gated solver paths for parts of the lifecycle. Each domain crate README records its own current coverage and known gaps.

The Network / Gantt migration targets Kotlin snapshots `907d28d` and `3dc1225`; current scope and
breaking APIs are documented in the [migration guide](crates/frameworks/network-scheduling/MIGRATION.md).
The old `99/99` delivery status cannot establish acceptance for this migration. Shared graphs/paths,
seven standalone Network services and the optional Gantt bridge are connected.
[Demo4](apps/example/src/framework/demo4/README.md) now connects recovery records, RMP/full duals,
path columns and final MILP through the existing Gantt lifecycle rather than remaining a scaffold.
Demo4/Demo5 native gates require working Gurobi/SCIP runtimes and licenses and actual feature-specific
test execution, not compilation alone. Finite-candidate optima and heuristic pricing termination do
not establish full-path lower bounds or global integer proofs; incomplete regression/strict checks
must be disclosed separately.

## Related Modules

- [OSPF Kotlin workspace](../ospf-kotlin/README.md)
- [OSPF Rust examples](apps/example/README.md)

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
