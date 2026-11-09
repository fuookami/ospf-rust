# ospf-rust-framework-csp2d

:us: English | :cn: [简体中文](README_ch.md)

## Introduction

`ospf-rust-framework-csp2d` is the Rust migration of Kotlin `ospf-kotlin-framework-csp2d`. It provides a 2D
cutting stock framework that reuses the CSP semantics of
[`ospf-rust-framework-csp1d`](../ospf-rust-framework-csp1d/README.md) (products, demands, patterns, plans,
yield, waste, sheet length assignment, column generation) while adopting the generic layering discipline of
[`ospf-rust-framework-bpp2d`](../ospf-rust-framework-bpp2d/README.md): the domain is generic over `V` and
solver value types appear only at the modelling boundary and in the application layer.

The 2D geometry is shared with BPP2D through
[`ospf-rust-framework-geometry2d`](../ospf-rust-framework-geometry2d/README.md).

The built-in master heuristic and service fallback reuse coverage within each solve and lazily evaluate a plan's scoring waste only when it contributes to a deficient demand. This domain-owned cache borrows the assignment, is discarded after the solve and never survives public plan mutations. Candidate order, tie-breaking and usage bounds are unchanged; no background workers or global plan snapshots are introduced.

## Scope

Two supply modes, one model:

| Mode | Problem class | Material model | Decision variables |
| --- | --- | --- | --- |
| Coil cutting | Strip packing (fixed width, length to be determined) | `width` fixed, `length = None` | consumed length and layout |
| Fixed sheet | 2D bin packing | `width × height` fixed | sheet count and layout |

Cutting processes are configurable and orthogonal to the mode:

| Process | Meaning | Equipment |
| --- | --- | --- |
| `Freeform` | Arbitrary nesting, coordinates only | laser, waterjet, CNC |
| `GuillotineTwoStage` | Strips then pieces, emits a cut tree | shear, panel saw |
| `GuillotineThreeStage` | Strips, blocks, pieces, emits a cut tree | shear, panel saw |

Non-rectangular parts are explicitly out of scope.

## Module structure

| Rust module | Kotlin boundary | Responsibility |
| --- | --- | --- |
| [`src/infrastructure`](src/infrastructure) | `csp2d-infrastructure` | Cutting regions, kerf, guillotine cut lines and trees, cutting processes, errors. |
| [`src/domain/material`](src/domain/material) | `csp2d-domain-material-context` | Dual-mode materials, products and demands, patterns and plans. |
| [`src/domain/pattern_generation`](src/domain/pattern_generation) | `csp2d-domain-pattern-generation-context` | Pattern generation requests and the shelf generator. |
| [`src/domain/produce`](src/domain/produce) | `csp2d-domain-produce-context` | Master problem: pattern usage, demand coverage, material availability, objectives. |
| [`src/domain/yield`](src/domain/yield) | `csp2d-domain-yield-context` | Under- and over-production analysis, over-production bounds. |
| [`src/domain/wasting_minimization`](src/domain/wasting_minimization) | `csp2d-domain-wasting-minimization-context` | Remnant area and ratio analysis, waste penalty terms. |
| [`src/application`](src/application) | `csp2d-application` | Problem definition, the MILP path and the column generation path. |

## Cut tree semantics

A guillotine layout is not a set of coordinates but a **cut tree**: internal nodes are through cuts and leaves
are final pieces or remnants. The root cut is stage 1: parallel child cuts keep their parent's stage and
a change of orientation increments it. A two-stage process therefore allows a family of parallel strip
cuts followed by perpendicular piece cuts; stage count is not tree depth.

`CutTree2::validate` rejects inconsistent region splits (including the kerf strip), stages beyond the process
limit, and child stages inconsistent with the parent cut orientation.

The shelf generator constructs row and piece cutting chains. A constructed tree passing this legacy
validator is not by itself proof that finished leaves match the placements. The explicit native gate below
adds that physical correspondence, actual split and resource validation rather than accepting a merely
stage-valid tree as a complete cutting proof.

## Column generation and pricing

Column addition runs in two steps: candidates are generated from the residual demand, then filtered by reduced
cost `rc = consumption · price − Σ coefficient · π`, keeping only `rc < -tolerance` in ascending order.

In the legacy `Csp2dSolvers` service, the shadow prices `π` are **estimated from the coverage deficit ratio**, not taken
from LP duals. This is therefore an engineering approximation of reduced-cost pricing rather than a strict
pricing certificate. The separate explicit native-master API below does not replace that service or its fallback.

## Explicit native report masters

`ProduceContext::register_model` registers real `MetaModel<f64>` variables, objectives and constraints for
`ProduceStage::Rmp`, `FeasibilityLp` and `Integer`. `Csp2dNativeBackend::solve_master` executes that model with
an explicitly injected `LinearSolver`, sharing the framework report executor's time, cancellation, progress
and report-count budgets. It never substitutes the legacy heuristic on backend failure. This API itself
executes masters only; the separate Rust-only workflow below combines registration and generation.
These Rust entries remain native; the explicit SDK HTTP/N-API facade described below delegates to them, never to a browser solver.

- All stages share demand lower bounds, optional hard upper bounds and hard material-stock rows. Count and
  area demands for the same product remain distinct. Phase I adds original-unit lower-bound deficits with
  objective `sum(deficit / positive lower bound)`; real-column costs are zero. It does not relax upper bounds
  or stock. Zero deficits do not identify a unique or minimum-use real-column solution.
- LP usages are continuous; integer usages are unsigned integral variables. The legacy assignment's
  heuristic `usage_upper_bounds` values are not imposed: actual stock rows bound use. Each column use
  consumes **one material batch**, including coil segments; this is not shared-roll-length optimization.
- The default RMP/integer objective is the original plan's consumed-region area. Explicit weights may add
  original waste area, coil segment height and per-use costs. A sheet's region may be smaller than the
  full sheet: neither consumption nor waste is silently promoted to the whole sheet. This does not change
  the legacy service's default waste-scoring policy.
- Registration is transactional and preserves duplicate physical columns and pool order. The solver boundary
  accepts `f64`/`Meter`; count, stock and count upper-bound inputs are restricted to `0..=2^53-1`, while areas
  must be finite and nonnegative. This boundary does not add arbitrary precision to native solvers.
- Physical validation reuses original rectangle/kerf predicates, enforces material/catalog consistency within
  the supplied pool, finite noncollapsed geometry, containment and nonoverlap. A material cannot specify
  both sheet height and coil length; declared material areas must remain finite. Weight, machine,
  remnant-threshold policies and by-product placements are explicitly unsupported here.
- Supplied cut trees are bounded to depth 128 and 4096 nodes **before** recursive validation. Actual cuts,
  cached splits, root and finished-part leaf/placement bijections are checked, followed by the original
  process/stage validation. A legacy generator's tree without matching part leaves may therefore be rejected.
  Floating-point split matching allows only scale-relative last-bit rounding, not the old absolute `1e-9`
  tolerance as a physical proof for tiny parts. No external product catalog is supplied: source dimensions
  and rotation permissions still require caller authentication.
- Reports retain the actual compiled model, identity/fingerprint, original report and objective constant
  counted once. Complete original snapshots (including extra pipelines) are revalidated before extraction;
  usages are neither rounded nor truncated. Nonoptimal incumbents are retained, but only certificate-validated
  optimal LP reports deliver original-row prices. Restricted-master conclusions are not global source proofs.
- `Csp2dMasterShadowPrices` combines each demand's lower/upper contributions and keeps stock-row prices.
  `ProduceContext::reduced_cost` uses the original count/area coverage, stage cost and stock contribution.
  It is not a certificate or candidate-source authenticator. Extra pipelines need matching pricing
  corrections; ignoring those rows cannot establish that no improving column exists.

Append business pipelines to the same model after registration, before execution:

```rust
use std::sync::Arc;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::LinearSolver;
use ospf_rust_framework_csp2d::application::{Csp2dMasterResult, Csp2dNativeBackend};
use ospf_rust_framework_csp2d::domain::produce::{
    Csp2dMasterProblem, ProduceContext, ProduceModelInput, ProduceStage,
};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;

fn solve_integer_master(
    master: &Csp2dMasterProblem,
    solver: Arc<dyn LinearSolver>,
    kerf: &Quantity<f64, Meter>,
) -> ospf_rust_core::Result<Csp2dMasterResult> {
    let mut model = MetaModel::new("csp2d-master");
    let registration = ProduceContext::default().register_model(&mut model, ProduceModelInput {
        master, stage: ProduceStage::Integer, kerf,
    })?;
    if let Some(&usage) = registration.usage_variables().first() {
        model.add_le_constraint(&[(usage, 1.0)], 2.0, "business-use-cap")?;
    }
    Csp2dNativeBackend::new(solver, Default::default())?.solve_master(&model, &registration)
}
```

Real SCIP/Gurobi tests exercise all three stages, empty Phase I, binding stock duals, hard demand caps,
infeasibility certificates, additional rules/constants and original report fingerprints. Fake tests cover
invalid certificates/identities, incomplete assignments, limits and cancellation without pretending to solve.

## Report-driven native column generation

`solve_csp2d_column_generation(solver, execution, input, generators)` drives actual Phase I, certified LP
pricing and final integer masters using the shared registration above. Supply an explicit backend and
ordered `PatternGenerator2<f64, Meter>` chain; no default solver or heuristic-master fallback is chosen.
The original `ShelfPatternGenerator2` can be passed explicitly. The old `Csp2dSolvers` service is unchanged.

`Csp2dColumnGenerationInput` contains one source generation request per material, independent master demands,
initial columns, a shared RMP/final `ProduceObjective` and `Csp2dColumnGenerationConfig`. The requests' product
counts are **generation hints**, not demand constraints or usage bounds; use independent master demands for
count/area lower and upper bounds. All source requests must share a finite nonnegative kerf. Catalog product
dimensions and rotation permission must agree across materials. Source stock rows exist even when that
material has no current column. Coil generation is capped by a declared material length and any smaller
explicit segment-height cap; a segment-height cap on a sheet is rejected instead of ignored.

- The loop starts with RMP for a nonempty pool, or Phase I for an empty one. Certified RMP infeasibility
  triggers Phase I recovery. Actual original-unit deficits, not a small normalized objective alone, decide
  recovery. Only certificate-validated optimal LPs are allowed to drive a pricing batch.
- Each observed initial/generated column is checked against its source material, process, product dimensions
  and rotation permissions, then through the shared physical/kerf/cut-tree gate. This validates consistency
  with the supplied catalog, not its external provenance. Invalid observed candidates fail even when they
  would not improve the objective; unsupported domain rules are not silently dropped.
- Every candidate's reduced cost is recomputed using separate count/area bases, demand upper-bound
  contributions and **stock-row prices**. The legacy generator's area-priced hints combine count price /
  product area and area price only; they are not authoritative pricing evidence. Supplied request iteration,
  history, shadow prices and candidate limits are replaced by actual workflow state; no existing candidate
  pool or price is trusted from those hints.
- Initial duplicate positions are retained. New physical duplicates are not added, independently of plan/
  pattern names and placement order. Identity includes material, region and physical placements; alternate
  valid cut trees for the same layout are equivalent under the supported default objectives.
- Defaults are 20 pricing batches, 32 observed candidates per batch across **all** materials/generators,
  and 10000 pooled columns; each configured cap has a maximum of 100000. Zero iterations still allows a
  final integer master after a certified LP. The original generator is synchronous: observation limits do
  not bound arbitrary generator allocations or preempt its execution. Time, cancellation, raw child
  progress and report count use one shared execution budget, not a hard real-time deadline.
- Output retains the complete pool, each actual model/report with its pool-prefix length, and every
  completed pricing batch's added indices/recomputed costs. Final integer execution uses the same backend
  and entire pool after normal no-improvement/iteration/column stops, unless disabled or the time/report/
  cancellation budget is exhausted. Uncertified or otherwise stopped masters do not start final execution.
  The pricing stop and the final raw report's termination remain separate, including late cancellation.
- `candidate_complete` is always false: existing generators do not provide exact pricing. Even an optimal
  or infeasible final report is scoped to its actual restricted master. An empty generated pool may be
  infeasible while the supplied source problem is feasible. No global source conclusion is fabricated.

This workflow registers the default rows only. Use the explicit master/context boundary for extra business
pipelines and provide matching pricing corrections before integrating them into an iterative workflow.
Do not inject arbitrary TS callbacks as Rust strategies. Strict cut-tree matching still rejects some legacy
shelf guillotine outputs without complete finished-part leaves or exact physical leaf extents; valid supplied
guillotine trees are supported, but automatic tree repair is not part of this loop.

Real SCIP/Gurobi tests run empty-pool recovery with the original shelf generator, incomplete-pool recovery,
binding multi-material stock pricing, final infeasibility under hard demand caps, coil weighted objectives,
valid positive-kerf guillotine candidates and actual child-report identities. Fake contracts additionally
cover budgets, nonoptimal LPs, cancellation races, duplicate layouts, source forgery and tiny normalized
shortages. The bridge now provides a shared lossless source codec, generated source DTOs and portable
`ospf-wasm/csp2d` source/column validation via `CuttingContext`. `Csp2dColumnGenerationInput::validate`
and `validate_columns` reuse original registration/physical gates without generators, solvers or clocks.
They do not prove usage feasibility. `column_keys` exposes original-order physical identities for independent
trace validation, retaining duplicate positions. Shared execution report codecs and the portable
`Csp2dReport` reader now revalidate actual children, original-unit deficits, certified pricing and exact
final production, without global proof claims. `Csp2dColumnGeneration` now uses explicit HTTP/N-API clients
and the shared native executor with bounded configuration, original progress/cancellation and no fallback.
Real SCIP/Gurobi execution, in-process authenticated HTTP and isolated N-API acceptance are separate from
the explicitly gated standalone browser HTTP test. See the
[source bridge limits](../docs/typescript-bridge.md#csp2d-native-workflow-and-portable-source-validation).

The runnable [native example](examples/report_column_generation.rs) starts with an empty pool and explicitly
selects its backend; add the selected solver runtime to `PATH` first:

```powershell
cargo +nightly-2026-08-01 run -p ospf-rust-framework-csp2d --example report_column_generation --features scip-bundled --target x86_64-pc-windows-msvc -- scip
```

For a licensed Gurobi installation, use `--features gurobi10` and the final argument `gurobi` instead.

## Generic value boundary

1. The domain layer is generic over `V` and keeps the compile-time unit `U`.
2. Coordinates, sizes and areas use `Quantity<V, U>`; dimensionless values use a bare `V`.
3. Solver value types appear only in `produce` (the modelling boundary) and in the application layer.
4. Generic code must not call `to_f64()`.

## Public API

| API | Responsibility | Stability |
| --- | --- | --- |
| `Region2` / `CutTree2` / `CuttingProcess2` | Cutting geometry, guillotine trees and process capability. | migration |
| `Material2` / `Product2` / `Pattern2` / `CuttingPlan2` | Dual-mode domain model. | migration |
| `ShelfPatternGenerator2` | Constructive pattern generation with kerf pitch. | migration |
| `Csp2dMasterProblem` | Master problem modelling. | migration |
| `Csp2dSolvers` | MILP path and column generation path. | migration |
| `Csp2dPricingConfig` / `improving_columns` | Reduced-cost pricing and improving-column selection. | migration |
| `SheetAssignment` | Sheet-count and coil-segment-height assignment conventions. | migration |
| `CostarFiller` | By-product filling of the remnant. | migration |
| `Csp2dMasterSolver` | Injectable master-problem solver interface with a built-in heuristic. | migration |
| `ProduceContext` / `ProduceModel` / `ProduceStage` | Transactional real master registration, extraction and explicit LP pricing basis. | explicit native prerequisite |
| `Csp2dNativeBackend` / `Csp2dMasterResult` | Injected report-backed master execution, without heuristic fallback. | explicit native prerequisite |
| `solve_csp2d_column_generation` / `Csp2dColumnGenerationInput` | Report-backed recovery, certified LP pricing and final integer execution with explicit generators. | native; default-rule HTTP/N-API SDK facade |

## Related documents

- [ospf-rust README](../README.md) lists every crate and its Kotlin boundary.
- [csp1d](../ospf-rust-framework-csp1d/README.md) is the mature 1D reference.
- [geometry2d](../ospf-rust-framework-geometry2d/README.md) provides the shared 2D geometry.
- [csp2d.md](csp2d.md) records the migration handover and known gaps.

## Local verification

```powershell
$env:RUSTC_BOOTSTRAP = "1"
cargo test -p ospf-rust-framework-csp2d
```

The explicit-master native tests require the selected solver runtime on `PATH`; Gurobi additionally requires
the matching installation and a valid license. Solver features remain optional and Gurobi versions mutually exclusive.

```powershell
cargo +nightly-2026-08-01 test -p ospf-rust-framework-csp2d --features scip-bundled --target x86_64-pc-windows-msvc --lib application::report_solver
cargo +nightly-2026-08-01 test -p ospf-rust-framework-csp2d --features gurobi10 --target x86_64-pc-windows-msvc --lib application::report_solver
```

### Service workload observation

`tests/workload_profile_contract.rs` uses the existing `Csp2dMasterSolver` injection point to record
aggregate master-call counts, candidate counts, demand modes and placement counts for the complete service
paths. It checks delegation, fallback, final-stage callbacks and shared-recorder concurrency without adding
production counters or background threads. It is a contract-scale observation, not production traffic or
a timing benchmark; it does not measure internal table reads, cache hit rates or native solver CPU usage.

```powershell
cargo +nightly-2026-08-01 test --offline -p ospf-rust-framework-csp2d --test workload_profile_contract -- --nocapture --test-threads=1
```

Native solver thread settings are per instance, not a process-wide budget shared with table builders.
The default service has no automatic parallel table construction. Production adoption requires real input
and reuse-frequency evidence, an explicit native-solver/concurrency budget, and mixed-load tail-latency
validation. Matching candidate counts alone does not establish that two table contents are interchangeable,
and repeated master calls must not be removed when an injected solver may change state between stages.
