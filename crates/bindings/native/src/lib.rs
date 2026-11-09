//! HTTP 和 Node 共用的原生执行库 / Shared native execution engine for HTTP and Node.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use ospf_rust_core::solver::{CancellationOrigin, LinearSolver, SolveHandle, SolveOptions, SolveProgressReporter, SolveReport, Solver};
use ospf_rust_core::solver::constraint_programming::{ConstraintProgrammingSolveOptions, ConstraintProgrammingSolver};
use ospf_rust_bridge::{
    BackendDescriptor, BridgeError, HostCapabilities, JobStatus, JobView, ModelKind, PROTOCOL_VERSION, Result, SolveRequest,
    SolverTuningOptions, decode_cp_report, decode_cp_snapshot, decode_model, decode_quadratic_model,
};

mod combinatorial;
mod linear_identity;
mod compiled_linear;
mod bpp_loading;
mod bpp_partition;
mod fixed_recourse;
mod column_generation;
mod bpp1d;
mod bpp2d;
mod bpp3d;
mod gantt_activity;
mod csp1d;
mod csp2d;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Job {
    view: Mutex<JobView>,
    cancellation: SolveHandle,
}

struct State {
    jobs: BTreeMap<String, Arc<Job>>,
    active: bool,
}

/// 有界单任务执行器，克隆共享同一任务集合。
/// Bounded single-job executor; clones share the same jobs.
#[derive(Clone)]
pub struct NativeEngine {
    state: Arc<Mutex<State>>,
    submission: Arc<Mutex<()>>,
    next: Arc<AtomicU64>,
    capacity: usize,
}

impl Default for NativeEngine {
    fn default() -> Self { Self::new(64) }
}

impl NativeEngine {
    /// 创建有限报告存储 / Create bounded report storage.
    pub fn new(capacity: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(State { jobs: BTreeMap::new(), active: false })),
            submission: Arc::new(Mutex::new(())),
            next: Arc::new(AtomicU64::new(1)), capacity: capacity.clamp(1, 1024),
        }
    }

    /// 编译能力不代表许可证或运行时已就绪 / Compiled capability does not imply runtime or license availability.
    pub fn capabilities(&self) -> HostCapabilities {
        let serial_compiled = cfg!(any(feature = "scip", feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"));
        let backend = |id: &str, compiled: bool, quadratic: bool| {
            let mut capabilities = if quadratic { vec!["linear".into(), "compiledLinear".into(), "mip".into(), "quadratic".into(), "miqp".into(), "qcp".into(), "cpExactLowering".into(), "cpSolveOptions".into(), "cpObjectiveConstant".into()] }
                else if compiled { vec!["linear".into(), "compiledLinear".into(), "mip".into(), "cpExactLowering".into(), "cpSolveOptions".into(), "cpObjectiveConstant".into()] } else { vec![] };
            if compiled { capabilities.push("intermediateProgress".into()); }
            // IIS 只在 Gurobi 后端原样实现，能力描述必须跟着真实编译情况走。
            // IIS is implemented for the Gurobi backend only, so the descriptor must follow the real build.
            if id == "gurobi" && compiled { capabilities.push("linearIis".into()); }
            // SCIP 与 Gurobi 都原生实现了解池，能力描述跟着真实编译情况走。
            // Both SCIP and Gurobi implement the native solution pool, so the descriptor follows the real build.
            if matches!(id, "scip" | "gurobi") && compiled { capabilities.push("linearSolutionPool".into()); }
            // 固定整数 LP 敏感性分析需要后端 LP，SCIP 与 Gurobi 都能提供。
            // Fixed-integer LP sensitivity needs a backend LP, which both SCIP and Gurobi provide.
            if matches!(id, "scip" | "gurobi") && compiled { capabilities.push("linearSensitivity".into()); }
            BackendDescriptor {
                id: id.into(), compiled, capabilities,
                warnings: vec!["runtime libraries and applicable licenses are checked when solving".into(),
                    "CP uses conditional exact finite MIP lowering, not a native CP engine; unsupported or inexact models are rejected".into()],
            }
        };
        HostCapabilities {
            protocol_version: PROTOCOL_VERSION,
            backends: vec![
                backend("scip", cfg!(feature = "scip"), cfg!(feature = "scip-quadratic")),
                backend("gurobi", cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")), cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))),
                BackendDescriptor {
                    id: "serial-linear".into(),
                    compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["serialLinear".into()] } else { vec![] },
                    warnings: vec!["native Rust framework serial composition; every configured backend must be compiled; runtime or license failures remain attempt diagnostics".into()],
                },
                BackendDescriptor {
                    id: "parallel-linear".into(),
                    compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["parallelLinear".into()] } else { vec![] },
                    warnings: vec!["native Rust framework parallel composition; first cancels losers cooperatively and joins every worker; all configured backends must be compiled".into()],
                },
                BackendDescriptor {
                    id: "fixed-recourse".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["fixedRecourse".into()] } else { vec![] },
                    warnings: vec!["native fixed continuous-recourse Benders; verified certificates required; no silent full-MILP fallback or arbitrary callbacks".into()],
                },
                BackendDescriptor {
                    id: "column-generation".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["columnGeneration".into()] } else { vec![] },
                    warnings: vec!["native source-projected Phase I/pricing/restricted integer master; restricted final optimality is not a global source proof; no arbitrary callbacks".into()],
                },
                BackendDescriptor {
                    id: "bpp1d".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["bpp1d".into(), "bpp1dMilp".into(), "bpp1dColumnGeneration".into()] } else { vec![] },
                    warnings: vec!["native default BPP1D domain rules; bounded candidates; column-generation mode activates a finite pool, not arbitrary pricing callbacks; global flags require complete candidates and actual proof".into()],
                },
                BackendDescriptor {
                    id: "csp1d".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["csp1d".into(), "csp1dMilp".into(), "csp1dColumnGeneration".into()] } else { vec![] },
                    warnings: vec!["native default Roll/Sheet CSP1D rules; original LP/pricing/final-MILP workflow with bounded Simple/N-Same pricing; candidateComplete is always false; no arbitrary callbacks or heuristic solver fallback".into()],
                },
                BackendDescriptor {
                    id: "bpp2d".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["bpp2d".into(), "bpp2dColumnGeneration".into()] } else { vec![] },
                    warnings: vec!["native default count/area BPP2D Phase I/pricing/final workflow; preregistered heuristic generators; candidateComplete is always false; bounding-square circle geometry; no mass/load, custom pricing corrections or arbitrary callbacks".into()],
                },
                BackendDescriptor {
                    id: "bpp3d".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["bpp3d".into(), "bpp3dOneRoundExecution".into()] } else { vec![] },
                    warnings: vec!["native default-cuboid BPP3D initial RMP/heuristic generation/refreshed RMP/final workflow with strict grouped physical replay; preregistered default generators are never replayed, so candidateComplete is always false; execution reports carry completed/failure instead of claiming global optimality or source infeasibility".into()],
                },
                BackendDescriptor {
                    id: "gantt-activity".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["ganttActivity".into()] } else { vec![] },
                    warnings: vec!["native single MILP over declared finite candidate starts; incumbent placements require independent native revalidation; no source-wide optimality, provenance proof or arbitrary policy callbacks".into()],
                },
                BackendDescriptor {
                    id: "csp2d".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["csp2d".into(), "csp2dColumnGeneration".into()] } else { vec![] },
                    warnings: vec!["native default sheet/coil CSP2D Phase I/certified LP pricing/final integer workflow; preregistered shelf heuristic; candidateComplete is always false; each pattern use consumes one material batch; strict kerf/cut-tree validation never repairs legacy trees; no arbitrary callbacks".into()],
                },
                BackendDescriptor {
                    id: "bpp-loading".into(), compiled: serial_compiled,
                    capabilities: if serial_compiled { vec!["bppLoading".into(), "bppLoadingMilp".into(), "bppLoadingBenders".into(), "bppLoadingSourceFinishing".into(), "bppLoadingPartitionColumnGeneration".into(), "bppLoadingPartitionSourceFinishing".into()] } else { vec![] },
                    warnings: vec!["native original shared BPP ordinary/Benders applications; explicit continuous resource recourse and opt-in fresh MILP fallback; merged decomposition reports remain proof-free; no partition column generation, completion, certification, arbitrary callbacks or three-dimensional geometry".into()],
                },
            ],
        }
    }

    /// 校验后启动后台求解，不阻塞调用线程 / Validate and start background solving without blocking the caller.
    pub fn submit(&self, request: SolveRequest) -> Result<JobView> {
        // 大模型解码本身也需要限流，不将等待者堆积在线程池中。
        // Bound model decoding too, rather than accumulating waiting work in the thread pool.
        let _submission = match self.submission.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(BridgeError::new("Busy", "another submission is being prepared")),
        };
        if request.protocol_version != PROTOCOL_VERSION {
            return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported bridge protocol major"));
        }
        if request.time_limit_ms == Some(0) {
            return Err(BridgeError::new("InvalidInput", "time limit must be positive"));
        }
        if request.model.model_json.len() > 8 * 1024 * 1024 {
            return Err(BridgeError::new("PayloadTooLarge", "model exceeds 8 MiB"));
        }
        if let Some(tuning) = request.tuning_options.as_ref() {
            tuning.validate()?;
            let direct = matches!(request.solver.as_str(), "scip" | "gurobi")
                && request.serial_options.is_none() && request.parallel_options.is_none() && request.fixed_recourse_options.is_none()
                && request.column_generation_options.is_none() && request.bpp1d_options.is_none() && request.csp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp2d_options.is_none()
                && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none() && request.iis_options.is_none();
            if !direct {
                return Err(BridgeError::new("InvalidInput", "solver tuning is accepted only by the direct scip/gurobi entries without strategy options"));
            }
        }
        // 四个构建期/观测入口各接受且只接受自己的字段，并且必须显式声明对应 `modelKind`。
        // Each of the four build-time/observability entries accepts only its own fields and must declare the matching
        // `modelKind`.
        {
            // 只有当请求**确实**声明了这四个入口之一时才进入独占检查；其它入口一律不受影响。
            // The exclusivity check only applies when the request actually declares one of these four entries; every
            // other entry is untouched.
            let declared_helper = match request.model_kind {
                Some(ModelKind::IndexedRegistration) => Some(("indexed registration", true)),
                Some(ModelKind::LinearColumnCertificate) => Some(("linear column certificate",
                    request.column_batch.is_some() == request.column_scope.is_some())),
                Some(ModelKind::HeartBeat) => Some(("heartbeat", true)),
                Some(ModelKind::ResponseEnvelope) => Some(("response envelope", true)),
                Some(ModelKind::LinearLifecycle) => Some(("linear lifecycle", true)),
                Some(ModelKind::RelationalQueryPlan) => Some(("relational query plan", true)),
                Some(ModelKind::LinearBendersMaster) => Some(("linear benders master", true)),
                Some(ModelKind::PredicateFieldFilters) => Some(("predicate field filters", true)),
                Some(ModelKind::UpdateAssignments) => Some(("update assignments", true)),
                Some(ModelKind::SortPlan) => Some(("sort plan", true)),
                Some(ModelKind::RepositoryQuery) => Some(("repository query", true)),
                Some(ModelKind::PipelinePlan) => Some(("pipeline plan", true)),
                _ => None,
            };
            if let Some((label, paired)) = declared_helper {
                let exclusive = request.cp_options.is_none() && request.serial_options.is_none()
                    && request.parallel_options.is_none() && request.fixed_recourse_options.is_none()
                    && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                    && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                    && request.csp2d_options.is_none() && request.bpp_loading_options.is_none()
                    && request.gantt_activity_options.is_none() && request.iis_options.is_none()
                    && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                    && request.sweep_cases.is_none() && request.network_flow_problem.is_none()
                    && request.network_generalized_flow_problem.is_none() && request.network_design_problem.is_none()
                    && request.network_structure_problem.is_none() && request.allocation_problem.is_none()
                    && request.network_resilience_problem.is_none() && request.vrp_problem.is_none()
                    && request.quadratic_orchestration.is_none();
                // 这些入口互斥：声明了其中一个就不能再带另一个的字段。
                // These four entries are mutually exclusive: declaring one forbids carrying another's fields.
                let foreign = match request.model_kind {
                    Some(ModelKind::IndexedRegistration) => request.column_batch.is_some()
                        || request.column_scope.is_some() || request.heart_beat.is_some()
                        || request.response_envelope.is_some() || request.linear_lifecycle.is_some(),
                    Some(ModelKind::LinearColumnCertificate) => request.indexed_registration.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some(),
                    Some(ModelKind::HeartBeat) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.response_envelope.is_some() || request.linear_lifecycle.is_some(),
                    Some(ModelKind::LinearLifecycle) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.relational_query_plan.is_some(),
                    Some(ModelKind::RelationalQueryPlan) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some(),
                    Some(ModelKind::LinearBendersMaster) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some() || request.relational_query_plan.is_some()
                        || request.predicate.is_some(),
                    Some(ModelKind::PredicateFieldFilters) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some() || request.relational_query_plan.is_some()
                        || request.benders_cuts.is_some() || request.assignments.is_some()
                        || request.sort_plan.is_some(),
                    Some(ModelKind::UpdateAssignments) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some() || request.relational_query_plan.is_some()
                        || request.benders_cuts.is_some() || request.predicate.is_some()
                        || request.sort_plan.is_some(),
                    Some(ModelKind::SortPlan) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some() || request.relational_query_plan.is_some()
                        || request.benders_cuts.is_some() || request.predicate.is_some()
                        || request.assignments.is_some() || request.repository_query.is_some(),
                    Some(ModelKind::RepositoryQuery) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some() || request.relational_query_plan.is_some()
                        || request.benders_cuts.is_some() || request.predicate.is_some()
                        || request.assignments.is_some() || request.sort_plan.is_some()
                        || request.pipeline_steps.is_some(),
                    Some(ModelKind::PipelinePlan) => request.indexed_registration.is_some()
                        || request.column_batch.is_some() || request.column_scope.is_some()
                        || request.heart_beat.is_some() || request.response_envelope.is_some()
                        || request.linear_lifecycle.is_some() || request.relational_query_plan.is_some()
                        || request.benders_cuts.is_some() || request.predicate.is_some()
                        || request.assignments.is_some() || request.sort_plan.is_some()
                        || request.repository_query.is_some(),
                    _ => request.indexed_registration.is_some() || request.column_batch.is_some()
                        || request.column_scope.is_some() || request.heart_beat.is_some()
                        || request.linear_lifecycle.is_some() || request.relational_query_plan.is_some()
                        || request.assignments.is_some() || request.sort_plan.is_some()
                        || request.repository_query.is_some() || request.pipeline_steps.is_some(),
                };
                if !exclusive || !paired || foreign {
                    return Err(BridgeError::new("InvalidInput",
                        &format!("{label} accepts only its own fields and must declare the matching modelKind")));
                }
            }
            // 反过来：带了这四个入口的字段却没有声明对应 `modelKind`，也必须被拒。
            // Conversely, carrying one of these fields without declaring the matching `modelKind` is refused too.
            if request.model_kind.is_none() || !matches!(request.model_kind,
                Some(ModelKind::IndexedRegistration) | Some(ModelKind::LinearColumnCertificate)
                    | Some(ModelKind::HeartBeat) | Some(ModelKind::ResponseEnvelope)
                    | Some(ModelKind::LinearLifecycle) | Some(ModelKind::RelationalQueryPlan)
                    | Some(ModelKind::LinearBendersMaster) | Some(ModelKind::PredicateFieldFilters)
                    | Some(ModelKind::UpdateAssignments) | Some(ModelKind::SortPlan)
                    | Some(ModelKind::RepositoryQuery) | Some(ModelKind::PipelinePlan)
                    | Some(ModelKind::QuadraticBendersMaster) | Some(ModelKind::LinearBendersSub)
                    | Some(ModelKind::LogicBasedBendersPlan) | Some(ModelKind::RemoteArtifactCodec)
                    | Some(ModelKind::ConflictExplanationVerify) | Some(ModelKind::CandidateFunnelVerify)
                    | Some(ModelKind::EffectivenessVerify) | Some(ModelKind::CriticalAnalysisVerify)
                    | Some(ModelKind::RelaxationRecommend) | Some(ModelKind::FixedIntegerLpVerify)
                    | Some(ModelKind::PerturbationPolicyVerify) | Some(ModelKind::TargetVerify)
                    | Some(ModelKind::MultiTargetVerify) | Some(ModelKind::CriticalityProfileVerify)
                    | Some(ModelKind::HyperHeuristicConfigVerify) | Some(ModelKind::RewardPolicyVerify)
                    | Some(ModelKind::CheckpointVerify) | Some(ModelKind::LinearMappingVerify)
                    | Some(ModelKind::LbbResumeIdentityVerify) | Some(ModelKind::CpCheckpointVerify)
                    | Some(ModelKind::BendersMetricsVerify) | Some(ModelKind::BendersCutVerify)
                    | Some(ModelKind::SymbolConditionVerify) | Some(ModelKind::SymbolLinearizationVerify)
                    | Some(ModelKind::SymbolCoreRelationsVerify) | Some(ModelKind::HeuristicScheduleSample)
                    | Some(ModelKind::QTableSimulate) | Some(ModelKind::MasterCutEncodingVerify)
                    | Some(ModelKind::AnalysisGroupAggregate)
                    | Some(ModelKind::PredicateFieldResolutionVerify)
                    | Some(ModelKind::PredicateFieldFilterExtract)
                    | Some(ModelKind::ModelConfigurationVerify)
                    | Some(ModelKind::CacheKeyIdentityVerify)
                    | Some(ModelKind::CacheKeyAddressVerify)
                    | Some(ModelKind::FlattenCacheVerify)
                    | Some(ModelKind::UpdateAssignmentVerify)
                    | Some(ModelKind::BooleanScopeVerify)
                    | Some(ModelKind::UnsupportedPredicateVerify)
                    | Some(ModelKind::RepositoryCallVerify)
                    | Some(ModelKind::FunctionUsageVerify)
                    | Some(ModelKind::InputDomainVerify)
                    | Some(ModelKind::NativeWriteVerify)
                    | Some(ModelKind::ElasticModelVerify)
                    | Some(ModelKind::CombineStatusVerify)
                    | Some(ModelKind::NumberTextVerify)
                    | Some(ModelKind::BigMPolicyVerify)
                    | Some(ModelKind::ConditionalSemanticsVerify)
                    | Some(ModelKind::SearchBudgetVerify)
                    | Some(ModelKind::PredicateSchemaVerify)
                    | Some(ModelKind::SortByVerify)
                    | Some(ModelKind::VariableKindDomainVerify)
                    | Some(ModelKind::CriticalAssemblyVerify)
                    | Some(ModelKind::FlattenIdentityVerify)
                    | Some(ModelKind::MasterCutVerify)
                    | Some(ModelKind::BooleanFoldVerify)
                    | Some(ModelKind::FieldResolutionVerify)
                    | Some(ModelKind::ScalarFunctionVerify)
                    | Some(ModelKind::PolynomialLiftVerify)
                    | Some(ModelKind::BoundedNoGoodVerify)
                    | Some(ModelKind::EffectivenessAggregateVerify)
                    | Some(ModelKind::AssumptionBindingVerify)
                    | Some(ModelKind::TierClassificationVerify)
                    | Some(ModelKind::PolarityEncodingVerify)
                    | Some(ModelKind::QuadraticMonomialVerify)
                    | Some(ModelKind::DeferredMaterializeVerify)
                    | Some(ModelKind::BackendFeaturesVerify)
                    | Some(ModelKind::CanonicalMonomialVerify)
                    | Some(ModelKind::ExpressionKeyVerify)
                    | Some(ModelKind::EncodingLabelVerify)
                    | Some(ModelKind::ActivityBandsVerify)
                    | Some(ModelKind::EvidencePredicatesVerify)
                    | Some(ModelKind::ActivityReportCheckVerify)
                    | Some(ModelKind::AuxiliaryDomainVerify)
                    | Some(ModelKind::BindingAssignmentVerify)
                    | Some(ModelKind::SourceIdentityVerify)
                    | Some(ModelKind::CutValidityVerify)
                    | Some(ModelKind::AssignmentCompositionVerify)
                    | Some(ModelKind::DumpOptionsVerify)
                    | Some(ModelKind::NativeWriteScopeVerify)
                    | Some(ModelKind::WriterRegistryVerify)
                    | Some(ModelKind::CandidateShortlistVerify)
                    | Some(ModelKind::LazyFlattenInitVerify)
                    | Some(ModelKind::DumpBatchVerify)
                    | Some(ModelKind::QuadraticElasticVerify)
                    | Some(ModelKind::StableValueVerify)
                    | Some(ModelKind::MasterAssignmentVerify)
                    | Some(ModelKind::SparseTriadVerify)
                    | Some(ModelKind::MatrixRowsVerify)
                    | Some(ModelKind::PolynomialProbeVerify)
                    | Some(ModelKind::ModelViewVerify)
                    | Some(ModelKind::AnalysisOptionsVerify)
                    | Some(ModelKind::CapabilityMatrixVerify)
                    | Some(ModelKind::CpSupportMergeVerify)
                    | Some(ModelKind::SubproblemResultVerify)
                    | Some(ModelKind::NoGoodOracleVerify)
                    | Some(ModelKind::CpModelBuildVerify)
                    | Some(ModelKind::SnapshotGateVerify)
                    | Some(ModelKind::ConstraintCanonVerify)
                    | Some(ModelKind::AssignmentEvalVerify)
                    | Some(ModelKind::ExpressionBoundsVerify)
                    | Some(ModelKind::DomainSurfaceVerify)
                    | Some(ModelKind::IdentityWrapVerify)
                    | Some(ModelKind::ContainerSurfaceVerify)
                    | Some(ModelKind::ActivationSetVerify)
                    | Some(ModelKind::TierSelectionVerify)
                    | Some(ModelKind::EvidenceRemapVerify)
                    | Some(ModelKind::DominanceVerify)
                    | Some(ModelKind::DispersionVerify)
                    | Some(ModelKind::ReferencePointsVerify)
                    | Some(ModelKind::ParetoArchiveVerify)
                    | Some(ModelKind::ConstraintRankingVerify)
                    | Some(ModelKind::ConstraintPriorityVerify)
                    | Some(ModelKind::PopulationConfigVerify)
                    | Some(ModelKind::BoundaryVerify)
                    | Some(ModelKind::AcceptanceVerify)
                    | Some(ModelKind::RankingProbeVerify)
                    | Some(ModelKind::TopologyVerify)
                    | Some(ModelKind::MigrationVerify)
                    | Some(ModelKind::ArchiveProbeVerify)
                    | Some(ModelKind::PiecewiseVerify)
                    | Some(ModelKind::DiversityVerify)
                    | Some(ModelKind::SolutionStatusVerify)
                    | Some(ModelKind::FunnelVerify)
                    | Some(ModelKind::EffectivenessProbeVerify)
                    | Some(ModelKind::CriticalReportVerify)
                    | Some(ModelKind::GroupSummaryVerify)
                    | Some(ModelKind::BandProbeVerify)
                    | Some(ModelKind::EvidenceProbeVerify)
                    | Some(ModelKind::ProofProbeVerify)
                    | Some(ModelKind::GapProbeVerify)
                    | Some(ModelKind::DiagnosticsProbeVerify)
                    | Some(ModelKind::FingerprintProbeVerify)
                    | Some(ModelKind::BlockingSetVerify)
                    | Some(ModelKind::CorrectionProbeVerify)
                    | Some(ModelKind::SensitivityProbeVerify)
                    | Some(ModelKind::FunnelRankingVerify)
                    | Some(ModelKind::EffectivenessRankingVerify)
                    | Some(ModelKind::PersistenceSortVerify)
                    | Some(ModelKind::PersistenceSchemaVerify)
                    | Some(ModelKind::PredicateScopeVerify)
                    | Some(ModelKind::NativeTerminalProofVerify)
                    | Some(ModelKind::NativeLoweringVerify)
                    | Some(ModelKind::SolveValueVerify)
                    | Some(ModelKind::SelectionPolicyVerify)
                    | Some(ModelKind::NonStationaryVerify)
                    | Some(ModelKind::TrainingSplitVerify)
                    | Some(ModelKind::AcceptanceRecoveryVerify)
                    | Some(ModelKind::HypervolumeVerify)
                    | Some(ModelKind::OperatorCapabilityVerify)
                    | Some(ModelKind::ScheduleLogVerify)) {
                if request.indexed_registration.is_some() || request.column_batch.is_some()
                    || request.column_scope.is_some() || request.heart_beat.is_some()
                    || request.response_envelope.is_some() || request.linear_lifecycle.is_some()
                    || request.relational_query_plan.is_some() {
                    return Err(BridgeError::new("InvalidInput",
                        "build-time and observability fields require their matching modelKind"));
                }
            }
        }
        // 单 payload 入口只接受**自己那一个**字段：任一其它 payload 字段在场即拒绝。
        // 这里用唯一性检查取代了原先对 7 个旧字段的硬编码——那版会把 `predicate` / `sort_plan` 之类的夹带放过去。
        // 注意：必须放在上面那个 `!matches!` 分支**之外**，否则名单内的 kind 根本走不到这里。
        // A single-payload entry accepts **only its own** field; any other payload field present is refused. This
        // uniqueness check replaces the hard-coded seven legacy fields that let smuggled ones through. It must live
        // **outside** the `!matches!` branch above, or listed kinds never reach it.
        if let Some(own) = continuation_payload_of(request.model_kind) {
            if continuation_payloads(&request).iter().any(|field| *field != own) {
                return Err(BridgeError::new("InvalidInput",
                    "each continuation entry accepts only its own payload field"));
            }
        }
        // 二次编排入口只接受自己的配置，且必须显式声明 `quadraticOrchestration`。
        // The quadratic-orchestration entry accepts only its own configuration and must declare
        // `quadraticOrchestration`.
        if request.quadratic_orchestration.is_some() || matches!(request.model_kind, Some(ModelKind::QuadraticOrchestration)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none() && request.network_flow_problem.is_none()
                && request.network_generalized_flow_problem.is_none() && request.network_design_problem.is_none()
                && request.network_structure_problem.is_none() && request.allocation_problem.is_none()
                && request.network_resilience_problem.is_none() && request.vrp_problem.is_none();
            if !matches!(request.model_kind, Some(ModelKind::QuadraticOrchestration)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "quadratic orchestration accepts only its own configuration"));
            }
            request.quadratic_orchestration.as_ref()
                .ok_or_else(|| BridgeError::new("InvalidInput", "quadratic orchestration requires its configuration"))?
                .validate()?;
        }
        // VRP 入口只接受自己的领域问题，且必须显式声明 `vrp`。
        // The VRP entry accepts only its own domain problem and must declare `vrp`.
        if request.vrp_problem.is_some() || matches!(request.model_kind, Some(ModelKind::Vrp)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none() && request.network_flow_problem.is_none()
                && request.network_generalized_flow_problem.is_none() && request.network_design_problem.is_none()
                && request.network_structure_problem.is_none() && request.allocation_problem.is_none()
                && request.network_resilience_problem.is_none();
            if !matches!(request.model_kind, Some(ModelKind::Vrp)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "vrp accepts only its own problem"));
            }
            request.vrp_problem.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "vrp requires its problem"))?.to_domain()?;
        }
        // 韧性入口只接受自己的领域问题，且必须显式声明 `networkResilience`。
        // The resilience entry accepts only its own domain problem and must declare `networkResilience`.
        if request.network_resilience_problem.is_some() || matches!(request.model_kind, Some(ModelKind::NetworkResilience)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none() && request.network_flow_problem.is_none()
                && request.network_generalized_flow_problem.is_none() && request.network_design_problem.is_none()
                && request.network_structure_problem.is_none() && request.allocation_problem.is_none();
            if !matches!(request.model_kind, Some(ModelKind::NetworkResilience)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "network resilience accepts only its own problem"));
            }
            request.network_resilience_problem.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "network resilience requires its problem"))?.to_domain()?;
        }
        // 分配入口只接受自己的领域问题，且必须显式声明 `networkAllocation`。
        // The allocation entry accepts only its own domain problem and must declare `networkAllocation`.
        if request.allocation_problem.is_some() || matches!(request.model_kind, Some(ModelKind::NetworkAllocation)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none() && request.network_flow_problem.is_none()
                && request.network_generalized_flow_problem.is_none() && request.network_design_problem.is_none()
                && request.network_structure_problem.is_none();
            if !matches!(request.model_kind, Some(ModelKind::NetworkAllocation)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "network allocation accepts only its own problem"));
            }
            request.allocation_problem.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "network allocation requires its problem"))?.to_domain()?;
        }
        // 结构入口只接受自己的领域问题，且必须显式声明 `networkStructure`。
        // The structure entry accepts only its own domain problem and must declare `networkStructure`.
        if request.network_structure_problem.is_some() || matches!(request.model_kind, Some(ModelKind::NetworkStructure)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none() && request.network_flow_problem.is_none()
                && request.network_generalized_flow_problem.is_none() && request.network_design_problem.is_none();
            if !matches!(request.model_kind, Some(ModelKind::NetworkStructure)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "network structure accepts only its own problem"));
            }
            request.network_structure_problem.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "network structure requires its problem"))?.to_domain()?;
        }
        // 建设入口只接受自己的领域问题，且必须显式声明 `networkDesign`。
        // The design entry accepts only its own domain problem and must declare `networkDesign`.
        if request.network_design_problem.is_some() || matches!(request.model_kind, Some(ModelKind::NetworkDesign)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none() && request.network_flow_problem.is_none()
                && request.network_generalized_flow_problem.is_none();
            if !matches!(request.model_kind, Some(ModelKind::NetworkDesign)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "network design accepts only its own problem"));
            }
            request.network_design_problem.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "network design requires its problem"))?.to_domain()?;
        }
        // 广义流入口只接受自己的领域问题，且必须显式声明 `networkGeneralizedFlow`。
        // The generalized-flow entry accepts only its own domain problem and must declare `networkGeneralizedFlow`.
        if request.network_generalized_flow_problem.is_some() || matches!(request.model_kind, Some(ModelKind::NetworkGeneralizedFlow)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none() && request.network_flow_problem.is_none();
            if !matches!(request.model_kind, Some(ModelKind::NetworkGeneralizedFlow)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "generalized flow accepts only its own problem"));
            }
            request.network_generalized_flow_problem.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "generalized flow requires its problem"))?.to_domain()?;
        }
        // 网络流入口只接受自己的领域问题，且必须显式声明 `networkFlow`。
        // The network-flow entry accepts only its own domain problem and must declare `networkFlow`.
        if request.network_flow_problem.is_some() || matches!(request.model_kind, Some(ModelKind::NetworkFlow)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none()
                && request.sweep_cases.is_none();
            if !matches!(request.model_kind, Some(ModelKind::NetworkFlow)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "network flow accepts only its own problem"));
            }
            request.network_flow_problem.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "network flow requires its problem"))?.to_domain()?;
        }
        // 扫描入口只接受自己的选项，且必须显式声明 `linearSweep` 并给出 case 列表。
        // The sweep entry accepts only its own options and must declare `linearSweep` with a case list.
        if request.sweep_cases.is_some() || matches!(request.model_kind, Some(ModelKind::LinearSweep)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none() && request.incumbent_values.is_none();
            if !matches!(request.model_kind, Some(ModelKind::LinearSweep)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "linear sweep accepts only its own options"));
            }
            ospf_rust_bridge::LinearSweepCase::validate(request.sweep_cases.as_deref().ok_or_else(|| BridgeError::new("InvalidInput", "linear sweep requires its cases"))?)?;
        }
        // 敏感性入口只接受自己的选项，且必须显式声明 `linearSensitivity` 并给出完整 incumbent。
        // The sensitivity entry accepts only its own options and must declare `linearSensitivity` with a complete
        // incumbent.
        if request.incumbent_values.is_some() || matches!(request.model_kind, Some(ModelKind::LinearSensitivity)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none() && request.solution_pool_options.is_none();
            if !matches!(request.model_kind, Some(ModelKind::LinearSensitivity)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "linear sensitivity accepts only its own options"));
            }
            let incumbent = request.incumbent_values.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "linear sensitivity requires an incumbent vector"))?;
            if incumbent.is_empty() || incumbent.iter().any(|value| !value.is_finite()) {
                return Err(BridgeError::new("InvalidInput", "linear sensitivity requires a non-empty finite incumbent"));
            }
        }
        // 解池入口只接受自己的选项，且必须显式声明 `linearSolutionPool` 并给出数量。
        // The solution-pool entry accepts only its own options and must declare `linearSolutionPool` with an amount.
        if request.solution_pool_options.is_some() || matches!(request.model_kind, Some(ModelKind::LinearSolutionPool)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none()
                && request.iis_options.is_none();
            if !matches!(request.model_kind, Some(ModelKind::LinearSolutionPool)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "linear solution pool accepts only its own options"));
            }
            request.solution_pool_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "linear solution pool requires its options"))?.validate()?;
        }
        // IIS 入口只接受自己的选项，且必须显式声明 `linearIis`；任何混用都必须在执行前拒绝。
        // The IIS entry accepts only its own options and must declare `linearIis` explicitly; any mixing fails before
        // execution.
        if request.iis_options.is_some() || matches!(request.model_kind, Some(ModelKind::LinearIis)) {
            let exclusive = request.cp_options.is_none() && request.serial_options.is_none() && request.parallel_options.is_none()
                && request.fixed_recourse_options.is_none() && request.column_generation_options.is_none() && request.bpp1d_options.is_none()
                && request.bpp2d_options.is_none() && request.bpp3d_options.is_none() && request.csp1d_options.is_none()
                && request.csp2d_options.is_none() && request.bpp_loading_options.is_none() && request.gantt_activity_options.is_none();
            if !matches!(request.model_kind, Some(ModelKind::LinearIis)) || !exclusive {
                return Err(BridgeError::new("InvalidInput", "linear IIS accepts only its own options"));
            }
            let options = request.iis_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "linear IIS requires its options"))?;
            if options.max_iterations == Some(0) {
                return Err(BridgeError::new("InvalidInput", "IIS max iterations must be positive"));
            }
        }
        if request.bpp_loading_options.is_some() || request.solver == "bpp-loading" || matches!(request.model_kind, Some(ModelKind::BppLoading)) {
            if request.solver != "bpp-loading" || !matches!(request.model_kind, Some(ModelKind::BppLoading)) || request.bpp_loading_options.is_none()
                || request.cp_options.is_some() || request.serial_options.is_some() || request.parallel_options.is_some() || request.fixed_recourse_options.is_some()
                || request.column_generation_options.is_some() || request.bpp1d_options.is_some() || request.bpp2d_options.is_some()
                || request.bpp3d_options.is_some() || request.csp1d_options.is_some() || request.csp2d_options.is_some()
                || request.gantt_activity_options.is_some() {
                return Err(BridgeError::new("InvalidInput", "BPP loading requires its explicit domain strategy, modelKind bppLoading and exclusive bppLoadingOptions"));
            }
        }
        if request.csp2d_options.is_some() || request.solver == "csp2d" || matches!(request.model_kind, Some(ModelKind::Csp2d)) {
            if request.solver != "csp2d" || !matches!(request.model_kind, Some(ModelKind::Csp2d)) || request.cp_options.is_some() || request.serial_options.is_some()
                || request.parallel_options.is_some() || request.fixed_recourse_options.is_some() || request.column_generation_options.is_some()
                || request.bpp1d_options.is_some() || request.csp1d_options.is_some() || request.bpp2d_options.is_some() || request.bpp3d_options.is_some()
                || request.gantt_activity_options.is_some() {
                return Err(BridgeError::new("InvalidInput", "CSP2D options require their domain strategy and modelKind csp2d exclusively"));
            }
            request.csp2d_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "csp2d requires explicit csp2dOptions"))?.config()?;
        }
        if request.bpp2d_options.is_some() || request.solver == "bpp2d" || matches!(request.model_kind, Some(ModelKind::Bpp2d)) {
            if request.solver != "bpp2d" || !matches!(request.model_kind, Some(ModelKind::Bpp2d)) || request.cp_options.is_some() || request.serial_options.is_some()
                || request.parallel_options.is_some() || request.fixed_recourse_options.is_some() || request.column_generation_options.is_some()
                || request.bpp1d_options.is_some() || request.csp1d_options.is_some() || request.bpp3d_options.is_some()
                || request.gantt_activity_options.is_some() {
                return Err(BridgeError::new("InvalidInput", "BPP2D options require their domain strategy and modelKind bpp2d exclusively"));
            }
            request.bpp2d_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "bpp2d requires explicit bpp2dOptions"))?.config()?;
        }
        if request.csp1d_options.is_some() || request.solver == "csp1d" || matches!(request.model_kind, Some(ModelKind::Csp1d)) {
            if request.solver != "csp1d" || !matches!(request.model_kind, Some(ModelKind::Csp1d)) || request.cp_options.is_some() || request.serial_options.is_some()
                || request.parallel_options.is_some() || request.fixed_recourse_options.is_some() || request.column_generation_options.is_some() || request.bpp1d_options.is_some()
                || request.bpp3d_options.is_some() || request.gantt_activity_options.is_some() {
                return Err(BridgeError::new("InvalidInput", "CSP1D options require their domain strategy and modelKind csp1d exclusively"));
            }
            request.csp1d_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "csp1d requires explicit csp1dOptions"))?.config()?;
        }
        if request.bpp3d_options.is_some() || request.solver == "bpp3d" || matches!(request.model_kind, Some(ModelKind::Bpp3d)) {
            if request.solver != "bpp3d" || !matches!(request.model_kind, Some(ModelKind::Bpp3d)) || request.cp_options.is_some() || request.serial_options.is_some()
                || request.parallel_options.is_some() || request.fixed_recourse_options.is_some() || request.column_generation_options.is_some()
                || request.bpp1d_options.is_some() || request.csp1d_options.is_some() || request.bpp2d_options.is_some() || request.csp2d_options.is_some()
                || request.bpp_loading_options.is_some() || request.gantt_activity_options.is_some() {
                return Err(BridgeError::new("InvalidInput", "BPP3D options require their domain strategy and modelKind bpp3d exclusively"));
            }
            request.bpp3d_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "bpp3d requires explicit bpp3dOptions"))?.budgets()?;
        }
        if request.gantt_activity_options.is_some() || request.solver == "gantt-activity" || matches!(request.model_kind, Some(ModelKind::GanttActivity)) {
            if request.solver != "gantt-activity" || !matches!(request.model_kind, Some(ModelKind::GanttActivity)) || request.cp_options.is_some()
                || request.serial_options.is_some() || request.parallel_options.is_some() || request.fixed_recourse_options.is_some()
                || request.column_generation_options.is_some() || request.bpp1d_options.is_some() || request.bpp2d_options.is_some() || request.bpp3d_options.is_some()
                || request.csp1d_options.is_some() || request.csp2d_options.is_some() || request.bpp_loading_options.is_some() {
                return Err(BridgeError::new("InvalidInput", "Gantt activity options require their domain strategy and modelKind ganttActivity exclusively"));
            }
            request.gantt_activity_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "gantt-activity requires explicit ganttActivityOptions"))?;
        }
        if request.bpp1d_options.is_some() || request.solver == "bpp1d" || matches!(request.model_kind, Some(ModelKind::Bpp1d)) {
            if request.solver != "bpp1d" || !matches!(request.model_kind, Some(ModelKind::Bpp1d)) || request.cp_options.is_some() || request.serial_options.is_some()
                || request.parallel_options.is_some() || request.fixed_recourse_options.is_some() || request.column_generation_options.is_some()
                || request.bpp3d_options.is_some() || request.gantt_activity_options.is_some() {
                return Err(BridgeError::new("InvalidInput", "BPP1D options require their domain strategy and modelKind bpp1d exclusively"));
            }
            request.bpp1d_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "bpp1d requires explicit bpp1dOptions"))?.config()?;
        }
        if request.cp_options.is_some() && !matches!(request.model_kind, Some(ModelKind::Cp)) {
            return Err(BridgeError::new("InvalidInput", "CP options require modelKind cp"));
        }
        if request.serial_options.is_some() || request.solver == "serial-linear" {
            if request.solver != "serial-linear" || request.cp_options.is_some() || request.parallel_options.is_some() || request.fixed_recourse_options.is_some() || request.column_generation_options.is_some() || !matches!(request.model_kind, None | Some(ModelKind::Linear)) {
                return Err(BridgeError::new("InvalidInput", "serial options require the serial-linear strategy and a linear model"));
            }
            request.serial_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "serial-linear requires explicit serialOptions"))?.validate()?;
        }
        if request.parallel_options.is_some() || request.solver == "parallel-linear" {
            if request.solver != "parallel-linear" || request.cp_options.is_some() || request.serial_options.is_some() || request.fixed_recourse_options.is_some() || request.column_generation_options.is_some() || !matches!(request.model_kind, None | Some(ModelKind::Linear)) {
                return Err(BridgeError::new("InvalidInput", "parallel options require the parallel-linear strategy and a linear model"));
            }
            request.parallel_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "parallel-linear requires explicit parallelOptions"))?.validate()?;
        }
        if request.fixed_recourse_options.is_some() || request.solver == "fixed-recourse" {
            if request.solver != "fixed-recourse" || request.cp_options.is_some() || request.serial_options.is_some() || request.parallel_options.is_some() || request.column_generation_options.is_some()
                || !matches!(request.model_kind, None | Some(ModelKind::Linear)) {
                return Err(BridgeError::new("InvalidInput", "fixed-recourse options require their strategy and a linear model"));
            }
            request.fixed_recourse_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "fixed-recourse requires explicit fixedRecourseOptions"))?.validate()?;
        }
        if request.column_generation_options.is_some() || request.solver == "column-generation" {
            if request.solver != "column-generation" || request.cp_options.is_some() || request.serial_options.is_some() || request.parallel_options.is_some()
                || request.fixed_recourse_options.is_some() || !matches!(request.model_kind, None | Some(ModelKind::Linear)) {
                return Err(BridgeError::new("InvalidInput", "column-generation options require their strategy and a linear model"));
            }
            request.column_generation_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "column-generation requires explicit columnGenerationOptions"))?.validate()?;
        }
        let cp_options = request.cp_options.clone().unwrap_or_default();
        let hint = cp_options.decode_hint()?;
        {
            let state = lock(&self.state);
            if state.active { return Err(BridgeError::new("Busy", "one solve is already running")); }
            if state.jobs.len() >= self.capacity {
                return Err(BridgeError::new("CapacityExceeded", "release completed jobs before submitting"));
            }
        }
        if request.serial_options.is_some() { return self.start_serial(request); }
        if request.parallel_options.is_some() { return self.start_parallel(request); }
        if request.fixed_recourse_options.is_some() { return self.start_fixed_recourse(request); }
        if request.column_generation_options.is_some() { return self.start_column_generation(request); }
        if request.bpp1d_options.is_some() { return self.start_bpp1d(request); }
        if request.csp1d_options.is_some() { return self.start_csp1d(request); }
        if request.bpp2d_options.is_some() { return self.start_bpp2d(request); }
        if request.bpp3d_options.is_some() { return self.start_bpp3d(request); }
        if request.gantt_activity_options.is_some() { return self.start_gantt_activity(request); }
        if request.csp2d_options.is_some() { return self.start_csp2d(request); }
        if request.bpp_loading_options.is_some() { return self.start_bpp_loading(request); }
        match request.model_kind.unwrap_or(ModelKind::Linear) {
            ModelKind::CompiledLinear => self.start_compiled_linear(request),
            ModelKind::BppLoading => Err(BridgeError::new("InvalidInput", "BPP loading requires its domain strategy")),
            ModelKind::Csp2d => Err(BridgeError::new("InvalidInput", "csp2d requires its domain strategy")),
            ModelKind::Bpp2d => Err(BridgeError::new("InvalidInput", "bpp2d requires its domain strategy")),
            ModelKind::Bpp3d => Err(BridgeError::new("InvalidInput", "bpp3d requires its domain strategy")),
            ModelKind::Csp1d => Err(BridgeError::new("InvalidInput", "csp1d requires its domain strategy")),
            ModelKind::Bpp1d => Err(BridgeError::new("InvalidInput", "bpp1d requires its domain strategy")),
            ModelKind::GanttActivity => Err(BridgeError::new("InvalidInput", "gantt activity requires its domain strategy")),
            ModelKind::Linear => {
                let solver = create_solver(&request.solver, request.tuning_options.as_ref())?;
                let model = decode_model(&request.model)?;
                self.start(request, solver, model)
            }
            // 这四个入口都不求解；它们把框架层的构建期/观测能力直接暴露出去，因此走同一套作业生命周期。
            // None of these four entries solves anything; they expose the framework's build-time and observability
            // capabilities directly, so they share the same job lifecycle.
            ModelKind::IndexedRegistration => {
                let request_config = request.indexed_registration.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "indexed registration requires its request"))?;
                let report = ospf_rust_bridge::register_indexed_keys(&request_config)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::LinearColumnCertificate => {
                let batch = request.column_batch.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "linear column certificate requires a batch"))?;
                let scope = request.column_scope.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "linear column certificate requires an expected scope"))?;
                let certificate = ospf_rust_bridge::verify_column_batch(&batch, &scope)?;
                let json = serde_json::to_string(&certificate).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::HeartBeat => {
                let heartbeat = request.heart_beat.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "heartbeat requires its request"))?;
                let snapshot = ospf_rust_bridge::observe_heartbeat(&heartbeat)?;
                let json = serde_json::to_string(&snapshot).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::UpdateAssignments => {
                let assignments = request.assignments.clone().unwrap_or_default();
                let report = ospf_rust_bridge::validate_assignments(&assignments)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PipelinePlan => {
                let steps = request.pipeline_steps.clone().unwrap_or_default();
                let report = ospf_rust_bridge::plan_pipeline_registration(&steps)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::RepositoryQuery => {
                let spec = request.repository_query.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "repository query requires its request"))?;
                let report = ospf_rust_bridge::build_repository_query(&spec)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SortPlan => {
                let plan = request.sort_plan.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "sort plan requires its plan"))?;
                let report = ospf_rust_bridge::validate_sort_plan(plan.support, &plan.items)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PredicateFieldFilters => {
                let predicate = request.predicate.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "field filters require a predicate"))?;
                let report = ospf_rust_bridge::extract_field_filters(&predicate)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ScheduleLogVerify => {
                let document = request.schedule_log_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "schedule log verification requires the request"))?;
                // 纯数据：只跑日志校验，不执行任何真实调度、不建模型。
                // Pure data: only the log validation runs; no real scheduling and no model.
                let report = ospf_rust_bridge::verify_schedule_log(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::OperatorCapabilityVerify => {
                let document = request.operator_capability_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "operator capability verification requires the request"))?;
                // 纯数据：只跑能力声明与注册面，工厂闭包永不被调用。
                // Pure data: only the capability declaration and the registration faces run; the factory closure never runs.
                let report = ospf_rust_bridge::verify_operator_capability(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::HypervolumeVerify => {
                let document = request.hypervolume_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "hypervolume verification requires the request"))?;
                // 纯数据：只跑超体积的几何测量，不建模型、不碰求解器。
                // Pure data: only the hypervolume geometry runs.
                let report = ospf_rust_bridge::verify_hypervolume_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AcceptanceRecoveryVerify => {
                let document = request.acceptance_recovery_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "acceptance recovery verification requires the request"))?;
                // 纯数据：只跑接受准则的 save/load，不执行任何接受判定、不碰求解器。
                // Pure data: only the acceptance policies' save/load run; no acceptance judgement, no solver.
                let report = ospf_rust_bridge::verify_acceptance_recovery(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::TrainingSplitVerify => {
                let document = request.training_split_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "training split verification requires the request"))?;
                // 纯数据：只跑切分校验，不训练、不建模型、不碰求解器。
                // Pure data: only the split validation runs; no training, no model, no solver.
                let report = ospf_rust_bridge::verify_training_split_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::NonStationaryVerify => {
                let document = request.non_stationary_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "non-stationary verification requires the request"))?;
                // 纯数据：只跑非平稳选择器自己的 select/observe/discard，不建模型、不碰求解器。
                // Pure data: only the selector's own select/observe/discard run.
                let report = ospf_rust_bridge::verify_non_stationary_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SelectionPolicyVerify => {
                let document = request.selection_policy_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "selection policy verification requires the request"))?;
                // 纯数据：只在选择策略本身上跑选择与归因，不建模型、不碰求解器。
                // Pure data: only the selection policy's own select/observe run.
                let report = ospf_rust_bridge::verify_selection_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SolveValueVerify => {
                let document = request.solve_value_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "solve value verification requires the request"))?;
                // 纯数据：只走数值域转换与上界，不解模型、不碰求解器。
                // Pure data: only the numeric-domain conversion and the upper bound run.
                let report = ospf_rust_bridge::verify_solve_value_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::NativeLoweringVerify => {
                let document = request.native_lowering_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "native lowering verification requires the request"))?;
                // 纯数据：只走 lowering 报告的三个判定，不写任何原生结构、不碰求解器。
                // Pure data: only the lowering report's three predicates run; nothing is written.
                let report = ospf_rust_bridge::verify_native_lowering_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::NativeTerminalProofVerify => {
                let document = request.native_terminal_proof_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "native terminal proof verification requires the request"))?;
                // 纯数据：只走 native 终态证明门，不解任何模型、不碰任何求解器。
                // Pure data: only the native terminal proof gate runs.
                let report = ospf_rust_bridge::verify_native_terminal_proof_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PredicateScopeVerify => {
                let document = request.predicate_scope_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "predicate scope verification requires the request"))?;
                // 纯数据：只走谓词收集与策略判定，不解任何模型、不碰任何后端。
                // Pure data: only the predicate collection and the policy readings run.
                let report = ospf_rust_bridge::verify_predicate_scope_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PersistenceSchemaVerify => {
                let document = request.persistence_schema_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "persistence schema verification requires the request"))?;
                // 纯数据：只走谓词 schema 的映射与解析，不解任何模型、不碰任何后端。
                // Pure data: only the schema mapping and resolution run.
                let report = ospf_rust_bridge::verify_persistence_schema_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PersistenceSortVerify => {
                let document = request.persistence_sort_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "persistence sort verification requires the request"))?;
                // 纯数据：只走排序描述的构建与支持判据，不解任何模型、不碰任何后端。
                // Pure data: only the sort descriptor's building and support readings run.
                let report = ospf_rust_bridge::verify_persistence_sort_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EffectivenessRankingVerify => {
                let document = request.effectiveness_ranking_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "effectiveness ranking verification requires the request"))?;
                // 纯数据：只走有效性排序的两个视角与校验，不解任何模型。
                // Pure data: only the ranking's two views and validation run.
                let report = ospf_rust_bridge::verify_effectiveness_ranking_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FunnelRankingVerify => {
                let document = request.funnel_ranking.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "funnel ranking verification requires the request"))?;
                // 纯数据：只走候选漏斗的分层与排序，不解任何模型。
                // Pure data: only the candidate funnel's tiers and ordering run.
                let report = ospf_rust_bridge::verify_funnel_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SensitivityProbeVerify => {
                let document = request.sensitivity_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "sensitivity verification requires the request"))?;
                // 纯数据：只走灵敏度配置与报告的口径，不解任何模型。
                // Pure data: only the sensitivity configuration and report readings run.
                let report = ospf_rust_bridge::verify_sensitivity_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CorrectionProbeVerify => {
                let document = request.correction_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "correction verification requires the request"))?;
                // 纯数据：只走修正建议的排序与截断，不解任何模型。
                // Pure data: only the recommendation ordering and truncation run.
                let report = ospf_rust_bridge::verify_correction_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BlockingSetVerify => {
                let document = request.blocking_set.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "blocking set verification requires the request"))?;
                // 纯数据：只走最小阻塞集合的最小性判据，不解任何模型。
                // Pure data: only the minimality verdict runs, without solving anything.
                let report = ospf_rust_bridge::verify_blocking_set(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FingerprintProbeVerify => {
                let document = request.fingerprint_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "fingerprint verification requires the request"))?;
                // 纯数据：只走指纹结构与证书闸门的模型比对，不解任何模型。
                // Pure data: only the fingerprint structure and the gate's model comparison run.
                let report = ospf_rust_bridge::verify_fingerprint_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DiagnosticsProbeVerify => {
                let document = request.diagnostics_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "diagnostics verification requires the request"))?;
                // 纯数据：只走诊断面与读侧谓词，不解任何模型。
                // Pure data: only the diagnostic face and the read-side predicates run.
                let report = ospf_rust_bridge::verify_diagnostics_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::GapProbeVerify => {
                let document = request.gap_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "gap verification requires the request"))?;
                // 纯数据：只走报告的 gap 算术与逐轮快照不变量，不解任何模型。
                // Pure data: only the gap arithmetic and the trace invariants run.
                let report = ospf_rust_bridge::verify_gap_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ProofProbeVerify => {
                let document = request.proof_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "proof verification requires the request"))?;
                // 纯数据：只走报告的证明合同与精确定价闸门，不解任何模型。
                // Pure data: only the report's proof contracts and the exact-pricing gate run.
                let report = ospf_rust_bridge::verify_proof_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EvidenceProbeVerify => {
                let document = request.evidence_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "evidence verification requires the request"))?;
                // 纯数据：只走证据权威性判定，不解任何模型。
                // Pure data: only the evidence authority gate runs, without solving anything.
                let report = ospf_rust_bridge::verify_evidence_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BandProbeVerify => {
                let document = request.band_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "band verification requires the request"))?;
                // 纯数据：只走活动性分档与报告计数，不解任何模型。
                // Pure data: only the activity banding and the report counting run, without solving anything.
                let report = ospf_rust_bridge::verify_band_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::GroupSummaryVerify => {
                let document = request.group_summary.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "group summary verification requires the request"))?;
                // 纯数据：只走跨阶段组汇总，不解任何模型。
                // Pure data: only the cross-stage group summary runs, without solving anything.
                let report = ospf_rust_bridge::verify_group_summary(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CriticalReportVerify => {
                let document = request.critical_report.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "critical report verification requires the request"))?;
                // 纯数据：只走统一报告的校验与摘要，不解任何模型。
                // Pure data: only the unified report's validation and summary run, without solving anything.
                let report = ospf_rust_bridge::verify_critical_report(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EffectivenessProbeVerify => {
                let document = request.effectiveness_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "effectiveness verification requires the request"))?;
                // 纯数据：只走有效性排序的过滤与校验，不解任何模型。
                // Pure data: only the effectiveness ranking's filters and validation run, without solving anything.
                let report = ospf_rust_bridge::verify_effectiveness_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FunnelVerify => {
                let document = request.funnel.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "funnel verification requires the request"))?;
                // 纯数据：分类器与短名单各跑一遍，不解任何模型。
                // Pure data: the classifier and the shortlist run over given candidates without solving anything.
                let report = ospf_rust_bridge::verify_funnel(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SolutionStatusVerify => {
                let document = request.solution_status.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "solution status verification requires the request"))?;
                // 纯数据：只走六个构造器与两条判据，不解任何模型。
                // Pure data: the six constructors and two predicates only, without solving anything.
                let report = ospf_rust_bridge::verify_solution_status(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DiversityVerify => {
                let document = request.diversity.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "diversity verification requires the request"))?;
                // 纯数据：两个多样性度量对同一组解各算一遍，不解任何模型。
                // Pure data: both diversity metrics run over one solution set without solving anything.
                let report = ospf_rust_bridge::verify_diversity(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PiecewiseVerify => {
                let document = request.piecewise.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "piecewise verification requires the request"))?;
                // 纯数据：只走构造器，不注册符号也不建模型；结构违规由 panic 表达并被兜住。
                // Pure data: the constructor only, with no symbol registration and no model; a structural violation is
                // expressed by a panic and caught.
                let report = ospf_rust_bridge::verify_piecewise(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ArchiveProbeVerify => {
                let document = request.archive_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "archive verification requires the request"))?;
                // 纯数据：两条档案对各批候选各更新一遍，个体用全局编号认。
                // Pure data: both archives update over the same batches, with individuals identified by a global id.
                let report = ospf_rust_bridge::verify_archive_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::MigrationVerify => {
                let document = request.migration.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "migration verification requires the request"))?;
                // 纯数据：两种迁移对同一组岛屿各跑一遍，个体用全局编号认。
                // Pure data: both migrations run over one island set, with individuals identified by a global id.
                let report = ospf_rust_bridge::verify_migration(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::TopologyVerify => {
                let document = request.topology.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "topology verification requires the request"))?;
                // 纯整数：五种邻域结构各查一遍，唯一随机性是随机拓扑的 seed。
                // Pure integers: each of the five neighbourhood structures is queried once, with the random topology's
                // seed as the only randomness.
                let report = ospf_rust_bridge::verify_topology(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::RankingProbeVerify => {
                let document = request.ranking_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "ranking verification requires the request"))?;
                // 纯数据：三路排序对同一批个体排序，排序本身由模型的 `rank_indices` 决定。
                // Pure data: the three rankers sort one batch, and the order itself comes from the model's
                // `rank_indices`.
                let report = ospf_rust_bridge::verify_ranking(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AcceptanceVerify => {
                let document = request.acceptance.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "acceptance verification requires the request"))?;
                // 纯决策：四种接受判据各自对同一对个体投票，随机流由请求里的 seed 派生。
                // Pure decision: the four acceptance criteria each vote on one individual pair, with the random stream
                // derived from the request seed.
                let report = ospf_rust_bridge::verify_acceptance(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BoundaryVerify => {
                let document = request.boundary.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "boundary verification requires the request"))?;
                // 纯数值：三个边界算子各自把越界值收回可行域，一个求解器都不用。
                // Pure numerics: each boundary handler rolls an out-of-range value back into the feasible region, with no
                // solver at all.
                let report = ospf_rust_bridge::verify_boundary(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PopulationConfigVerify => {
                let document = request.population_config.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "population config verification requires the request"))?;
                // 纯配置：算子槽位用 crate 自己的默认组合填充，不跑任何算子本体。
                // Pure configuration: operator slots are filled with the crate defaults and no operator body runs.
                let report = ospf_rust_bridge::verify_population_config(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ConstraintPriorityVerify => {
                let document = request.constraint_priority.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "constraint priority verification requires the request"))?;
                // 纯数据：优先级的枚举身份与存储值这一层，一个求解器都不用。
                // Pure data: the enum identity and the storage layer of a priority, with no solver.
                let report = ospf_rust_bridge::verify_constraint_priority(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ConstraintRankingVerify => {
                let document = request.constraint_ranking.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "constraint ranking verification requires the request"))?;
                // 纯数据：已计费评估的排序器，随机流由请求里的 seed 派生（可复现）。
                // Pure data: rankers over already-metered assessments, with the random stream derived from the request
                // seed so the run stays reproducible.
                let report = ospf_rust_bridge::verify_constraint_ranking(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ParetoArchiveVerify => {
                let document = request.pareto_archive.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "pareto archive verification requires the request"))?;
                // 纯数据：有界档案的收/淘汰/裁剪规则，一个求解器、一个算子都不需要。
                // Pure data: the bounded archive's accept/evict/trim rules, with no solver and no operator.
                let report = ospf_rust_bridge::verify_pareto_archive(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ReferencePointsVerify => {
                let document = request.reference_points.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "reference points verification requires the request"))?;
                // 纯数据：单纯形格点与垂直距离，一个求解器、一个算子都不需要。
                // Pure data: the simplex lattice and the perpendicular distance, with no solver and no operator.
                let report = ospf_rust_bridge::verify_reference_points(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DispersionVerify => {
                let document = request.dispersion.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "dispersion verification requires the request"))?;
                // 纯数据：NSGA-II 的分层与多样性度量，一个求解器、一个算子都不需要。
                // Pure data: the NSGA-II layering and diversity measure, with no solver and no operator.
                let report = ospf_rust_bridge::verify_dispersion(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DominanceVerify => {
                let document = request.dominance.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "dominance verification requires the request"))?;
                // 纯数据：多目标偏序判据，一个求解器、一个算子都不需要。
                // Pure data: the multi-objective partial-order predicate, with no solver and no operator.
                let report = ospf_rust_bridge::verify_dominance(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EvidenceRemapVerify => {
                let document = request.evidence_remap.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "evidence remap verification requires the request"))?;
                // 纯数据：后端证据怎么翻译回原始证据，一个求解器都不用。
                // Pure data: how backend evidence is translated back, without a single solver.
                let report = ospf_rust_bridge::verify_evidence_remap(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::TierSelectionVerify => {
                let document = request.tier_selection.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "tier selection verification requires the request"))?;
                // 纯数据：能力矩阵下游的选择器，一个求解器都不用。
                // Pure data: the selector downstream of the capability matrix, without a single solver.
                let report = ospf_rust_bridge::verify_tier_selection(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ActivationSetVerify => {
                let document = request.activation_set.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "activation set verification requires the request"))?;
                // 纯数据：缺席即未激活，且约束永不变成 assumption。
                // Pure data: absence means inactive and a constraint never becomes an assumption.
                let report = ospf_rust_bridge::verify_activation_set(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ContainerSurfaceVerify => {
                let document = request.container_surface.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "container surface verification requires the request"))?;
                // 纯数据：`is_empty` 不是"值为零"，而改值不动缓存键。
                // Pure data: `is_empty` is not "value is zero", and mutating the value keeps the cache key.
                let report = ospf_rust_bridge::verify_container_surface(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::IdentityWrapVerify => {
                let document = request.identity_wrap.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "identity wrapping verification requires the request"))?;
                // 纯数据：三个身份包装器共用一道"非空白"闸门，但文案分两组。
                // Pure data: three identity wrappers share one blank-only gate but split into two message groups.
                let report = ospf_rust_bridge::verify_identity_wrapping(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DomainSurfaceVerify => {
                let document = request.domain_surface.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "domain surface verification requires the request"))?;
                // 纯数据：区间从不物化、枚举有闸门、成员判定看空洞。
                // Pure data: a range is never materialized, enumeration is gated and membership sees the gaps.
                let report = ospf_rust_bridge::verify_domain_surface(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ExpressionBoundsVerify => {
                let document = request.expression_bounds.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "expression bounds verification requires the request"))?;
                // 纯数据：保守界只读域的包络，求值则精确且不看域。
                // Pure data: the conservative bounds read only the domain hull while evaluation is exact and domain-blind.
                let report = ospf_rust_bridge::verify_expression_bounds(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AssignmentEvalVerify => {
                let document = request.assignment_eval.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "assignment evaluation verification requires the request"))?;
                // 纯数据：同一份快照上的赋值怎么被接受、在哪一层被拒。
                // Pure data: how an assignment on the same snapshot is accepted and which layer refuses it.
                let report = ospf_rust_bridge::verify_assignment_evaluation(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ConstraintCanonVerify => {
                let document = request.constraint_canon.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "constraint canonicalization verification requires the request"))?;
                // 纯数据：`canonicalize` 只排序顺序无意义的变体，从不合并也从不化简。
                // Pure data: `canonicalize` sorts only order-irrelevant variants and never merges nor simplifies.
                let report = ospf_rust_bridge::verify_constraint_canonicalization(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SnapshotGateVerify => {
                let document = request.snapshot_gate.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "snapshot gate verification requires the request"))?;
                // 纯数据：一份快照交回来之后，谁能证明它没被改过。
                // Pure data: once a snapshot comes back, what proves it was not edited.
                let report = ospf_rust_bridge::verify_snapshot_gates(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CpModelBuildVerify => {
                let document = request.cp_model_build.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "CP model build verification requires the request"))?;
                // 纯数据：builder 收什么、拒什么，以及 `freeze()` 造出的指纹口径。
                // Pure data: what the builder accepts and refuses, and what `freeze()` fingerprints.
                let report = ospf_rust_bridge::verify_cp_model_build(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::NoGoodOracleVerify => {
                let document = request.no_good_oracle.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "no-good oracle verification requires the request"))?;
                // 纯数据：oracle 的四个入口与它生成的 cuts，一次求解器调用都不需要。
                // Pure data: the oracle's four entries and the cuts it emits, with no solver call at all.
                let report = ospf_rust_bridge::verify_no_good_oracle(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SubproblemResultVerify => {
                let document = request.subproblem_result.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "subproblem result verification requires the request"))?;
                // 纯数据：六个构造器各自的闸门，一次求解器调用都不需要。
                // Pure data: the six constructors' own gates, with no solver call at all.
                let report = ospf_rust_bridge::verify_subproblem_result(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CpSupportMergeVerify => {
                let document = request.cp_support_merge.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "cp support merge verification requires the request"))?;
                // 纯数据：合并口径的八条规则，一次求解器调用都不需要。
                // Pure data: the eight merge rules, with no solver call at all.
                let report = ospf_rust_bridge::verify_cp_support_merge(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CapabilityMatrixVerify => {
                let document = request.capability_matrix.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "capability matrix verification requires the request"))?;
                // 纯数据：十三个能力的推断格，一个求解器都不用。
                // Pure data: the inference lattice over thirteen capabilities, with no solver at all.
                let report = ospf_rust_bridge::verify_capability_matrix(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AnalysisOptionsVerify => {
                let document = request.analysis_options.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "analysis options verification requires the request"))?;
                // 纯配置：四个 `run_*` 从不被校验，`validate()` 按阶段顺序短路，两套 CP 参数互不影响。
                // Pure configuration: the four `run_*` flags are never validated, `validate()` short-circuits in stage
                // order, and the two CP option sets are independent.
                let report = ospf_rust_bridge::verify_analysis_options(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ModelViewVerify => {
                let document = request.model_view.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "model view verification requires the request"))?;
                // 纯数据：两个视图都取线性部分的名字，`num_constraints` 只数线性行，目标向量不对账。
                // Pure data: both views read the linear name, `num_constraints` counts linear rows only, and the objective
                // is never reconciled.
                let report = ospf_rust_bridge::verify_model_views(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PolynomialProbeVerify => {
                let document = request.polynomial_probe.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "polynomial probe verification requires the request"))?;
                // 纯数据：`add_monomial` 纯追加、`zero`/`constant` 同形、`from_linear` 只产出线性 kind。
                // Pure data: `add_monomial` is pure append, `zero`/`constant` share a shape, and `from_linear` yields linear
                // kinds only.
                let report = ospf_rust_bridge::verify_polynomial_probe(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::MatrixRowsVerify => {
                let document = request.matrix_rows.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "matrix rows verification requires the request"))?;
                // 纯数据：`num_constraints()` 只读 `b`，八个平行向量同进同退，`add_row` 不做校验。
                // Pure data: `num_constraints()` reads only `b`, the eight parallel vectors move together, and `add_row`
                // validates nothing.
                let report = ospf_rust_bridge::verify_matrix_rows(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SparseTriadVerify => {
                let document = request.sparse_triad.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "sparse triad verification requires the request"))?;
                // 纯数据：`add` 不合并重复索引与零、重复 ID 只覆盖索引、`set_bounds` 不校验空域。
                // Pure data: `add` keeps duplicate indices and zeros, a duplicate ID only overwrites the index, and
                // `set_bounds` accepts an empty domain.
                let report = ospf_rust_bridge::verify_sparse_triad(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::MasterAssignmentVerify => {
                let document = request.master_assignment.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "master assignment verification requires the request"))?;
                // 纯数据：只收最优报告、`round()` 半数远离零、绝对差容差、两道范围闸。
                // Pure data: optimal reports only, `round()` half away from zero, an absolute-difference tolerance, and two
                // range gates.
                let report = ospf_rust_bridge::verify_master_assignment(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::StableValueVerify => {
                let document = request.stable_value.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "stable value verification requires the request"))?;
                // 纯数据：先 `validate()`、缺解与缺 ID 共用一条消息、绑定不参与取值。
                // Pure data: `validate()` first, one message for both failures, and the binding never gates the lookup.
                let report = ospf_rust_bridge::verify_stable_value(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::QuadraticElasticVerify => {
                let document = request.quadratic_elastic.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "quadratic elastic verification requires the request"))?;
                // 纯数据：每行一个单侧松弛、关掉开关不加松弛也不改目标方向。
                // Pure data: one one-sided slack per row, and disabling the switch adds no slack while leaving the
                // objective direction alone.
                let report = ospf_rust_bridge::verify_quadratic_elastic(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DumpBatchVerify => {
                let document = request.dump_batch.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "dump batch verification requires the request"))?;
                // 纯数据 + 临时目录：`concurrent` 只改调度、`OPM` 忽略转储选项、失败不回滚。
                // Pure data plus a temporary directory: `concurrent` only changes scheduling, `OPM` ignores the dump
                // options, and a failure rolls nothing back.
                let report = ospf_rust_bridge::verify_dump_batch(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::LazyFlattenInitVerify => {
                let document = request.lazy_flatten_init.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "lazy flatten init verification requires the request"))?;
                // 纯数据：`OnceLock` 先到先得，未初始化读 `token_list()` 会 panic（本层用 catch_unwind 观测）。
                // Pure data: `OnceLock` is first-wins, and reading `token_list()` before init panics (observed with
                // `catch_unwind` here).
                let report = ospf_rust_bridge::verify_lazy_flatten_init(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CandidateShortlistVerify => {
                let document = request.candidate_shortlist.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "candidate shortlist verification requires the request"))?;
                // 纯数据：A/B 优先进短名单、低层填剩余名额、`shortlist()` 从不按 priority 排序。
                // Pure data: A/B are preferred, lower tiers fill the remaining slots, and `shortlist()` never sorts by
                // priority.
                let report = ospf_rust_bridge::verify_candidate_shortlist(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::WriterRegistryVerify => {
                let document = request.writer_registry.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "writer registry verification requires the request"))?;
                // 纯数据：先注册的 writer 先决定、已决定的结构离开批次、跳过不是回退、条数不符立即失败。
                // Pure data: the first registered writer decides, decided structures leave the batch, a skip is not a
                // fallback, and a length mismatch fails immediately.
                let report = ospf_rust_bridge::verify_writer_registry(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::NativeWriteScopeVerify => {
                let document = request.native_write_scope.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "native write scope verification requires the request"))?;
                // 纯数据：指纹两侧缺一不可、函数名不参与匹配、两个计数不与 `outcomes` 交叉校验。
                // Pure data: both fingerprints are required, the function name is out of the match, and neither counter is
                // cross-checked against the outcomes.
                let report = ospf_rust_bridge::verify_native_write_scope(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DumpOptionsVerify => {
                let document = request.dump_options.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "dump options verification requires the request"))?;
                // 纯数据：`include_bounds` 与 `force_bounds` 互相独立、可矛盾；`with_fixed_variables` 整表替换。
                // Pure data: `include_bounds` and `force_bounds` are independent and may contradict each other, and
                // `with_fixed_variables` replaces the whole map.
                let report = ospf_rust_bridge::verify_dump_options(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AssignmentCompositionVerify => {
                let document = request.assignment_composition.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "assignment composition verification requires the request"))?;
                // 纯数据：`then` 是 FIFO 追加、同一路径不合并、`set_null` 与 `set(_, null)` 不同变体、
                // 属性路径有三种校验强度。
                // Pure data: `then` appends FIFO, the same path is never merged, `set_null` and `set(_, null)` are different
                // variants, and a property path has three validation strengths.
                let report = ospf_rust_bridge::verify_assignment_composition(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CutValidityVerify => {
                let document = request.cut_validity.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "cut validity verification requires the request"))?;
                // 纯数据：`Local` 从不被拒，有效性与证明状态都不进 `expression_key`，但 `Verified` 必须挂非空引用。
                // Pure data: `Local` is never refused, validity and proof status stay out of the `expression_key`, and
                // `Verified` must carry a non-blank reference.
                let report = ospf_rust_bridge::verify_cut_validity(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SourceIdentityVerify => {
                let document = request.source_identity.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "source identity verification requires the request"))?;
                // 纯数据：六种来源的 `kind()` 与 `stable_id()`，以及下界与带侧边界共享同一个 ID。
                // Pure data: the six kinds' `kind()` and `stable_id()`, and the shared ID between a lower bound and a sided
                // lower bound.
                let report = ospf_rust_bridge::verify_source_identity(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BindingAssignmentVerify => {
                let document = request.binding_assignment.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "binding assignment verification requires the request"))?;
                // 纯数据：绑定集合先建成，赋值再要求**恰好覆盖**；少给与多给是同一个错误。
                // Pure data: the binding set is built first, then the assignment must cover it **exactly**; too few and too
                // many give the same error.
                let report = ospf_rust_bridge::verify_binding_assignment(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AuxiliaryDomainVerify => {
                let document = request.auxiliary_domain.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "auxiliary domain verification requires the request"))?;
                // 纯数据：整数值域必须能被 f64 精确表示，界颠倒被拒，`Binary` 域不参与校验。
                // Pure data: an integer domain must be exactly representable in f64, inverted bounds are refused, and the
                // `Binary` domain takes neither check.
                let report = ospf_rust_bridge::verify_auxiliary_domain(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ActivityReportCheckVerify => {
                let document = request.activity_report_check.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "activity report verification requires the request"))?;
                // 纯数据：聚合字段重算比对，数值字段只查有限性。
                // Pure data: aggregates are recomputed and compared, numeric fields only need to be finite.
                let verdict = ospf_rust_bridge::verify_activity_report(document)?;
                let json = serde_json::to_string(&verdict).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EvidencePredicatesVerify => {
                let document = request.evidence_predicates.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "evidence predicates verification requires the request"))?;
                // 纯数据：`hasSlackMetric()` 与 `satisfied()` 对同一个变体的覆盖不一致。
                // Pure data: `hasSlackMetric()` and `satisfied()` disagree in coverage on the same variant.
                let report = ospf_rust_bridge::verify_evidence_predicates(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ActivityBandsVerify => {
                let document = request.activity_bands.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "activity bands verification requires the request"))?;
                // 纯数据：判定顺序是先违约、再活动、再接近、最后不活动，且非有限值先判为 Unknown。
                // Pure data: the order is violation, active, nearly-active, inactive, and non-finite values short-circuit to
                // Unknown.
                let report = ospf_rust_bridge::verify_activity_bands(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EncodingLabelVerify => {
                let document = request.encoding_label.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "encoding label verification requires the request"))?;
                // 纯数据：编码标签不进 `expressionKey()`，也不进 `normalize()` 的校验。
                // Pure data: the encoding label enters neither `expressionKey()` nor `normalize()`'s validation.
                let report = ospf_rust_bridge::verify_encoding_label(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ExpressionKeyVerify => {
                let document = request.expression_key.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "expression key verification requires the request"))?;
                // 纯数据：浮点走 `to_bits()` 定宽十六进制，所以 `0.0` 与 `-0.0` 的 key 不同。
                // Pure data: floats go through `to_bits()` in fixed-width hex, so `0.0` and `-0.0` key differently.
                let report = ospf_rust_bridge::verify_expression_key(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CanonicalMonomialVerify => {
                let document = request.canonical_monomial.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "canonical monomial verification requires the request"))?;
                // 纯数据：幂次表不校验符号，`totalPower` 只是求和，追加不合并。
                // Pure data: the power table is unvalidated in sign, `totalPower` is only a sum, and appending never merges.
                let report = ospf_rust_bridge::verify_canonical_monomial(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BackendFeaturesVerify => {
                let document = request.backend_features.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "backend features verification requires the request"))?;
                // 纯配置：族开关与方言开关互不蕴含，`anyEnabled()` 只看族。
                // Pure configuration: family and dialect flags never imply each other, and `anyEnabled()` reads only the
                // families.
                let report = ospf_rust_bridge::verify_backend_features(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::DeferredMaterializeVerify => {
                let document = request.deferred_materialize.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "deferred materialization verification requires the request"))?;
                // 纯数据（用可观测的假结构）：失败不写一行，索引排序去重，未处理的保持原顺序。
                // Pure data (with observable fake structures): a failure writes nothing, indices dedupe, and the untouched
                // structures keep their order.
                let report = ospf_rust_bridge::verify_materialize(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::QuadraticMonomialVerify => {
                let document = request.quadratic_monomial.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "quadratic monomial verification requires the request"))?;
                // 纯数据：kind 是 `varIndex2` 的推导，序号不给排序，平方仍是二次项。
                // Pure data: the kind is derived from `varIndex2`, indices stay unsorted, and a square stays quadratic.
                let report = ospf_rust_bridge::verify_quadratic_monomial(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PolarityEncodingVerify => {
                let document = request.polarity_encoding.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "polarity encoding verification requires the request"))?;
                // 纯数据：系数是取反的位，右端项是 1 减 1 的个数，边行的右端项就是值域边界。
                // Pure data: coefficients are the negated bits, the rhs is 1 minus the ones count, and a side row's rhs is
                // the domain bound.
                let report = ospf_rust_bridge::verify_polarity_encoding(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::TierClassificationVerify => {
                let document = request.tier_classification.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "tier classification verification requires the request"))?;
                // 纯数据：分层只由活动性与对偶证据推导，"未知"绝不变成"不重要"。
                // Pure data: the tier comes only from activity plus dual evidence, and "unknown" never becomes
                // "unimportant".
                let report = ospf_rust_bridge::verify_tier_classification(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AssumptionBindingVerify => {
                let document = request.assumption_binding.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "CP assumption binding verification requires the request"))?;
                // 纯数据：assumption 的稳定身份是长度前缀拼接，`sparseDomain` 的身份**丢掉值域内容**。
                // Pure data: an assumption's stable identity is a length-prefixed concatenation, and a `sparseDomain`'s
                // identity **drops its values**.
                let report = ospf_rust_bridge::verify_assumption_binding(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EffectivenessAggregateVerify => {
                let document = request.effectiveness_aggregate.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "effectiveness aggregation requires the request"))?;
                // 纯数据：只有已证明的观察进排序，而删除测试的改善进不了 `max_improvement`。
                // Pure data: only proven observations feed the ranking, and a removal improvement never reaches
                // `max_improvement`.
                let report = ospf_rust_bridge::verify_effectiveness_aggregate(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BoundedNoGoodVerify => {
                let document = request.bounded_no_good.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "bounded no-good verification requires the request"))?;
                // 纯数据：一次调用产出整个家族（3n+1 行），每行都声明完整辅助集。
                // Pure data: one call yields a whole family (3n+1 rows) and every row declares the full auxiliary set.
                let report = ospf_rust_bridge::verify_bounded_no_good(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PolynomialLiftVerify => {
                let document = request.polynomial_lift.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "polynomial lift verification requires the request"))?;
                // 纯数据：线性→二次有现成的桥，二次→规范化没有；规范化态靠幂次表且缺项读回 0。
                // Pure data: linear-to-quadratic has a ready bridge while quadratic-to-canonical does not; the canonical
                // state lives on a power table where an absent variable reads back as 0.
                let report = ospf_rust_bridge::verify_polynomial_lift(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ScalarFunctionVerify => {
                let document = request.scalar_function.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "scalar function verification requires the request"))?;
                // 纯数据：六个名字常量与六个构造器的对应关系，以及裸构造器完全不校验。
                // Pure data: the six constants against their builders, and how the raw constructor validates nothing.
                let report = ospf_rust_bridge::verify_scalar_functions(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FieldResolutionVerify => {
                let document = request.field_resolution.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "field resolution verification requires the request"))?;
                // 纯数据：四态词汇，以及可空接口把三种失败压成同一个 `None`。
                // Pure data: the four-state vocabulary and the nullable interface flattening three failures into one `None`.
                let report = ospf_rust_bridge::verify_field_resolution(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BooleanFoldVerify => {
                let document = request.boolean_fold.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "boolean fold verification requires the request"))?;
                // 纯数据：空作用域的常量、单元素拆壳、只折叠被收集的尾部后缀。
                // Pure data: the empty-scope constants, single-element unwrapping, and the collected-suffix fold.
                let report = ospf_rust_bridge::verify_boolean_fold(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::MasterCutVerify => {
                let document = request.master_cut.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "master cut verification requires the request"))?;
                // 纯数据：构造与规范化两级不变量，以及忽略出处的表达式身份。
                // Pure data: the construct-vs-normalize invariants and the provenance-free expression identity.
                let report = ospf_rust_bridge::verify_master_cut(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FlattenIdentityVerify => {
                let document = request.flatten_identity.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "flatten identity verification requires the request"))?;
                // 纯数据：缓存键是堆地址，内容相等不等于键相等，也不能跨运行持久化。
                // Pure data: the cache key is a heap address, so equal content is not an equal key and nothing persists.
                let report = ospf_rust_bridge::verify_flatten_identity(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CriticalAssemblyVerify => {
                let document = request.critical_assembly.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "critical assembly verification requires the request"))?;
                // 纯数据：七段分析报告聚合器的前置、粘性标记与最弱环节状态。
                // Pure data: the seven-stage report aggregator's prerequisites, sticky flag and weakest-link status.
                let report = ospf_rust_bridge::verify_critical_assembly(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::VariableKindDomainVerify => {
                let document = request.variable_kind_domain.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "variable kind domain verification requires the request"))?;
                // 纯数据：八种变量类型的界、整数性、符号性与取值判定。
                // Pure data: the eight variable kinds' bounds, integrality, signedness and value checks.
                let report = ospf_rust_bridge::verify_variable_kind_domain(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SortByVerify => {
                let document = request.sort_by.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "sort by verification requires the request"))?;
                // 纯数据：排序描述符的拼接代数与空值排序支持矩阵。
                // Pure data: the descriptor's composition algebra and the null-ordering support matrix.
                let report = ospf_rust_bridge::verify_sort_by(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PredicateSchemaVerify => {
                let document = request.predicate_schema.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "predicate schema verification requires the request"))?;
                // 纯数据：字段映射表的两套解析口径与重复注册行为。
                // Pure data: the field table's two lookups and its duplicate-registration behaviour.
                let report = ospf_rust_bridge::verify_predicate_schema(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SearchBudgetVerify => {
                let document = request.search_budget.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "search budget verification requires the request"))?;
                // 纯数据：评估预算的嵌套计费与随机流可恢复性，不跑搜索。
                // Pure data: the budget's nested accounting and the random stream's resumability; no search runs.
                let report = ospf_rust_bridge::verify_search_budget(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ConditionalSemanticsVerify => {
                let document = request.conditional_semantics.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "conditional semantics verification requires the request"))?;
                // 纯数据：三值分类、界的可分性与六个入口的落地形状，不求解。
                // Pure data: three-valued classification, bounds separability and the six entries' shapes.
                let report = ospf_rust_bridge::verify_conditional_semantics(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BigMPolicyVerify => {
                let document = request.big_m_policy.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "big M verification requires the request"))?;
                // 纯数据：夹紧探测 + 三个指示约束构造器，不求解。
                // Pure data: clamp probes plus the three indicator builders; nothing is solved.
                let report = ospf_rust_bridge::verify_big_m_policy(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::NumberTextVerify => {
                let document = request.number_text.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "number text verification requires the request"))?;
                // 纯数据：数值文本的格式化、解析与四个编解码器的往返，不碰任何状态机。
                // Pure data: number formatting, parsing and the four codecs' round-trips; no state machine involved.
                let report = ospf_rust_bridge::verify_number_text(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CombineStatusVerify => {
                let document = request.combine_status.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "combine status verification requires the request"))?;
                // 纯数据：合并分析状态，穷举 20 格全表与三条代数律。
                // Pure data: combine analysis statuses, enumerating the twenty-cell table and the three laws.
                let report = ospf_rust_bridge::verify_combine_status(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ElasticModelVerify => {
                let document = request.elastic_model.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "elastic model verification requires the request"))?;
                // 纯数据：建一个内存模型，走弹性化 / 可行性 / 线性松弛三个纯变换，不求解。
                // Pure data: build an in-memory model and walk the three pure transforms; nothing is solved.
                let report = ospf_rust_bridge::verify_elastic_model(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::NativeWriteVerify => {
                let document = request.native_write.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "native write verification requires the request"))?;
                // 纯数据：指纹成对判定、绑定构造与域证明方向，不写任何原生结构。
                // Pure data: the pairwise fingerprint check, binding construction and proof direction; nothing is written.
                let report = ospf_rust_bridge::verify_native_write_record(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::InputDomainVerify => {
                let document = request.input_domain.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "input domain verification requires the request"))?;
                // 纯数据：区间代数与浮点指纹，不建模型、不连求解器。
                // Pure data: interval algebra and float fingerprints; no model is built and no solver is touched.
                let report = ospf_rust_bridge::verify_input_domain(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FunctionUsageVerify => {
                let document = request.function_usage.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "function usage verification requires the request"))?;
                // 纯数据：走领域的四个派生判定，并把 32 种标志组合全部枚举一遍。
                // Pure data: walk the domain's four derived predicates and enumerate all 32 flag combinations.
                let report = ospf_rust_bridge::verify_function_usage(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::RepositoryCallVerify => {
                let document = request.repository_call.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "repository call verification requires the request"))?;
                // 纯数据：走一遍仓储接口的默认方法，用的是内存里的测试仓储。
                // Pure data: walk the repository interface's default methods over an in-memory test repository.
                let report = ospf_rust_bridge::verify_repository_calls(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::UnsupportedPredicateVerify => {
                let document = request.unsupported_predicate.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "unsupported predicate verification requires the request"))?;
                // 纯配置：只解析策略名与注解，不连任何后端。
                // Pure configuration: only the policy names and annotations are resolved; no backend is touched.
                let report = ospf_rust_bridge::verify_unsupported_predicate(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BooleanScopeVerify => {
                let document = request.boolean_scope.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "boolean scope verification requires the request"))?;
                // 纯数据：用领域的收集器搭表达式并组合，不连任何后端。
                // Pure data: assemble the expression through the domain collectors and combine it; no backend is touched.
                let report = ospf_rust_bridge::verify_boolean_scope(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::UpdateAssignmentVerify => {
                let document = request.update_assignment.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "update assignment verification requires the request"))?;
                // 纯数据：用领域构造器叠加赋值，不连任何后端。
                // Pure data: layer the assignments through the domain constructors; no backend is touched.
                let report = ospf_rust_bridge::verify_update_assignments(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FlattenCacheVerify => {
                let document = request.flatten_cache.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "flatten cache verification requires the request"))?;
                // 纯数据：回放平展缓存操作，不建模型、不求值。
                // Pure data: replay the flatten-cache operations; no model is built and nothing is evaluated.
                let report = ospf_rust_bridge::verify_flatten_cache(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CacheKeyAddressVerify => {
                let document = request.cache_key_address.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "cache key address verification requires the request"))?;
                // 纯数据：造对象取地址键，不建模型、不求值。
                // Pure data: build objects and read their address keys; no model is built and nothing is evaluated.
                let report = ospf_rust_bridge::verify_cache_key_address(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CacheKeyIdentityVerify => {
                let document = request.cache_key_identity.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "cache key identity verification requires the request"))?;
                // 纯数据：两个缓存的操作序列回放，不建模型、不求值。
                // Pure data: replay the operation log against both caches; no model is built and nothing is evaluated.
                let report = ospf_rust_bridge::verify_cache_key_identity(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ModelConfigurationVerify => {
                let document = request.model_configuration.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "model configuration verification requires the request"))?;
                // 纯配置：从领域默认值出发叠覆盖项，不建模型、不编译。
                // Pure configuration: start from the domain defaults and layer overrides; no model is built or compiled.
                let report = ospf_rust_bridge::verify_model_configuration(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PredicateFieldFilterExtract => {
                let document = request.predicate_field_filter.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "predicate field filter extraction requires the request"))?;
                // 纯数据：编成领域表达式后跑三个提取器，不连任何后端。
                // Pure data: compile into a domain expression, run the three extractors, touch no backend.
                let report = ospf_rust_bridge::verify_predicate_field_filters(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PredicateFieldResolutionVerify => {
                let document = request.predicate_field_resolution.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "predicate field resolution requires the request"))?;
                // 纯数据：领域构建器堆 schema + 静态映射解析，不连任何后端。
                // Pure data: the domain builder assembles the schema and a static mapping resolves it; no backend is touched.
                let report = ospf_rust_bridge::verify_predicate_field_resolution(document)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::AnalysisGroupAggregate => {
                let reports = request.analysis_group_aggregate.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "analysis group aggregation requires the request"))?;
                // 纯数据：三份报告各自解码后交给领域汇总，不跑任何分析阶段。
                // Pure data: each report is decoded and handed to the domain aggregation; no analysis stage runs.
                let report = ospf_rust_bridge::aggregate_analysis_groups(reports)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::MasterCutEncodingVerify => {
                let encoding = request.master_cut_encoding.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "master cut encoding verification requires the request"))?;
                // 纯数据：映射成领域编码值 + 领域冲突构造器校验，不跑任何 Benders 迭代。
                // Pure data: map into the domain encoding value and let the domain conflict constructor validate; no
                // Benders iteration runs.
                let report = ospf_rust_bridge::verify_master_cut_encoding(encoding)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::QTableSimulate => {
                let table = request.q_table.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "q table replay requires the request"))?;
                // 纯数值：解码初始表、按序施加转移、投影最终表与动作选择；不建 agent、不跑搜索。
                // Purely numeric: decode the initial table, apply the transitions in order, project the final table and the
                // selections; no agent and no search.
                let report = ospf_rust_bridge::verify_q_table(table)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::HeuristicScheduleSample => {
                let schedule = request.heuristic_schedule.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "heuristic schedule sampling requires the request"))?;
                // 只采样参数演化曲线：不建种群、不评估适应度、不跑任何算子。
                // Only the parameter curve is sampled: no population, no fitness evaluation and no operator run.
                let report = ospf_rust_bridge::sample_heuristic_schedule(schedule)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SymbolCoreRelationsVerify => {
                // 纯表读取：不接受参数，也不建变量。 / A pure table read: no parameters and no variables.
                let report = ospf_rust_bridge::verify_symbol_core_relations()?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SymbolLinearizationVerify => {
                let parameters = request.symbol_linearization.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "symbol linearization verification requires the request"))?;
                // 纯参数：只读常量与纯标量规则，不建变量、不生成约束。
                // Pure parameters: constants and purely scalar rules only; no variable and no constraint.
                let report = ospf_rust_bridge::verify_symbol_linearization(parameters)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::SymbolConditionVerify => {
                let condition = request.symbol_condition.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "symbol condition verification requires the request"))?;
                // 纯数值：只做词汇与参数校验，不求值 `Linear`、不建变量、不生成约束。
                // Purely numeric: vocabulary and parameter validation only; no `Linear` is evaluated, no variable is
                // created and no constraint is generated.
                let report = ospf_rust_bridge::verify_symbol_condition(condition)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BendersCutVerify => {
                let cuts = request.benders_cut_request.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "benders cut verification requires the request"))?;
                // 纯数据：领域转换器校验 cut 结构，再把领域值反投影成线格式。
                // Pure data: the domain converters validate the cut structure, then the domain values are projected back
                // into the wire format.
                let report = ospf_rust_bridge::verify_benders_cuts(cuts)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::BendersMetricsVerify => {
                let metrics = request.benders_metrics.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "benders metrics verification requires the request"))?;
                // 纯数据：映射到真实领域结构 + 桥接层一致性检查 + 投影，全程不跑求解器。
                // Pure data: map onto the real domain structs, run the bridge-level consistency checks and project; no
                // solver runs.
                let report = ospf_rust_bridge::verify_benders_metrics(metrics)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CpCheckpointVerify => {
                let checkpoint = request.cp_checkpoint_request.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "CP checkpoint verification requires the request"))?;
                // 纯文本：解码 → 领域校验（含完整 hint 整体复验）→ 规范字节比对 → 类型化状态投影。
                // Pure text: decode, domain validation (whole-assignment hint revalidation included), canonical-byte
                // comparison, then the typed-state projection.
                let report = ospf_rust_bridge::verify_cp_checkpoint(checkpoint)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::LbbResumeIdentityVerify => {
                let identity = request.lbb_resume_identity.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "resume identity verification requires the request"))?;
                // 纯校验：不重建引擎，也不读检查点本体。 / Pure validation: no engine rebuild, no checkpoint body.
                let report = ospf_rust_bridge::verify_lbb_resume_identity(identity)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::LinearMappingVerify => {
                // 请求本身可省略：只查映射、不核对声称的映射时才这样用。
                // The request itself may be omitted when only the mapping is wanted with no claimed cross-check.
                let mapping_request = request.linear_mapping_request.clone().unwrap_or(ospf_rust_bridge::LinearMappingRequest {
                    claimed_mapping: None });
                // 纯数据：解码模型 → 校验结构 → 生成稳定 ID → 证明双射，全部在领域里。
                // Pure data: decode the model, validate it, derive the stable IDs and prove the bijection, all in the domain.
                let report = ospf_rust_bridge::verify_linear_mapping(&request.model, &mapping_request)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CheckpointVerify => {
                let checkpoint = request.checkpoint_request.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "checkpoint verification requires the request"))?;
                // 纯文本：严格解码 + 版本校验；Kotlin v1 审计的结论是数据，不是错误。
                // Pure text: strict decoding plus version checks; the Kotlin-v1 verdict is data, not an error.
                let report = ospf_rust_bridge::verify_checkpoint(checkpoint)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::RewardPolicyVerify => {
                let policy = request.reward_policy.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "reward verification requires the request"))?;
                // 纯校验 + 可恢复状态：三个构造器都不执行任何业务目标。
                // Pure validation plus restorable state: none of the three constructors evaluates a business objective.
                let report = ospf_rust_bridge::verify_reward_policy(policy)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::HyperHeuristicConfigVerify => {
                let config = request.hyper_heuristic_config.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "hyper-heuristic config verification requires the request"))?;
                // 纯文本能力：严格解码 + 结构校验 + 规范化，不需要求解器或注册表。
                // Pure text: strict decoding, structural validation and normalization; no solver or registry.
                let report = ospf_rust_bridge::verify_hyper_heuristic_config(config)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CriticalityProfileVerify => {
                let criticality = request.criticality_request.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "criticality verification requires the request"))?;
                // 纯数据：分类规则只在领域里实现一次，桥接不重写。 / Pure data: the classification rules live once.
                let report = ospf_rust_bridge::verify_criticality_profile(criticality)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::MultiTargetVerify => {
                let report = request.multi_target_report.as_deref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "multi-target verification requires the report"))?;
                // 纯数据：跨目标重算核对，不需要任何后端特性。 / Pure data: cross-target recomputation; no backend feature.
                let summary = ospf_rust_bridge::verify_multi_target_report(
                    report, request.relaxation_policy.as_ref())?;
                let json = serde_json::to_string(&summary).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::TargetVerify => {
                let spec = request.target_spec.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "target verification requires the spec"))?;
                // 纯函数：目标构造（六种构造器）+ 整数边界派生 + 可选报告校验，不需要任何后端特性。
                // Pure functions: target construction (six constructors), integer-bound derivation and an optional report
                // check; no backend feature.
                let verification = ospf_rust_bridge::verify_target(spec, request.target_report.as_deref())?;
                let json = serde_json::to_string(&verification).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::PerturbationPolicyVerify => {
                let policy = request.perturbation_policy.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "perturbation policy verification requires the request"))?;
                // 纯数据：策略构造/校验 + 可选报告校验与投影，不需要任何后端特性。
                // Pure data: policy construction/validation plus an optional report check; no backend feature.
                let verification = ospf_rust_bridge::verify_perturbation_policy(policy)?;
                let json = serde_json::to_string(&verification).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::FixedIntegerLpVerify => {
                // 纯数据：配置校验 + 报告校验与投影，不需要任何后端特性。
                // Pure data: config validation plus report validation and projection; no backend feature.
                let report = ospf_rust_bridge::verify_fixed_integer_lp(
                    request.fixed_integer_config.as_ref(),
                    request.fixed_integer_report.as_deref(),
                )?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::RelaxationRecommend => {
                let relaxation = request.relaxation_request.as_ref()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "relaxation recommendations require the request"))?;
                // 纯函数组合：给定已验证冲突直接产出建议，不需要任何后端特性。
                // A pure-function composition: a verified conflict yields recommendations with no backend feature.
                let report = ospf_rust_bridge::generate_relaxation_recommendations(relaxation)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CriticalAnalysisVerify => {
                let report = request.critical_analysis.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "critical analysis verification requires the report"))?;
                // 纯领域校验与摘要，不需要任何后端特性。 / Pure domain validation and summary; no backend feature.
                let summary = ospf_rust_bridge::verify_critical_analysis(&report)?;
                let json = serde_json::to_string(&summary).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::EffectivenessVerify => {
                let ranking = request.effectiveness_ranking.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "effectiveness verification requires the ranking"))?;
                // 纯领域校验与投影，不需要任何后端特性。 / Pure domain validation and projections; no backend feature.
                let report = ospf_rust_bridge::verify_effectiveness_ranking(&ranking)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::CandidateFunnelVerify => {
                let funnel = request.candidate_funnel.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "candidate funnel verification requires the funnel"))?;
                // 纯领域校验与短名单，不需要任何后端特性。 / Pure domain validation and shortlist; no backend feature.
                let report = ospf_rust_bridge::verify_candidate_funnel(&funnel, request.shortlist_limit)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ConflictExplanationVerify => {
                let explanation = request.conflict_explanation.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "conflict explanation verification requires the explanation"))?;
                // 纯领域校验，不需要任何后端特性。 / Pure domain validation needing no backend feature.
                let summary = ospf_rust_bridge::verify_conflict_explanation(&explanation)?;
                let json = serde_json::to_string(&summary).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::RemoteArtifactCodec => {
                let request_payload = request.remote_artifact.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "the remote artifact codec requires its request"))?;
                // 纯数据面，不需要任何后端特性：解码与校验都由框架侧完成。
                // Pure data surface needing no backend feature: the framework does the decoding and validation.
                let report = ospf_rust_bridge::decode_remote_artifact(request_payload.kind,
                    &request_payload.payload, request_payload.operation, request_payload.expected.as_ref())?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::LogicBasedBendersPlan => {
                let plan = request.logic_based_benders_plan.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput",
                        "logic-based Benders planning requires its request"))?;
                let report = ospf_rust_bridge::plan_logic_based_benders(&plan)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::LinearBendersSub => {
                let master_solution = request.master_solution.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "the Benders subproblem step requires a master solution"))?;
                #[cfg(feature = "scip")]
                {
                    // 子问题单步同样不需要 cut 生成上下文：桥接交出对偶，cut 由调用方以数据在下一轮 master 给出。
                    // The subproblem step likewise needs no cut-generation context: the bridge hands out the duals and the
                    // caller supplies the cut as data in the next master round.
                    use ospf_rust_core_plugin_scip::extension::ScipLinearBendersDecompositionSolver;
                    let report = ospf_rust_bridge::solve_benders_sub(
                        &ScipLinearBendersDecompositionSolver::new(), &request.model, &master_solution)?;
                    let json = serde_json::to_string(&report)
                        .map_err(|error| BridgeError::new("CodecFailure", error))?;
                    self.start_encoded(request, move |_| Ok(json))
                }
                #[cfg(not(feature = "scip"))]
                {
                    let _ = master_solution;
                    Err(BridgeError::new("UnsupportedDialect",
                        "the Benders subproblem step requires a backend feature"))
                }
            }
            ModelKind::QuadraticBendersMaster => {
                let linear_cuts = request.benders_cuts.clone().unwrap_or_default();
                let quadratic_cuts = request.benders_quadratic_cuts.clone().unwrap_or_default();
                #[cfg(feature = "scip-quadratic")]
                {
                    // 二次主问题与线性侧对称：cuts 由调用方以数据给出，不需要 cut 生成上下文。
                    // The quadratic master is symmetric with the linear side: cuts arrive as data and no cut-generation
                    // context is needed.
                    use ospf_rust_core_plugin_scip::extension::ScipQuadraticBendersDecompositionSolver;
                    let report = ospf_rust_bridge::solve_benders_quadratic_master(
                        &ScipQuadraticBendersDecompositionSolver::new(), &request.model, &linear_cuts,
                        &quadratic_cuts)?;
                    let json = serde_json::to_string(&report)
                        .map_err(|error| BridgeError::new("CodecFailure", error))?;
                    self.start_encoded(request, move |_| Ok(json))
                }
                #[cfg(not(feature = "scip-quadratic"))]
                {
                    let _ = (linear_cuts, quadratic_cuts);
                    Err(BridgeError::new("UnsupportedDialect",
                        "the quadratic Benders master step requires the scip-quadratic feature"))
                }
            }
            ModelKind::LinearBendersMaster => {
                let cuts = request.benders_cuts.clone().unwrap_or_default();
                #[cfg(feature = "scip")]
                {
                    // 主问题单步只需要已绑定的 Benders 求解器，不需要 cut 生成上下文。
                    // The master single step needs only a bound Benders solver, not a cut-generation context.
                    use ospf_rust_core_plugin_scip::extension::ScipLinearBendersDecompositionSolver;
                    let report = ospf_rust_bridge::solve_benders_master(
                        &ScipLinearBendersDecompositionSolver::new(), &request.model, &cuts)?;
                    let json = serde_json::to_string(&report)
                        .map_err(|error| BridgeError::new("CodecFailure", error))?;
                    self.start_encoded(request, move |_| Ok(json))
                }
                #[cfg(not(feature = "scip"))]
                {
                    let _ = cuts;
                    Err(BridgeError::new("UnsupportedDialect",
                        "the Benders master step requires a backend feature"))
                }
            }
            ModelKind::RelationalQueryPlan => {
                let spec = request.relational_query_plan.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "relational query plan requires its plan"))?;
                let report = ospf_rust_bridge::validate_query_plan(&spec)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::LinearLifecycle => {
                let lifecycle = request.linear_lifecycle.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "linear lifecycle requires its request"))?;
                let report = ospf_rust_bridge::apply_lifecycle(&lifecycle)?;
                let json = serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::ResponseEnvelope => {
                let envelope = request.response_envelope.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "response envelope requires its envelope"))?;
                // 复核而不是重造：调用方给的形态必须自洽，桥接不替它补 message/data。
                // Recheck rather than rebuild: the caller's shape must be self-consistent, and the bridge never fills in a
                // message or data on its behalf.
                envelope.validate()?;
                let json = serde_json::to_string(&envelope).map_err(|error| BridgeError::new("CodecFailure", error))?;
                self.start_encoded(request, move |_| Ok(json))
            }
            ModelKind::Vrp => {
                let problem = request.vrp_problem.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "vrp requires its problem"))?;
                let instance = problem.to_domain()?;
                let backend = request.solver.clone();
                let supported = cfg!(feature = "scip") && backend == "scip"
                    || cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) && backend == "gurobi";
                if !supported {
                    return Err(BridgeError::new("UnsupportedCapability", &format!("vrp has no backend {backend}")));
                }
                // 分支定价的策略接线（距离/时间/成本/支配/定价选列）在 Rust 侧固定为领域默认组合，
                // 调用方只能调预算，不能替换算法组件。
                //
                // The branch-and-price strategy wiring (distance/travel time/arc cost/dominance/pricing column
                // selection) is fixed to the domain defaults inside Rust; callers can only adjust the budget, not swap
                // algorithm components.
                let search = problem.search.clone().unwrap_or(ospf_rust_bridge::VrpSearchOptions {
                    node_limit: None, relative_gap_tolerance: None, max_cg_iterations_per_node: None });
                let gap = search.relative_gap_tolerance.unwrap_or(problem.tolerances.relative_gap);
                self.start_encoded(request, move |options| {
                    // 预算与策略接线都在泛型 `run` 里构造一次，闭包只负责分派后端。
                    // The budget and strategy wiring are built once inside the generic `run`; the closure only dispatches
                    // the backend.
                    // Strategy wiring and solving happen inside one generic nested function; the backend is only injected
                    // as the solver.
                    fn run<S>(solver: S, instance: std::sync::Arc<ospf_rust_framework_network_scheduling::domain::vrp::VrptwInstance<f64>>,
                        time_limit: Option<std::time::Duration>, search: ospf_rust_bridge::VrpSearchOptions, gap: f64)
                        -> std::result::Result<String, BridgeError>
                    where S: ospf_rust_core::solver::LinearSolver + 'static {
                        use ospf_rust_framework_network_scheduling::application::{
                            BranchAndPriceConfig, BranchNodeSolver, BranchNodeSolverConfig, VrptwApplicationService };
                        use ospf_rust_framework_network_scheduling::domain::route_generation::{
                            DefaultLabelDominancePolicy, DefaultPricingColumnSelector };
                        use ospf_rust_framework_network_scheduling::domain::vrp::{
                            DefaultArcFeasibilityPolicy, DistanceArcCostCalculator, DistanceAsTravelTimeCalculator,
                            EuclideanDistanceCalculator, FixedPlusArcCostPolicy };
                        let config = BranchNodeSolverConfig {
                            distance_calculator: EuclideanDistanceCalculator,
                            travel_time_calculator: DistanceAsTravelTimeCalculator::new(EuclideanDistanceCalculator),
                            arc_cost_calculator: DistanceArcCostCalculator,
                            route_cost_policy: FixedPlusArcCostPolicy,
                            arc_feasibility_policy: std::sync::Arc::new(DefaultArcFeasibilityPolicy),
                            dominance_policy: DefaultLabelDominancePolicy,
                            column_selector: DefaultPricingColumnSelector,
                            extension_factories: Vec::<
                                std::sync::Arc<dyn ospf_rust_framework_network_scheduling::domain::route_compilation::RouteCompilationExtensionFactory<f64>>>::new(),
                            pricing_rows: std::sync::Arc::new(Default::default()),
                        };
                        let mut branch_config = BranchAndPriceConfig::default();
                        branch_config.time_limit = time_limit;
                        branch_config.node_limit = search.node_limit
                            .map(|limit| usize::try_from(limit).unwrap_or(usize::MAX)).unwrap_or(usize::MAX);
                        branch_config.relative_gap_tolerance = gap;
                        if let Some(iterations) = search.max_cg_iterations_per_node {
                            branch_config.max_cg_iterations_per_node = usize::try_from(iterations).unwrap_or(usize::MAX);
                        }
                        branch_config.relative_gap_tolerance = gap;
                        let provider = BranchNodeSolver::new(instance.clone(), std::sync::Arc::new(solver), config)
                            .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                        let service = VrptwApplicationService::new(instance, provider, branch_config)
                            .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                        let report = service.solve().map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                        let solution = report.solution.as_ref().and_then(|solution| solution.value.as_ref())
                            .map(ospf_rust_bridge::decode_vrp_solution).transpose()?;
                        let mapped = ospf_rust_bridge::summarize_vrp_report(&report, solution)?;
                        serde_json::to_string(&mapped).map_err(|error| BridgeError::new("CodecFailure", error))
                    }
                    #[cfg(feature = "scip")]
                    if backend.as_str() == "scip" {
                        use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
                        return run(SCIPSolver::with_config(SCIPConfig::default().with_output(false)), instance.clone(),
                            options.time_limit, search.clone(), gap);
                    }
                    #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
                    if backend.as_str() == "gurobi" {
                        use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
                        return run(GurobiSolver::with_config(GurobiConfig::new().with_output(false)), instance.clone(),
                            options.time_limit, search.clone(), gap);
                    }
                    Err(BridgeError::new("UnsupportedCapability", &format!("vrp has no backend {backend}")))
                })
            }
            ModelKind::NetworkResilience => {
                let problem = request.network_resilience_problem.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "network resilience requires its problem"))?;
                let domain = problem.to_domain()?;
                let backend = request.solver.clone();
                let supported = cfg!(feature = "scip") && backend == "scip"
                    || cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) && backend == "gurobi";
                if !supported {
                    return Err(BridgeError::new("UnsupportedCapability", &format!("network resilience has no backend {backend}")));
                }
                self.start_encoded(request, move |options| {
                    use ospf_rust_framework_network_scheduling::application::NetworkApplicationService;
                    use ospf_rust_framework_network_scheduling::domain::resilience::NetworkResilienceContext;
                    let planning = ospf_rust_core::solver::SolveOptions { time_limit: options.time_limit,
                        cancellation_handle: options.cancellation_handle,
                        progress_reporter: options.progress_reporter, ..Default::default() };
                    let solved: std::result::Result<ospf_rust_core::solver::SolveReport<
                        ospf_rust_framework_network_scheduling::domain::resilience::NetworkResilienceSolution<f64>>,
                        ospf_rust_framework_network_scheduling::NetworkSchedulingError> = match backend.as_str() {
                        #[cfg(feature = "scip")]
                        "scip" => {
                            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
                            NetworkApplicationService::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false)))
                                .solve(|| NetworkResilienceContext::new(domain, "network-resilience"), &planning)
                        }
                        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
                        "gurobi" => {
                            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
                            NetworkApplicationService::new(GurobiSolver::with_config(GurobiConfig::new().with_output(false)))
                                .solve(|| NetworkResilienceContext::new(domain, "network-resilience"), &planning)
                        }
                        other => return Err(BridgeError::new("UnsupportedCapability",
                            &format!("network resilience has no backend {other}"))),
                    };
                    let report = solved.map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                    let solution = report.solution.as_ref().and_then(|solution| solution.value.as_ref())
                        .map(ospf_rust_bridge::decode_network_resilience_solution).transpose()?;
                    let mapped = ospf_rust_bridge::summarize_network_resilience_report(&report, solution)?;
                    serde_json::to_string(&mapped).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::NetworkAllocation => {
                let problem = request.allocation_problem.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "network allocation requires its problem"))?;
                let domain = problem.to_domain()?;
                let backend = request.solver.clone();
                let supported = cfg!(feature = "scip") && backend == "scip"
                    || cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) && backend == "gurobi";
                if !supported {
                    return Err(BridgeError::new("UnsupportedCapability", &format!("network allocation has no backend {backend}")));
                }
                self.start_encoded(request, move |options| {
                    use ospf_rust_framework_network_scheduling::application::NetworkApplicationService;
                    use ospf_rust_framework_network_scheduling::domain::allocation::AllocationContext;
                    let planning = ospf_rust_core::solver::SolveOptions { time_limit: options.time_limit,
                        cancellation_handle: options.cancellation_handle,
                        progress_reporter: options.progress_reporter, ..Default::default() };
                    let solved: std::result::Result<ospf_rust_core::solver::SolveReport<
                        ospf_rust_framework_network_scheduling::domain::allocation::AllocationSolution<f64>>,
                        ospf_rust_framework_network_scheduling::NetworkSchedulingError> = match backend.as_str() {
                        #[cfg(feature = "scip")]
                        "scip" => {
                            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
                            NetworkApplicationService::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false)))
                                .solve(|| AllocationContext::new(domain, "network-allocation"), &planning)
                        }
                        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
                        "gurobi" => {
                            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
                            NetworkApplicationService::new(GurobiSolver::with_config(GurobiConfig::new().with_output(false)))
                                .solve(|| AllocationContext::new(domain, "network-allocation"), &planning)
                        }
                        other => return Err(BridgeError::new("UnsupportedCapability",
                            &format!("network allocation has no backend {other}"))),
                    };
                    let report = solved.map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                    let solution = report.solution.as_ref().and_then(|solution| solution.value.as_ref())
                        .map(ospf_rust_bridge::decode_allocation_solution).transpose()?;
                    let mapped = ospf_rust_bridge::summarize_allocation_report(&report, solution)?;
                    serde_json::to_string(&mapped).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::NetworkStructure => {
                let problem = request.network_structure_problem.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "network structure requires its problem"))?;
                let domain = problem.to_domain()?;
                let backend = request.solver.clone();
                let supported = cfg!(feature = "scip") && backend == "scip"
                    || cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) && backend == "gurobi";
                if !supported {
                    return Err(BridgeError::new("UnsupportedCapability", &format!("network structure has no backend {backend}")));
                }
                // 结构入口不注册商品流，因此不需要调用方提供任何流供需之外的额外证据。
                // The structure entry does not register commodity flows, so callers provide no extra evidence beyond the
                // design problem.
                self.start_encoded(request, move |options| {
                    use ospf_rust_framework_network_scheduling::application::NetworkApplicationService;
                    use ospf_rust_framework_network_scheduling::domain::structure::NetworkStructureContext;
                    let planning = ospf_rust_core::solver::SolveOptions { time_limit: options.time_limit,
                        cancellation_handle: options.cancellation_handle,
                        progress_reporter: options.progress_reporter, ..Default::default() };
                    let solved: std::result::Result<ospf_rust_core::solver::SolveReport<
                        ospf_rust_framework_network_scheduling::domain::structure::NetworkStructureSolution<f64>>,
                        ospf_rust_framework_network_scheduling::NetworkSchedulingError> = match backend.as_str() {
                        #[cfg(feature = "scip")]
                        "scip" => {
                            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
                            NetworkApplicationService::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false)))
                                .solve(|| NetworkStructureContext::new(domain, "network-structure"), &planning)
                        }
                        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
                        "gurobi" => {
                            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
                            NetworkApplicationService::new(GurobiSolver::with_config(GurobiConfig::new().with_output(false)))
                                .solve(|| NetworkStructureContext::new(domain, "network-structure"), &planning)
                        }
                        other => return Err(BridgeError::new("UnsupportedCapability",
                            &format!("network structure has no backend {other}"))),
                    };
                    let report = solved.map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                    let solution = report.solution.as_ref().and_then(|solution| solution.value.as_ref())
                        .map(ospf_rust_bridge::decode_network_structure_solution).transpose()?;
                    let mapped = ospf_rust_bridge::summarize_network_structure_report(&report, solution)?;
                    serde_json::to_string(&mapped).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::NetworkDesign => {
                let problem = request.network_design_problem.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "network design requires its problem"))?;
                let domain = problem.to_domain()?;
                let backend = request.solver.clone();
                let supported = cfg!(feature = "scip") && backend == "scip"
                    || cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) && backend == "gurobi";
                if !supported {
                    return Err(BridgeError::new("UnsupportedCapability", &format!("network design has no backend {backend}")));
                }
                self.start_encoded(request, move |options| {
                    use ospf_rust_framework_network_scheduling::application::NetworkApplicationService;
                    use ospf_rust_framework_network_scheduling::domain::design::NetworkDesignContext;
                    let planning = ospf_rust_core::solver::SolveOptions { time_limit: options.time_limit,
                        cancellation_handle: options.cancellation_handle,
                        progress_reporter: options.progress_reporter, ..Default::default() };
                    let solved: std::result::Result<ospf_rust_core::solver::SolveReport<
                        ospf_rust_framework_network_scheduling::domain::design::NetworkDesignSolution<f64>>,
                        ospf_rust_framework_network_scheduling::NetworkSchedulingError> = match backend.as_str() {
                        #[cfg(feature = "scip")]
                        "scip" => {
                            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
                            NetworkApplicationService::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false)))
                                .solve(|| NetworkDesignContext::new(domain, "network-design"), &planning)
                        }
                        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
                        "gurobi" => {
                            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
                            NetworkApplicationService::new(GurobiSolver::with_config(GurobiConfig::new().with_output(false)))
                                .solve(|| NetworkDesignContext::new(domain, "network-design"), &planning)
                        }
                        other => return Err(BridgeError::new("UnsupportedCapability",
                            &format!("network design has no backend {other}"))),
                    };
                    let report = solved.map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                    let solution = report.solution.as_ref().and_then(|solution| solution.value.as_ref())
                        .map(ospf_rust_bridge::decode_network_design_solution).transpose()?;
                    let mapped = ospf_rust_bridge::summarize_network_design_report(&report, solution)?;
                    serde_json::to_string(&mapped).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::NetworkGeneralizedFlow => {
                let problem = request.network_generalized_flow_problem.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "generalized flow requires its problem"))?;
                let domain = problem.to_domain()?;
                let backend = request.solver.clone();
                // 后端名在提交时校验，未知后端快速失败而不是变成一个失败的作业。
                // The backend name is checked at submit time, so an unknown backend fails fast instead of producing a
                // failed job.
                let supported = cfg!(feature = "scip") && backend == "scip"
                    || cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) && backend == "gurobi";
                if !supported {
                    return Err(BridgeError::new("UnsupportedCapability", &format!("generalized flow has no backend {backend}")));
                }
                self.start_encoded(request, move |options| {
                    use ospf_rust_framework_network_scheduling::application::NetworkApplicationService;
                    use ospf_rust_framework_network_scheduling::domain::generalized_flow::GeneralizedFlowContext;
                    let planning = ospf_rust_core::solver::SolveOptions { time_limit: options.time_limit,
                        cancellation_handle: options.cancellation_handle,
                        progress_reporter: options.progress_reporter, ..Default::default() };
                    let solved: std::result::Result<ospf_rust_core::solver::SolveReport<
                        ospf_rust_framework_network_scheduling::domain::generalized_flow::GeneralizedFlowSolution<f64>>,
                        ospf_rust_framework_network_scheduling::NetworkSchedulingError> = match backend.as_str() {
                        #[cfg(feature = "scip")]
                        "scip" => {
                            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
                            NetworkApplicationService::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false)))
                                .solve(|| GeneralizedFlowContext::new(domain, "generalized-flow"), &planning)
                        }
                        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
                        "gurobi" => {
                            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
                            NetworkApplicationService::new(GurobiSolver::with_config(GurobiConfig::new().with_output(false)))
                                .solve(|| GeneralizedFlowContext::new(domain, "generalized-flow"), &planning)
                        }
                        other => return Err(BridgeError::new("UnsupportedCapability",
                            &format!("generalized flow has no backend {other}"))),
                    };
                    let report = solved.map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                    let solution = report.solution.as_ref().and_then(|solution| solution.value.as_ref())
                        .map(ospf_rust_bridge::decode_generalized_flow_solution).transpose()?;
                    let mapped = ospf_rust_bridge::summarize_generalized_flow_report(&report, solution)?;
                    serde_json::to_string(&mapped).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::NetworkFlow => {
                let problem = request.network_flow_problem.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "network flow requires its problem"))?;
                // 身份引用与数值先在桥接层校验，坏输入不会进入领域层。
                // Identity references and values are validated in the bridge first, so bad input never reaches the
                // domain layer.
                let domain = problem.to_domain()?;
                let backend = request.solver.clone();
                // 后端名在提交时校验，未知后端快速失败而不是变成一个失败的作业。
                // The backend name is checked at submit time, so an unknown backend fails fast instead of producing a
                // failed job.
                let supported = cfg!(feature = "scip") && backend == "scip"
                    || cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) && backend == "gurobi";
                if !supported {
                    return Err(BridgeError::new("UnsupportedCapability", &format!("network flow has no backend {backend}")));
                }
                self.start_encoded(request, move |options| {
                    use ospf_rust_framework_network_scheduling::application::NetworkApplicationService;
                    use ospf_rust_framework_network_scheduling::domain::flow::FlowContext;
                    let planning = ospf_rust_core::solver::SolveOptions { time_limit: options.time_limit,
                        cancellation_handle: options.cancellation_handle,
                        progress_reporter: options.progress_reporter, ..Default::default() };
                    // 每个后端构造原框架求解器，不做隐式回退；领域服务复用共享预算/取消/进度。
                    // Construct the original framework solver per backend without implicit fallback; the domain
                    // service reuses the shared budget, cancellation and progress.
                    // 显式标注结果类型：没有任何后端特性时匹配只剩 `return` 分支（类型 `!`），标注后仍能统一。
                    // The result type is annotated explicitly: with no backend feature the match only has a `return`
                    // arm of type `!`, and the annotation still unifies it.
                    let solved: std::result::Result<ospf_rust_core::solver::SolveReport<
                        ospf_rust_framework_network_scheduling::domain::flow::FlowSolution<f64>>,
                        ospf_rust_framework_network_scheduling::NetworkSchedulingError> = match backend.as_str() {
                        #[cfg(feature = "scip")]
                        "scip" => {
                            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
                            NetworkApplicationService::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false)))
                                .solve(|| Ok(FlowContext::new(domain)), &planning)
                        }
                        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
                        "gurobi" => {
                            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
                            NetworkApplicationService::new(GurobiSolver::with_config(GurobiConfig::new().with_output(false)))
                                .solve(|| Ok(FlowContext::new(domain)), &planning)
                        }
                        other => return Err(BridgeError::new("UnsupportedCapability",
                            &format!("network flow has no backend {other}"))),
                    };
                    let report = solved.map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                    let solution = report.solution.as_ref().and_then(|solution| solution.value.as_ref())
                        .map(ospf_rust_bridge::decode_network_flow_solution).transpose()?;
                    let mapped = ospf_rust_bridge::summarize_flow_report(&report, solution)?;
                    serde_json::to_string(&mapped).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::LinearSweep => {
                let cases = request.sweep_cases.clone().ok_or_else(|| BridgeError::new("InvalidInput", "linear sweep requires its cases"))?;
                ospf_rust_bridge::LinearSweepCase::validate(&cases)?;
                let model = std::sync::Arc::new(decode_model(&request.model)?);
                let fingerprint = ospf_rust_core::solver::linear_model_fingerprint(&model)
                    .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                self.start_encoded(request, move |options| {
                    // 取消句柄跨线程克隆，case 级求解也能被同一个作业取消。
                    // The cancellation handle is cloned across threads so case-level solves share the job's cancellation.
                    let cancellation = options.cancellation_handle.cloned();
                    let fallback_limit = options.time_limit;
                    let prepared = cases.into_iter().map(|case| {
                        let case_id = case.case_id.clone();
                        ospf_rust_framework::ExperimentCase::new(case_id, "ospf-typescript-sweep", "1", case)
                    }).collect::<Vec<_>>();
                    // 并发预算取 case 数与 4 的较小值：再多也不会更快，只会抢 LP 后端。
                    // The concurrency budget is the smaller of the case count and 4: more workers would not be faster,
                    // they would only contend for the LP backend.
                    let runner = ospf_rust_framework::ExperimentRunner::new(prepared.len().min(4));
                    let results = runner.run(prepared, move |case| {
                        let solver = create_solver(&case.solver, None).map_err(|error| error.to_string())?;
                        let solve_options = ospf_rust_core::solver::SolveOptions {
                            time_limit: case.time_limit_ms.map(std::time::Duration::from_millis).or(fallback_limit),
                            cancellation_handle: cancellation.as_ref(), ..Default::default() };
                        let mut report = solver.solve_linear_report_with_options(&model, &solve_options)
                            .map_err(|error| error.to_string())?;
                        // 取消身份与原线性入口一致，case 报告不能被当成另一个模型的结论。
                        // Cancellation identity matches the plain linear entry so a case report cannot be taken as a
                        // conclusion about another model.
                        linear_identity::bind_cancelled_identity(&mut report, &model).map_err(|error| error.to_string())?;
                        serde_json::to_string(&report).map_err(|error| error.to_string())
                    });
                    // 运行器已按 case ID 稳定排序；这里只做摘要映射，不改变顺序、不吞掉任何 case。
                    // The runner already sorts stably by case ID; this only maps summaries without reordering or
                    // dropping any case.
                    let mapped = results.into_iter().map(|result| {
                        let case_id = result.metadata.case_id;
                        match result.outcome {
                            Ok(json) => Ok(ospf_rust_bridge::LinearSweepCaseResult { case_id,
                                summary: Some(ospf_rust_bridge::summarize_report(&json)?), error_code: None, error_message: None }),
                            Err(error) => Ok(ospf_rust_bridge::LinearSweepCaseResult { case_id, summary: None,
                                error_code: Some("ExecutionFailure".into()), error_message: Some(error.message) }),
                        }
                    }).collect::<Result<Vec<_>>>()?;
                    let report = ospf_rust_bridge::LinearSweepReport { model_fingerprint: fingerprint.value.clone(), cases: mapped };
                    report.validate()?;
                    serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::LinearSensitivity => {
                let incumbent = request.incumbent_values.clone().ok_or_else(|| BridgeError::new("InvalidInput", "linear sensitivity requires an incumbent vector"))?;
                let solver = create_solver(&request.solver, request.tuning_options.as_ref())?;
                let model = decode_model(&request.model)?;
                self.start_encoded(request, move |_options| {
                    // 原 core 会在分析前校验 incumbent 的完整性与可行性；这里不预先过滤，让它给出原始诊断。
                    // The original core validates incumbent completeness and feasibility before analyzing; nothing is
                    // pre-filtered here so its own diagnostics stay intact.
                    let analyzer = ospf_rust_core::analysis::FixedIntegerLpSensitivityAnalyzer::new(
                        ospf_rust_core::analysis::FixedIntegerLpSensitivityConfig::default())
                        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
                    let report = analyzer.analyze(&*solver, &model, &incumbent)
                        .map_err(|error| BridgeError::new(error.solver_error_class().code(), &error))?;
                    report.validate().map_err(|error| BridgeError::new("InvalidReport", &error))?;
                    serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::LinearSolutionPool => {
                let pool = request.solution_pool_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "linear solution pool requires its options"))?;
                pool.validate()?;
                let solver = create_solver(&request.solver, request.tuning_options.as_ref())?;
                let model = decode_model(&request.model)?;
                let artifact = request.model.clone();
                let amount = pool.amount;
                self.start_encoded(request, move |options| {
                    // 只有后端原生实现了多解才返回；`None` 表示该后端不支持，必须显式拒绝而不是回退成单解。
                    // Return only when the backend implements multi-solution natively; `None` means unsupported and must
                    // be rejected explicitly instead of falling back to a single solution.
                    let outcome = solver.solve_linear_with_solution_pool_with_options(&model, usize::try_from(amount)
                        .map_err(|_| BridgeError::new("InvalidInput", "solution pool amount overflow"))?, options)
                        .map_err(|error| BridgeError::new(error.solver_error_class().code(), &error))?;
                    let (output, solutions) = outcome.ok_or_else(|| BridgeError::new("UnsupportedCapability",
                        "backend does not implement a native solution pool"))?;
                    let mapping = ospf_rust_core::solver::linear_model_mapping(&model)
                        .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
                    let report = ospf_rust_bridge::LinearSolutionPoolReport {
                        model_fingerprint: artifact.fingerprint,
                        variable_ids: mapping.variables_by_column.iter().map(|id| id.0.clone()).collect(),
                        solutions: solutions.into_iter().map(|vector| {
                            if vector.len() != model.num_variables() {
                                return Err(BridgeError::new("InvalidReport", "pool solution vector has the wrong length"));
                            }
                            if vector.iter().any(|value| !value.is_finite()) {
                                return Err(BridgeError::new("InvalidReport", "pool solution contains a non-finite value"));
                            }
                            Ok(vector)
                        }).collect::<Result<Vec<_>>>()?,
                        best_objective: output.objective_value.filter(|value| value.is_finite()),
                    };
                    serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::LinearIis => {
                // IIS 的两种过滤算法在原 core 里只对 Gurobi 后端实现；其它后端必须显式拒绝而不是假装支持。
                // Both IIS filters are implemented for the Gurobi backend only in the original core; other backends must
                // be rejected explicitly instead of pretending support.
                if request.solver != "gurobi" {
                    return Err(BridgeError::new("UnsupportedCapability", "linear IIS requires the gurobi backend"));
                }
                if !cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")) {
                    return Err(BridgeError::new("UnsupportedCapability", "linear IIS requires a compiled gurobi feature"));
                }
                let artifact = request.model.clone();
                let iis = request.iis_options.clone().unwrap_or(ospf_rust_bridge::LinearIisOptions { algorithm: None, include_bounds: None, max_iterations: None });
                // 时间预算沿共享 SolveOptions 传入；取消无法中断 core 的同步过滤过程，这里如实不宣称可中断。
                // The time budget flows through the shared `SolveOptions`; cancellation cannot interrupt core's synchronous
                // filters, so interruptibility is not claimed here.
                self.start_encoded(request, move |options| {
                    let report = ospf_rust_bridge::compute_linear_iis(&artifact, &iis, options.time_limit)?;
                    serde_json::to_string(&report).map_err(|error| BridgeError::new("CodecFailure", error))
                })
            }
            ModelKind::QuadraticOrchestration => {
                let configuration = request.quadratic_orchestration.clone()
                    .ok_or_else(|| BridgeError::new("InvalidInput", "quadratic orchestration requires its configuration"))?;
                configuration.validate()?;
                let model = decode_quadratic_model(&request.model)?;
                // 每个后端都是已绑定的二次求解器；编排只决定"串行还是并行、按什么策略选结果"。
                // Every backend is an already-bound quadratic solver; orchestration only decides "serial or parallel,
                // and which result-selection policy".
                let backends = configuration.backends.iter()
                    .map(|backend| Ok(ospf_rust_bridge::core_backend(create_solver(backend.id(), None)?)))
                    .collect::<Result<Vec<_>>>()?;
                self.start_encoded(request, move |options| {
                    let report = ospf_rust_bridge::solve_quadratic_orchestration(&model, backends, &configuration,
                        options.time_limit, options.cancellation_handle.cloned())?;
                    Ok(report.aggregate)
                })
            }
            ModelKind::Quadratic => {
                let solver = create_solver(&request.solver, request.tuning_options.as_ref())?;
                if !self.capabilities().backends.iter().any(|backend| backend.id == request.solver && backend.capabilities.iter().any(|kind| kind == "quadratic")) {
                    return Err(BridgeError::new("UnsupportedCapability", "quadratic support is not enabled for this backend"));
                }
                let model = decode_quadratic_model(&request.model)?;
                self.start_task(request, move |options| solver.solve_quadratic_report_with_options(&model, options))
            }
            ModelKind::Cp => {
                let solver = create_cp_solver(&request.solver)?;
                let snapshot = decode_cp_snapshot(&request.model)?;
                self.start_task(request, move |options| {
                    let mut report = solver.solve_constraint_programming_with_conflict(&snapshot, &ConstraintProgrammingSolveOptions {
                        time_limit: options.time_limit, cancellation_handle: options.cancellation_handle,
                        progress_reporter: options.progress_reporter,
                        node_limit: cp_options.node_limit.map(|value| value as usize),
                        solution_limit: cp_options.solution_limit.map(|value| value as usize),
                        solution_hint: hint.as_ref(), request_conflict: cp_options.request_conflict.unwrap_or(false),
                        ..Default::default()
                    })?;
                    // 预取消报告可能没有模型指纹；在已认证的执行边界补齐源身份。
                    // Pre-cancelled reports may lack a model fingerprint; bind the authenticated source identity here.
                    if report.fingerprints.model.is_none() && report.solution.is_none()
                        && matches!(report.termination_reason, ospf_rust_core::solver::TerminationReason::Cancelled | ospf_rust_core::solver::TerminationReason::Interrupted)
                    {
                        report.fingerprints.model = Some(snapshot.fingerprint.clone());
                    }
                    let json = serde_json::to_string(&report).map_err(|err| ospf_rust_core::SolverError::ContractViolation(err.to_string()))?;
                    decode_cp_report(&json, &snapshot).map_err(|err| ospf_rust_core::SolverError::ContractViolation(err.to_string()))?;
                    Ok(report)
                })
            }
        }
    }

    fn start(
        &self,
        request: SolveRequest,
        solver: Arc<dyn LinearSolver>,
        model: ospf_rust_core::model::intermediate::LinearTriadModel,
    ) -> Result<JobView> {
        self.start_task(request, move |options| {
            let mut report = solver.solve_linear_report_with_options(&model, options)?;
            linear_identity::bind_cancelled_identity(&mut report, &model)?;
            Ok(report)
        })
    }

    fn start_task<V, F>(&self, request: SolveRequest, solve: F) -> Result<JobView>
    where
        V: serde::Serialize,
        F: FnOnce(&SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<V>> + Send + 'static,
    {
        self.start_encoded(request, move |options| {
            let report = solve(options).map_err(|err| BridgeError::new(err.solver_error_class().code(), &err))?;
            report.validate().map_err(|err| BridgeError::new("InvalidReport", err))?;
            serde_json::to_string(&report).map_err(|err| BridgeError::new("CodecFailure", err))
        })
    }

    fn start_encoded<F>(&self, request: SolveRequest, encode: F) -> Result<JobView>
    where
        F: FnOnce(&SolveOptions<'_>) -> Result<String> + Send + 'static,
    {
        let mut state = lock(&self.state);
        if state.active { return Err(BridgeError::new("Busy", "one solve is already running")); }
        if state.jobs.len() >= self.capacity {
            return Err(BridgeError::new("CapacityExceeded", "release completed jobs before submitting"));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed).to_string();
        let view = JobView {
            id: id.clone(), protocol_version: PROTOCOL_VERSION, status: JobStatus::Running,
            fingerprint: request.model.fingerprint, report_json: None, progress_json: None, error: None,
        };
        let job = Arc::new(Job { view: Mutex::new(view.clone()), cancellation: SolveHandle::new() });
        state.jobs.insert(id.clone(), job.clone());
        state.active = true;
        let state_ref = self.state.clone();
        let spawn = std::thread::Builder::new().name(format!("ospf-{id}")).spawn(move || {
            let progress_job = job.clone();
            let reporter: SolveProgressReporter = Arc::new(move |progress| {
                if let Ok(json) = serde_json::to_string(progress) { lock(&progress_job.view).progress_json = Some(json); }
                Ok(())
            });
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let options = SolveOptions {
                    time_limit: request.time_limit_ms.map(|ms| Duration::from_millis(ms.into())),
                    cancellation_handle: Some(&job.cancellation), progress_reporter: Some(&reporter),
                    ..SolveOptions::default()
                };
                encode(&options)
            })).unwrap_or_else(|_| Err(BridgeError::new("SolverPanic", "native backend panicked")));
            job.cancellation.mark_completed();
            let mut state = lock(&state_ref);
            let mut view = lock(&job.view);
            match outcome {
                Ok(json) => { view.report_json = Some(json); view.status = JobStatus::Completed; }
                Err(error) => { view.error = Some(error); view.status = JobStatus::Failed; }
            }
            state.active = false;
        });
        if let Err(error) = spawn {
            state.jobs.remove(&id);
            state.active = false;
            return Err(BridgeError::new("ExecutionFailure", error));
        }
        Ok(view)
    }

    fn job(&self, id: &str) -> Result<Arc<Job>> {
        lock(&self.state).jobs.get(id).cloned().ok_or_else(|| BridgeError::new("NotFound", "job not found"))
    }

    /// 获取当前快照 / Get the current snapshot.
    pub fn get(&self, id: &str) -> Result<JobView> { Ok(lock(&self.job(id)?.view).clone()) }

    /// 请求后端取消，不伪造终态 / Request backend cancellation without inventing a terminal state.
    pub fn cancel(&self, id: &str) -> Result<JobView> {
        let job = self.job(id)?;
        job.cancellation.cancel(CancellationOrigin::External);
        Ok(lock(&job.view).clone())
    }

    /// 删除终态任务以释放有限存储 / Release a terminal job from bounded storage.
    pub fn release(&self, id: &str) -> Result<()> {
        let mut state = lock(&self.state);
        let job = state.jobs.get(id).ok_or_else(|| BridgeError::new("NotFound", "job not found"))?;
        if lock(&job.view).status == JobStatus::Running {
            return Err(BridgeError::new("Busy", "cannot release a running job"));
        }
        state.jobs.remove(id);
        Ok(())
    }
}

fn create_cp_solver(id: &str) -> Result<Arc<dyn ConstraintProgrammingSolver>> {
    match id {
        #[cfg(feature = "scip")]
        "scip" => {
            use ospf_rust_core_plugin_scip::ScipConstraintProgrammingSolver;
            use ospf_rust_core_plugin_scip::SCIPConfig;
            Ok(Arc::new(ScipConstraintProgrammingSolver::with_config(SCIPConfig::default().with_output(false))))
        }
        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
        "gurobi" => {
            use ospf_rust_core::solver::constraint_programming::MipBackedConstraintProgrammingSolver;
            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
            Ok(Arc::new(MipBackedConstraintProgrammingSolver::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false)))))
        }
        _ => { create_solver(id, None)?; Err(BridgeError::new("UnsupportedCapability", "CP exact lowering is unavailable")) }
    }
}

fn create_solver(id: &str, tuning: Option<&SolverTuningOptions>) -> Result<Arc<dyn Solver>> {
    match id {
        #[cfg(feature = "scip")]
        "scip" => {
            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
            let mut config = SCIPConfig::default().with_output(false);
            if let Some(tuning) = tuning {
                if let Some(threads) = tuning.threads { config = config.with_threads(threads as i32); }
                if let Some(seed) = tuning.seed { config = config.with_seed(seed); }
                if let Some(gap) = tuning.mip_gap { config = config.with_mip_gap(gap); }
            }
            Ok(Arc::new(SCIPSolver::with_config(config)))
        }
        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
        "gurobi" => {
            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
            let mut config = GurobiConfig::default().with_output(false);
            if let Some(tuning) = tuning {
                if let Some(threads) = tuning.threads { config = config.with_threads(threads as i32); }
                if let Some(seed) = tuning.seed { config = config.with_seed(seed); }
                if let Some(gap) = tuning.mip_gap { config = config.with_mip_gap(gap); }
            }
            Ok(Arc::new(GurobiSolver::with_config(config)))
        }
        #[cfg(not(feature = "scip"))]
        "scip" => Err(BridgeError::new("BackendNotCompiled", "scip feature is not enabled")),
        #[cfg(not(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")))]
        "gurobi" => Err(BridgeError::new("BackendNotCompiled", "gurobi feature is not enabled")),
        _ => Err(BridgeError::new("UnsupportedBackend", "unknown solver backend")),
    }
}

#[cfg(test)]
mod helper_entries_tests;

#[cfg(test)]
mod quadratic_orchestration_tests;

#[cfg(test)]
mod network_vrp_tests;

#[cfg(test)]
mod network_resilience_tests;

#[cfg(test)]
mod network_allocation_tests;

#[cfg(test)]
mod network_structure_tests;

#[cfg(test)]
mod network_design_tests;

#[cfg(test)]
mod generalized_flow_tests;

#[cfg(test)]
mod network_flow_tests;

#[cfg(test)]
mod sweep_tests;

#[cfg(test)]
mod sensitivity_tests;

#[cfg(test)]
mod solution_pool_tests;

#[cfg(test)]
mod iis_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::solver::{SolverInfo, SolverCapability, SolverOutput};

    struct Fake;

    #[test]
    fn column_generation_requires_exclusive_typed_linear_configuration() {
        let engine = NativeEngine::default();
        let mut request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "column-generation".into(), model_kind: None,
            tuning_options: None,
            model: ospf_rust_bridge::encode_model(&ospf_rust_core::model::intermediate::LinearTriadModel::new("source")).unwrap(),
            time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, iis_options: None, fixed_recourse_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None };
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "InvalidInput");
        request.column_generation_options = Some(serde_json::from_str(r#"{"backend":"scip","blocks":[{"id":"a","variableIndices":[0]}]}"#).unwrap());
        request.solver = "scip".into();
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "InvalidInput");
        request.solver = "column-generation".into(); request.model_kind = Some(ModelKind::Cp);
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "InvalidInput");
        request.model_kind = None; request.column_generation_options.as_mut().unwrap().iteration_limit = Some(0);
        assert_eq!(engine.submit(request).unwrap_err().code, "InvalidInput");
        assert!(lock(&engine.state).jobs.is_empty());
    }
    impl SolverInfo for Fake {
        fn name(&self) -> &str { "contract-test-only" }
        fn capabilities(&self) -> Vec<SolverCapability> { vec![] }
    }
    impl LinearSolver for Fake {
        fn solve_linear(&self, _: &ospf_rust_core::model::intermediate::LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> {
            Err(ospf_rust_core::SolverError::NotAvailable("contract test".into()).into())
        }
    }

    #[test]
    fn quadratic_capabilities_follow_the_compiled_backend_features() {
        let capabilities = NativeEngine::default().capabilities();
        let scip = &capabilities.backends[0];
        let gurobi = &capabilities.backends[1];
        assert_eq!(scip.capabilities.iter().any(|kind| kind == "quadratic"), cfg!(feature = "scip-quadratic"));
        assert_eq!(gurobi.capabilities.iter().any(|kind| kind == "quadratic"), cfg!(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")));
        assert_eq!(scip.capabilities.iter().any(|kind| kind == "cpExactLowering"), cfg!(feature = "scip"));
        assert_eq!(gurobi.capabilities.iter().any(|kind| kind == "cpExactLowering"), gurobi.compiled);
        assert_eq!(scip.capabilities.iter().any(|kind| kind == "cpSolveOptions"), scip.compiled);
        assert_eq!(gurobi.capabilities.iter().any(|kind| kind == "cpSolveOptions"), gurobi.compiled);
        assert_eq!(scip.capabilities.iter().any(|kind| kind == "cpObjectiveConstant"), scip.compiled);
        assert_eq!(gurobi.capabilities.iter().any(|kind| kind == "cpObjectiveConstant"), gurobi.compiled);
        assert_eq!(scip.capabilities.iter().any(|kind| kind == "intermediateProgress"), scip.compiled);
        assert_eq!(gurobi.capabilities.iter().any(|kind| kind == "intermediateProgress"), gurobi.compiled);
        let columns = capabilities.backends.iter().find(|backend| backend.id == "column-generation").unwrap();
        assert_eq!(columns.compiled, scip.compiled || gurobi.compiled);
        assert_eq!(columns.capabilities.iter().any(|capability| capability == "columnGeneration"), columns.compiled);
        let serial = &capabilities.backends[2];
        assert_eq!(serial.id, "serial-linear");
        assert_eq!(serial.compiled, scip.compiled || gurobi.compiled);
        assert_eq!(serial.capabilities.iter().any(|kind| kind == "serialLinear"), serial.compiled);
        let parallel = &capabilities.backends[3];
        assert_eq!(parallel.id, "parallel-linear");
        assert_eq!(parallel.compiled, serial.compiled);
        assert_eq!(parallel.capabilities.iter().any(|kind| kind == "parallelLinear"), parallel.compiled);
        let recourse = &capabilities.backends[4];
        assert_eq!(recourse.id, "fixed-recourse");
        assert_eq!(recourse.compiled, serial.compiled);
        assert_eq!(recourse.capabilities.iter().any(|kind| kind == "fixedRecourse"), recourse.compiled);
    }

    #[test]
    #[cfg(feature = "scip")]
    fn scip_cp_jobs_preserve_exact_report_and_reject_inexact_lowering() {
        use ospf_rust_core::model::constraint_programming::{ConstraintProgrammingModel, IntegerDomain, IntegerExpression, IntegerObjective, IntegerTerm, IntegerVariable};
        let mut model = ConstraintProgrammingModel::new("cp-native");
        let x = IntegerVariable::new("x");
        model.add_variable(x.clone(), IntegerDomain::range(0, 3).unwrap()).unwrap();
        model.set_objective(IntegerObjective::maximize_with_id("cost", IntegerExpression::linear(7, [IntegerTerm { variable: x, coefficient: 1 }]).unwrap()));
        let snapshot = model.freeze().unwrap();
        let engine = NativeEngine::default();
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "scip".into(), model: ospf_rust_bridge::encode_cp_snapshot(&snapshot).unwrap(), model_kind: Some(ModelKind::Cp), time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None };
        let job = engine.submit(request.clone()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let wait = |id: &str| loop {
            let view = engine.get(id).unwrap();
            if view.status != JobStatus::Running { break view; }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        };
        let view = wait(&job.id);
        assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
        let report = decode_cp_report(view.report_json.as_ref().unwrap(), &snapshot).unwrap();
        assert_eq!(report.solution.unwrap().objective, Some(10));
        assert_eq!(report.statistics.best_bound, Some(10));
        let progress: ospf_rust_core::solver::SolveProgressSnapshot = serde_json::from_str(view.progress_json.as_ref().unwrap()).unwrap();
        assert!(progress.terminal);
        assert_eq!(progress.objective_value, Some(10.0));
        assert_eq!(progress.best_bound, Some(10.0));
        engine.release(&job.id).unwrap();
        let mut wrong = request.clone();
        wrong.model.fingerprint = "wrong".into();
        assert_eq!(engine.submit(wrong).unwrap_err().code, "FingerprintMismatch");
        let mut inexact = ConstraintProgrammingModel::new("inexact");
        inexact.add_variable(IntegerVariable::new("x"), IntegerDomain::range(9_007_199_254_740_993, 9_007_199_254_740_995).unwrap()).unwrap();
        let inexact_request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, model: ospf_rust_bridge::encode_cp_snapshot(&inexact.freeze().unwrap()).unwrap(), ..request.clone() };
        let job = engine.submit(inexact_request).unwrap();
        assert_eq!(wait(&job.id).status, JobStatus::Failed);
        engine.release(&job.id).unwrap();

        let mut inexact = ConstraintProgrammingModel::new("inexact-zero-constant");
        let x = IntegerVariable::new("x");
        let y = IntegerVariable::new("y");
        let one = IntegerDomain::range(1, 1).unwrap();
        inexact.add_variable(x.clone(), one.clone()).unwrap();
        inexact.add_variable(y.clone(), one).unwrap();
        inexact.set_objective(IntegerObjective::maximize(IntegerExpression::linear(0, [
            IntegerTerm { variable: x, coefficient: 9_007_199_254_740_992 },
            IntegerTerm { variable: y, coefficient: 1 },
        ]).unwrap()));
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, model: ospf_rust_bridge::encode_cp_snapshot(&inexact.freeze().unwrap()).unwrap(), ..request };
        let job = engine.submit(request).unwrap();
        let view = wait(&job.id);
        assert_eq!(view.status, JobStatus::Failed);
        assert_eq!(view.error.unwrap().code, "NUMERICAL");
        engine.release(&job.id).unwrap();
    }

    #[test]
    fn solver_tuning_is_bounded_and_exclusive_to_direct_backend_entries() {
        use ospf_rust_core::intermediate::LinearTriadModel;
        // 同时提交多个探测请求，因此需要并发容量；不代表生产并发承诺。 / Several probes are submitted concurrently, so the engine needs capacity; this is not a production concurrency claim.
        let engine = NativeEngine::new(8);
        let base = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "scip".into(),
            model: ospf_rust_bridge::encode_model(&LinearTriadModel::new("tuning")).unwrap(), model_kind: None, time_limit_ms: None, cp_options: None,
            serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None,
            csp1d_options: None, column_generation_options: None, fixed_recourse_options: None };
        for tuning in [SolverTuningOptions { threads: Some(0), seed: None, mip_gap: None }, SolverTuningOptions { threads: Some(65), seed: None, mip_gap: None },
            SolverTuningOptions { threads: None, seed: None, mip_gap: None }, SolverTuningOptions { threads: None, seed: None, mip_gap: Some(-0.1) },
            SolverTuningOptions { threads: None, seed: None, mip_gap: Some(1.0) }, SolverTuningOptions { threads: None, seed: None, mip_gap: Some(f64::NAN) },
            SolverTuningOptions { threads: None, seed: None, mip_gap: Some(f64::INFINITY) }] {
            let error = engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: Some(tuning), ..base.clone() }).unwrap_err();
            assert_eq!(error.code, "InvalidInput", "{error:?}");
        }
        // 领域与框架策略各自持有配置，不接受全局调优。 / Domain and framework strategies own their configuration and reject global tuning.
        let mut foreign = base.clone();
        foreign.column_generation_options = Some(serde_json::from_str(r#"{"backend":"scip","blocks":[{"id":"a","variableIndices":[0]}]}"#).unwrap());
        foreign.solver = "column-generation".into();
        foreign.tuning_options = Some(SolverTuningOptions { threads: Some(2), seed: Some(7), mip_gap: None });
        assert_eq!(engine.submit(foreign).unwrap_err().code, "InvalidInput");
        // 直接后端入口接受线程数与种子。 / Direct backend entries accept a thread count and a seed.
        let accepted = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: Some(SolverTuningOptions { threads: Some(1), seed: Some(7), mip_gap: None }), ..base.clone() };
        assert!(engine.submit(accepted).is_ok());
        // 上界是两后端的交集：64 合法，65 必须在入口被拒，而不是让后端抛 ParameterWrongVal。
        // The bound is the intersection of both backends: 64 is valid and 65 must be rejected at the boundary instead of failing inside the backend.
        assert!(SolverTuningOptions { threads: Some(64), seed: None, mip_gap: None }.validate().is_ok());
        assert!(SolverTuningOptions { threads: Some(65), seed: None, mip_gap: None }.validate().is_err());
        // 零间隙（精确最优）本身合法；此处只复验取值而不占用引擎并发。 / A zero gap (exact optimality) is valid; validate the value without consuming engine concurrency.
        assert!(SolverTuningOptions { threads: None, seed: None, mip_gap: Some(0.0) }.validate().is_ok());
    }

    #[test]
    #[cfg(feature = "scip")]
    fn scip_honours_explicit_threads_and_seed_without_changing_the_optimum() {
        use ospf_rust_core::intermediate::{BasicLinearTriadModel, LinearTriadModel, SparseVector};
        use ospf_rust_core::model::ObjectiveCategory;
        use ospf_rust_core::token::Token;
        use ospf_rust_core::variable::{BinaryVariableItem, VariableId, VariableType};
        let mut basic = BasicLinearTriadModel::new("tuning-real");
        let mut limit = SparseVector::new();
        for index in 0..12 {
            let variable = BinaryVariableItem::create(VariableId::standalone(90_000 + index), &format!("x{index}"));
            basic.add_variable_with_bounds(Token::from_generic(variable, index), 0.0, 1.0, VariableType::Binary);
            limit.add(index, 1.0);
        }
        basic.add_constraint(limit, 5.5);
        let mut model = LinearTriadModel::from_basic(basic);
        model.set_objective((0..12).map(|index| (index + 1) as f64).collect(), ObjectiveCategory::Maximum);
        let artifact = ospf_rust_bridge::encode_model(&model).unwrap();
        let base = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "scip".into(),
            model: artifact, model_kind: None, time_limit_ms: Some(30_000), cp_options: None, serial_options: None, parallel_options: None, iis_options: None,
            bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None,
            column_generation_options: None, fixed_recourse_options: None };
        let engine = NativeEngine::new(1);
        let objective = |tuning: Option<SolverTuningOptions>| {
            let job = engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: tuning, ..base.clone() }).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            loop {
                let view = engine.get(&job.id).unwrap();
                if view.status != JobStatus::Running {
                    assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
                    let report: ospf_rust_core::solver::SolveReport<f64> = serde_json::from_str(view.report_json.as_ref().unwrap()).unwrap();
                    engine.release(&job.id).unwrap();
                    return report.solution.as_ref().and_then(|solution| solution.objective_value).unwrap();
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        let plain = objective(None);
        let tuned = objective(Some(SolverTuningOptions { threads: Some(1), seed: Some(7), mip_gap: None }));
        assert_eq!(plain, tuned);
    }

    #[test]
    #[cfg(feature = "scip")]
    fn scip_gap_termination_never_claims_exact_optimality() {
        use ospf_rust_core::intermediate::{BasicLinearTriadModel, LinearTriadModel, SparseVector};
        use ospf_rust_core::model::ObjectiveCategory;
        use ospf_rust_core::solver::report::{ProofKind, ProofStatus};
        use ospf_rust_core::solver::{ProblemStatus, TerminationReason};
        use ospf_rust_core::token::Token;
        use ospf_rust_core::variable::{BinaryVariableItem, VariableId, VariableType};
        // 构造间隙难以立即闭合的背包：非零 mipGap 可能以 GapLimit 结束。
        // Build a knapsack whose gap rarely closes immediately: a nonzero mipGap may end in GapLimit.
        let mut basic = BasicLinearTriadModel::new("tuning-gap");
        let mut limit = SparseVector::new();
        for index in 0..160 {
            let variable = BinaryVariableItem::create(VariableId::standalone(91_000 + index), &format!("x{index}"));
            basic.add_variable_with_bounds(Token::from_generic(variable, index), 0.0, 1.0, VariableType::Binary);
            limit.add(index, 1.0);
        }
        basic.add_constraint(limit, 80.5);
        let mut model = LinearTriadModel::from_basic(basic);
        model.set_objective((0..160).map(|index| (((index * 37) % 61) + (index % 7) + 1) as f64).collect(), ObjectiveCategory::Maximum);
        let base = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "scip".into(),
            model: ospf_rust_bridge::encode_model(&model).unwrap(), model_kind: None, time_limit_ms: Some(30_000), cp_options: None,
            serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None,
            csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None };
        let engine = NativeEngine::new(1);
        for gap in [0.0, 0.5, 0.95] {
            let job = engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: Some(SolverTuningOptions { threads: Some(1), seed: Some(1), mip_gap: Some(gap) }), ..base.clone() }).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            let view = loop {
                let view = engine.get(&job.id).unwrap();
                if view.status != JobStatus::Running { break view; }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            };
            assert_eq!(view.status, JobStatus::Completed, "{:?}", view.error);
            let report: ospf_rust_core::solver::SolveReport<f64> = serde_json::from_str(view.report_json.as_ref().unwrap()).unwrap();
            // 通用诚实性不变式：验证过的“最优性证明”只允许出现在精确完成时；间隙终止必须是 Feasible 且无证书。
            // General honesty invariant: a verified optimality proof may only accompany exact completion, while gap termination must be Feasible and certificate-free.
            let certified = report.proof.as_ref().is_some_and(|proof| matches!(proof.kind, ProofKind::Optimality) && matches!(proof.status, ProofStatus::Verified));
            if certified {
                assert_eq!(report.termination_reason, TerminationReason::Completed, "gap={gap}");
                assert_eq!(report.problem_status, ProblemStatus::Feasible, "gap={gap}");
            }
            if report.termination_reason == TerminationReason::GapLimit {
                assert_eq!(report.problem_status, ProblemStatus::Feasible);
                assert!(!certified);
            }
            engine.release(&job.id).unwrap();
        }
    }

    #[test]
    #[cfg(feature = "scip")]
    fn scip_intermediate_progress_is_visible_while_a_native_job_runs() {
        use ospf_rust_core::intermediate::{BasicLinearTriadModel, LinearTriadModel, SparseVector};
        use ospf_rust_core::model::ObjectiveCategory;
        use ospf_rust_core::solver::{ProgressValue, SolveProgressSnapshot};
        use ospf_rust_core_plugin_scip::{PresolvingMode, SCIPConfig, SCIPNativeControl, SCIPNativeObserver, SCIPSolver};
        use ospf_rust_core::token::Token;
        use ospf_rust_core::variable::{BinaryVariableItem, VariableId, VariableType};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc::sync_channel;

        let mut basic = BasicLinearTriadModel::new("native-progress");
        let mut limit = SparseVector::new();
        for index in 0..48 {
            let variable = BinaryVariableItem::create(VariableId::standalone(80_000 + index), &format!("x{index}"));
            basic.add_variable_with_bounds(Token::from_generic(variable, index), 0.0, 1.0, VariableType::Binary);
            limit.add(index, 1.0);
        }
        basic.add_constraint(limit, 17.5);
        let mut model = LinearTriadModel::from_basic(basic);
        model.set_objective((0..48).map(|index| (index + 2) as f64).collect(), ObjectiveCategory::Maximum);
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "scip".into(), model: ospf_rust_bridge::encode_model(&model).unwrap(), model_kind: None, time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None };
        let (entered, ready) = sync_channel(1);
        let (finish, gate) = sync_channel(1);
        let gate = Mutex::new(gate);
        let first = AtomicBool::new(true);
        let observer: SCIPNativeObserver = Arc::new(move |_| {
            if first.swap(false, Ordering::SeqCst) {
                entered.send(()).map_err(|error| ospf_rust_core::CoreError::callback_error(error.to_string()))?;
                lock(&gate).recv_timeout(Duration::from_secs(10))
                    .map_err(|error| ospf_rust_core::CoreError::callback_error(error.to_string()))?;
            }
            Ok(SCIPNativeControl::Continue)
        });
        let config = SCIPConfig::default().with_output(false).with_presolving(PresolvingMode::Off)
            .with_heuristics_priority(0).add_native_observer(observer);
        let engine = NativeEngine::default();
        let job = engine.start(request, Arc::new(SCIPSolver::with_config(config)), model).unwrap();
        ready.recv_timeout(Duration::from_secs(30)).unwrap();
        let running = engine.get(&job.id).unwrap();
        finish.send(()).unwrap();
        assert_eq!(running.status, JobStatus::Running);
        let progress: SolveProgressSnapshot = serde_json::from_str(running.progress_json.as_ref().unwrap()).unwrap();
        assert_eq!(progress.stage_progress, ProgressValue::Indeterminate);
        assert!(!progress.terminal);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let terminal = loop {
            let view = engine.get(&job.id).unwrap();
            if view.status != JobStatus::Running { break view; }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(terminal.status, JobStatus::Completed, "{:?}", terminal.error);
        let progress: SolveProgressSnapshot = serde_json::from_str(terminal.progress_json.as_ref().unwrap()).unwrap();
        assert!(progress.terminal);
        engine.release(&job.id).unwrap();
    }

    #[test]
    #[cfg(all(feature = "scip", not(feature = "scip-quadratic")))]
    fn linear_only_scip_rejects_quadratic_requests_before_execution() {
        let engine = NativeEngine::default();
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, iis_options: None,
            tuning_options: None,
            protocol_version: 1, solver: "scip".into(), model_kind: Some(ModelKind::Quadratic),
            model: ospf_rust_bridge::ModelArtifact { model_json: "{}".into(), fingerprint: "not-decoded".into() }, time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None,
        };
        assert_eq!(engine.submit(request).unwrap_err().code, "UnsupportedCapability");
        assert!(lock(&engine.state).jobs.is_empty());
    }

    #[test]
    fn backend_failure_and_capacity_lifecycle() {
        let engine = NativeEngine::new(1);
        let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("contract-test");
        let artifact = ospf_rust_bridge::encode_model(&model).unwrap();
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "fake".into(), model: artifact, model_kind: None, time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None };
        let job = engine.start(request, Arc::new(Fake), model).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while engine.get(&job.id).unwrap().status == JobStatus::Running {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(engine.get(&job.id).unwrap().status, JobStatus::Failed);
        engine.cancel(&job.id).unwrap();
        engine.release(&job.id).unwrap();
        assert_eq!(engine.get(&job.id).unwrap_err().code, "NotFound");
    }

    #[test]
    fn protocol_and_limits_fail_before_execution() {
        let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("contract-test");
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None,
            tuning_options: None,
            protocol_version: 2, solver: "scip".into(), model_kind: None,
            model: ospf_rust_bridge::encode_model(&model).unwrap(), time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None,
        };
        let engine = NativeEngine::default();
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "UnsupportedProtocolVersion");
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, time_limit_ms: Some(0), ..request };
        assert_eq!(engine.submit(request).unwrap_err().code, "InvalidInput");
    }

    #[test]
    fn cp_options_validate_before_backend_creation_and_never_apply_to_linear_jobs() {
        let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("options-contract");
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None,
            tuning_options: None,
            protocol_version: 1, solver: "not-a-backend".into(), model_kind: None,
            model: ospf_rust_bridge::encode_model(&model).unwrap(), time_limit_ms: None,
            cp_options: Some(ospf_rust_bridge::ConstraintProgrammingOptions::default()),
            serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None,
        };
        let engine = NativeEngine::default();
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "InvalidInput");
        let mut request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, model_kind: Some(ModelKind::Cp), ..request };
        request.cp_options.as_mut().unwrap().solution_hint = Some(vec![ospf_rust_bridge::IntegerSolutionHint { variable_id: "x".into(), value: "1.5".into() }]);
        assert_eq!(engine.submit(request).unwrap_err().code, "InvalidInput");
        assert!(!lock(&engine.state).active);
        assert!(lock(&engine.state).jobs.is_empty());
    }

    #[test]
    fn serial_strategy_requires_its_configuration_and_a_linear_model() {
        let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("serial-options-contract");
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, iis_options: None,
            tuning_options: None,
            protocol_version: 1, solver: "serial-linear".into(), model_kind: None,
            model: ospf_rust_bridge::encode_model(&model).unwrap(), time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None,
        };
        let engine = NativeEngine::default();
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "InvalidInput");
        let configuration = ospf_rust_bridge::SerialLinearOptions { backends: vec![ospf_rust_bridge::NativeBackend::Scip], fallback_policy: None };
        for backend in [ospf_rust_bridge::NativeBackend::Scip, ospf_rust_bridge::NativeBackend::Gurobi] {
            if !engine.capabilities().backends.iter().any(|entry| entry.id == backend.id() && entry.compiled) {
                let options = ospf_rust_bridge::SerialLinearOptions { backends: vec![backend], fallback_policy: None };
                assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, serial_options: Some(options), ..request.clone() }).unwrap_err().code, "BackendNotCompiled");
            }
        }
        assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, serial_options: Some(configuration.clone()), model_kind: Some(ModelKind::Quadratic), ..request.clone() }).unwrap_err().code, "InvalidInput");
        assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, serial_options: Some(configuration), solver: "scip".into(), ..request }).unwrap_err().code, "InvalidInput");
        assert!(lock(&engine.state).jobs.is_empty());
    }

    #[test]
    fn parallel_strategy_requires_exclusive_typed_linear_configuration() {
        use ospf_rust_bridge::{NativeBackend, ParallelLinearOptions, SerialLinearOptions};
        let engine = NativeEngine::default();
        let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("parallel-options-contract");
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, iis_options: None,
            tuning_options: None,
            protocol_version: 1, solver: "parallel-linear".into(), model_kind: None,
            model: ospf_rust_bridge::encode_model(&model).unwrap(), time_limit_ms: None,
            cp_options: None, serial_options: None, parallel_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None,
        };
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "InvalidInput");
        let configuration = ParallelLinearOptions { backends: vec![NativeBackend::Scip], mode: None };
        for kind in [ModelKind::Cp, ModelKind::Quadratic] {
            assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, parallel_options: Some(configuration.clone()), iis_options: None, model_kind: Some(kind), ..request.clone() }).unwrap_err().code, "InvalidInput");
        }
        assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, parallel_options: Some(configuration.clone()), iis_options: None, solver: "scip".into(), ..request.clone() }).unwrap_err().code, "InvalidInput");
        assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, parallel_options: Some(configuration.clone()), iis_options: None, serial_options: Some(SerialLinearOptions { backends: vec![NativeBackend::Scip], fallback_policy: None }), ..request.clone() }).unwrap_err().code, "InvalidInput");
        for backend in [NativeBackend::Scip, NativeBackend::Gurobi] {
            if !engine.capabilities().backends.iter().any(|descriptor| descriptor.id == backend.id() && descriptor.compiled) {
                assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, parallel_options: Some(ParallelLinearOptions { backends: vec![backend], mode: None }), ..request.clone() }).unwrap_err().code, "BackendNotCompiled");
            }
        }
        assert!(lock(&engine.state).jobs.is_empty());
    }

    #[test]
    fn fixed_recourse_requires_exclusive_linear_configuration_and_compiled_backend() {
        use ospf_rust_bridge::{FixedRecourseOptions, NativeBackend};
        let engine = NativeEngine::default();
        let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("fixed-recourse-options-contract");
        let mut request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, protocol_version: 1, solver: "fixed-recourse".into(), model_kind: None,
            tuning_options: None,
            model: ospf_rust_bridge::encode_model(&model).unwrap(), time_limit_ms: None,
            cp_options: None, serial_options: None, parallel_options: None, iis_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None };
        assert_eq!(engine.submit(request.clone()).unwrap_err().code, "InvalidInput");
        let configuration = FixedRecourseOptions { backend: NativeBackend::Scip, stages: None, iteration_limit: None, tolerance: None, recover_missing_certificates: None };
        request.fixed_recourse_options = Some(configuration);
        for kind in [ModelKind::Cp, ModelKind::Quadratic] {
            assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, model_kind: Some(kind), ..request.clone() }).unwrap_err().code, "InvalidInput");
        }
        assert_eq!(engine.submit(SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None, solution_pool_options: None, incumbent_values: None, tuning_options: None, gantt_activity_options: None, bpp3d_options: None, solver: "scip".into(), ..request.clone() }).unwrap_err().code, "InvalidInput");
        for backend in [NativeBackend::Scip, NativeBackend::Gurobi] {
            if !engine.capabilities().backends.iter().any(|descriptor| descriptor.id == backend.id() && descriptor.compiled) {
                request.fixed_recourse_options.as_mut().unwrap().backend = backend;
                assert_eq!(engine.submit(request.clone()).unwrap_err().code, "BackendNotCompiled");
            }
        }
        request.fixed_recourse_options.as_mut().unwrap().iteration_limit = Some(0);
        assert_eq!(engine.submit(request).unwrap_err().code, "InvalidInput");
        assert!(lock(&engine.state).jobs.is_empty());
    }

    #[test]
    fn concurrent_submissions_do_not_queue_model_decoding() {
        let engine = NativeEngine::default();
        let _guard = lock(&engine.submission);
        let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("contract-test");
        let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, iis_options: None,
            tuning_options: None,
            protocol_version: 1, solver: "scip".into(), model_kind: None,
            model: ospf_rust_bridge::encode_model(&model).unwrap(), time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None,
        };
        assert_eq!(engine.submit(request).unwrap_err().code, "Busy");
    }

    #[test]
    fn running_jobs_are_bounded_and_cancellation_preserves_terminal_reports() {
        use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
        use ospf_rust_core::solver::{ProblemStatus, SolveReport, SolverProvenance, TerminationReason, cancelled_solve_report};

        struct Gated {
            entered: SyncSender<()>,
            finish: Mutex<Receiver<()>>,
        }
        impl SolverInfo for Gated {
            fn name(&self) -> &str { "gated-contract-test" }
            fn capabilities(&self) -> Vec<SolverCapability> { vec![] }
        }
        impl LinearSolver for Gated {
            fn solve_linear(&self, _: &ospf_rust_core::model::intermediate::LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> {
                unreachable!("the executor must use the options-aware report path")
            }

            fn solve_linear_report_with_options(
                &self,
                _: &ospf_rust_core::model::intermediate::LinearTriadModel,
                options: &SolveOptions<'_>,
            ) -> ospf_rust_core::Result<SolveReport<f64>> {
                self.entered.send(()).unwrap();
                lock(&self.finish).recv_timeout(Duration::from_secs(5)).unwrap();
                let handle = options.cancellation_handle.unwrap();
                if handle.is_cancelled() {
                    cancelled_solve_report(SolverProvenance::default(), handle)
                } else {
                    SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed).build()
                }
            }
        }

        for cancel_before_completion in [true, false] {
            let engine = NativeEngine::new(1);
            let model = ospf_rust_core::model::intermediate::LinearTriadModel::new("gated");
            let request = SolveRequest { schedule_log_probe: None, operator_capability_probe: None, hypervolume_probe: None, acceptance_recovery_probe: None, training_split_probe: None, non_stationary_probe: None, selection_policy_probe: None, solve_value_probe: None, native_lowering_probe: None, native_terminal_proof_probe: None, predicate_scope_probe: None, persistence_schema_probe: None, persistence_sort_probe: None, effectiveness_ranking_probe: None, funnel_ranking: None, sensitivity_probe: None, correction_probe: None, blocking_set: None, fingerprint_probe: None, diagnostics_probe: None, gap_probe: None, proof_probe: None, evidence_probe: None, band_probe: None, group_summary: None, critical_report: None, effectiveness_probe: None, funnel: None, solution_status: None, diversity: None, piecewise: None, archive_probe: None, migration: None, topology: None, ranking_probe: None, acceptance: None, boundary: None, population_config: None, constraint_priority: None, constraint_ranking: None, pareto_archive: None, reference_points: None, dispersion: None, dominance: None, evidence_remap: None, tier_selection: None, activation_set: None, container_surface: None, identity_wrap: None, domain_surface: None, expression_bounds: None, assignment_eval: None, constraint_canon: None, snapshot_gate: None, cp_model_build: None, no_good_oracle: None, subproblem_result: None, cp_support_merge: None, capability_matrix: None, analysis_options: None, model_view: None, polynomial_probe: None, matrix_rows: None, sparse_triad: None, master_assignment: None, stable_value: None, quadratic_elastic: None, dump_batch: None, lazy_flatten_init: None, candidate_shortlist: None, writer_registry: None, native_write_scope: None, dump_options: None, assignment_composition: None, cut_validity: None, source_identity: None, binding_assignment: None, auxiliary_domain: None, activity_report_check: None, evidence_predicates: None, activity_bands: None, encoding_label: None, expression_key: None, canonical_monomial: None, backend_features: None, deferred_materialize: None, quadratic_monomial: None, polarity_encoding: None, tier_classification: None, assumption_binding: None, effectiveness_aggregate: None, bounded_no_good: None, polynomial_lift: None, scalar_function: None, field_resolution: None, boolean_fold: None, master_cut: None, flatten_identity: None, critical_assembly: None, variable_kind_domain: None, sort_by: None, predicate_schema: None, search_budget: None, conditional_semantics: None, big_m_policy: None, number_text: None, combine_status: None, elastic_model: None, native_write: None, input_domain: None, function_usage: None, repository_call: None, unsupported_predicate: None, boolean_scope: None, update_assignment: None, flatten_cache: None, cache_key_address: None, cache_key_identity: None, model_configuration: None, predicate_field_filter: None, predicate_field_resolution: None, analysis_group_aggregate: None, master_cut_encoding: None, q_table: None, heuristic_schedule: None, symbol_core_relations: None, symbol_linearization: None, symbol_condition: None, benders_cut_request: None, benders_metrics: None, cp_checkpoint_request: None, lbb_resume_identity: None, linear_mapping_request: None, checkpoint_request: None, reward_policy: None, hyper_heuristic_config: None, criticality_request: None, multi_target_report: None, relaxation_policy: None, target_spec: None, target_report: None, perturbation_policy: None, fixed_integer_report: None, fixed_integer_config: None, relaxation_request: None, critical_analysis: None, effectiveness_ranking: None, shortlist_limit: None, candidate_funnel: None, conflict_explanation: None, remote_artifact: None, logic_based_benders_plan: None, master_solution: None, benders_quadratic_cuts: None, pipeline_steps: None, repository_query: None, assignments: None, sort_plan: None, predicate: None, benders_cuts: None, relational_query_plan: None, linear_lifecycle: None,  indexed_registration: None, column_batch: None, column_scope: None, heart_beat: None, response_envelope: None, quadratic_orchestration: None, vrp_problem: None, network_resilience_problem: None, allocation_problem: None, network_structure_problem: None, network_design_problem: None, network_generalized_flow_problem: None, network_flow_problem: None, sweep_cases: None,  incumbent_values: None, solution_pool_options: None, gantt_activity_options: None, bpp3d_options: None, iis_options: None,
                tuning_options: None,
                protocol_version: 1, solver: "fake".into(), model_kind: None,
                model: ospf_rust_bridge::encode_model(&model).unwrap(), time_limit_ms: None, cp_options: None, serial_options: None, parallel_options: None, bpp1d_options: None, bpp2d_options: None, bpp_loading_options: None, csp2d_options: None, csp1d_options: None, column_generation_options: None, fixed_recourse_options: None,
            };
            let (entered, ready) = sync_channel(1);
            let (finish, gate) = sync_channel(1);
            let job = engine.start(request.clone(), Arc::new(Gated { entered, finish: Mutex::new(gate) }), model.clone()).unwrap();
            ready.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(engine.release(&job.id).unwrap_err().code, "Busy");
            assert_eq!(engine.start(request.clone(), Arc::new(Fake), model.clone()).unwrap_err().code, "Busy");
            if cancel_before_completion {
                assert_eq!(engine.cancel(&job.id).unwrap().status, JobStatus::Running);
            }
            finish.send(()).unwrap();
            if !cancel_before_completion { engine.cancel(&job.id).unwrap(); }
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while engine.get(&job.id).unwrap().status == JobStatus::Running {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            let terminal = engine.get(&job.id).unwrap();
            assert_eq!(terminal.status, JobStatus::Completed);
            let report: SolveReport<f64> = serde_json::from_str(terminal.report_json.as_ref().unwrap()).unwrap();
            if cancel_before_completion { assert_eq!(report.termination_reason, TerminationReason::Cancelled); }
            assert_eq!(engine.cancel(&job.id).unwrap().report_json, terminal.report_json);
            assert_eq!(engine.start(request, Arc::new(Fake), model).unwrap_err().code, "CapacityExceeded");
            engine.release(&job.id).unwrap();
        }
    }

}

/// 续挖入口的 payload 字段清单：把**在场的**字段名收集起来（用于"只接受自己那一个"的唯一性检查）。
/// The continuation payload fields present on the request, collected for the "only its own field" uniqueness check.
fn continuation_payloads(request: &SolveRequest) -> Vec<&'static str> {
    let mut present: Vec<&'static str> = Vec::new();
    macro_rules! collect {
        ($($field:ident),* $(,)?) => {
            $( if request.$field.is_some() { present.push(stringify!($field)); } )*
        };
    }
    collect!(
        master_cut_encoding,
        master_cut,
        boolean_fold,
        field_resolution,
        scalar_function,
        polynomial_lift,
        bounded_no_good,
        effectiveness_aggregate,
        assumption_binding,
        tier_classification,
        polarity_encoding,
        quadratic_monomial,
        deferred_materialize,
        backend_features,
        canonical_monomial,
        expression_key,
        encoding_label,
        activity_bands,
        evidence_predicates,
        activity_report_check,
        auxiliary_domain,
        binding_assignment,
        source_identity,
        cut_validity,
        assignment_composition,
        dump_options,
        native_write_scope,
        writer_registry,
        candidate_shortlist,
        lazy_flatten_init,
        dump_batch,
        quadratic_elastic,
        stable_value,
        master_assignment,
        sparse_triad,
        matrix_rows,
        polynomial_probe,
        model_view,
        analysis_options,
        capability_matrix,
        cp_support_merge,
        subproblem_result,
        no_good_oracle,
        cp_model_build,
        snapshot_gate,
        constraint_canon,
        assignment_eval,
        expression_bounds,
        domain_surface,
        identity_wrap,
        container_surface,
        activation_set,
        tier_selection,
        evidence_remap,
        dominance,
        dispersion,
        reference_points,
        pareto_archive,
        constraint_ranking,
        constraint_priority,
        population_config,
        boundary,
        acceptance,
        ranking_probe,
        topology,
        migration,
        archive_probe,
        piecewise,
        diversity,
        solution_status,
        funnel,
        effectiveness_probe,
        critical_report,
        group_summary,
        band_probe,
        evidence_probe,
        proof_probe,
        gap_probe,
        diagnostics_probe,
        fingerprint_probe,
        blocking_set,
        correction_probe,
        sensitivity_probe,
        funnel_ranking,
        effectiveness_ranking_probe,
        persistence_sort_probe,
        persistence_schema_probe,
        predicate_scope_probe,
        native_terminal_proof_probe,
        native_lowering_probe,
        solve_value_probe,
        selection_policy_probe,
        non_stationary_probe,
        training_split_probe,
        acceptance_recovery_probe,
        hypervolume_probe,
        operator_capability_probe,
        schedule_log_probe,
        analysis_group_aggregate,
        predicate_field_resolution,
        predicate_field_filter,
        model_configuration,
        cache_key_identity,
        cache_key_address,
        flatten_cache,
        update_assignment,
        boolean_scope,
        unsupported_predicate,
        repository_call,
        function_usage,
        input_domain,
        native_write,
        elastic_model,
        combine_status,
        number_text,
        big_m_policy,
        conditional_semantics,
        search_budget,
        predicate_schema,
        sort_by,
        variable_kind_domain,
        critical_assembly,
        flatten_identity,
        predicate,
        assignments,
        sort_plan,
        repository_query,
        pipeline_steps,
        benders_metrics,
        benders_cut_request,
        symbol_condition,
        symbol_linearization,
        symbol_core_relations,
        heuristic_schedule,
        q_table,
        lbb_resume_identity,
        cp_checkpoint_request,
        linear_mapping_request,
        checkpoint_request,
        reward_policy,
        hyper_heuristic_config,
        criticality_request,
        relaxation_policy,
        multi_target_report,
        target_report,
        target_spec,
        perturbation_policy,
        fixed_integer_report,
        fixed_integer_config,
        relaxation_request,
        critical_analysis,
        effectiveness_ranking,
        shortlist_limit,
        candidate_funnel,
        conflict_explanation,
        remote_artifact,
        logic_based_benders_plan,
        master_solution,
        benders_quadratic_cuts,
        benders_cuts,
    );
    present
}

/// 该 kind 自己的那一个 payload 字段名；**只覆盖续挖的 26 个单 payload 入口**（`master_cut_encoding` …
/// `flatten_identity`）。旧 pipeline 入口允许两个字段（`candidate_funnel` + `shortlist_limit` 等），这里不碰。
/// The kind's own payload field name, covering **only the 26 single-payload continuation entries**. Legacy pipeline
/// entries legitimately take two fields (for example `candidate_funnel` + `shortlist_limit`), so they are left alone.
fn continuation_payload_of(kind: Option<ModelKind>) -> Option<&'static str> {
    Some(match kind? {
        ModelKind::MasterCutEncodingVerify => "master_cut_encoding",
        ModelKind::MasterCutVerify => "master_cut",
        ModelKind::BooleanFoldVerify => "boolean_fold",
        ModelKind::FieldResolutionVerify => "field_resolution",
        ModelKind::ScalarFunctionVerify => "scalar_function",
        ModelKind::PolynomialLiftVerify => "polynomial_lift",
        ModelKind::BoundedNoGoodVerify => "bounded_no_good",
        ModelKind::EffectivenessAggregateVerify => "effectiveness_aggregate",
        ModelKind::AssumptionBindingVerify => "assumption_binding",
        ModelKind::TierClassificationVerify => "tier_classification",
        ModelKind::PolarityEncodingVerify => "polarity_encoding",
        ModelKind::QuadraticMonomialVerify => "quadratic_monomial",
        ModelKind::DeferredMaterializeVerify => "deferred_materialize",
        ModelKind::BackendFeaturesVerify => "backend_features",
        ModelKind::CanonicalMonomialVerify => "canonical_monomial",
        ModelKind::ExpressionKeyVerify => "expression_key",
        ModelKind::EncodingLabelVerify => "encoding_label",
        ModelKind::ActivityBandsVerify => "activity_bands",
        ModelKind::EvidencePredicatesVerify => "evidence_predicates",
        ModelKind::ActivityReportCheckVerify => "activity_report_check",
        ModelKind::AuxiliaryDomainVerify => "auxiliary_domain",
        ModelKind::BindingAssignmentVerify => "binding_assignment",
        ModelKind::SourceIdentityVerify => "source_identity",
        ModelKind::CutValidityVerify => "cut_validity",
        ModelKind::AssignmentCompositionVerify => "assignment_composition",
        ModelKind::DumpOptionsVerify => "dump_options",
        ModelKind::NativeWriteScopeVerify => "native_write_scope",
        ModelKind::WriterRegistryVerify => "writer_registry",
        ModelKind::CandidateShortlistVerify => "candidate_shortlist",
        ModelKind::LazyFlattenInitVerify => "lazy_flatten_init",
        ModelKind::DumpBatchVerify => "dump_batch",
        ModelKind::QuadraticElasticVerify => "quadratic_elastic",
        ModelKind::StableValueVerify => "stable_value",
        ModelKind::MasterAssignmentVerify => "master_assignment",
        ModelKind::SparseTriadVerify => "sparse_triad",
        ModelKind::MatrixRowsVerify => "matrix_rows",
        ModelKind::PolynomialProbeVerify => "polynomial_probe",
        ModelKind::ModelViewVerify => "model_view",
        ModelKind::AnalysisOptionsVerify => "analysis_options",
        ModelKind::CapabilityMatrixVerify => "capability_matrix",
        ModelKind::CpSupportMergeVerify => "cp_support_merge",
        ModelKind::SubproblemResultVerify => "subproblem_result",
        ModelKind::NoGoodOracleVerify => "no_good_oracle",
        ModelKind::CpModelBuildVerify => "cp_model_build",
        ModelKind::SnapshotGateVerify => "snapshot_gate",
        ModelKind::ConstraintCanonVerify => "constraint_canon",
        ModelKind::AssignmentEvalVerify => "assignment_eval",
        ModelKind::ExpressionBoundsVerify => "expression_bounds",
        ModelKind::DomainSurfaceVerify => "domain_surface",
        ModelKind::IdentityWrapVerify => "identity_wrap",
        ModelKind::ContainerSurfaceVerify => "container_surface",
        ModelKind::ActivationSetVerify => "activation_set",
        ModelKind::TierSelectionVerify => "tier_selection",
        ModelKind::EvidenceRemapVerify => "evidence_remap",
        ModelKind::DominanceVerify => "dominance",
        ModelKind::DispersionVerify => "dispersion",
        ModelKind::ReferencePointsVerify => "reference_points",
        ModelKind::ParetoArchiveVerify => "pareto_archive",
        ModelKind::ConstraintRankingVerify => "constraint_ranking",
        ModelKind::ConstraintPriorityVerify => "constraint_priority",
        ModelKind::PopulationConfigVerify => "population_config",
        ModelKind::BoundaryVerify => "boundary",
        ModelKind::AcceptanceVerify => "acceptance",
        ModelKind::RankingProbeVerify => "ranking_probe",
        ModelKind::TopologyVerify => "topology",
        ModelKind::MigrationVerify => "migration",
        ModelKind::ArchiveProbeVerify => "archive_probe",
        ModelKind::PiecewiseVerify => "piecewise",
        ModelKind::DiversityVerify => "diversity",
        ModelKind::SolutionStatusVerify => "solution_status",
        ModelKind::FunnelVerify => "funnel",
        ModelKind::EffectivenessProbeVerify => "effectiveness_probe",
        ModelKind::CriticalReportVerify => "critical_report",
        ModelKind::GroupSummaryVerify => "group_summary",
        ModelKind::BandProbeVerify => "band_probe",
        ModelKind::EvidenceProbeVerify => "evidence_probe",
        ModelKind::ProofProbeVerify => "proof_probe",
        ModelKind::GapProbeVerify => "gap_probe",
        ModelKind::DiagnosticsProbeVerify => "diagnostics_probe",
        ModelKind::FingerprintProbeVerify => "fingerprint_probe",
        ModelKind::BlockingSetVerify => "blocking_set",
        ModelKind::CorrectionProbeVerify => "correction_probe",
        ModelKind::SensitivityProbeVerify => "sensitivity_probe",
        ModelKind::FunnelRankingVerify => "funnel_ranking",
        ModelKind::EffectivenessRankingVerify => "effectiveness_ranking_probe",
        ModelKind::PersistenceSortVerify => "persistence_sort_probe",
        ModelKind::PersistenceSchemaVerify => "persistence_schema_probe",
        ModelKind::PredicateScopeVerify => "predicate_scope_probe",
        ModelKind::NativeTerminalProofVerify => "native_terminal_proof_probe",
        ModelKind::NativeLoweringVerify => "native_lowering_probe",
        ModelKind::SolveValueVerify => "solve_value_probe",
        ModelKind::SelectionPolicyVerify => "selection_policy_probe",
        ModelKind::NonStationaryVerify => "non_stationary_probe",
        ModelKind::TrainingSplitVerify => "training_split_probe",
        ModelKind::AcceptanceRecoveryVerify => "acceptance_recovery_probe",
        ModelKind::HypervolumeVerify => "hypervolume_probe",
        ModelKind::OperatorCapabilityVerify => "operator_capability_probe",
        ModelKind::ScheduleLogVerify => "schedule_log_probe",
        ModelKind::AnalysisGroupAggregate => "analysis_group_aggregate",
        ModelKind::PredicateFieldResolutionVerify => "predicate_field_resolution",
        ModelKind::PredicateFieldFilterExtract => "predicate_field_filter",
        ModelKind::ModelConfigurationVerify => "model_configuration",
        ModelKind::CacheKeyIdentityVerify => "cache_key_identity",
        ModelKind::CacheKeyAddressVerify => "cache_key_address",
        ModelKind::FlattenCacheVerify => "flatten_cache",
        ModelKind::UpdateAssignmentVerify => "update_assignment",
        ModelKind::BooleanScopeVerify => "boolean_scope",
        ModelKind::UnsupportedPredicateVerify => "unsupported_predicate",
        ModelKind::RepositoryCallVerify => "repository_call",
        ModelKind::FunctionUsageVerify => "function_usage",
        ModelKind::InputDomainVerify => "input_domain",
        ModelKind::NativeWriteVerify => "native_write",
        ModelKind::ElasticModelVerify => "elastic_model",
        ModelKind::CombineStatusVerify => "combine_status",
        ModelKind::NumberTextVerify => "number_text",
        ModelKind::BigMPolicyVerify => "big_m_policy",
        ModelKind::ConditionalSemanticsVerify => "conditional_semantics",
        ModelKind::SearchBudgetVerify => "search_budget",
        ModelKind::PredicateSchemaVerify => "predicate_schema",
        ModelKind::SortByVerify => "sort_by",
        ModelKind::VariableKindDomainVerify => "variable_kind_domain",
        ModelKind::CriticalAssemblyVerify => "critical_assembly",
        ModelKind::FlattenIdentityVerify => "flatten_identity",
        _ => return None,
    })
}

