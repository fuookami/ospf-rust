# ospf-rust-framework-csp1d

:us: English | :cn: [简体中文](README_ch.md)

## Introduction

`ospf-rust-framework-csp1d` is the Rust migration of Kotlin `ospf-kotlin-framework-csp1d`. It provides a reusable one-dimensional cutting-stock framework and keeps downstream request DTOs, formula languages, tenant context, heartbeat logic, and solver plugin selection outside the shared domain crate.

## Scope

This crate owns the reusable CSP1D kernel: material/product models, cutting-plan generation, master-problem produce modeling, yield/waste/length extensions, render DTOs, solution enrichment, recovery, warm start, and application-level MILP/column-generation entries.

Explicit non-goals:

1. Business request protocols, formula languages, project runtime parameters, tenant context, heartbeat logic, or solver plugin orchestration.
2. Downstream-only defect, segmentation, position, or project-specific constraint models until they become general domain entities.
3. Solver backend installation and license management.

## Module Structure

The Kotlin implementation is split into several Gradle modules. The Rust migration keeps the same ownership boundaries inside one crate:

| Rust module | Kotlin module boundary | Responsibility |
| --- | --- | --- |
| `infrastructure` | `csp1d-infrastructure` | Render DTOs and serialization boundary. |
| `domain::material` | `csp1d-domain-material-context` | Products, demands, materials, machines, costars, cutting plans, quantities, and shadow-price keys. |
| `domain::cutting_plan_generation` | `csp1d-domain-cutting-plan-generation-context` | Simple, DFS, N-Same, N-Sum, FullSum, Costar filler, reduced-cost pricing, generation constraints, statistics, and benchmark snapshots. |
| `domain::produce` | `csp1d-domain-produce-context` | Master-problem input, produce aggregation, `MetaModel` registration context, incremental column lifecycle, extension points, policies, and shadow-price lifecycle. |
| `domain::yield` | `csp1d-domain-yield-context` | Under/over-production analysis, yield slack aggregation, yield constraints, and yield objective terms. |
| `domain::wasting_minimization` | `csp1d-domain-wasting-minimization-context` | Trim width, rest material, material cost, over-production area, and waste objective terms. |
| `domain::length_assignment` | `csp1d-domain-length-assignment-context` | Dynamic product length assignment, assigned/over-length slack aggregation, bounds, and length objective terms. |
| `application` | `csp1d-application` | Assignment helper, problem/config builders, MILP and column-generation entries, schedule entry, warm start, recovery, solution enrichment, KPI, trace, and render output. |

## Architecture Overview

The modeling path is centered on `Csp1dProduceContext` and `ProduceAggregation`. Built-in and extension pipelines register variables, intermediate values, constraints, objectives, and shadow-price metadata into `MetaModel`. Plain MILP, column-generation LP master, and final MILP reuse the same produce context and policy families where possible.

Application services build the problem, choose generator and solve configuration, coordinate MILP/LP/final-MILP stages, apply warm starts or recovery, and assemble `Csp1dSolution` plus trace/KPI/render output. Domain contexts own modeling semantics and should not be duplicated in application solver flow.

## Core Concepts

1. `Product`, `ProductDemand`, `Material`, `Machine`, `Costar`, `CuttingPlanSlice`, and `CuttingPlan` define the reusable material domain.
2. `ProduceInput`, `Produce`, `ProduceAggregation`, and `Csp1dProduceContext` define the master-problem modeling surface.
3. `Csp1dShadowPriceKey` and `Csp1dShadowPriceLifecycle` keep LP dual extraction stable across column generation.
4. Generation strategies create initial and pricing cutting plans with feasibility, dominance, cache, and statistics contracts.
5. Yield, waste, and length-assignment contexts add optional constraint and objective families through standard pipelines.

## Public API

### Explicit Native Report Backend

`Csp1dMilp::default()`, `Csp1dColumnGeneration::default()` and the low-level
`Csp1dMilpSolver` retain their legacy heuristic behavior. Enabling a solver feature
does **not** automatically replace it. To run actual LP/MILP solves, explicitly
inject `Csp1dNativeBackend`; column generation needs **both** `lp_solver` and
`milp_solver` set to the same per-invocation backend. Plain MILP only needs
`milp_solver`.

```rust,no_run
use std::sync::Arc;
use ospf_rust_core::solver::LinearSolver;
use ospf_rust_framework_csp1d::{Csp1dColumnGeneration, Csp1dNativeBackend};

fn service(solver: Arc<dyn LinearSolver>) -> ospf_rust_core::Result<Csp1dColumnGeneration<f64>> {
    let backend = Arc::new(Csp1dNativeBackend::new(solver, Default::default())?);
    Ok(Csp1dColumnGeneration {
        lp_solver: Some(backend.clone()),
        milp_solver: Some(backend),
        ..Default::default()
    })
}
```

Construct a fresh backend before **each** domain invocation; do not reuse the
example service for another solve. Keep an `Arc` clone of the backend to retrieve
`reports()` (raw child reports with their stage and separate objective constant).
SCIP pricing requires `SCIPConfig::with_lp_subproblem_defaults()` to retain usable
LP certificates. The backend never selects another solver or fabricates duals.
LP certificates are independently verified; standardized `>=` and split equality
duals map back by real row origins/signs to the registered context. The trace uses
the actual LP objective. Phase I prices real columns at zero cost.

Ordinary/final MILP and legacy extraction share the same context registration,
extensions and policies. Before extraction, report identity, complete vector,
original variable domains/integrality, all source rows and objective are checked.
Rounded plan counts are revalidated, capped at `2^53 - 1` per plan, and aggregate
overflow is rejected. Invalid or no-incumbent native reports never silently fall
back to heuristic business output or an empty `Partial` result. Reports retained
before a later certificate/source-feasibility failure remain raw diagnostics,
not certified results. Warm starts must reference current plans and have safe
aggregate counts; acceptance does not guarantee backend use.

`Csp1dExecutionOptions` provides a shared total cooperative time budget,
cancellation, raw child progress and retained-report limit. Each child gets an
independent completion/cancellation race; generation is not preemptively
interrupted. Concurrent backend execution is rejected. Legacy business status
does not encode every cancellation/timeout detail: inspect the raw child reports.

`reports()` returns independent copies of each actual submitted compiled model,
raw report and separate objective constant. Model-local row identities must not
be replaced by recompiling an equivalent problem. The runtime-independent
framework `remote-codec` offers a separate `CompiledLinearModelArtifact` for
preserving exact token/solver types and row metadata with canonical digest,
core fingerprint and stable-mapping checks. The Rust bridge can validate these
child artifacts with full-vector and optimal-LP certificate checks, including
portable u64 report counters. The bridge additionally binds integer children to
the complete source candidate pool: original-context rows, columns, types,
bounds and objective must agree before production is derived from the actual
vector. This is exposed as the SDK's `CuttingContext.readProductionReport`;
candidate-pool proof semantics are never promoted to source-wide claims.

The separate [bridge](../../../docs/typescript-bridge.md#complete-native-csp1d-execution)
exposes default-rule Roll/Sheet MILP and column generation through the shared
native executor, HTTP and N-API, with explicit SCIP/Gurobi selection. WASM only
handles portable calculations and source-bound integer/complete workflow readers;
it never runs these native solvers. The SDK subpath is `ospf-wasm/csp1d`.
Phase I attaches under-production to original demand rows (no duplicate yield row),
using each demand's own unit, so a restricted infeasible pool can recover through
real pricing, including mixed Roll/Sheet demand. Ordinary yield/hard-demand
registration and legacy heuristic defaults remain unchanged.
The default native initial generator emits single-product, one-slice patterns;
default pricing searches a restricted generator, not all possible patterns.
`PricingConverged`, a restricted master proof or a final-pool incumbent does not
establish source-wide optimality or infeasibility. Optional yield/waste/length
and arbitrary custom policies are not a claim of a complete LP relaxation or an
exact pricing oracle. This boundary has Windows x64 runtime tests for real SCIP
and Gurobi 10 (LP row mapping, MILP, column generation, registered constraints and
infeasibility); other runtimes/versions remain unverified here.

| API | Responsibility | Stability |
| --- | --- | --- |
| `csp1d_problem` / `Csp1dProblemBuilder` | Builder DSL for problem construction. | migration |
| `Csp1dMilp` | Plain MILP application entry. | migration |
| `Csp1dColumnGeneration` | Column-generation application entry with trace. | migration |
| `Csp1dSchedule` | Kotlin-compatible scheduling entry that delegates to column generation by default. | migration |
| `Csp1dMilpSolver` | Low-level Kotlin `Csp1dMilpSolver` parity over `ProduceInput<V>`. | migration |
| `Csp1dNativeBackend` / `Csp1dMilpBackend` | Explicit native report execution; shared budget, cancellation, child reports and strict incumbent gates. | migration |
| `Csp1dSolveConfig` / `Csp1dExtensionSet` | Modeling extensions, policy families, Top-K, partial solution, and recovery controls. | migration |
| `Csp1dModelingExtension` / `Csp1dIncrementalPipeline` | Extension pipeline and accepted-column refresh hooks. | migration |
| `SimpleInitialCuttingPlanGenerator`, `DFSGenerator`, `NSameGenerator`, `NSumGenerator`, `FullSumGenerator`, `CostarFiller` | Initial plan generation. | migration |
| `SimplePricingGenerator`, `ReducedCostPricingGenerator` | Pricing generation. | migration |
| `RenderSchemaDTO` and related DTOs | Renderer serialization boundary. | stable within migration |

The report-driven `Csp1dNativeBackend` now delegates to the shared framework `LinearReportExecutor`, also used by BPP2D. Existing CSP1D API names, options, stage records and wire contracts stay intact. Original-snapshot feasibility and LP certificates are validated before reports enter accepted history; invalid report/backend contracts return errors without heuristic fallback. Infeasible integer reports must also carry reliable, complete, matching core evidence.

## Modeling Extensions

`Csp1dSolveConfig<V>` exposes modeling extensions and a policy set. Modeling extensions wrap a `Pipeline<MetaModel<f64>>` and are filtered by `Csp1dExtensionMode`:

- `MILP`: plain MILP only.
- `LP`: column-generation LP master only.
- `FINAL_MILP`: column-generation final MILP only.
- `ALL`: every stage.

Use `Csp1dModelingExtension::new(...)`, `Csp1dModelingExtension::with_mode(...)`, or the builder helpers `extension_pipeline(...)`, `extension_pipeline_with_mode(...)`, `context_aware_extension_pipeline(...)`, and `context_aware_extension_pipeline_with_mode(...)`.

Extensions that need to react to accepted pricing columns can implement `Csp1dIncrementalPipeline`. `Csp1dProduceContext::add_columns(...)` registers new plan variables, refreshes built-in demand/material/machine/yield/length constraint groups, resets the objective, and then lets incremental extensions adjust the confirmed column batch.

## Extension Policies

`Csp1dExtensionSet<V>` aggregates the same policy families as Kotlin:

- `Csp1dDomainPolicy`: candidate feasibility and width-feasibility override.
- `Csp1dObjectivePolicy`: objective batch-coefficient modification.
- `Csp1dGenerationStrategy`: candidate acceptance, canonical key override, and dominance acceptance.
- `Csp1dPricingPolicy`: reduced-cost cost/benefit modification and custom improvement judgment.
- `Csp1dFlowPolicy`: initial plan filtering, column equivalence, early stop, termination selection, partial acceptance, and recovery fallback.
- `Csp1dExtractionPolicy`: solution enrichment into KPI details and render KPI maps.

Policies have no-op defaults that preserve the built-in behavior. Extraction policy failures are caught during enrichment so one downstream output hook cannot break the solve result.

## Generic Numeric Boundaries

Public domain models use generic numeric values where the quantity semantics are reusable. Solver registration currently converts to `MetaModel<f64>` through explicit material/domain conversion helpers such as `to_f64`, `from_f64`, and `convert_solver_value`. Raw `f64` should stay in solver adapter, registration, extraction, and renderer/serialization edges.

## Physical Quantity Boundaries

Widths, lengths, material dimensions, product dimensions, weights, demand amounts, production amounts, rest material, trim width, assigned length, and over-length values should be expressed through `Csp1dQuantity`, `QuantityRange`, `WidthRange`, or explicit quantity wrappers. Bare numeric values are acceptable for dimensionless configuration limits, counts, statistics, and solver-internal coefficients.


## Warm Start and Recovery

### Warm Start

The framework supports warm start from previous solutions:

- **Previous solution warm start**: Accepts `List<CuttingPlanUsage<V>>` where each element pairs a `CuttingPlan` with a usage amount.
- **Native initial values**: Warm start usages are written as native initial assignment values into the final MILP model.
- **Configuration**: `warmStartPlanUsages` parameter in `Csp1dColumnGeneration` constructor.

### Recovery

The recovery mechanism handles solution adaptation:

- **Previous solution recovery**: `Csp1dRecovery` adapts a previous solution to a modified problem.
- **Compatible subset filtering**: Automatically filters usages for cutting plans still valid in the new problem.
- **Machine capacity recovery**: Special handling for machine capacity constraints with yield adjustments.
- **Fallback control**: `retryWithoutWarmStart` flag controls behavior when warm start fails.

### Error Handling

- `Csp1dRecoveryFallbackDisabledException`: Thrown when recovery fails and fallback is disabled.
- `Csp1dRecoverySolveException`: Wraps solver failures with trace context.

## Solve Lifecycle

The tested lifecycle is:

1. Build `Csp1dProblem<V>` and optional `Csp1dSolveConfig<V>`.
2. Generate or accept initial cutting plans.
3. Register `Csp1dProduceContext` into `MetaModel`.
4. For plain MILP, solve and extract `Produce` directly.
5. For column generation, solve LP master, refresh shadow prices, run pricing, and call `add_columns`.
6. Solve final MILP over the refreshed plan pool.
7. Extract produce/yield/waste/length results, KPI details, render DTOs, Top-K plans, trace, and partial/recovery status.

## Shadow Price Lifecycle

Rust keeps the Kotlin `CGPipeline` shape for shadow-price-aware constraints. `DemandConstraintPipeline`, `MaterialConstraintPipeline`, `MachineConstraintPipeline`, and `YieldConstraintPipeline` register constraints with `Csp1dShadowPriceKey` metadata and implement `Csp1dCGPipeline`.

`Csp1dShadowPriceLifecycle` refreshes framework shadow prices from model constraints and a dual slice, converts them into lightweight `ShadowPriceMap<V>`, and exposes plan-level contribution extraction for pricing. The current service path feeds a placeholder optimistic dual until the real LP solver backend is wired in, but the refresh/extractor API is already in place.

## Generation Semantics

Generation supports max/min knife constraints, max over-production length filtering, canonical de-duplication, candidate filters, width feasibility override from domain policies, same-contribution and cross-contribution dominance pruning, quantity cache statistics, material width-index cache statistics, and sequential slice-template cache statistics. Parallel large-scale generation and the full Kotlin statistics detail set are still being migrated.

## Outputs

`Csp1dSolution<V>` contains selected produce data, optional yield/waste/length results, generated plans, KPI details, render schema, solution status, failure message, and optional Top-K cutting plans.

`Csp1dColumnGenerationTrace` records initial/final plan counts, priced plan counts, iteration records, termination reason, initial/pricing statistics, final MILP status, partial availability, and failure messages.

`RenderSchemaDTO`, `RenderCuttingPlanDTO`, `RenderCuttingPlanProductionDTO`, and `RenderProductionType` are available from `infrastructure::dto`. Enable the `serde` feature to derive `Serialize` / `Deserialize`; DTO fields use Kotlin-style camelCase names such as `cuttingPlans`, `unitLength`, and `standardWidth`.

## Usage

Build a problem directly with `Csp1dProblem::new(...)` or through the builder helper:

```rust
use ospf_rust_framework_csp1d::{
    csp1d_problem, Csp1dColumnGeneration, Csp1dConfiguration, Csp1dMilp,
};

let problem = csp1d_problem::<f64, _>(|builder| {
    builder
        .products(products)
        .materials(materials)
        .machines(machines)
        .demands(demands)
        .configuration(Csp1dConfiguration {
            max_initial_plans: 128,
            max_pricing_plans: 32,
            iteration_limit: 16,
        })
        .solve_config_with(|config| {
            config
                .top_k_plan_limit(Some(10))
                .allow_partial_solution(true);
        });
});

let milp_solution = Csp1dMilp::default().solve(problem.clone(), None);
let cg_result = Csp1dColumnGeneration::default().solve_with_trace(problem, None);
```

`solve_config` can be attached to `Csp1dProblem`, passed directly to `solve(...)` or `solve_with_trace(...)`, or supplied through service-level defaults such as configured generators and warm-start usages. Explicit solve arguments take priority over problem defaults.

## Local Validation

```powershell
cargo check -p ospf-rust-framework-csp1d
cargo test -p ospf-rust-framework-csp1d
cargo check -p ospf-rust-framework-csp1d --features serde
cargo test -p ospf-rust-framework-csp1d --features serde render_schema_serializes_with_kotlin_camel_case_fields
cargo test -p ospf-rust-core remove_constraints_by_group_id --lib
```

## Current Boundaries

The public Rust model covers the Kotlin framework-level entities: `Product`, `ProductDemand`, `Production`, `Costar`, `Material`, `Machine`, `CuttingPlanSlice`, `CuttingPlan`, demand contributions, `Csp1dAssignment`, yield/waste/length outputs, render DTOs, warm start, recovery, and extension policies.

The migration is still in progress. Plain MILP, LP relaxation, and the final MILP stage currently use deterministic heuristic backends while keeping the Kotlin-compatible public surface and model-registration path.

Known pending work:

1. Replace heuristic MILP/LP/final-MILP backends with real solver adapters.
2. Drive shadow prices from real LP duals instead of placeholder dual values.
3. Finish the full Kotlin generator statistics and large-scale/parallel cache behavior.
4. Expand Kotlin fixture parity, real-solver warm-start smoke coverage, and module-level README files.

## Initial restricted-master feasibility and LP terminal states

When the initial LP is infeasible because its columns do not cover every demand, column generation switches to
a separate **feasibility master**:

1. Its objective minimises artificial demand deficits only (`csp1d_phase_one`); real plan columns carry a zero
   objective coefficient. Otherwise "adding columns" would be mixed with the business objective and it would be
   impossible to tell whether the master was merely infeasible.
2. The deficit weight is fixed at 1 and over-production at 0: a deficit is the only source of infeasibility
   that has to be removed.
3. If the feasibility master is still infeasible, or pricing finds no restoring columns, or the iteration limit
   is reached, the failure is kept and flagged as `LpInfeasible`. Artificial deficits are **never** reported as
   production or as an optimal pricing certificate.
4. This only shows that the current generation scope cannot restore feasibility; it does not prove that no
   feasible column exists outside the generator's search.

LP terminal validation (`application::lp_solver`):

1. A pricing-ready LP result requires an **optimal solution with real duals**.
2. Infeasible, unbounded and any other non-optimal terminal state return an explicit error that keeps the
   original terminal description.
3. Missing duals return an explicit error; zeros are never fabricated.

The LP failure reason is written to both `trace.lp_failure_message` and `solution.failureMessage`: the final
MILP may still succeed with the initial pool, in which case the business status is feasible and the reason is
only visible through this message.

### Solver injection boundary

`Csp1dLpSolver` is an injectable interface: an implementation hands the registered `MetaModel` to a real solver
and reports the terminal state, objective and duals faithfully. When nothing is injected, column generation
keeps the existing heuristic shadow-price path, so this crate pulls in no solver dependency; Gurobi/SCIP access
stays behind the feature-gated core/framework backends.

### Rebuilding after restoration

When the feasibility master is optimal with an objective at or below `1e-7` (deficits removed), the deficits
are discarded, the **business master is rebuilt** from the accumulated columns and solved again; after a
successful rebuild this round's pricing uses the rebuilt master's **real duals**. If the rebuild is still
infeasible or abnormal the failure is kept and restoration is not claimed.

### Solver terminal-state adaptation

`application::solver_adapter` is the single adaptation point for real backends: it groups backend terminal
strings into four semantic classes (infeasible, unbounded, optimal, other), allows only optimal to deliver a
pricing result, and explicitly rejects unrecognised and sub-optimal states. Missing duals produce an explicit
message instead of fabricated zeros.

### Not yet done

The explicit `Csp1dNativeBackend` now obtains real Gurobi/SCIP duals through the
unified-report API and verifies complete LP certificates. Legacy default entry
points remain heuristic; HTTP/N-API execution, full workflow reports and
full-pattern-space pricing are not exported by this bridge yet.

## Related Modules

- [Root README](../../../README.md)
- [Migration handoff](csp1d.md)
- [Kotlin CSP1D README](../../../../ospf-kotlin/ospf-kotlin-framework-csp1d/README.md)
