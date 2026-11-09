# TypeScript Bridge

English | [简体中文](typescript-bridge_ch.md)

## Responsibility

Rust owns computations, modeling, codecs and execution. The independent `ospf-typescript` repository owns the public npm SDK, **`ospf-wasm`**. This is an initial 0.1 implementation, not full workspace coverage.

- `ospf-rust-bridge`: protocol major 1, generated TS DTO declarations, existing linear/quadratic codecs and canonical CP codec, MetaModel compilation, fingerprints and exact CP report verification.
- `ospf-wasm`: portable wasm-bindgen objects without native solvers or Tokio.
- `ospf-rust-native`: one shared bounded background executor for HTTP and N-API.
- `ospf-rust-host`: authenticated loopback HTTP with explicit CORS and job lifecycle endpoints.
- `ospf-rust-node`: napi-rs bindings and generated low-level declarations.

## Coverage

`summarizeReport` now also exposes `hasOptimalityProof`, `relativeGap`, `absoluteGap`, `nodes` and `solveTimeMs`, so callers can judge the strength of a conclusion instead of inferring it from an objective value alone. Available: Decimal/Rational text construction, addition, subtraction, multiplication, negation, exact comparison and zero tests, plus exact Rational division that rejects a zero divisor, while Decimal division stays unexposed because it is generally inexact; row-major f64 dynamic arrays with shape copies, 2D matrix product, 1D dot product, 2D transpose, square-matrix trace delegating to the original einsum kernel, global/single-axis/multi-axis summation plus prefix sums delegating to the original FastSum kernel (summed axes disappear), 1D `outer`, general axis `contract` delegating to the original generic einsum engine and cross-checked against matrix multiplication and dot products, and `reshape` keeping the original truncate/zero-fill semantics; runtime quantities for m/km/cm/dm/mm plus in/ft/yd and their square units including dm^2, volumes m³/L (1 L = 0.001 m³), s/min/h/kg/g/t/lb with generated quantity DTOs, supporting same-dimension addition/subtraction/comparison, dimensionless scaling, and dimension multiplication/division through the original unit algebra (`L^2`, `L·T^-1`) while rejecting cross-dimension arithmetic; a merged unit may have no vocabulary entry, in which case `symbol()` is empty and `dimension()` is authoritative; shared 2D geometry including exact disk intersection that compares the centre distance against the summed radii, treats tangency as non-intersecting and returns null for non-circular shapes; continuous, binary and exact-f64-range integer variables; linear/quadratic objectives and constraints compiled through MetaModel; lossless model exchange; native Gurobi/SCIP solving, progress, cancellation and reports. Default-rule BPP1D imports two strict CSV tables into the same portable problem (closed schema header, empty inventory means unlimited, identical fingerprint to JSON construction) and has portable physical computation, and HTTP/N-API execution supports `tailWindow` tail re-optimization (only the last N bins are re-solved; a changed packing withdraws the global optimality claim), native candidate-MILP/finite-pool execution and the original domain renderer adapter, which revalidates complete placements before emitting display-unit numbers that are never precision or optimality evidence. CSP1D, BPP2D and CSP2D expose the explicit source/report and native workflows documented below; shared BPP sparse loading exposes portable modeling and validation through the existing native linear execution path, with a separate original ordinary-MILP application entry and continuous-resource Benders, with explicit partition lifecycle primitives but a separate original native root-partition application.

BPP3D has a portable, source-bound cuboid and axis-aware cylinder protocol, SDK source validation, strict source-layer plan replay and a bounded, versioned one-round execution report revalidated against the source, both RMP children, the final MILP and strict grouped physical replay, described below. The original one-round flow runs only natively: HTTP, N-API and SDK expose explicit `bpp3d` execution and a Rust report reader instead of solving in WASM.

Object-based `Linear` and `LinearInequality` are available through `MetaModel.linearExpression` and variable `toLinear`. Immutable arithmetic, term simplification and evaluation reuse Rust math. Relations capture independent expression snapshots and normalize constraint constants in Rust. Cross-model expressions are rejected by Rust as well as the SDK. Disposing a model invalidates its expressions and relations; each object still needs disposal. Evaluation requires all current variables in model order. Nonzero objective constants remain explicitly unsupported rather than silently discarded.

Ordinary linear and quadratic export checks both the round-trip core fingerprint and the actual solver variable types, which core fingerprints alone do not cover. Models whose execution metadata or types cannot survive that codec are rejected with `Unsupported`. The separate compiled-linear codec retains those types and metadata for the explicit child-model execution paths; it is not an automatic fallback.

Raw Rust quadratic models with nonzero constraint polynomial constants are also rejected at this export boundary: the core fingerprint does not cover those constants and shifting them to the right-hand side can round them away. WASM-created relations normalize their constants before registration.

Token handles now reference a persistent Rust MetaModel: bigint identities survive structural mutations within the same model/instance, but are not transport identities. `addLinearSymbol` and `addQuadraticSymbol` register real Rust `LinearExpressionSymbol` and `QuadraticExpressionSymbol` values. Their respective `toLinear` and `toQuadratic` methods retain the current symbol as a direct dependency, exposed as a copied BigUint64Array by `dependencyIds`. Both use the same name namespace, preserve their original terms and evaluate through Rust. Disposing a handle does not unregister the symbol. Model disposal invalidates every associated handle; other symbol specializations remain unbound.

`setVariableRange(variable, { lowerBound, upperBound })` replaces the Rust token's bounds transactionally without changing its identity or prior exported artifacts. Omitted continuous/integer endpoints are unbounded; omitted binary endpoints default to 0/1. Integer endpoints remain within the exact f64 range. A successful change clears all explicit assignments; invalid bounds and cross-model variables leave the model and assignments untouched.

`setSolution` binds complete explicit assignments after shape, finite-value, bound and integrality checks; it does not prove feasibility or optimality. Failed assignments are atomic; successful structural mutations clear assignments. `result`/`evaluateSolution` return undefined when required assignments are absent. Duplicate symbol names and cross-model references are rejected.

Quadratic modeling is available: `Linear.mul(Linear)` reuses Rust polynomial multiplication, while `toQuadratic` promotes losslessly. `Quadratic` supports arithmetic, evaluation, copied dense-linear/sparse-quadratic coefficients and `le/ge/eq` snapshots. Commutative terms are canonicalized; sparse coefficients encode each polynomial term once, without an implicit cross-term factor. `tryToLinear` only succeeds when all quadratic terms cancel. `addInequality` registers quadratic constraints in Rust MetaModel. `setQuadraticObjective` registers its linear part in MetaModel and applies Q at the Rust tetrad compilation boundary, since core MetaModel objectives themselves remain linear. `toQuadraticTetradModel` reuses the existing remote codec and core fingerprint; encoding verifies the round trip and rejects information the codec cannot retain. Linear export rejects quadratic constraints or nonzero Q. Replacing the objective with a linear one clears Q; later variables zero-pad its dimensions.

`solveQuadratic` explicitly submits QP/MIQP/QCP models over HTTP or N-API, reusing job progress, cancellation and report semantics. SCIP requires the native `scip-quadratic` feature in addition to its runtime source/linking feature; Gurobi uses the selected mutually exclusive version feature. A browser builds the model but never runs those native solvers inside WASM. Clients require the advertised `quadratic` capability; no silent linear downgrade occurs.

The initial exact CP builder, independent canonical snapshots, portable activity analysis and a candidate funnel (`candidateFunnel`, priority-ordered and grouped, assigning only Tier C/Unclassified because no dual evidence is available) are available through the SDK `core` entry. See the boundaries below.

### Native Serial Framework Orchestration

`SerialCombinatorialLinearSolver(client, configuration)` from `ospf-wasm/framework` invokes the host's built-in `serial-linear` strategy, reusing the actual Rust framework solver rather than implementing fallback in TS. Rust generates `SerialLinearOptions`, `NativeBackend` and `SerialFallbackPolicy`. Backends must be explicit, ordered, distinct and nonempty; every configured backend must be compiled. No backend or HTTP/Node path is silently skipped or substituted, and arbitrary TS callbacks are not accepted. The SDK copies configuration and requires the `serial-linear` descriptor's `serialLinear` capability. `serialOptions` is accepted only for this strategy and linear/omitted modelKind, never CP options.

The default stopOnCertifiedTerminal stops on the first incumbent or certified mathematical terminal. continueOnTerminal tries later backends after certified infeasibility/unboundedness but still stops on an incumbent. Cancellation, interruption and timeout stop the chain; a timeout without an incumbent remains TIME_LIMIT rather than BACKEND_FAILURE. The time limit applies per backend solve, not as an end-to-end orchestration deadline. Progress is the latest child snapshot, not overall workflow percentage. The shared bounded executor, job status, polling, cancellation and release remain authoritative.

For this strategy, JobView.reportJson contains raw `CombinatorialSolveReport<f64>`, not a standalone SolveReport. `CombinatorialReport.fromJson(text, artifact)` uses the shared Rust codec to validate schema, core aggregate/attempt contracts, source linear-model fingerprint and incumbent/pool shapes, without independently proving feasibility/optimality. `summary()`/`reportJson()` read the selected unified report. `attemptIds()`/`attemptJson(id)` retain every attempt trace (errors/terminal summaries, not complete child reports); `selectedAttemptId()`/`selectionReason()` retain selection identity/rationale. Exact u64 metadata stays Rust-owned text, never parsed/re-encoded through JS numbers. Reports own independent data and require disposal even after source/job release.

Runtime/license failures remain attempt diagnostics; no usable result yields core's UNKNOWN/BACKEND_FAILURE aggregate rather than fabricated success. Missing model identity on pre-cancelled/all-failed no-incumbent aggregates is bound only at the authenticated native execution boundary. Other column-generation/domain strategies, other Benders variants and arbitrary context/pipeline/strategy registration remain unbound; fixed-continuous-recourse Benders is documented separately.

### Native Parallel Framework Orchestration

`ParallelCombinatorialLinearSolver(client, { backends: ['scip', 'gurobi'], mode: 'best' })` invokes the actual native Rust parallel solver through the built-in `parallel-linear` strategy (`parallelLinear` capability). `ParallelLinearOptions` and `ParallelCombinatorialMode` are Rust-generated. All configured backends must be compiled; the explicit ordered list is distinct and bounded to two workers. Omitted mode uses Rust's `best` default: wait for all workers and select by the source model's objective direction, breaking ties by configured order; without incumbents it retains the first certified mathematical terminal rather than inventing a backend failure. `first` accepts the first selectable report, requests cooperative FrameworkLoser cancellation of others and **still joins all workers** before completing the job. It does not guarantee immediate return or a winner that is globally optimal.

`parallelOptions` requires a linear model and cannot coexist with `serialOptions` or CP configuration. HTTP and N-API share the same bounded single-active-job executor; worker concurrency is internal to that job and does not allow concurrent job submissions. Time limits apply independently to each backend solve, not to model decoding or an end-to-end deadline. Progress remains the latest child snapshot, so interleaved worker events are neither monotonic overall progress nor a final-job marker. Only JobView.status indicates completion, after workers are joined. External cancellation propagates to children; cancelling losers does not cancel the parent or rewrite an already completed backend result. The same `CombinatorialReport` reader preserves attempts, exact metadata, selection and source identity. No parallel runtime is enabled in WASM.

Network flow from the original `network-scheduling` crate is bound through `modelKind: networkFlow` (the SDK `NetworkFlowReport` keeps flows, spills, shortages and all four cost components); generalized flow is bound too through `modelKind: networkGeneralizedFlow` (production yield and unit conversion really take effect, and `GeneralizedFlowReport` keeps arc and node results apart), network design is bound through `modelKind: networkDesign` (shared construction choices plus nominal flow, with construction and operating cost kept separate), network structure is bound through `modelKind: networkStructure` (rooted tree plus facility ownership, with no commodity flows registered), transportation allocation is bound through `modelKind: networkAllocation` (shared quotas and hub construction, with shipped amounts delivered per request-service pair), resilience planning is bound through `modelKind: networkResilience` (explicit failure scenarios plus connectivity requirements, with each scenario reported separately and `NetworkResilienceReport` keeping the realized worst cost apart from the solver epigraph), vehicle routing with time windows is bound through `modelKind: vrp` (branch and price, with routes delivered in stop order and a budget that can only tighten the algorithm), so every `network-scheduling` sub-domain is now bound; quadratic orchestration is bound through `modelKind: quadraticOrchestration` (several quadratic backends serially or in parallel with an explicit result-selection policy). Not yet bound: the remaining math surface and symbol specializations, advanced CP sessions/assumption-conflict shrinking, CP persistent sessions, other analysis/checkpoint APIs (a fixed-integer LP sensitivity entry is bound natively through `modelKind: linearSensitivity` and needs a feasible incumbent, with a `LocalConstraintSensitivityReport` SDK reader validated by the original core `validate`) (an IIS entry is bound natively through `modelKind: linearIis` and requires the Gurobi backend, and the SDK exposes a read-only `LinearIisReport` reader that validates self-consistency in Rust/WASM and never computes an IIS), complete framework orchestration (a multi-backend sweep reuses the framework `ExperimentRunner` through `modelKind: linearSweep`, returning one verdict per case sorted by case ID; the certified `pricing_duals` certificate is bound: `CertifiedPricingDuals.verify` re-checks an optimal LP report against the actual model before handing out duals keyed by stable constraint IDs), BPP/CSP execution variants beyond the shared BPP loading, BPP1D and default-rule BPP2D/CSP1D/CSP2D scopes documented here, and Network; Gantt binds a bounded activity-scheduling source codec, explicit-plan replay against the native rules, a native single-MILP strategy (`solver: "gantt-activity"`, `modelKind: ganttActivity`, `ganttActivityOptions`) executed over HTTP/N-API, and the `ospf-wasm/gantt` SDK entry with a Rust report reader; solver proofs, material flows, baselines, execution facts and arbitrary policies remain unbound. CSP1D has an explicit Rust-native report backend and SDK HTTP/N-API execution entries; its legacy Rust defaults remain heuristic. Products may declare a length and unit weight, and `weight` demand converts through the original geometry. Costars are declarable, roundtrip losslessly and can be referenced from explicit plans, but the original `CostarFiller` has no in-domain caller, so default generators emit no costar slices. See its [native boundary](../ospf-rust-framework-csp1d/README.md#explicit-native-report-backend). The current core CP constraint families have factories, not every public core operation. Nonzero MetaModel objective constants and codec-inexpressible metadata remain explicitly unsupported in the ordinary core exchange API; the separate compiled-child entry preserves its own metadata and exposes objective constants independently. An LP solution pool is bound natively through `modelKind: linearSolutionPool` on the SCIP and Gurobi backends, with a read-only `LinearSolutionPoolReport` SDK reader that never certifies optimality. These are not exposed as successful placeholders. The detailed module matrix is maintained in the [Chinese companion](typescript-bridge_ch.md).

### BPP3D Portable Source And Rust Report Foundation

`ospf-wasm/bpp3d` exposes `PackingContext` for default cuboids, individual candidate bins and traced initial layers. Rust generates the DTO declarations, preserves demand as canonical decimal text, binds the canonical source to a domain-separated fingerprint and materializes the existing CSV-application inputs. Each initial layer is replayed through the original strict physical adapter and geometry guard, including allowed orientations, complete single-item coordinates, overlap, bounds and per-layer weight. Source validation alone does not establish complete demand satisfaction, and the fingerprint does not authenticate provenance. The SDK accepts `bigint` demand and returns independent DTO copies; dispose each context explicitly.

`packLayers` strictly replays an explicitly supplied list of source-layer/bin assignments, in source-bin and source-layer order, checking combined depth, geometry, total load and exact grouped demand across all bins. A layer can be reused in different bins; duplicate assignments to the same bin are rejected. It returns source-bound, absolute meter coordinates, but neither generates columns nor identifies a solver or proves optimality. A plan permits at most 4096 assignments and 4096 expanded placements, independently of the source demand limit; plan text is limited to 8 MiB.

Inputs allow 64 items, 64 individual bins, 256 layers, 256 placements per layer and 4096 total placements; total positive demand is at most 100000, and text is limited to 8 MiB. Repeated bin types represent distinct candidate bins but must have identical normalized dimensions, capacity and main flag. Only cuboids, axis-aware cylinders and the six original orientations are accepted; unknown shapes, custom callbacks, implicit multi-item placement summaries and unsupported fields are rejected. A cylinder carries its alignment axis and radius, takes its axial length from the matching dimension and is checked by the original domain geometry guard against real circular footprints rather than bounding squares. Lengths normalize to meters; mass and capacity must have mass dimensions and normalize to kilograms. The existing BPP3D domain stores those kilogram values in `Meter`-parameterized weight/capacity fields: the boundary checks dimensions, but the internal Rust type does not provide compile-time mass safety. The execution report reader, HTTP/N-API execution and the SDK `ExecutionReport`/`Bpp3dOneRoundExecution` entries are described below; discrete or continuous radius candidates and additional domain rules remain pending; CSV one-generation-round entries now forward declared item quantities, while the full-flow CSV entries reject grouped demand explicitly.

The shared `ReportMetaModelBackend` now plugs into BPP3D's existing RMP/final executors. Its one-round native CSV flow retains actual compiled models and original child reports, provides pricing duals only after full optimal LP certification, and prevents no-incumbent terminals from falling through to legacy no-op defaults. Clones share total budget, progress, cancellation and accepted history. Actual SCIP/Gurobi 10 tests cover RMP, generation refresh and final MILP; the same flow is now dispatched by the native engine for HTTP/N-API and its bounded execution report is read by WASM/SDK without re-solving. Legacy assignment-to-layer extraction can lose cross-bin multiplicity and missing physical traces can trigger coverage-based replay fallback; the separate strict-final path described below is required. See [BPP3D execution boundaries](../ospf-rust-framework-bpp3d/README.md#actual-report-backend-foundation).

An opt-in Rust strict-final path now retains assignments and rejects missing or ambiguous physical evidence, inconsistent item coverage, unsatisfied demand and overweight bins. Dedicated materialized entries accept traced initial layers; a CSV-materialized one-round entry also accepts externally supplied per-layer traces, derives missing item coverage from them, and preserves package and continuous-radius context. This explicit entry carries grouped `item_amounts` through quantity-aware executors, generation and strict physical validation; legacy CSV application flows still do not apply them to RMP/final demand. Initial and generated traces are associated with the accepted normalized layer identity rather than merely source and depth. Actual SCIP and Gurobi 10 tests cover grouped items in one layer and reuse across bins, as well as the original strict-final and one-round flows with two certified RMPs. Execution reports also retain the original flow's `continuous_radius_*` info entries (selected count, per-variable radius/squared/segment, prototype and weight-function counts), validated only for self-consistency since the text is never treated as an authoritative solution. A `radiusComponent` declaration (prototypes with unit-bearing bounds plus business weight functions) reaches the original continuous-radius component, so the final model registers real radius, radius-squared and PWL variables; a missing weight function or zero coefficient fails at the boundary instead of surfacing an opaque final-stage diagnostic. Cylinders also accept discrete `radius_candidates` (`min`/`max`/`step`, expanded exactly like the CSV path and bounded by the cross-section), which drive the original circle-packing layer generator. Sources also accept a `package_attributes` subset (`allow_mixed_loading`, `max_stack_layers`, `tags`) matching exactly what the CSV path materializes; other package rules keep the domain defaults. Broader native acceptance, grouped model extensions, CSV-embedded traces, additional source rules and further objective policies remain unfinished.

The Rust bridge also has a source-bound one-round column-pool codec and an individual RMP/final stage-report codec. The pool retains generated block grids and placement traces with integer fields as decimal text; the stage reader rebuilds the default grouped-demand model, compares its complete mathematics with the actual submitted child, validates the original report and derives final assignments from the incumbent. It does not authenticate the generator or solver, and an assignment is not a physical packing until strict replay succeeds. Limits are 512 columns, 16384 trace entries and 8 MiB per text. The complete execution-report lifecycle is exposed end to end: the Rust WASM reader (`Bpp3dPackingContext.readExecutionReport`), the HTTP/N-API one-round strategy and the SDK `ExecutionReport`/`Bpp3dOneRoundExecution` entries described below.

`encode_bpp3d_execution_report`/`decode_bpp3d_execution_report` complete that stack with a bounded, versioned one-round execution envelope that keeps the actual initial-RMP, refreshed-RMP and final child models and original reports plus both pools. Decoding re-derives each child from the source-bound pool, requires the initial RMP to solve exactly the source initial prefix at iteration 0, requires the refreshed RMP at iteration 1, requires the final pool to be verbatim identical to the refreshed pool, rechecks every optimal-LP claim against an exact dual certificate, requires any report-free child to terminate a failed run, and finally re-runs strict grouped physical replay with the recorded expansion budget before exposing source-bound packing. A missing final incumbent, a certified-infeasible final, an analysis failure or an insufficient expansion budget keeps the real evidence and the original failure text: `completed` and `failure` in the summary carry the outcome, and `candidateComplete` is always false because preregistered heuristic generators are never replayed. The whole report is limited to 8 MiB, three stages and 100000 expanded placements. The native engine dispatches this flow for HTTP and N-API with shared total budget, progress, cancellation and report limits; the WASM reader and the SDK `ExecutionReport` never re-solve.

The executed chain is explicit: `bpp3dOptions.generators` accepts 1..=7 distinct registered strategies (`block`, `blLocal`, `blGlobal`, `circlePacking`, `pattern`, `pile`, `historical`) and defaults to the original standard chain without the history replay, because a history generator that re-emits an initial layer identity produces a duplicate column without a physical trace, which the strict final must reject (`FinalMilp: selected assignment has no physical trace; coverage fallback forbidden`). No arbitrary callbacks are loaded and `candidateComplete` stays false for every chain.

### Exact CP and Activity Analysis

`snapshot.withAssumptions()` calls core `snapshot_with_assumptions` for literal (including negation), equal, lowerBound, upperBound and sparseDomain assumptions. `variableId` explicitly addresses a stable ID in the receiving snapshot, not a builder handle. It returns an independent snapshot without mutating the source; Rust rejects unknown IDs, duplicate identities, non-subset domains and invalid values. Temporary batches are freed on success and failure. Inputs retain bigint/i64 precision. Individually valid contradictory assumptions remain an infeasible model. Fingerprints reuse core canonical encoding; validate native reports against the derived snapshot, not the original. This is not an incremental session or assumption-aware conflict diagnostic.

`snapshot.analyzeMipLowering()` synchronously executes the same whole-model execution lowering with default budgets in WASM, then drops the temporary MIP. A nonzero objective constant adds one fixed auxiliary column to the statistics and budgets. It invokes no solver, checks no runtime/license and proves no feasibility. Rust generates the `MipLoweringAnalysis` discriminated union: lowered (source fingerprint/statistics), unsupported (constraint/formulation), inexact (lossless original text), budgetExceeded (resource/limit/actual) and invalid (message). It returns the first lowering failure, not a complete per-constraint support report. Preflight has no cancellation/custom-budget API, and success guarantees neither backend capability/readiness nor solving success. Cumulative remains disabled by default. Protocol major stays unchanged; derived snapshots use the existing canonical CP payload.

`ConstraintProgrammingModel` owns a real core CP builder, separate from the f64 `MetaModel`. Factories cover every constraint family in the current core AST: exact integer relations, Boolean literal/and/or/xor/atMostOne/exactlyOne, implication, reification, expression all-different, Element, allowed/forbidden assignment tables, no-overlap, Cumulative, Circuit, Automaton and Reservoir. Arithmetic and validation execute in Rust. Registration clones the builder and commits only after core freeze validation, preserving the previous state on failure. This is constraint-family coverage, not a claim to bind every public core operation.

`allDifferent`, table columns, Circuit successors and Automaton inputs accept owned variables or integer expressions. `element({ index, values, target })` retains exact bigint table values; the whole declared index domain must fit the table. `implication(literal, constraint)` and `reification(literal, constraint, direction)` capture independent consequent AST copies. Directions are `equivalent`, `impliedByLiteral` and `impliesLiteral`; literals accept a variable or `{ variable, negated }`. Tables check row widths before creating BigInt64Array, retain empty/zero-column tuple semantics, and enforce a binding-side maximum of 1,048,576 rows (separate from smaller native lowering budgets). Temporary expression batches and variable-conversion handles are freed internally, including on failure.

`addInterval` accepts bigint or an owned nonnegative integer variable as duration; presence accepts a variable or a negated Boolean literal. Use `IntegerDomain.boolean()` for presence variables. Cumulative tasks pair intervals with fixed bigint demand and capacity. Automaton states are checked signed i32 numbers before TypedArray conversion; transition labels are exact bigint. Caller-created Int32Array values that have already wrapped cannot be repaired. Reservoir accepts time/change expressions and exact level bounds; simultaneous changes are aggregated by core before checking bounds. All these objects retain source ownership and snapshot/disposal rules.

`IntegerDomain` preserves sparse values and the entire signed i64 range. Scalar inputs are bigint checked before conversion to text; batches use BigInt64Array, with readonly bigint arrays range-checked before allocation to avoid silent wrapping. Full-domain cardinality is bigint (2^64). A BigInt64Array already truncated by caller code cannot be repaired. Expression evaluation uses builder registration order; snapshot assignment uses `snapshot.variableIds()` canonical stable-ID order, which may differ.

`freeze()` creates an independent immutable `ConstraintProgrammingSnapshot`, valid after builder mutation/disposal. Model-owned expressions, variables and constraints are invalid after model disposal; all WASM handles require individual disposal. `toJson`/`fromJson` reuse the core canonical artifact codec, checking schema, identities, fingerprint, canonical bytes and artifact digest. `toArtifact`/`fromArtifact` additionally verify the outer model fingerprint. Keep inner JSON opaque: JS parse/re-encode destroys exact integers and canonical bytes. `intervalValuesJson` validates a complete feasible assignment, omits absent intervals, and retains exact interval values in text. Import or factory coverage does not imply native lowering support.

`ConstraintActivityAnalyzer` performs solver-free analysis of complete CP candidates or fingerprint-checked linear artifacts. CP candidates may violate constraints for diagnostics. `ConstraintActivityReport.integerLhs/integerRhs` return exact bigint evidence for integer relations; report JSON also retains those integers. Slack, normalized metrics and variable-bound metrics are core f64 values, not arbitrary-precision guarantees. Other evidence returns undefined from integer getters. Report import checks the supported schema and core report invariants. Reports are independent of their source model/snapshot lifetimes. This is not IIS, conflict extraction, sensitivity or re-solving, and does not itself perform CP solving. The separate native CP path is described below.

### Native CP Execution

SCIP/Gurobi descriptors advertise `cpExactLowering` only when the corresponding native backend is compiled. SDK `solveConstraintProgramming(snapshot.toArtifact(), options)` checks that capability and explicitly submits `modelKind: 'cp'`. Old hosts without it are rejected before submission. HTTP and N-API use the same bounded executor, progress, cancellation handle, job storage and raw report transport as linear/quadratic jobs; no browser solver or automatic fallback is involved. `timeLimitMs` bounds the backend solve, not an interruptible deadline for all codec/lowering work.

The SCIP path reuses `ScipConstraintProgrammingSolver`; Gurobi reuses `MipBackedConstraintProgrammingSolver<GurobiSolver>`. Both use core's default strict finite lowering budgets and reject unsupported, over-budget or inexact models instead of rounding or dropping constraints. Supporting full i64 snapshots does not imply all i64 models can cross the f64 MIP boundary. Runtime numeric/budget errors retain core's `NUMERICAL` class; unsupported constructs retain `UNSUPPORTED`. WASM can derive assumption-restricted snapshots for ordinary CP submission. Persistent sessions, native assumption parameters, assumption-conflict shrinking and custom lowering budgets are not yet exposed by the bridge.

For nonzero CP objective constants, execution lowering uses a fixed-one auxiliary column so the backend's objective, bounds, gap and progress include the constant. The source report excludes that column. For every CP objective, including zero-constant objectives, the absolute constant plus the sum of absolute coefficient/domain bounds must not exceed `2^53`; this conservative gate can reject cancellation-heavy expressions. The existing general Rust lowering API continues to expose the constant separately. New hosts advertise `cpObjectiveConstant`; when it is absent, the SDK uses the initialized Rust WASM codec to reject nonzero constants and runs the complete current default-budget MIP preflight before submitting zero-constant snapshots. That synchronous preflight can reject other unsupported or over-budget models and cannot be interrupted. Old SDKs do not enforce this gate.

New native runtimes advertise `intermediateProgress` for compiled SCIP and Gurobi. Direct solver jobs can publish intermediate snapshots with indeterminate percentage, throttled to 100 ms by default; they may include an incumbent, bound and gap when available. The final report remains authoritative. Older hosts without the capability do not promise intermediate snapshots. Polling observes only the latest snapshot, so short solves and individual iterations can be missed.

`ConstraintProgrammingSolveOptions` reuses Rust-generated `ConstraintProgrammingOptions`. `nodeLimit`/`solutionLimit` are u32 values, including zero for immediate NODE_LIMIT/SOLUTION_LIMIT reports. `solutionHint` is a ReadonlyMap of stable IDs to bigint; generated `IntegerSolutionHint` DTO entries carry canonical decimal i64 text, never JSON numbers. Rust caps hints at 100000 entries and rejects duplicate IDs and invalid encodings. Core validates IDs/domains and source feasibility for complete hints. Partial hints are projected only to source MIP variables, not auxiliary variables; backend adoption is not guaranteed. Zero limits do not search; returning a validated complete hint as an incumbent does not prove a backend solve took place.

`requestConflict: true` invokes core `solve_constraint_programming_with_conflict`, requesting whole-submitted-model evidence only, not assumption-aware deletion shrinking or IIS/irreducibility. Core gates fallback seeds on verified infeasibility proofs and preserves authoritative evidence. Missing proof yields UNAVAILABLE evidence/issues rather than fabricated members. `infeasibilityEvidenceJson` returns original evidence text; `diagnosticIssueCodes` and `diagnosticExtension` read unified diagnostics with the report's independent lifecycle.

The optional `SolveRequest.cpOptions` field requires explicit `modelKind: 'cp'`. Before sending it, the SDK also requires advertised `cpSolveOptions`; omitted configuration preserves the existing default CP path. Older hosts cannot silently drop hints/limits/conflict requests. Native linear/quadratic submissions reject CP options, as do unknown configuration fields. Protocol major remains 1 and all DTO declarations are Rust-generated.

In particular, Circuit, Automaton and Reservoir are available for WASM modeling/source validation but remain unsupported by the native lowering path. Cumulative lowering is disabled by core defaults and is not silently enabled by this SDK. Both HTTP and N-API acceptance tests require explicit `UNSUPPORTED` failures for all four families; Element, tables, reification and variable-duration/negated-presence intervals have real SCIP/Gurobi solve gates.

Native reports stay `SolveReport<i64>`, not f64 summaries. `ConstraintProgrammingReport.fromJson(reportJson, snapshot)` checks the unified report schema/invariants, source fingerprint, complete stable/canonical assignment, original CP feasibility, pool assignments and exact objective. It reads typed values/objective/bound as bigint, provides copied BigInt64Array values, and preserves the whole report as text. The report remains valid after snapshot disposal. Infeasible, limited and cancelled solves keep existing unified-report semantics. Source checking is not an independent optimality/proof re-solve. Do not use the f64-oriented `summarizeReport` for CP exact values. Early cancellation without a core fingerprint is bound to the verified submitted snapshot at the executor boundary.

## Initial Portability Audit

Only currently exported call paths are audited, not every Rust public API. Numeric operations, arrays, quantities, shared 2D geometry, linear/quadratic compilation, CP snapshots, activity analysis and fingerprints do not use host clocks, randomness, threads or the filesystem. Each Worker initializes its own instance. Clock-dependent cancellation and stop acknowledgements, local object storage, background execution, native solvers and Tokio clients remain native-only; future WASM exports must adapt those dependencies explicitly. `remote-codec` reuses codecs without enabling Tokio. Node's loader reads only the packaged WASM resource. Transfers are copied and object disposal is explicit; f64 modeling is not arbitrary-precision modeling.

Validation includes a real wasm32 build, wasm-bindgen tests in Node, and SDK loading in browsers, Workers and Node. Compilation alone is not a portability guarantee.

## Build

The existing base crate requires nightly. Use `nightly-2026-08-01`; newer installed nightlies change allocator APIs incompatibly. No bootstrap bypass is required.

```powershell
rustup toolchain install nightly-2026-08-01
rustup target add --toolchain nightly-2026-08-01 wasm32-unknown-unknown
cargo +nightly-2026-08-01 install wasm-bindgen-cli --version 0.2.129 --locked
./scripts/build-wasm-bindings.ps1
./scripts/build-node-bindings.ps1 -Features scip-bundled,scip-quadratic
```

By default candidate packages are generated under `target/npm`. The WASM script follows Cargo's configured target directory, including `CARGO_TARGET_DIR`; the Node script still writes to the workspace `target/npm`. Generated WASM declarations reference TypeScript's disposable library and require TypeScript 5.2 or later. SDK production dependencies use exact package versions, not neighboring source paths. Native packaging initially targets Windows x64. Check SCIP shared-runtime dependencies and license obligations before distributing packages; a linked `.node` is not by itself a portable distribution. Gurobi runtime and licenses must be installed separately; its DLLs are not redistributed.

The loaded SCIP runtime must match the headers used to build the binding exactly, including the patch version. A loaded but mismatched runtime is rejected before creating a SCIP problem with an `ENVIRONMENT` error; a missing or incompatible DLL may fail earlier during loading. Capabilities report compiled features, not runtime availability.

## HTTP Contract

Set `OSPF_TOKEN` to a random printable ASCII token of at least 32 characters. Set comma-separated `OSPF_ORIGINS` explicitly for browser access. Run:

```powershell
cargo +nightly-2026-08-01 run -p ospf-rust-host --features scip-bundled,scip-quadratic
```

The host binds to `127.0.0.1:8787`; `OSPF_BIND` accepts loopback addresses only. All application requests require Bearer authentication. Endpoints under `/ospf/v1` are `GET /capabilities`, `POST /jobs`, `GET /jobs/{id}`, `POST /jobs/{id}/cancel`, `GET /jobs/{id}/result`, and `DELETE /jobs/{id}`.

There is one active solve, no queue, and storage for 64 jobs. Release terminal jobs explicitly. No persistence or process-restart recovery is implied. Body size is limited to 9 MiB and inner model text to 8 MiB. Public deployment requires a secured HTTPS reverse proxy; this host is not a multi-tenant dispatcher.

The `http_network` host test binds a real loopback TCP listener and exercises authentication, CORS, SCIP solving, progress, cancellation and shutdown. The server and Rust test client run in one process; this is not the independent host executable or SDK/browser HTTP acceptance. The SDK separately tests real socket disconnects and cancellation races against a fake HTTP server.

`SolveRequest.modelKind` is an additive optional protocol-1 field: omission preserves `linear`, while `quadratic` selects its own codec and native solver path. Unknown kinds are rejected. The generated Rust declaration owns this field; the SDK checks capabilities before sending it, so an older linear-only host is not treated as a quadratic host.

`cp` selects the canonical CP codec and native exact finite lowering path. The SDK requires `cpExactLowering` before sending it; protocol major remains 1 and no old-host CP support is inferred.

## Data and Validation

The existing model schema 1.0 is reused, with signed Infinity strings for unbounded variables. NaN is rejected. MetaModel integer modeling uses the exact f64 range; the separate CP builder uses exact i64 and is submitted only through the explicit CP transport kind and conditional exact lowering. Raw model/report JSON crosses HTTP and N-API as strings and is validated in Rust; core fingerprints bind model semantics. Normal solver outcomes remain unified reports.

SDK objects have explicit `dispose()`. TypedArray transfers copy data. Variables cannot cross model ownership boundaries. Complete multi-solve algorithms and compiled strategy extensions remain native-side.

```powershell
cargo +nightly-2026-08-01 test -p ospf-rust-bridge -p ospf-rust-native -p ospf-rust-host --lib
cargo +nightly-2026-08-01 test -p ospf-rust-framework --features remote-solver --test checkpoint_wire_contract
```

Release low-level Rust packages first, run SDK integration gates, then release the public SDK. Generated declarations, glue, WASM, native binaries and test evidence are build artifacts, never Git sources. Initial workflows build candidates only and do not publish automatically. Real Gurobi validation requires its own configured runtime; fake solvers are contract tests only.

### Native Fixed-Continuous-Recourse Benders

`FixedRecourseAlgorithm(client, configuration)` invokes the actual Rust framework algorithm through `fixed-recourse` / `fixedRecourse`, using an explicitly selected `scip` or `gurobi` backend. Only linear source artifacts are accepted. `FixedRecourseOptions`, stages, stop reasons and summary types are generated from Rust. This is one built-in strategy, not arbitrary Benders, column generation or domain-pipeline registration.

`stages` uses source exchange column indices, never process-local token IDs. Explicit continuous recourse is required for nonzero-cost recourse variables: omitted stages retain Rust's default of keeping integers and nonzero-cost variables in the master. Integer recourse and unsupported partitions fail explicitly. The existing Rust partition/certificate/cut machinery is reused; missing certificates may be recovered within the remaining budget, invalid existing certificates are not replaced, and there is no full-MILP fallback. A bounded recourse box may be needed to seed a finite master bound; unsupported cases remain `unsupported`.

`timeLimitMs` is the algorithm's total solve budget, including master, subproblem and certificate recovery solves, each receiving the remaining time. Codec/partition initialization precedes that budget and is not an interruptible end-to-end deadline. Progress remains raw latest child progress, not an overall percentage. HTTP and N-API share the bounded single-active-job lifecycle and cooperative cancellation. Partition errors can occur after submission; SDK errors retain `jobId` so the failed job can be released.

`FixedRecourseReport.fromJson(job.reportJson, artifact)` owns independent Rust data. `summary()` exposes the actual stop, complete source-order incumbent, source-direction objective/best bound, iteration/cut counts, native terminal status/reason and retained backend error. For maximization the best bound is an upper bound. Iteration/time/cancellation stops can retain an incumbent; they are not fabricated success. `masterReportJson()` and `subproblemReportJson()` retain the last actual child reports as exact text, including their own proofs/identities; certificate-recovery terminal metadata is retained separately. They are **not** a unified source-model SolveReport or an independent source optimality certificate. Import validates schema, source fingerprint, full assignment/domain, objective, bound consistency, counters and core child-report invariants; it does not replay cuts to prove the bound. Release jobs and dispose reports separately.

Portable Rust report readers explicitly use `u64` metadata counters (statistics, provenance and iteration traces); native execution APIs keep their default `usize`. Exact integers, including counts above 2^53 and through u64::MAX, remain report text, not JS JSON numbers. Only the generated safe summary is parsed by the SDK.


### Native Source-Projected Column Generation

`ColumnGenerationAlgorithm(client, { backend, blocks })` invokes the native `column-generation` / `columnGeneration` strategy. `ColumnGenerationOptions`, block definitions, stop reasons and summaries are generated from Rust. Blocks explicitly map distinct source exchange column indices; unmapped variables remain global. Each block is a local configuration with exactly one selected column, **not** an unbounded multiplicity pattern master for arbitrary packing domains. Blank/duplicate blocks, repeated/unknown variables and arbitrary callbacks are rejected. Limits: 256 blocks, 100000 mapped variables, 10000 RMP rounds/columns; defaults are 128 rounds, 10000 columns and tolerance 1e-7.

The actual Rust `ProjectedColumnGeneration` uses `ColumnProjection` throughout: empty-pool artificial-variable Phase I (no big M), independently validated optimal RMP LP certificates and row mappings, local pricing models with original bounds/integrality/local rows, one improving column per block/round, atomic batch addition, dual invalidation/rebuild, then a restricted integer master and complete source-assignment validation. Cross-block/global rows enter local pricing only through duals. Pricing masters omit column upper bounds already implied by convexity, avoiding spurious negative reduced costs from redundant bound duals; existing ordinary master defaults remain unchanged. SCIP uses its explicit LP-subproblem preset to retain usable duals.

`timeLimitMs` is a total solve budget including RMP, pricing and final; each child gets the remaining time. Source codec/decision initialization precedes this budget, and compilation is not preemptible. Without an explicit limit Rust uses 60 seconds. Progress is the latest raw child snapshot, not an overall percentage. HTTP/N-API share cancellation, job storage and release; errors after receiving the job ID retain `jobId`. A lost submission response can leave an unknown job: the host has no job-list or idempotent-submission endpoint. No source-MILP fallback, implicit backend substitution or TS-to-trait conversion occurs.

`ColumnGenerationReport.fromJson(job.reportJson, artifact)` validates schemas, source identity, counters, source-domain full assignments/objective and core child-report contracts. `summary()` distinguishes Phase I completion, Phase II pricing convergence, limits/stalls and actual child termination; infeasibility/unboundedness remains in child reports and is not fabricated into a source proof. `masterReportJson()`, `pricingReportJson()` and `finalReportJson()` retain the last actual reports with u64 metadata as lossless text; `pricingBlock` identifies the last pricing block. The final solve is **only over generated columns**: even completed pricing and an optimal restricted integer master do not establish global source-MILP optimality, and restricted infeasibility does not prove source infeasibility. There is deliberately no source `bestBound` or source SolveReport proof. Iteration/column limits may still finalize a feasible current pool after Phase I; time/cancellation do not start a new final solve, but retain a validated incumbent if returned by an already running final solve. Dispose reports separately from releasing jobs.

This does not expose arbitrary context/pipeline registration, dynamic column-removal policies, branch-and-price or all domain workflows. Those remain subsequent implementation work, not silently emulated SDK functionality.

### Shared Two-Dimensional Geometry

`ospf-wasm/geometry2d` provides a unit-explicit `Geometry2d` context and Rust-generated DTOs under natural names: `QuantityPoint2`, `QuantityVector2`, `PackingShape2`, `Box2Geometry`, `Placement2`, `Sheet2Shape`/`Sheet2Geometry`, `Shape2Candidate`. Mixed m/km/cm/mm inputs normalize to the original Meter/f64 specialization. `countFitting(cell, container)` reports how many cells fit along one direction and keeps the original kernel's zero result for non-positive sizes. Outputs use the context unit and corresponding `m^2`/`km^2`/`cm^2`/`mm^2` area units, never mislabeled lengths. The existing `Quantity` facade also supports these square units with runtime dimension checks.

Point/vector addition/subtraction, y-before-x ordering, 90-degree rotation/candidates, actual/bounding areas, pitch, shape/placement inflation, box translation/relocation/bounds and point/box/sheet containment delegate to original Rust APIs. `overlapPairs` accepts at most 512 boxes and returns all pairs as a copied interleaved u32 `Uint32Array` (i,j with i<j, in input order), without truncation or packing/solver proof claims. DTOs and arrays are independent data; only the context needs disposal. They can cross contexts and are not cross-model handles.

Incompatible dimensions, invalid shapes, nonfinite values, nonpositive extents, negative spacing, overflow/underflow and bounds collapsed at coordinate precision fail explicitly. Input text is limited to 8 MiB; non-null itemId must contain 1..1024 UTF-8 bytes. The original strict f64 predicates have no added tolerance; unit conversions remain subject to floating-point rounding. `rotated` records the supplied physical shape's orientation and does not swap its dimensions again.

**Existing semantic boundaries**: original `Box2Geometry` uses enclosing squares for circles. Overlap tests box interiors and containment tests closed boxes, not exact disk collision/containment; diagonally disjoint disks may still have overlapping squares. Original `inflated_by_placement` preserves the lower-left corner, not the centre. Earlier native comments were corrected without changing kernel behavior. This bridge is not BPP2D/CSP2D execution, a domain geometry guard or an arbitrary-high-precision WASM specialization.

### Shared BPP sparse loading

`ospf-wasm/bpp` now exposes original `DiscreteLoadingContext` default-rule registration and validation:
independent cargo, explicit sparse assignments, true count/weight capacities, whole-position occupancy,
non-transitive conflict edges and explicit layout groups. Positions may also carry optional 1D intervals or 2D
rectangles (never both); the original geometry guard and the original 2D discrete-loading factory then derive
conflict edges for strict overlaps inside one space only, so touching edges and separate spaces create none. Fixed cargo can remain in disabled positions;
empty reservations and external occupancy are preserved. No loading rules are reimplemented in TS and
no coordinates or 3D collision guarantees are implied.

Schema-1.0 canonical source text has a domain-separated SHA-256 digest. Pre-registration `with_namespace`
includes that source digest in actual model names, preventing reports from being exchanged between
mathematically equal sources with different cargo identities. Default Rust context isolation remains
unchanged. The linear bridge now preserves fixed binary [0,0]/[1,1] bounds while rejecting fractional,
unbounded or reversed binary intervals. Positive full-u64 `maxCount` uses text/bigint, even beyond 2^53:
original capacity registration tightens coefficients to real sparse adjacency, never a floating-point big M.
Explicit kg/g quantities delegate to quantities conversion; nonfinite/negative weights and conversion
overflow/underflow fail. Budgets are 512 items, 128 positions, 4096 assignments, 2048 edges, 64 layout groups,
512 layouts total and 4096 layout-position memberships, with 8 MiB each for source text/execution model.
Duplicate/reversed edges, duplicate assignments/members and unknown references are rejected. Conflict
endpoints are checked before original index construction to bound work on invalid sources.

`toLinearTriadModel()` exports the original **zero-objective feasibility MILP** when costs are omitted, solved through existing
SCIP/Gurobi facades and explicit HTTP/Node clients. It is not the full BPP application service or a maximum
loading optimizer; optional cargo need not be selected. `solutionValues` returns copied Float64Array data;
`verifySolution`/`extractSolution` validate complete rows/bounds/integrality and original domain rules.
`readReport`/`DiscreteLoadingReport.fromJson` require actual exported-model identity, full incumbent and
objective/domain consistency while preserving raw report text and exact u64 metadata. Only actual
incumbents yield independent `DiscreteLoadingResult` objects; no-solution terminals never fabricate zero
plans. At the authenticated native submission boundary, missing pre-cancel identity uses the existing
BPP1D binding rule without overwriting present identities, terminals or proofs. Mathematical consistency
does not authenticate external solver provenance.

Real SCIP/Gurobi cases cover fixed/disabled cargo, infeasibility, capacity, layouts, empty reservations,
cancellation and units; authenticated in-process HTTP and isolated N-API acceptance remain separate.
`OSPF_BPP_LOADING_FIXTURE_DIR` supplies ignored evidence to SDK/browser/Worker readers, not a cross-repo
source or release dependency. Arbitrary business extensions and branch-and-price remain pending; BPP3D is bound only for its cuboid and axis-aware cylinder one-round flow.
partition lifecycle/resource-completion primitives and the original native root-partition application are documented separately; arbitrary TS
callbacks are not automatically converted into Rust extensions.

#### Original ordinary loading application

`DiscreteLoadingContext(problem, objective?)` accepts the generated `LoadingObjective` minimizing costs:
`itemCosts`, `positionCosts` and `layoutCosts: [{ groupId, layoutId, cost }]`. Negative costs reward selection;
occupancy costs include empty reservations. Unknown/nonfinite or duplicate layout terms fail before registration.
The original transactional context registers all costs. Optional objective data enters the canonical source digest;
omission preserves existing zero-objective source encoding.

`DiscreteLoadingApplicationService(client, { backend })` invokes the original ordinary-MILP Rust service through
`solver: bpp-loading`, `modelKind: bppLoading` and exclusive `bppLoadingOptions`. Capability gates require both
`bppLoading`/`bppLoadingMilp` and the explicit compiled LP/MIP backend. HTTP/N-API share factory/solve budgets,
progress, cancellation and release; default time is 60 seconds, with no implicit transport or solver fallback.
Bounded codec work is not a hard interruptible deadline.

`toApplicationModel()` exposes the actual snapshot compilation, not the existing mechanism export identity.
The original BPP default limits use explicit stable row IDs so independent registrations with the same source
namespace have identical application fingerprints. Default context isolation is unchanged. Original solver reports
are never relabeled or rewritten to match another compile path. `DiscreteLoadingApplicationReport.fromJson` and
`readApplicationReport` validate full model identity, incumbent, minimizing objective and domain extraction;
`solverReportJson` preserves exact metadata. Pre-factory cancellation/time limits retain proof-free unbound core
reports and `registrationPresent: false`, with no fabricated plan. Arbitrary domain extensions remain unbound;
partition primitives and the original native root-partition application are documented separately.

#### Bounded continuous additional weight

`DiscreteLoadingContext(problem, objective?, resources?)` accepts generated `LoadingResource[]` in its third
argument: 1..128 entries when supplied, with unique IDs and at most one per known position. Fields are `id`,
`positionId`, kg/g-bearing `minimumWeight`/`maximumWeight`, nonnegative dimensionless
`minimumPerLoadedWeight`, and finite `costPerWeight`. Costs are per `problem.weightUnit`; changing the model
from kg to g does not implicitly rescale prices. Negative costs are allowed, but every variable remains bounded.
The Rust registration adapter adds continuous variables and minimum-cargo ratios; the original context's
`link_additional_weight` enforces true capacity and whole occupancy. Stable additional-row IDs preserve
independent snapshot reconstruction. Resource definitions enter the source digest, including business identity.

With resources, `solutionValues(plan, weights)`/`verifySolution(plan, weights)` require a complete explicit
`ReadonlyMap<string, PhysicalQuantity>`. Missing/unknown resources, wrong units, invalid ratios, bounds,
capacity or occupancy are rejected, never filled with guessed zero/optimal values. Full-vector extraction and
both report readers validate continuous values too. `result.resources()` separately returns `weights`,
`positionTotalWeights` and `totalWeight`; existing `loadedWeight`/`positionWeights` remain cargo-only.
Weights use the model unit and returned DTOs are independent copies. Omitting resources preserves old source
and result encoding. This extends ordinary MILP execution, not decomposition by itself; shared-BPP Benders is documented next.

#### Original Shared BPP Continuous-Resource Benders

`DiscreteLoadingBendersApplicationService(client, { backend, recourseResources, iterationLimit?,
recoverMissingCertificates?, fallbackToMilp? })` invokes the original Rust application. Select 1..128 unique
declared resource IDs, mapped to this registration's actual continuous variables, including nonzero-cost recourse.
Raw tokens, empty recourse and JS callbacks are rejected. Iterations are 1..10000 (default 128), the total budget
defaults to 60 seconds and tolerance is 1e-7. Original missing-certificate recovery is enabled by default, without
replacing invalid evidence. Only explicit `fallbackToMilp: true` permits fresh-equivalent-factory MILP after a
recoverable non-cancellation failure. HTTP/N-API share `bpp-loading`/`bppLoading` jobs with nested
`bppLoadingOptions.benders`; the SDK requires `bppLoadingBenders` and the selected LP/MIP backend, never
silently downgrading to ordinary MILP or HTTP. The ordinary facade still accepts only `{ backend }`.

Original registration, fixed recourse, certificate/cut validation, progress and cooperative cancellation remain in
Rust. Initial construction consumes the same total budget. Encoding retains the initial `LinearModelSnapshot`,
never interpreting decomposition values through fresh fallback tokens. Actual fallback text is retained separately,
even when the better decomposition incumbent is kept. Cancellation/time exhaustion/convergence do not launch
fallback. Real SCIP/Gurobi acceptance covers optimality/feasibility cuts, negative recourse costs, g units, iteration
limits, cancellation and fresh-factory fallback, not compilation alone.

`DiscreteLoadingBendersReport.fromJson`/`context.readBendersReport` validate source/configuration, actual full
model, resource plans, decomposition incumbent/bound consistency, atomic schemas, fallback selection and proof
boundaries. Reading **does not independently replay every cut, certify the bound or authenticate external solver
provenance**. Even converged merged source reports remain proof-free `FEASIBLE`, or `UNKNOWN` without an
incumbent; an infeasible master is not promoted into source infeasibility. Only an actual full-source MILP fallback
may retain its own proof. `decompositionSummary`, `decompositionReportJson`, `fallbackReportJson`,
`decompositionError` and `fallbackUsed` preserve separate evidence without relabeling children. Outer text is
bounded by 64 MiB + 4 KiB; application text retains 16 MiB + 1 KiB and fixed-recourse/fallback texts each retain
8 MiB limits. Exact u64 child metadata stays nested lossless text. Plans dispose independently. Arbitrary strategies,
branch-and-price and 3D geometry remain unbound; partition primitives and the original root application are separate.

### CSP2D native workflow and portable source validation

CSP2D now has explicit Rust `ProduceContext` registration shared by RMP, normalized-shortage Phase I and
integer masters, and `Csp2dNativeBackend` execution through the framework `LinearReportExecutor`. Real
SCIP/Gurobi 10 tests verify original reports, LP certificates, separate count/area demand bases, binding
stock duals, hard demand caps, extra rules/constants and infeasibility. Only certified optimal LPs provide
prices; nonoptimal incumbents remain reports, not pricing certificates. The legacy service and heuristic
fallback are unchanged. Each pattern use consumes one material batch, even for coil segments; consumed/waste
area uses the original region, which may be a partial sheet, not an invented whole-sheet charge.

`solve_csp2d_column_generation` now connects Phase I recovery, certified LP pricing and final integer execution
with explicit original generators. It validates source material/process/product/rotation consistency and
physical geometry before pricing; prices retain count/area, demand upper bounds and stock contributions.
Actual original-unit shortages decide recovery, not a tiny normalized objective alone. One shared execution
budget covers all children; the complete pool, actual child models/reports and pricing trace remain intact.
`candidate_complete` is always false: restricted optimality/infeasibility is not a global source conclusion.
Real SCIP/Gurobi 10 tests run the original shelf, recovery, multi-material stock pricing, hard upper bounds,
coil weighted costs and supplied valid positive-kerf guillotine columns.

Execution remains **native-only**, now exposed through shared `NativeEngine` HTTP/N-API jobs and the SDK
`Csp2dColumnGeneration` facade. Requests require `solver: "csp2d"`, `modelKind: "csp2d"` and explicit
`csp2dOptions`, mutually exclusive with all other strategies. The facade requires both domain/algorithm
capabilities and the selected SCIP/Gurobi backend's LP/MIP capabilities; it never falls back to HTTP or WASM.
The shared bridge now generates source DTO declarations and provides `encode_csp2d_problem` /
`decode_csp2d_problem`: schema-1.0 canonical text with a domain-separated SHA-256 fingerprint, retaining
counts as decimal strings. `ospf-wasm/csp2d` exposes `CuttingContext` construction, `fromArtifact`,
`toArtifact`, `toJson`, `validateColumns` and independent disposal in browser, Worker and Node. Validation
reuses original source/physical-column gates and MetaModel registration, without generators, solvers or clocks.
It does not establish demand satisfaction, solver provenance or global optimality.

SDK counts use bigint in 0..2^53-1 (larger values are rejected); stock must be positive, and per-material
positive generation hints total at most 4096. Sources allow 64 products, 16 materials, 128 demands and 256
initial columns; validation pools allow 512 columns/4096 placements. Source/column text is limited to 8 MiB;
the outer artifact allows 16 MiB + 1 KiB for JSON re-escaping, while its inner source remains limited to 8 MiB. Cut trees
use flat postorder nodes with the last node as root, distinct earlier child indices and no shared/orphan
nodes; limits are 4096 nodes/tree, depth 128 and 65536 nodes/pool. Lengths/areas normalize to meters/square
meters; f64 physical predicates remain strict. Catalog consistency is not external provenance authentication.
Some legacy shelf guillotine trees lack
matching finished-part leaves and fail the strict gate rather than being silently repaired. Unsupported
weight, machine, remnant-threshold and by-product rules fail explicitly. Extra pipelines need corresponding
pricing corrections. See [the native workflow boundary](../ospf-rust-framework-csp2d/README.md#report-driven-native-column-generation).

`encode_csp2d_execution_report` / `decode_csp2d_execution_report` now bind the complete physical pool and
all actual child models/reports to the source artifact. Original child identities and report text remain
unchanged; rebuilt `ProduceContext` models must match full mathematics, types, bounds, objectives and execution
metadata. The reader checks LP/infeasibility certificates, replays Phase I/RMP/pricing/final transitions and
recomputes accepted reduced costs including hard count/area caps and stock contributions. Recovery checks
original-unit deficits, not a tiny normalized objective alone. This is mathematical consistency, not solver
provenance authentication or independent exhaustive-search proof.

`CuttingContext.readExecutionReport` and independent `Csp2dReport.fromJson` run through WASM in browser,
Worker and Node. The report retains original text, raw child reports, complete columns, pricing iterations
and Rust-derived summaries. `production()` returns an independently disposable `ProductionResult` only
from a revalidated final integer incumbent; missing/fractional-only results never fabricate production.
Piece/batch counts accumulate checked u128 before decimal-text/bigint conversion, with exact demand and
stock rechecks; physical areas remain finite f64. Consumption uses the actual region, waste includes kerf
strips, and coil-height totals do not introduce a shared-roll-length rule. `candidateComplete` stays false;
final restricted infeasibility is not source infeasibility. Late cancellation never rewrites a completed child.

Execution text is limited to 8 MiB. Generated configuration allows 0..64 pricing batches, 1..512 observed
candidates/batch, pool 1..512, 1..1024 reports and reduced-cost tolerance in [0, 0.5); defaults are 8/128/512/196
and 1e-6. `finalIntegerEnabled` defaults true; only the original shelf strategy is registered, and strict
guillotine failures are not repaired or replaced. `OSPF_CSP2D_FIXTURE_DIR` supplies ignored real SCIP/Gurobi
evidence to SDK/Worker/isolated-package acceptance, not a release dependency. Actual SCIP/Gurobi shared-engine,
authenticated in-process HTTP-route and isolated N-API tests cover solving, progress, limits, no-final results,
late cancellation and release. Standalone browser HTTP acceptance is explicitly gated on `OSPF_E2E_URL` and
`OSPF_TOKEN`; without a configured host it is skipped, not counted as passing. Further domain rules remain unfinished.

### BPP2D native workflow and portable source/report validation

The original BPP2D layer-assignment context has transactional shared MetaModel registration for RMP, normalized-deficit Phase I and integer masters, with hard demand lower/upper bounds, checked solver-number conversion and original-snapshot extraction. `Bpp2dNativeBackend` runs registered masters through the same framework `LinearReportExecutor` as CSP1D: actual compiled-model/report identity, full primal/integrality/objective validation, certified LP duals, one total budget, raw progress and independent child cancellation. `solve_bpp2d_column_generation` composes Phase I recovery, certified ordinary whole-sheet-area pricing and same-backend final MILP. It validates observed layouts against the source catalog/rotation/kerf policies, preserves the complete pool and each actual child model/report with pool-prefix mappings, and checks iteration/pool/report/time/cancellation limits. Existing generators are heuristic: `candidate_complete` is always false, and no restricted optimal/infeasible report becomes a global source conclusion. Nonzero mass/load rules and extra-rule pricing corrections remain unsupported by this default-rule loop. Legacy closure executors remain compatible and are not relabeled; see the [native column-generation boundary](../ospf-rust-framework-bpp2d/README.md#report-driven-native-column-generation).

`encode_bpp2d_problem` / `decode_bpp2d_problem` provide schema-1.0 canonical source artifacts with a domain-separated fingerprint. `encode_bpp2d_execution_report` / `decode_bpp2d_execution_report` bind the complete physical pool and every actual child/report to that source. The decoder reconstructs original-domain masters, compares full mathematics/types/bounds/objectives/execution metadata without replacing actual report identity, checks LP and infeasibility certificates, replays Phase I/RMP/pricing/final transitions and recomputes accepted reduced costs. It derives production only from a revalidated final integer incumbent; checked u128 aggregates become decimal strings, never JS numbers. Circles retain bounding-square geometry, while produced areas use the original actual-shape area. Mathematical consistency does not authenticate solver provenance.

The portable `ospf-wasm/bpp2d` SDK subpath exposes `PackingContext`, `Bpp2dReport` and independently disposable `ProductionResult` / `PackingResult`. Each browser/Worker/Node realm initializes WASM explicitly. Source count inputs use bigint within 0..2^53-1; generator counts are limited to 4096. Sources allow at most 64 items, 16 sheets, 128 demands and 256 initial columns; complete pools allow 512 columns and 4096 placements in total. A conservative sheet/item bounding-area ratio of at most 4096 also bounds exposed native generation inputs. Text artifacts are limited to 8 MiB; normalization uses meters/square meters. Initial duplicate positions survive, generated physical duplicates do not. Raw child reports retain exact u64 metadata as text. `OSPF_BPP2D_FIXTURE_DIR` optionally supplies ignored real-SCIP/Gurobi evidence to SDK/Worker/isolated-package acceptance, not a release dependency. BPP2D HTTP/N-API execution facades now share the same native engine; bounded packing/rendering reuses original Rust pure computation; nondefault rules remain pending. The SDK reader itself does not execute generators or solvers.

### BPP1D Domain Bridge

`ospf-rust-bridge` generates the quantity/BPP1D DTO declarations and implements schema-1.0 canonical problem artifacts, a domain-separated SHA-256 identity, bounded default-pattern generation and physical result/report revalidation. Exact u64 demands, materials, stock and usage counts cross JSON as canonical decimal **strings**; safe counters use bounded u32. The independent SDK consumes these generated types and exposes bigint/Map facades through `ospf-wasm/bpp1d`, not neighboring Rust source paths. Quantities include m/km/cm/dm/mm/in/ft/yd and t/lb with runtime dimensional checks.

| BPP1D capability | WASM | Node N-API / HTTP | Remaining Rust-only APIs |
| --- | --- | --- | --- |
| Unit normalization, pattern validation/generation, exact full-demand usage verification, bounded physical packing, coordinate verification, material totals, KPIs | Yes, original domain code | Used internally by the same codec/workflow | CSV, renderer adapters, tail optimization, custom policy/context/pipeline registration (2D position rectangles already derive conflict edges through the shared BPP loading entry) |
| Candidate MILP and finite-pool column activation | No native solver in WASM | Actual `Bpp1dMilp` / `Bpp1dColumnGeneration`, explicit SCIP/Gurobi | Arbitrary cross-process strategy callbacks are not supported |
| Source-bound report import, physical/KPI/objective verification, exact candidate report text | Yes, independently owned | Native result encoding | Import does not replay exhaustive search to independently prove native global flags |

Problem limits: 1000 item types, 128 bin types, 10000 candidate patterns, 100000 search nodes, 100000 expanded pieces/bins, 8 MiB text. Generation defaults/maxima are 10000/100000; expansion default/maximum is 100000. Native iterations/columns-per-iteration default to 32 and cap at 10000. Length/mass normalization uses existing `PackingContext` and `RuntimePackingValueAdapter`; spacing is inter-piece only, and remnant KPI measures tails only. Exact usage checks can validate u64::MAX without expansion; existing native exact f64 conversion gates still reject unrepresentable solver inputs.

Protocol-1 requests require `solver: bpp1d`, `modelKind: bpp1d` and exclusive `bpp1dOptions`; clients require `bpp1d` plus the mode-specific `bpp1dMilp`/`bpp1dColumnGeneration` capability and selected compiled backend. The executor reconstructs non-Send domain/MetaModel objects inside its worker and invokes original shared MILP/LP/final registration, never manual bridge matrices or TS algorithms. HTTP and N-API share job limits, progress, cancellation and release. `timeLimitMs` uses the original 60-second default or one shared domain budget across generation, seeds, LP and final; every child gets remaining time. Codec setup and bounded non-preemptible generation/compilation are not a hard deadline. Only proof-free, no-incumbent generic cancelled/interrupted child reports receive their actual child's missing fingerprint/mapping; existing conflicting identities are never replaced.

BPP1D column generation is **finite-pool activation**, not the general source-projected DW strategy: final uses the complete current seed/generated pool, even after a pricing iteration limit when budget allows. Truncation always retains `candidateComplete: false` and cannot establish global optimality/infeasibility. Global flags preserve the existing native default-rule complete-candidate/evidence scope. Reports bind the source problem separately, retain the candidate model's raw SolveReport identity, and independently recheck placements, exact demand/stock/materials, KPIs and objective. u64 child counters remain lossless text in WASM. Releasing a job, context, report or independently returned physical result are separate operations. Other domain coverage is limited to the explicit shared BPP loading/CSP1D/BPP2D/CSP2D sections; no further domain bindings are implied.

#### Shared BPP2D execution boundary

`NativeEngine` accepts only `solver: bpp2d`, `modelKind: bpp2d` and exclusive explicit `bpp2dOptions`. It prevalidates source identity/budgets before creating a job, then constructs non-Send domain objects inside the worker. Source decoding, generation and all child solves share the time budget. The selected compiled SCIP/Gurobi backend runs the original column-generation application; SCIP uses original LP-subproblem defaults. Source-bound report encoding is the same for HTTP and N-API, without solver fallback or relabeling cancellation. Default budgets are 8 pricing batches, 128 observed candidates/batch, pool 512 and 196 reports, with optional final MILP and preregistered generator chains. A disabled final or early budget stop returns no invented production. Completed final reports remain unchanged on late parent cancellation. Raw child progress and thread-safe polling reuse the existing engine lifecycle/capacity/release mechanism.

The SDK exports `Bpp2dColumnGeneration` through `framework` and `bpp2d`. It snapshots generated-contract configuration and requires both domain/algorithm and explicit native LP/MIP capabilities before submission. Authenticated in-process HTTP route tests execute real SCIP/Gurobi Phase I/pricing/final, report budgets, final omission, cancellation and result retrieval; isolated npm tests exercise actual N-API execution. Independent browser/SDK HTTP tests remain explicit-host gates, not inferred passes when no host is configured. Nondefault mass/load/rule extensions remain separate unfinished work.

### Bounded Compressed BPP2D Packing And Rendering

`PackingContext.packLayers(layers)` treats each input column as one independent sheet, calling the original `Packer`, `FreeSpaceAnalyzer` and `PackingRendererAdapter`; it does not establish source-demand satisfaction. `Bpp2dReport.packing()` derives only from a revalidated final integer incumbent and returns `undefined` without one. Each call yields an independently disposable `PackingResult`, outliving contexts, reports and production copies. Loading order is stable by y, x and item identity, retaining input positions rather than identifying placements by quantized coordinates.

`patterns()` retains complete-pool indices, bigint copies, physical layers, unit-bearing render placements, loading order/rate and single-sheet remnants. Copies never expand. `sheetCount()`, `itemCount()` and `materialCounts()` accumulate checked u128 counts before bigint conversion, including totals beyond 2^53. Default material keys retain the original packer's finished-item IDs, not raw-material, supplier, mass or load rules. Render lengths use meters and areas square meters; DTOs/Maps are independent copies.

Independent packing limits: 128 placements per selected pattern, conservative aggregate remnant-work budget `sum((2*n+1)*(n+1)) <= 65536`, and 8 MiB output. Exceeding them returns `PayloadTooLarge`, without truncating or invalidating the report/production; report limits remain 512 columns/4096 placements. Original remnant slicing retains its 1e-9 tolerance and subtracts enclosing squares for circles, without deducting kerf loss. Remnant area is not necessarily whole-sheet minus actual-shape area or a reclaimable-stock/cutting-process proof. This provides typed rendering data, not SVG/Canvas UI; TS reimplements neither packing nor geometry.

### Portable CSP1D

`ospf-wasm/csp1d` consumes Rust-generated contracts for `CuttingContext`, independently owned `PatternGenerationResult` and `ProductionResult`. Schema-1.0 artifacts retain canonical Rust text and domain-separated SHA-256 identities. The bridge normalizes real length units, reconstructs plans from source product/material references, and computes demand contributions only through the original domain formulas. `verifyUsages` reuses original context/MetaModel registration and full snapshot validation, additionally checking discrete demand, stock, machine batches and overflow with exact u64 arithmetic, without piece expansion.

Only discrete Roll/Sheet demand, static product widths and default material/machine-width/batch rules are exposed. Each input count is bigint in TS and canonical text on the wire, limited to [0, 2^53-1] by the existing f64 domain; demands/usages/slice amounts must be positive. Larger inputs are rejected, never rounded. Checked aggregate outputs may exceed 2^53 and remain bigint/text; physical widths remain f64. Limits: 64 products/demands, 16 materials/machines, 8 widths per product, 512 plans/usages, 64 slice groups/knives per plan and 8 MiB text. Source IDs allow 1..128 ASCII letters/digits/underscore/dot/hyphen; plan IDs allow up to 512 bytes of the same alphabet.

Generation calls original Simple or N-Same (default); maxPlans defaults to 128/caps at 512, maxKnives defaults to 16/caps at 64. Raw generation statistics and truncation are retained. **candidateComplete is always false**: neither generator exhausts arbitrary mixed-product patterns. Actual portable call paths use no clocks, randomness, threads or filesystem; unused native/benchmark paths are not thereby certified portable. Browser, module Worker, Node ESM, production bundling and isolated package consumers exercise the real WASM. Weight/Costar/dynamic-length/yield/waste/length/capacity/switch/custom-policy bindings remain unavailable. A feasible verified production is not an optimality proof.

#### Native Child Identity Prerequisite

`Csp1dStageReport` retains the **actual submitted compiled model** alongside its raw report and separate objective constant. Recreating a business-equivalent MetaModel is not an identity-preserving substitute: compiled row names can contain a model-local owner. The original schema-1 remote exchange also coalesces unsigned/specialized variable kinds and omits row metadata; it remains unchanged, not silently relabeled as a complete execution artifact.

The runtime-independent `remote-codec` feature now offers separate schema-1 `CompiledLinearModelArtifact` encode/decode functions. They reuse the existing sparse remote DTOs, carry exact token and solver variable kinds separately (including intentionally relaxed solver kinds), retain missing-vs-present constraint metadata and actual row names, and verify canonical bytes, a domain-separated full-payload digest, original core fingerprint and stable row/column mapping. Infinite bounds use the existing signed-text encoding. Local variable arena IDs, display handles and mutable token results are not cross-process handles; decoding allocates fresh local identities. The artifact preserves the backend mathematical/report contract, not ownership of the original MetaModel. Each model text is bounded to 8 MiB.

The Rust bridge's stage-report codec validates a retained model against its raw report, full vector/rows/bounds/integrality, stable values, objective and, for optimal LP stages, the existing complete primal/dual certificate gate. Portable gates read `SolveReport<f64, u64>` without narrowing metadata to wasm32 `usize`; ordinary native reports keep default usize counters. Objective constants stay separate from raw child objective values. Stage codecs alone do not establish a source-wide proof or certify caller-supplied children as source models.

#### Complete native CSP1D execution

`Csp1dMilp(client, { backend })` and `Csp1dColumnGeneration(client, { backend, generation })` expose the original domain services through explicit HTTP or Node clients. Requests use protocol 1, `solver: csp1d`, `modelKind: csp1d` and exclusive `csp1dOptions`. Clients check the domain, algorithm and selected compiled linear/MIP backend capabilities. There is no heuristic downgrade, native solver in browser WASM, implicit HTTP fallback or arbitrary TS callback registration.

Initial generation defaults to 128 plans. Pricing reuses the selected Simple/N-Same strategy and knife limit; `poolLimit` defaults/caps at 512 and cannot be below the initial plan budget. `iterationLimit` defaults to 8/caps at 64; `columnsPerIteration` defaults to 64/caps at 512. One total `timeLimitMs` budget (default 60 seconds) covers generation and child execution; each child receives the remainder. Bounded generation and model compilation are not preemptible, and codec setup is not a hard end-to-end deadline. Progress is the latest actual child snapshot, not an overall percentage. Cancellation/completion races preserve actual child termination semantics.

`CuttingContext.readExecutionReport(text)` / `Csp1dReport.fromJson(text, source)` validate the independent schema-1.0 envelope (8 MiB total), source identity, bounded configuration, pool and trace counts, stage order and retained actual child reports. Final production uses the source-bound integer validator below. `summary()` is Rust-generated and distinguishes business status, execution stop, counts and restricted integer results. `traceJson()` retains the canonical original Rust trace as text; `stageJson(i)` retains each actual compiled model and raw report; `stageReportJson(i)` returns raw exact-u64 report text. Out-of-range indices return undefined. Trace telemetry and locally consistent LP artifacts are not an independently replayed source-wide certificate or provenance authentication.

Every `integerReport()` and `production()` call returns an independently disposable copy, or undefined when unavailable. Job release, context disposal and report disposal are independent. No-initial-plans, restricted infeasibility, backend failure, time limit and cancellation never fabricate production. `candidateComplete` is always false; even successful pricing and an optimal final pool do not prove unrestricted CSP optimality/infeasibility.

Phase I now connects artificial under-production variables to the **original demand rows**, avoiding a duplicate yield-balance row and preserving the real demand dual identity. Penalty keys use each demand's own Roll/Sheet unit. Real SCIP/Gurobi tests cover a one-column infeasible initial pool, pricing recovery, mixed Roll/Sheet demands and final extraction. Ordinary hard-demand/yield paths and legacy heuristic defaults remain unchanged.

#### Source-bound integer production reports

`encode_csp1d_production_report` retains one actual MILP/final-MILP child and its **complete** pool, including unused plans. The independent schema-1.0 artifact is limited to 8 MiB. `decode_csp1d_production_report(text, source)` revalidates the canonical source identity, reconstructs default-domain plans and a fresh original context, and compares every column name, token/solver type, bound, objective coefficient, full row coefficient/RHS and execution metadata. Only local row identities are excluded from this cross-context mathematical comparison; the raw report must still match the retained actual child's fingerprint/mapping unchanged. Missing/foreign columns, duplicate identities/physical patterns, auxiliary/custom-rule models and relaxed integer columns are rejected. Pool order is not used as an implicit column mapping.

`CuttingContext.readProductionReport(text)` exposes this validator through WASM and the SDK `ProductionReport`. Production is derived only from the actual complete incumbent vector, rechecked after integer rounding and then through the original source context and exact u64 rules. No independent production claim is accepted from the payload. No incumbent means `production()` is undefined; cancellation/timeout/infeasibility remain the original child's status/reason. `restrictedOptimal` and certificate-gated `restrictedInfeasible` describe the candidate pool only; `candidateComplete` stays false. Neither fingerprints nor mathematical consistency authenticate report provenance. `solverReportJson()` retains raw exact-u64 report text; do not parse/re-encode it through JS numbers. Each returned production is independently owned, including after context/report disposal. The summary type is generated by Rust. Real SCIP/Gurobi acceptance compares both MILP and column-generation production directly with the original domain extraction. This integer-stage reader remains separate from the complete execution report described next.

### Complete-Source MILP Finishing After Benders

Set `sourceMilpFinishing: { reserveTimeMs: 3000 }` explicitly to reserve full-source solve time from the **same**
`timeLimitMs` total, not add another deadline. The reservation is a positive u32 number of milliseconds strictly
below the total (60000 ms when omitted). Omitting finishing preserves existing behavior. The SDK also requires
`bppLoadingSourceFinishing`; old hosts are rejected before submission, without silent downgrade.

```typescript
const solver = new DiscreteLoadingBendersApplicationService(client, {
    backend: 'scip', recourseResources: ['ballast'], iterationLimit: 1,
    sourceMilpFinishing: { reserveTimeMs: 3000 },
});
const job = await solver.solve(source, { timeLimitMs: 10000 });
try {
    const report = DiscreteLoadingBendersReport.fromJson(job.reportJson!, source);
    try {
        console.log(report.summary(), report.sourceFinishingInvoked());
        console.log(report.accelerationApplicationReportJson(), report.certificationReportJson());
        console.log(report.certificationError());
        const plan = report.solution();
        try { console.log(plan?.resources()); } finally { plan?.dispose(); }
    } finally { report.dispose(); }
} finally { await client.release(job.id); }
```

Rust uses the original `DiscreteLoadingCertificationService` with a fresh equivalent complete registration,
copying only validated values as a warm start. Initial construction, acceleration, fallback and intermediate codec
work consume the same budget; finishing receives the remaining time. If acceleration exhausts its stage budget
before registration, reserved time can still start a real ordinary full-source solve, without fabricating old
registration. Construction/compilation is not hard-preemptible, nor is output validation part of a preemptible
backend deadline. Existing verified full-source terminal proofs are retained unchanged, even with cancellation
or exhausted budget, without another solve. Prior cancellation/interruption never starts another backend.
Missing, worse nonoptimal or recoverably failed finishing never discards the previous incumbent. Retention uses
the original Rust transformation to rebuild audits, gaps and proof-free boundaries, not old traces, duals or pools.

`summary()`, `solution()`, `applicationReportJson()` and `solverReportJson()` describe the final selection.
`accelerationApplicationReportJson()` retains the previous original application;
`certificationReportJson()` retains the actual finishing core report even when not selected.
`certificationError()` exposes a recoverable finishing error. `sourceFinishingPresent()` means a stage record
exists; `sourceFinishingInvoked()` requires an actual backend report/error, not merely configuration. For old
reports the accelerator getter returns the original application. Accelerator proof boundaries remain unchanged;
source proofs must originate from actual complete-source fallback/finishing. Import revalidates mathematics and
selection consistency, **not external solver provenance**.

Optional `certification` retains raw atomic texts under protocol major 1. Core/finishing texts each retain 8 MiB
limits and application texts 16 MiB + 1 KiB. Envelopes with finishing evidence allow 128 MiB + 8 KiB;
without it the 64 MiB + 4 KiB limit remains. The default HTTP request limit stays 9 MiB; no large previous report
is reuploaded. Exact u64 metadata stays text, never parsed/re-encoded through JS numbers. This is not a binding
for branch-and-price or custom strategies; BPP3D only exposes its cuboid and axis-aware cylinder one-round native execution.

## Shared BPP Partition Lifecycle Primitives

`ospf-wasm/bpp` exposes `PartitionLoadingContext`, `PartitionLoadingMaster`,
`PartitionLoadingPricing` and `PartitionLoadingBatch`. `source.partition(options)` creates an
independent owned registration; disposing the source does not invalidate it. Explicit partitions must
cover every source position exactly once. Declared resources follow their position ownership;
`refinableResources` is an optional whitelist of resource IDs, not raw tokens or callbacks.

`decisions()` provides the actual local decision catalog. `addColumns` accepts complete finite
`ReadonlyMap<string, number>` vectors, never fills missing decisions with zero, and validates the
whole batch before mutation. `columns()` retains exact bigint cargo/count metadata and source-model
weight units. `removeColumns` is atomic and column IDs are never reused. Master rebuilds and successful
pool mutations invalidate old registrations/pricing epochs; failed transactional additions do not.
`acceptLpReport` binds prices only after original full optimal-LP certificate, model identity, mapping
and dual validation. Report-gate failures revoke old prices; revalidating the same report does not revive
old requests. Bare dual arrays are not a substitute for a report. A batch owns copied requests, and
`addPricedBatch` commits all blocks atomically against one current epoch. A successful pool change invalidates that epoch; an empty or fully deduplicated no-op preserves the original Rust epoch semantics.

Master/pricing `toModel()` returns an **opaque compiled-linear artifact**, not the ordinary linear codec:
LP relaxation can retain original token types while solver variable types become continuous. Use the
explicit `SCIPSolver.solveCompiledLinear` / `GurobiSolver.solveCompiledLinear` entry. It requires the
backend's `compiledLinear` capability; old hosts are rejected, with no fallback to ordinary linear, HTTP
or another backend. The Rust compiled-model codec preserves actual metadata and relaxed types. Its
transport/content digest is `ModelArtifact.fingerprint` / `JobView.fingerprint`; the raw core report keeps
its own actual model fingerprint. These digests are deliberately different and must not be relabeled. Original master/pricing rows use stable registration IDs, not process-local counters, so independently reconstructed child artifacts retain the same actual identity.
`objectiveConstant()` is separate from backend `c*x`; do not inject it into an unmodified child report.
Pricing `readReport` revalidates the actual full local model including layout auxiliaries, preserves the
original report text/u64 counters, and returns decisions plus reduced cost including the convexity constant.
`reducedCost` is a read-only snapshot computation, not proof that its request is still current.

```typescript
const partition = source.partition({
    partitions: [{ id: 'left', positionIds: ['a'] }, { id: 'right', positionIds: ['b'] }],
    refinableResources: ['ballast'],
});
// seedColumns contains validated complete per-partition Map vectors.
partition.addColumns(seedColumns);
const master = partition.master();
const job = await new SCIPSolver(client).solveCompiledLinear(master.toModel());
try {
    partition.acceptLpReport(master, job.reportJson!);
    const pricing = partition.pricing(master, 'left');
    try { console.log(pricing.toModel(), pricing.objectiveConstant()); }
    finally { pricing.dispose(); }
} finally { await client.release(job.id); master.dispose(); partition.dispose(); }
```

`master({ integer: true, refineResources: true })` releases only whitelisted continuous resources while
freezing discrete profiles. `sourceValues` / `extractSolution` require complete Float64Array vectors,
reject LP/artificial plans and revalidate the complete original model/domain. `materializeCompletedResources`
atomically adds completed profiles and invalidates the old master; rebuild an integer master afterwards.
Returned physical plans and master/pricing snapshots have independent disposal. Retaining a readable old
snapshot never authorizes reuse in a different session.

Bounds: 1..128 partitions, 4096 pooled columns / 262144 decision entries, 512 inputs per add, and
128 blocks / 512 candidates / 262144 entries per pricing batch. Capacity checks conservatively charge
existing plus all submitted columns/entries, including duplicates before deduplication. Text DTOs and
compiled artifact payloads are limited to 8 MiB; a bounded pool does not guarantee `columns()` fits that
serialized budget, and an oversized getter fails explicitly. Source outer wrappers allow 16 MiB + 1 KiB;
the HTTP request limit remains 9 MiB. Progress, cancellation and job release reuse native execution.

These primitives alone do not run the separately bound original `PartitionColumnGenerationApplicationService`.
Neither entry provides a source-MILP optimality proof or branch-and-price, and neither covers BPP3D. One pricing round and an optimal restricted final do
not establish the complete source optimum or infeasibility. Real SCIP/Gurobi tests cover these primitives;
WASM/browser/Worker/isolated-package readers reconstruct the actual child artifacts and resource-completed
plans. Explicit-host HTTP tests skip when unconfigured; authenticated in-process HTTP tests are separate.

## Original Native Root-Partition Application

`PartitionColumnGenerationApplicationService` is exported through `ospf-wasm/bpp` and `framework`.
It executes the original Rust service, not a TS loop, using the explicitly selected HTTP or Node client.
Requests use `bpp-loading` / `bppLoading` with exclusive `bppLoadingOptions.partition`; Benders cannot
be combined. The SDK requires `bppLoadingPartitionColumnGeneration` and the selected compiled LP/MIP
backend. Old hosts fail capability checks rather than falling back to ordinary MILP, WASM or HTTP.

```typescript
import { PartitionColumnGenerationApplicationService, PartitionColumnGenerationReport } from 'ospf-wasm/bpp';
const service = new PartitionColumnGenerationApplicationService(client, {
    backend: 'scip',
    partition: { partitions: [{ id: 'left', positionIds: ['a'] }, { id: 'right', positionIds: ['b'] }],
        refinableResources: ['ballast'] },
    iterationLimit: 64, columnLimit: 512, completionColumnLimit: 128, reportLimit: 256,
    sourceMilpFinishing: { reserveTimeMs: 2000 },
});
const job = await service.solve(sourceArtifact, { timeLimitMs: 10_000 });
try {
    const report = PartitionColumnGenerationReport.fromJson(job.reportJson!, sourceArtifact);
    try {
        console.log(report.summary(), report.executionSummary(), report.refinementReportJson(), report.finalReportJson());
        const plan = report.solution();
        try { console.log(plan?.solution(), plan?.resources()); } finally { plan?.dispose(); }
    } finally { report.dispose(); }
} finally { await client.release(job.id); }
```

The original flow seeds each block, runs Phase I and root LP/pricing rounds, submits each round atomically,
optionally solves resource completion and materializes its profiles, then solves an artificial-free restricted
integer final. SCIP uses its existing LP-subproblem defaults to retain certified duals. Each child receives
remaining time and the same cancellation handle; latest-child progress is not overall percentage progress.
One native-job budget includes worker source registration and model construction; preflight validation and
post-solve encoding/replay are not an interruptible end-to-end deadline.

`PartitionColumnGenerationReport` reconstructs the same original Rust service with recorded budget
observations and original child reports, **without reading a clock or executing a backend**. It checks exact
compiled child artifacts, full LP certificates, local/complete-source feasibility, objectives, control-flow
order, monotonic remaining-time/cancellation observations and the final source selection. It preserves all
raw child text and u64 counters. `calls()` exposes ordered original evidence; `executionSummary()` derives
root convergence, root/completion counts, trace and actual refinement/final call indices. Separate terminal
getters preserve raw reports, and `accelerationError()` retains the original recoverable failure after a
validated source incumbent. These are consistency checks, not authentication of external solver provenance.

The root-derived source report is always proof-free: `COMPLETED` can describe a completed restricted final even
when `lpConverged` is false or the root iteration budget ended. Restricted infeasibility/optimality and root
LP convergence are **not** source-MILP infeasibility/optimality or branch-and-price. The original service may
retain a resource-completion incumbent after final failure or cancellation; no extra root solve starts after an
accelerator terminal observation. Plans dispose independently. Explicit reserved complete-source finishing is
documented next; arbitrary trait callbacks remain unbound and BPP3D exposes only its cuboid and axis-aware cylinder one-round native execution.

Defaults/ranges: 64 root rounds (1..64), 512 root columns (partition count..4096), 128 additional completion
columns (0..128, at least partition count when resources are refinable), and 256 child solves (1..1024).
Exhausting the child-report guard follows original service error recovery: without a retained source plan the
job fails explicitly; it does not invent a successful report. At most one guard-error call beyond that cap is
recorded. Each model/report payload remains limited to 8 MiB; accumulated successful child text is limited
to 32 MiB, observations to 16384 and the outer envelope to 128 MiB + 8 KiB. HTTP requests remain 9 MiB.
Real SCIP/Gurobi, authenticated in-process HTTP, isolated npm/N-API, browser and module-Worker tests are
separate gates; standalone HTTP is skipped when not explicitly configured.

### Optional Complete-Source Finishing After Partition Acceleration

Set `sourceMilpFinishing: { reserveTimeMs: 2000 }` on the partition application to opt in. The positive
reservation must be strictly below `timeLimitMs` (default total 60000 ms); it is subtracted from the root
budget, never added to the job deadline. The SDK additionally requires `bppLoadingPartitionSourceFinishing`.
After a successful root application result, the native executor reuses `DiscreteLoadingCertificationService`
on a fresh equivalent full source, passing only the validated complete vector as a warm start. Root
pre-registration timeout may use the reserved time through the original ordinary source service. Prior
cancellation/interruption does not start finishing. Root errors that prevent a valid application result still
fail the job explicitly; this option is not an implicit retry or fallback for arbitrary accelerator failures.

The original root replay remains independent and proof-free. `accelerationApplicationReportJson()` preserves
its exact source report; `applicationReportJson()` / `solverReportJson()` expose the final selected source
result. `certificationReportJson()` and `certificationError()` retain the actual full-source backend evidence,
including exact u64 metadata. `sourceFinishingPresent()` distinguishes a processed finishing stage (including
pre-solve stops), while `sourceFinishingInvoked()` requires an actual report or recoverable backend error.
`calls()`, root trace, refinement/final indices and `accelerationError()` still describe the accelerator only.
The separate complete-source call is not charged to the root child-count/text guard and has its own 8 MiB
report limit, within the same outer envelope and time budget.

The portable reader replays the root service and then applies the shared certification gate: source identity,
complete incumbent/objective, actual-report selection, better-old-plan retention and audit/gap/proof rebuilding.
Only an actual complete-source report can contribute its own proof. Empty/worse unfinished candidates and
recoverable finishing errors retain the validated old plan; contradictory infeasibility or claimed optimality
is rejected. Raw certification text is retained even when a better old incumbent is selected. This verifies
mathematical/evidence consistency, not external solver provenance or full branch-and-price.

`SolveRequest.tuningOptions` carries solver `threads` (1..=64), a random `seed` and a relative `mipGap` (0.0 <= gap < 1.0). A nonzero gap may terminate with `GapLimit`, which reports `Feasible` and carries no optimality proof; only exact completion may certify optimality. It is accepted solely by the direct `scip`/`gurobi` entries; domain and framework strategies own their budgets and configuration and reject it with `InvalidInput`. Tuning never changes the model, certificates, budgets or proofs, and the SDK validates the same ranges before submission.
