# OSPF Rust Framework BPP3D

:us: English | :cn: [简体中文](README_ch.md)

## Introduction

`ospf-rust-framework-bpp3d` is the Rust migration target for Kotlin `ospf-kotlin-framework-bpp3d`. It provides reusable three-dimensional bin-packing domain capabilities, including BPP3D infrastructure, item modeling, BLA/block-loading/layer-generation contexts, layer-assignment pipelines, packing, CSV protocol adapters, renderer DTOs, and application-level column-generation orchestration.

## Scope

This crate owns the reusable BPP3D framework kernel. It keeps packing geometry, candidate generation, layer assignment, final packing, fixture protocols, and renderer DTO boundaries inside the shared domain crate.

Explicit non-goals:

1. Business-specific request DTOs, tenant runtime context, formula languages, or project deployment policies.
2. External renderer source code.
3. Solver installation, license management, or non-BPP3D backend plugin ownership.

## Module Structure

| Rust module or directory | Kotlin boundary | Responsibility |
| --- | --- | --- |
| [`src/infrastructure`](src/infrastructure/README.md) | `bpp3d-infrastructure` | Geometry primitives, orientation, packing shapes, PWL approximation helpers, and renderer DTOs. |
| [`src/domain/item`](src/domain/item/README.md) | `bpp3d-domain-item-context` | Items, packages, materials, bins, layers, patterns, demand coverage, shape metadata, and continuous-radius model components. |
| [`src/domain/bla`](src/domain/bla/README.md) | `bpp3d-domain-bla-context` | Bottom-up-left-justified placement and projection candidate construction. |
| [`src/domain/block_loading`](src/domain/block_loading/README.md) | `bpp3d-domain-block-loading-context` | Simple/complex block generation, DFS space splitting, MLHS search, and block-loading diagnostics. |
| [`src/domain/layer_generation`](src/domain/layer_generation/README.md) | `bpp3d-domain-layer-generation-context` | Block, BL, circle-packing, Pattern, Pile, and Historical layer candidate generation. |
| [`src/domain/layer_assignment`](src/domain/layer_assignment/README.md) | `bpp3d-domain-layer-assignment-context` | RMP/final MILP variables, dynamic columns, constraints, objectives, demand/depth/activation limits, and shadow prices. |
| [`src/domain/packing`](src/domain/packing/README.md) | `bpp3d-domain-packing-context` | Final packed-bin conversion, geometry guard, placement/block trace replay, material summaries, and render adaptation. |
| [`src/application`](src/application/README.md) | `bpp3d-application` and layer-selection flow | Solver backend selection, CSV materialization, column-generation lifecycle, fixture reports, KPI/trace diagnostics, and renderer output. |

## Architecture Overview

The modeling path is centered on `MetaModel`: RMP and final MILP assembly share `LayerAssignmentContext`, `LayerAssignmentAggregation`, variable components, demand/depth/activation limits, and objective pipelines. Application services coordinate solver backends, CSV materialization, trace/KPI diagnostics, and renderer adaptation. Placement and block trace replay remain in the packing domain.

Kotlin `bpp3d-domain-layer-selection-context` is mapped into Rust application orchestration, because Rust keeps layer selection as a solver lifecycle around reusable domain contexts.


## Cylinder Geometry Semantics

The framework supports axis-aligned cylinders with the following semantics:

### Vertical Cylinders (Axis3.Y)

- Full support for circle-packing grid candidates and conservative cuboid supported-stack candidates.
- Bottom overlap/support checks use real footprint geometry.
- `radius` and `diameter` fields in renderer DTOs.

### Horizontal Cylinders (Axis3.X / Axis3.Z)

- **Fixed/discrete radius**: Circle-packing grid path with axis-aware placement.
- **Supported stacks**: Single full-length support, repeated same-shape multi-support intervals, heterogeneous support intervals.
- **Hanging candidates**: Single or repeated narrow cuboid support-line hanging covering the cylinder axis.
- **Support requirement**: Must be on bin floor or have cuboid support intervals that cover the full cylinder axis and bottom support line.
- **Layer constraint**: A single `BinLayer` cannot mix multiple cylinder axes.

### Continuous Radius Modeling

- **PWL approximation**: `UnivariateLinearPiecewiseFunction` symbol for volume approximation via `π·r²·h`.
- **Conservative envelope**: `rMax` envelope bounds for production-ready continuous radius variables.
- **Production-ready path**: `RealVar` solver variables with constraint-based bounds and target equality constraint.
- **Renderer output**: Solver-selected radius applied to `actualVolume` and `radius`/`diameter` DTO fields.
- The one-round CSV flow supplies the continuous-radius component to both RMP states for extensions; this does not automatically register it in the RMP model.
- **PWL diagnostics**: `pwl_volume`, `pwl_error`, `pwl_segments`, `pwl_within_envelope` in renderer item info.

## Core Concepts

1. `BinLayer` is the selected-column unit in layer assignment.
2. Layer generation supplies candidate layers from BLA, block loading, circle packing, Pattern, Pile, and Historical sources.
3. Layer assignment registers RMP/final MILP variables and constraint/objective pipelines through `MetaModel`.
4. Packing converts selected layers into packed bins and validates shape-aware geometry before renderer output.
5. CSV fixture suites provide Kotlin comparison baselines and solver-backend diagnostics.

## Public API

| API | Responsibility | Stability |
| --- | --- | --- |
| `application::ColumnGenerationApplicationService` | Application-facing column-generation orchestration. | migration |
| `application::MetaModelSolverBackend` | Pluggable solver backend boundary for RMP/final executors. | migration |
| `application::ColumnGenerationStandardExecutors` | Standard RMP/final executor wiring. | migration |
| `application::TraceValidatedMetaModelFinalExecutor` | Opt-in report-driven final with strict physical replay. | experimental; verification pending |
| `application::Bpp3dOneRoundEvidence` | Actual per-stage models, reports, column pools and final selection for an opt-in one-round flow. | experimental; Rust only |
| `application::CsvDatasetLoader` | CSV dataset loading when `serde` is enabled. | migration |
| `application::Bpp3dRunReport` | Structured run and comparison reporting. | migration |
| `domain::layer_assignment::*` | Layer-assignment context, aggregation, limits, objectives, and dynamic columns. | migration |
| `domain::layer_generation::*` | Layer candidate generation traits, requests, diagnostics, and strategy implementations. | migration |
| `domain::packing::*` | Packing conversion, geometry guard, packed-bin solution, and render adaptation. | migration |
| `domain::packing::PackingGeometryContract` | Injectable final packing geometry validation. | migration |
| `infrastructure::*` | Shared geometry, orientation, shape, PWL, and renderer DTO types. | stable within migration |

## Modeling Extensions

Add solver behavior through `MetaModelSolverBackend` or the RMP/final executor traits. Add new layer candidate sources by implementing the layer-generation traits. Inject business-specific final geometry validation with `PackingGeometryContract` and `ColumnGenerationApplicationService::with_geometry_guard`. Add request-level business rules through domain policies such as package-rule policies instead of embedding arbitrary closures inside serializable models.

New constraints, objectives, and result extraction should be added in domain contexts, aggregations, model components, or pipelines. Application code should compose those extension points rather than duplicating domain modeling logic.

Column-generation lifecycle extensions provide `execute_result` on RMP/final executors with
typed failure stages, `ColumnGenerationRmpModelExtension` and
`ColumnGenerationFinalModelExtension` for additional model content, typed RMP
`additional_shadow_prices`, `create_algorithm_with` for algorithm factories, and fallible
initial-column/candidate-filter entry points.

## Generic Numeric Boundaries

Domain-facing APIs keep generic numeric values where they describe reusable geometry, quantities, and demand semantics. Solver-facing RMP/final execution currently uses `MetaModel<f64>` at the registration and adapter boundary. Conversion to raw `f64` should stay in solver, CSV, reporting, or renderer edges.

## Physical Quantity Boundaries

Widths, heights, depths, weights, volumes, radii, material dimensions, package dimensions, demand coverage, and capacity-like quantities should remain typed through `Quantity<V, U>` or infrastructure geometry wrappers. Renderer DTOs and CSV protocol values may expose scalar fields, but those scalar conversions belong at the serialization boundary.

## Solve Lifecycle

The current application flow is:

1. Load or construct a BPP3D request and candidate source set.
2. Generate BLA/block/circle/Pattern/Pile/Historical layer candidates.
3. Register layer-assignment RMP variables, constraints, objectives, and shadow-price metadata.
4. Solve RMP, extract stable demand shadow prices, generate or refresh columns, and register new layer variables.
5. Solve final MILP over the shared layer-assignment context.
6. Extract selected layers and replay placement/block traces through packing.
7. Validate final geometry and emit solution, KPI, diagnostics, and renderer DTOs.

## Outputs

`ColumnGenerationResult` records selected layers, packing analysis, solver status, and lifecycle diagnostics. `Bpp3dRunReport` and fixture-suite reports capture dataset comparison, solver availability, failures, demand coverage, packed-bin output, selected-layer diagnostics, and feature-matrix information. Renderer DTOs include shape metadata and `actualVolume` output for cuboids and cylinders.

## Usage

```rust,ignore
use ospf_rust_framework_bpp3d::application::{
    ColumnGenerationApplicationService, ColumnGenerationConfig,
};

let service = ColumnGenerationApplicationService::new(ColumnGenerationConfig::default());
let result = service.solve(request)?;
```

CSV and fixture-suite entry points require the `serde` feature.

## Local Validation

```powershell
cargo check -p ospf-rust-framework-bpp3d
cargo test -p ospf-rust-framework-bpp3d --lib
cargo check -p ospf-rust-framework-bpp3d --features serde
cargo test -p ospf-rust-framework-bpp3d --features serde --lib
```

Real-solver manifest suites are feature-gated and require local solver availability.

## Current Boundaries

Current migration coverage includes:

1. Solver-agnostic `MetaModel` RMP/final executors, pluggable backend wrappers, and a one-round RMP -> shadow-price generation -> column refresh -> final flow.
2. Demand coverage metadata on generated layers, stable demand shadow-price keys, selected-layer extraction, and packed-bin/render replay from placement or block traces.
3. BLA local/global layer candidates, simple-block layer candidates, multi-round axis `ComplexBlockGenerator`, bounded DFS space splitting, MLHS branch/depth candidate ranking, circle-packing grid candidates for fixed/discrete cylinder radii, and request-aware Pattern/Pile/Historical generators.
4. Axis-aware cuboid and cylinder geometry, guarded horizontal-cylinder final validation, PWL continuous-radius metadata/fixtures, and renderer `actualVolume` output.
5. CSV schema guards, Kotlin Gurobi grouped-layer/material-width-amount adapters, manifest and recursive directory fixture loading, backend survey/no-run/fake fallback reports, and feature-matrix diagnostics.
6. PatternedItem conservative demand coverage, PackageAttribute validation/packing diagnostics, and RestAmount/TailBinLoadingRate/BinLoadingOrder `MetaModel` registration hooks.

The manifest mirrors the full Kotlin Gurobi CSV sample set into 22 fixtures, including 19 Kotlin-derived dataset samples plus 3 local regression fixtures. Remaining gaps are tracked in the current-boundaries list above and should be compared with the Kotlin BPP3D README when updating migration coverage.

## Multiple bin types and weighted objectives

BPP3D now supports mixed multi-bin-type supply and per-bin-type weighted objectives:

1. `LayerGenerationRequest.bins` produces candidate columns per bin type, each carrying its type through
   `BinLayer.bin`. With no bin type specified the request passes through once, so generators that do not
   depend on a bin type (the historical one) keep working.
2. `BinLayer::identity_key` includes the bin type: the same layout on a different bin type is a different
   column, otherwise the multi-type capability would be erased by deduplication.
3. The primary objective is **minimum bin volume consumption** (`BinVolumeMinimization`). Because the demanded
   volume is fixed, `total waste = Σ bin volume − demanded volume = Σ bin volume − constant`, so minimising
   consumed volume minimises waste. Summing "bin volume − loaded layer volume" would be wrong: a fully packed
   bin would report zero waste and "one full large bin" would beat "several small bins", the opposite of
   reality.
4. The secondary `BinAmountMinimization` coefficient defaults to `1e-6` and only breaks ties between equal
   volumes; `BinTypeCostMinimization` prices per bin type with a default weight of 0 and requires explicit
   configuration.
5. `bin_waste_volume` / `bin_waste_ratio` / `per_type_bin_cost` feed KPI and reporting only, never the
   objective.

Default change: `final_bin_amount_coefficient` moved from a hard-coded `1.0` to `1e-6` and the volume weight
defaults to `1.0`. This is a **behaviour change**; callers that relied on "minimise bin count" must restore it
explicitly in the config.

## Solve backend

`MetaModelSolverBackend`, `NoopMetaModelSolverBackend`, `ColumnGenerationSolverMetaModelBackend` and
`MetaModelExecutionDiagnostics` moved into `ospf-rust-framework` so that BPP2D/BPP3D/CSP1D/CSP2D share one
solving boundary. This crate keeps compatibility re-exports and exposes `layer_count` / `bin_count` read-only
aliases plus `write_bpp3d_info` through `Bpp3dExecutionDiagnosticsExt`, leaving the existing info keys unchanged.

### Actual-Report Backend Foundation

`ReportMetaModelBackend` and `MetaModelStage` are also re-exported from this crate. Construct one backend with an explicit `Arc<dyn LinearSolver>` and `LinearExecutionOptions`, then pass clones to `SolverBackedMetaModelRmpExecutor` and `SolverBackedMetaModelFinalExecutor`. The existing `run_csv_materialized_one_generation_round` flow remains RMP -> original layer generators -> refreshed RMP -> final MILP. It is one generation round, not a complete automatic column-generation loop or complete pricing proof.

The shared backend preserves each accepted actual compiled model, fingerprint/mapping, original `SolveReport` and separate objective constant in `backend.reports()`. Only certified optimal LPs supply original-row pricing duals; final retains integer domains and actual incumbent/terminal semantics. A timeout/cancellation without an incumbent or an uncertified LP stops legacy result extraction instead of using configured no-op vectors, objectives or shadow prices. The typed `backend.execute` entry retains accepted no-incumbent reports without fabricating a vector. Clones share one total cooperative budget, raw progress, independent child cancellation and bounded history; create a new backend per application invocation.

Contract tests cover no-op fallback rejection and pre-cancellation. Feature-gated actual SCIP and Gurobi 10 tests exercise the original CSV one-round flow and retain both LP certificates and the final integer report; they require the respective runtime/license. To run both, explicitly enable `serde,scip,scip-bundled,gurobi10` (the bundled dependency feature alone does not enable this crate's `scip` test gate).

This is **the Rust execution foundation behind the bound BPP3D SDK/HTTP/N-API entry**, not a general BPP3D solver. The SDK exposes portable source validation, explicit source-layer plan replay and a bounded one-round execution report reader; native execution runs only through the native engine and its explicit `bpp3d` strategy. The legacy final extractor still collapses assignments across bins to a selected-layer set, and packing replay can construct fallback placement from coverage when physical traces are missing. Do not treat renderability, an optimal restricted final report or this one-round test as a source-wide feasible/optimal BPP3D certificate.

### Experimental strict final replay

`TraceValidatedMetaModelFinalExecutor` is an opt-in Rust path under verification; it does not change the legacy executor. `execute_with_selection` retains original bin/layer assignments and the backend report even when validation fails. For grouped demand, `execute_with_selection_and_item_amounts` accepts counts in item-catalog order and retains the same assignments, including when physical replay fails. Its strict replay rejects missing traces, ambiguous multi-item placement summaries and partially specified block grids rather than inventing positions. It checks geometry, item IDs, per-layer item coverage, exact item demand and bin weight capacity; `FinalPackingValidation` can add a geometry gate without weakening the base checks. A physical failure is propagated as `FinalMilp` through the application `execute_result` path.

`replay_strict_grouped_packing` reuses those physical and exact-demand gates for a supplied application state, selection, grouped amounts and placement limit, without invoking a solver. The caller remains responsible for binding the state to its source and for any model or solver provenance; passing physical replay is not an optimality proof.

The opt-in `run_materialized_with_traced_initial_layers` and `run_materialized_one_generation_round_with_traced_initial_layers` entries accept existing `LayerGenerationResult` values and carry their physical traces through final extraction. `run_csv_materialized_one_generation_round_with_traced_initial_layers` instead accepts a `LayerTracePair` for each CSV initial layer, in request order, with item indices into `request.items`. It derives empty item coverage from the supplied traces, retains explicit coverage, package attributes and continuous-radius configuration, and passes each unique CSV ID's positive grouped quantity into both RMPs, generation and final validation. The total demand is limited to 100,000 items; strict replay defaults to 10,000 and may be configured up to 100,000. For actual solving use `SolverBackedMetaModelRmpExecutor` with a real backend and `TraceValidatedMetaModelFinalExecutor`; `MetaModelRmpExecutor` uses a no-op backend. Legacy/custom executors reject grouped quantities unless they explicitly implement `execute_result_with_item_amounts`. Trace data is supplied separately, not read from the CSV itself. Empty traces are allowed for unused seeds, but selecting one fails strict replay.

`run_csv_materialized_one_generation_round_with_evidence` creates a fresh report backend for this same flow using an explicitly supplied solver and `Bpp3dOneRoundOptions`. It uses the default RMP and final model configurations. It retains each RMP's actual input state and accepted model/report, the final pool and original block/placement traces, and the actual final bin/layer selection. Earlier stages remain available if a later stage fails, and physical failure retains the final report and extracted selection. Reports rejected by the backend's identity or feasibility checks are not accepted as evidence. This Rust-only in-memory result is not a source-bound serialized report and does not support model extensions or establish global optimality. Its budget and cancellation are cooperative, not a hard interruption of generation or physical replay.

Feature-gated tests exercise strict final and one-round flows with two certified RMPs using actual SCIP and Gurobi 10. The grouped trace path has been run with both for two items in one layer and reuse of one layer across two bins. Broader native acceptance remains outstanding. The built-in grouped application path does not yet attach RMP/final model extensions; custom quantity-aware executors must implement their own extension handling. The legacy materialized and CSV entries still accept bare initial layers without traces. **Legacy CSV application flows do not currently apply `item_amounts` to RMP/final demand**, so they must not be treated as evidence that grouped quantities were satisfied. Initial and generated traces are associated with the first accepted normalized layer identity, never with an unrelated seed or a later duplicate. The gate does not certify arbitrary package rules or source-wide optimality. BPP3D source and plan validation, bounded one-round native execution over HTTP/N-API and its versioned execution-report reader are available through the TypeScript SDK; broader rules, a configurable generator chain and cylinder support remain unbound.

The evidence entry has additionally been tested with both real backends on a grouped instance whose final solution requires an actual two-item `BlockLayerGenerator` column, including its grid trace. Tests also cover a restricted-final infeasibility report and retention of earlier stages after physical failure, cancellation between children and report-limit stops; they do not constitute cross-environment report acceptance.

## Related Modules

- [Root README](../README.md)
- [BPP3D domain README](src/domain/README.md)
- [BPP3D application README](src/application/README.md)
- [Kotlin BPP3D README](../../ospf-kotlin/ospf-kotlin-framework-bpp3d/README.md)
