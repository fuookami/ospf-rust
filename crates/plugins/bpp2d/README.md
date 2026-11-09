# ospf-rust-framework-bpp2d

:us: English | :cn: [简体中文](README_ch.md)

## Introduction

`ospf-rust-framework-bpp2d` is the Rust migration of Kotlin `ospf-kotlin-framework-bpp2d`. It provides a
reusable 2D packing framework for rectangular and circular items, with column generation, kerf handling,
remnant reporting and weighted objectives across multiple sheet types.

The 2D geometry is shared with the cutting framework through
[`ospf-rust-framework-geometry2d`](../geometry2d/README.md), so packing and cutting stock
never drift apart on area conventions or overlap semantics.

## Scope

### Discrete Loading

`domain::discrete_loading::Bpp2dDiscreteLoading::context` adapts explicit `LoadingRectangle` bounds
to the dimension-independent shared BPP context. Geometry and weight units have separate explicit
adapters. Coordinates and extents remain in their original numeric domain; the adapter does not
require `Float`. The longitudinal sweep refines candidates through the shared geometry kernel's
normalized rectangle test, excludes touching edges, separates physical spaces and retains explicit
conflict edges. Every position needs exactly one finite, positive rectangle. Layout groups remain
optional and explicit. There is no dependency on BPP1D or aircraft-specific loading rules.

The crate owns the reusable BPP2D kernel: item/sheet/layer models, BLA and block-loading layout, candidate
layer generation, the layer-assignment modelling boundary, final packing with geometry validation, remnant
reporting, and the column-generation application flow.

Explicit non-goals:

1. Business request DTOs, tenant context, formula languages or deployment policy.
2. External renderer implementations.
3. Solver installation and licence management; solver backends stay in `ospf-rust-core-plugin-*` /
   `ospf-rust-framework` behind feature gates.

## Module structure

| Rust module | Kotlin boundary | Responsibility |
| --- | --- | --- |
| [`src/infrastructure`](src/infrastructure) | `bpp2d-infrastructure` | Domain errors and render DTOs; re-exports the shared 2D geometry. |
| [`src/domain/item`](src/domain/item) | `bpp2d-domain-item-context` | Items, sheets, layers, materials, demand statistics and identifier types. |
| [`src/domain/bla`](src/domain/bla) | `bpp2d-domain-bla-context` | Bottom-up left-justified layout; circle items are skipped explicitly. |
| [`src/domain/block_loading`](src/domain/block_loading) | `bpp2d-domain-block-loading-context` | Space model, simple/complex blocks, DFS and multi-row splitting, rotation guard. |
| [`src/domain/layer_generation`](src/domain/layer_generation) | `bpp2d-domain-layer-generation-context` | Candidate layers per sheet type, circle grids, patterns, mixed shapes and historical replay. |
| [`src/domain/layer_assignment`](src/domain/layer_assignment) | `bpp2d-domain-layer-assignment-context` | The modelling boundary: demand coverage, reduced-cost pricing, layered objectives and capacity. |
| [`src/domain/packing`](src/domain/packing) | `bpp2d-domain-packing-context` | Final geometry guard, packer, remnant analysis and render adaptation. |
| [`src/application`](src/application) | `bpp2d-application` | Column-generation orchestration, standard executors, tail sheet optimisation and CSV datasets. |

## Architecture overview

Modelling is anchored on `MetaModel`. The RMP and the final MILP share the same layer-assignment context,
demand coverage convention and objective family. The application layer owns solver selection, the generation
loop, trace/KPI assembly and the packing/rendering boundary; it never accumulates variable or constraint
details of its own.

## Core concepts

1. A **layer** is a layout on one sheet; `SheetLayer` combines layers on the same sheet.
2. **Column identity** includes the sheet type, source, bounding size and placement set: the same layout on a
   different sheet type is a different column.
3. **Demand coverage** is a lower bound that must be satisfied; treating demand as an upper bound only would
   let the minimisation degenerate to "use no sheet at all".
4. **Reduced cost** is `rc_j = sheet_area_j · c_area − Σ π_i · a_ij`; only `rc_j < −tolerance` counts as
   improving. The area is the bound sheet's area, not the occupied item area; it matches the RMP objective.
5. **Objectives** are layered: sheet area consumption is primary (equivalent to minimum waste), the sheet
   amount weight is tiny and only breaks ties, and the per-type cost weight defaults to zero.

## Public API

| API | Responsibility | Stability |
| --- | --- | --- |
| `LayerGenerationContext` | Composite candidate generation per sheet type. | migration |
| `LayerAssignmentContext` | Five-stage column-generation lifecycle. | migration |
| `LayerAssignmentContext::register_model` | Shared actual MetaModel registration for RMP, feasibility LP and integer masters. | initial native boundary |
| `Bpp2dNativeBackend` | Explicit report-driven native execution of registered masters, sharing budget/cancellation and certified LP gates. | initial native boundary |
| `solve_bpp2d_column_generation` | Report-driven Phase I → RMP pricing → same-backend final MILP, retaining the complete pool and actual child models. | initial native workflow |
| `ColumnGenerationAlgorithm` | RMP → pricing → add columns → final MILP orchestration. | migration |
| `ColumnGenerationApplicationService` | End-to-end entry point. | migration |
| `PackingGeometryGuard` / `Packer` / `FreeSpaceAnalyzer` | Final geometry validation, packing and remnant reporting. | migration |
| `TailSheetOptimizer` | Re-selects the tail column against the residual demand. | migration |
| `CsvDatasetLoader` | `grouped-layer` and `material-width-amount` datasets. | migration |

## Modelling extension points

1. `Bpp2dLayerGenerator` — add a candidate generation strategy without touching the loop.
2. `LayerAssignmentObjectiveConfig` — reweight area, sheet amount and per-type cost, or supply type costs.
3. `PackingRotationRule` — inject the rotation knowledge that geometry alone cannot know.
4. `ColumnGenerationConfig` — iteration limit, per-iteration candidates, time limit and reduced-cost tolerance.

### Actual master registration

`LayerAssignmentModelInput { layers, demands, stage }` selects `LayerAssignmentStage::{Rmp, FeasibilityLp,
Integer}`. Pass it to `LayerAssignmentContext::register_model(&mut model, input)`. Registration is
transactional and preserves every input column's position without relying on legacy identity-key
deduplication.

- RMP uses nonnegative continuous copies and `rmp_area_coefficient * sheet.area()`; Integer uses nonnegative
  integer copies and the configured area/count/type-cost objective. Neither inherits legacy
  `Load.less_enabled` nor introduces an artificial copy cap.
- Every demand has one original lower-bound row and, if configured, a separate hard upper-bound row. Only
  FeasibilityLp adds continuous deficits **to the original lower rows**. Real-column costs are zero; the
  artificial objective sums `deficit / lower` for positive lower bounds, with zero demands fixed to zero
  deficit. This is a dimensionless feasibility objective, not business cost.
- Amount demands/upper bounds must match their mode and fit the exact solver-integer range `0..=2^53-1`;
  area demands/upper bounds are finite nonnegative square meters. Duplicate demand keys, conflicting
  sheet/item definitions, empty columns, invalid geometry and non-finite/negative objective settings fail
  before mutation. Columns retain their original order even when their legacy identity strings collide.
- Basic geometry validation checks finite positive extents, bounds and pairwise bounding-box overlap
  through the existing geometry guard. It does **not** authenticate against an external item catalog,
  enforce item rotation permission/kerf or claim mass/load validation. The original `SheetType.max_load`
  shares the length unit parameter and is not a kilogram guarantee. Upstream domain policies must enforce
  these additional rules. Capacity is per sheet, not an aggregate one-sheet area bound across all copies.
- Returned `copy_variables()` allow downstream contexts/pipelines to append business constraints or
  objectives to the same MetaModel. `extract_copies` validates the complete stable-ID assignment against
  the current original snapshot (including added rules, bounds and integrality); it rejects foreign models,
  missing/extra/non-finite values and never silently rounds or expands copies into sheets.
- After validating an optimal LP certificate and restoring signed/split solver rows to original row order,
  use `extract_shadow_prices`. It requires the complete finite dual vector, preserves ItemAmount/ItemArea
  keys and sums lower/upper contributions. It does not itself certify duals or account for extra-pipeline
  pricing corrections. Phase I pricing uses zero real-column cost, not the ordinary area coefficient.

### Report-driven native master execution

`application::Bpp2dNativeBackend::new(explicit_solver, Bpp2dExecutionOptions { .. })` shares the framework
`LinearReportExecutor` with CSP1D, not CSP1D's domain code. Create it once per domain invocation, register
masters through the context, optionally append business pipelines, then call `solve_master(&model,
&assignment)`. The registration must belong to that model and remain active. RMP/FeasibilityLp require
actual LPs; Integer retains integer domains. The same backend instance shares one total budget, raw child
progress, independent child cancellation/completion and a bounded accepted-report history.

`Bpp2dMasterResult` exposes optional complete-pool `copies`, the original objective with the constant
counted once, original-unit Phase I `deficits`, and certificate-gated demand `shadow_prices`. Missing data
is never filled with zero. Source bounds, integrality, added rules, complete vectors, report identity/mapping
and both objective aliases are revalidated. Non-optimal LP incumbents are retained without pricing prices;
reported infeasibility requires reliable, complete, matching evidence. `stage_report` and `reports()` keep
the actual compiled model plus the unmodified core report (except narrowly completing absent identity on
proof-free, incumbent-free cancellation/interruption). Invalid contracts return errors and never enter
accepted history. Final copies are not rounded or expanded; this boundary neither constructs a domain
production claim nor authenticates solver provenance.

The legacy `register_rmp` / `register_final` artifacts and closure-based standard executors remain unchanged
for compatibility. Their existing application loop is not relabeled as report-driven execution; use the
separate native workflow below.

### Report-driven native column generation

`solve_bpp2d_column_generation(explicit_solver, execution, &input, &generators)` creates an independent
report-driven backend per invocation. `Bpp2dColumnGenerationInput` contains a meter-normalized generation
request, assignment demands (including optional hard upper bounds), objective configuration and
`Bpp2dColumnGenerationConfig`. Algorithms stay in Rust. Custom Rust generators implement the existing
`Bpp2dLayerGenerator`; `LayerGenerationContext::default_generators()` supplies the existing heuristic chain.
This workflow uses `generate_explicit`, honoring the supplied chain without discarding custom circle
generators by name; legacy `generate` retains its name-based shape selection.

- Validate unique source item IDs, consistent source sheet definitions, finite positive geometry, nonnegative
  kerf and exact generation counts. Authenticate initial/observed candidate placements against that catalog,
  including actual shape dimensions and orientation. Enforce the original bounding-box geometry guard,
  rotation policy and double-sided kerf checks on both sheet and layer envelope. Circles still use enclosing
  squares, not exact disk collision. Nonzero item weight or sheet load is explicitly unsupported here until
  the original shared length/weight unit parameter is corrected; it is not silently treated as kilograms.
- Preserve every initial column and its position, even initial duplicates. Generated duplicates use
  structured physical-layout keys including sheet ID, item ID, shape dimensions, coordinates and rotation,
  not the legacy source-dependent string key. Generator-provided scores/reduced costs are never accepted as
  pricing evidence.
- Start an empty pool in normalized-deficit Phase I. A certified infeasible nonempty RMP enters Phase I while
  retaining its actual infeasible report. Price only from certified optimal LP duals; Phase I uses zero
  real-column costs and original lower-row deficits. After its normalized objective reaches the execution
  tolerance, return to the ordinary whole-sheet-area RMP. Non-optimal, interrupted or invalid LP evidence
  never drives pricing. Lower/upper dual contributions are combined by the shared domain extraction.
- Recompute finite reduced costs, accept only values below `-reduced_cost_tolerance`, sort improving
  candidates, and append within the complete-pool cap. `iteration_limit` counts pricing batches (including
  unsuccessful batches); candidate, pool and accepted-report caps are explicit. One execution time budget
  includes validation, generation, compilation and all children. Cancellation/time exhaustion before or
  after generation prevents new solves/final MILP. These are cooperative checks, not hard preemption or a
  hard allocation limit for arbitrary generators.
- After a normal candidate/iteration/pool stop, optionally run Integer on the complete pool through the same
  explicit backend. Non-optimal final incumbents keep their real terminal report. Full-pool copies stay
  unrounded, are checked for original bounds/integrality and the exact solver-integer upper range, and are not
  expanded into many sheet objects. `stop` describes **pricing** termination; inspect the final stage's
  report separately for its actual status.
- `Bpp2dColumnGenerationResult.columns` is always the complete pool. `stages` retains actual compiled models,
  original reports, deficits and certified prices; `pool_size` maps each copy vector to the corresponding
  pool prefix. `iterations` records the LP stage index, observed candidate count/cap saturation, and appended
  pool indices with recomputed reduced costs. `final_stage` is absent when no final child ran.
  Contract/backend errors return `Err`, not an invented fallback result.

`candidate_complete` is always **false**. These bounded generators are heuristic, not exact pricing oracles:
no observed improving column is not global convergence, restricted final optimality is not global packing
optimality, and restricted infeasibility is not source infeasibility. RMP area pricing and final configured
business cost can differ. This workflow registers default demand rules only; extra contexts/pipelines remain
supported by `solve_master`, but must not be injected into this pricing loop without matching typed pricing
corrections.

The runtime-independent bridge provides source-bound input/full-execution codecs, phase/pricing validation
and exact-count production extraction from the actual final integer report. The SDK's `ospf-wasm/bpp2d`
subpath validates source/layout DTOs and reads reports through WASM, without running generators or solvers
there. Explicit HTTP/N-API facades share the native execution engine; bounded compressed packing/rendering
delegates to the original pure-computation services. Nondefault rules remain unbound. See the
[bridge boundary](../../../docs/typescript-bridge.md#bounded-compressed-bpp2d-packing-and-rendering).

Real SCIP/Gurobi lifecycle tests include empty/deficient pools, mixed count/area demands, circles, multiple
sheet types, hard upper bounds, zero demand and actual-model codec round trips. For SCIP's tiny
floating-point dual sign overshoots, the adapter may try at most eight ULP steps toward zero; every candidate
must still pass the unchanged exact-binary original-model certificate check. This is not a looser certificate
tolerance or a promise to repair every numerical LP.

### Portable Compressed Packing Bridge

The additive `ospf-rust-bridge::pack_bpp2d_layers` adapter validates source identity, complete physical
columns and increasing positive-copy usages before invoking the existing `Packer`, `FreeSpaceAnalyzer` and
`PackingRendererAdapter`. `ospf-wasm/bpp2d` exposes `PackingContext.packLayers` (one copy per input layout,
not demand verification) and `Bpp2dReport.packing` (only a validated final integer incumbent). Each result is
independently owned. Copies remain compressed; checked u128 sheet/item/material totals become lossless
text/bigint. The default material key remains the finished-item ID.

Packing is independently bounded to 128 placements per selected pattern, conservative aggregate remnant work
`sum((2*n+1)*(n+1)) <= 65536`, and 8 MiB output; errors do not truncate or invalidate existing
reports/production. Rendering carries meters/square meters, physical orientation and original loading order.
Distinct submicrometer origins now retain separate ranks rather than colliding in a quantized-coordinate map.
Remnants retain the original 1e-9 slicing tolerance, enclosing-square circle semantics and no kerf-loss
deduction; they are not recoverable inventory or a cutting-process proof. Nondefault mass/load/rules, tail
optimization and discrete loading remain separate unbound work. No TS algorithm, generator or solver is
introduced in the portable bridge.

## Generic value boundary

1. The domain layer is generic over `V` and keeps the compile-time unit `U`.
2. `f64` appears in `layer_assignment`, the native catalog-validation adapter and the application solver
   boundary; generic geometry/generation kernels stay generic.
3. Generic code must not call `to_f64()`; counting and scaling go through `count_fitting` and
   `scaled_quantity`.

## Physical quantity boundary

Widths, heights, coordinates and areas are `Quantity<V, U>`; utilisation ratios and reduced costs are bare
values. Areas come from `quantity_multiply`, so a squared length is never computed as a cube.

## Solve lifecycle

`register` → RMP solve → `add_columns` → shadow price refresh → pricing → final MILP → `extract_solution`.
Retirement is expressed as removing a column from the aggregation and reporting it to the application layer.

## Output

`ColumnGenerationResult` carries columns, sheets, iteration statistics, per-iteration LP objectives, elapsed
time, the packing result and the render DTO.

## Usage

```toml
[dependencies]
# Workspace-root-relative path / 相对 workspace 根目录的路径
ospf-rust-framework-bpp2d = { path = "crates/plugins/bpp2d" }
```

## Related documents

- [ospf-rust README](../../../README.md) lists every crate and its Kotlin boundary.
- [geometry2d](../geometry2d/README.md) provides the shared 2D geometry.
- [bpp2d.md](bpp2d.md) records the migration handover and known gaps.

## Local verification

```powershell
$env:RUSTC_BOOTSTRAP = "1"
cargo test -p ospf-rust-framework-bpp2d
```
