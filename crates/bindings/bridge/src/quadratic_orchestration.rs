//! 二次组合编排绑定：多个已绑定二次后端串行/并行求解同一模型，并按策略选择结果。
//! Quadratic orchestration binding: several already-bound quadratic backends solve the same model serially or in
//! parallel and a policy selects the result.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_core::model::intermediate::QuadraticTetradModel;
use ospf_rust_core::solver::{CombinatorialSolveReport, SolveOptions, SolveReport};

use crate::{BridgeError, NativeBackend, ParallelCombinatorialMode, Result, SerialFallbackPolicy};

/// 二次组合编排配置。 / Quadratic orchestration configuration.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuadraticOrchestrationOptions {
    /// 有序、非空且不重复的原生后端；顺序用于打破平局。 / Ordered nonempty distinct native backends; order breaks ties.
    pub backends: Vec<NativeBackend>,
    /// `serial` 或 `parallel`。 / Either `serial` or `parallel`.
    pub orchestration: QuadraticOrchestration,
    /// 并行选择模式；仅 `parallel` 接受。 / Parallel selection mode; only `parallel` accepts it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mode: Option<ParallelCombinatorialMode>,
    /// 串行终态策略；仅 `serial` 接受。 / Serial terminal policy; only `serial` accepts it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fallback_policy: Option<SerialFallbackPolicy>,
}

/// 编排方向。 / The orchestration direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QuadraticOrchestration {
    /// 依次尝试后端。 / Try the backends one after another.
    Serial,
    /// 并发尝试后端。 / Try the backends concurrently.
    Parallel,
}

impl QuadraticOrchestrationOptions {
    /// 校验后端集合与方向专属字段，拒绝跨方向混用。
    /// Validate the backend set and the direction-specific fields, rejecting cross-direction mixing.
    pub fn validate(&self) -> Result<()> {
        if self.backends.is_empty() || self.backends.len() > 2
            || self.backends.iter().collect::<BTreeSet<_>>().len() != self.backends.len()
        {
            return Err(BridgeError::new("InvalidInput",
                "quadratic orchestration backends must be a nonempty distinct list of scip and/or gurobi"));
        }
        match self.orchestration {
            QuadraticOrchestration::Serial => {
                if self.mode.is_some() {
                    return Err(BridgeError::new("InvalidInput", "serial quadratic orchestration does not accept a parallel mode"));
                }
            }
            QuadraticOrchestration::Parallel => {
                if self.fallback_policy.is_some() {
                    return Err(BridgeError::new("InvalidInput", "parallel quadratic orchestration does not accept a serial fallback policy"));
                }
            }
        }
        Ok(())
    }
}

/// 二次组合报告：聚合报告 + 提交模型身份复核，不独立证明最优性。
/// Quadratic aggregate report: the aggregate plus a submitted-model identity recheck, without independently proving
/// optimality.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuadraticOrchestrationReport {
    /// 后端名字符串化的聚合报告。 / The aggregate report with backend names as strings.
    pub aggregate: String,
    /// 提交模型指纹。 / The submitted model fingerprint.
    pub model_fingerprint: String,
}

/// 校验聚合报告契约与 schema，并确认它属于给定二次模型。
/// Validate the aggregate contract and schema, and confirm it belongs to the given quadratic model.
pub fn decode_quadratic_orchestration_report(json: &str, model: &QuadraticTetradModel)
    -> Result<CombinatorialSolveReport<f64, u64>> {
    if json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("PayloadTooLarge", "quadratic orchestration report exceeds 8 MiB"));
    }
    let aggregate: CombinatorialSolveReport<f64, u64> = serde_json::from_str(json)
        .map_err(|error| BridgeError::new("InvalidReport", &format!("quadratic aggregate report is not valid JSON: {error}")))?;
    if aggregate.report.schema_version != ospf_rust_core::solver::CURRENT_SOLVE_REPORT_SCHEMA_VERSION {
        return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported quadratic aggregate report schema"));
    }
    aggregate.report.validate().map_err(|error| BridgeError::new("InvalidReport", &error))?;
    let fingerprint = ospf_rust_core::solver::quadratic_model_fingerprint(model)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    if aggregate.report.fingerprints.model.as_ref() != Some(&fingerprint) {
        return Err(BridgeError::new("FingerprintMismatch", "quadratic aggregate report belongs to a different model"));
    }
    if aggregate.attempts.is_empty() {
        return Err(BridgeError::new("InvalidReport", "quadratic aggregate report has no attempts"));
    }
    // 每个 attempt 的结论类型必须与它携带的证据一致：`Report` 必须有结构化摘要，`Failed` 必须有结构化失败。
    // The declared outcome must match the evidence it carries: `Report` needs a structured summary, `Failed` needs a
    // structured issue.
    for attempt in &aggregate.attempts {
        match attempt.outcome {
            ospf_rust_core::solver::SolveAttemptOutcome::Report => {
                if attempt.summary.is_none() {
                    return Err(BridgeError::new("InvalidReport",
                        "a quadratic orchestration report attempt must carry a structured summary"));
                }
            }
            ospf_rust_core::solver::SolveAttemptOutcome::Failed => {
                if attempt.error.is_none() {
                    return Err(BridgeError::new("InvalidReport",
                        "a failed quadratic orchestration attempt must carry a structured issue"));
                }
            }
            _ => {}
        }
    }
    Ok(aggregate)
}

/// 把聚合报告包成 DTO。 / Wrap the aggregate report into the DTO.
pub fn encode_quadratic_orchestration_report(aggregate: &CombinatorialSolveReport<f64>, model: &QuadraticTetradModel)
    -> Result<QuadraticOrchestrationReport> {
    let fingerprint = ospf_rust_core::solver::quadratic_model_fingerprint(model)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    // 无 incumbent 的预取消聚合可能没有指纹；仅在已认证执行边界绑定源身份。
    // Pre-cancelled aggregates without incumbents may lack a fingerprint; bind the source identity only at this
    // authenticated execution boundary.
    let mut aggregate = aggregate.clone();
    if aggregate.report.fingerprints.model.is_none() && aggregate.report.solution.is_none() {
        aggregate.report.fingerprints.model = Some(fingerprint.clone());
    }
    let aggregate = serde_json::to_string(&aggregate).map_err(|error| BridgeError::new("CodecFailure", error))?;
    decode_quadratic_orchestration_report(&aggregate, model)?;
    Ok(QuadraticOrchestrationReport { aggregate, model_fingerprint: fingerprint.value })
}

/// core 二次求解器到 framework 组合编排的适配器。
/// framework 的 `QuadraticSolver` 与 `ospf-rust-core` 的 `QuadraticSolver` 是两个独立 trait，这里显式搭桥，
/// 而不是让桥接依赖任一方的内部构造细节。
/// An adapter from a core quadratic solver to the framework's combinatorial orchestration. The framework's
/// `QuadraticSolver` and `ospf-rust-core`'s `QuadraticSolver` are two distinct traits, so this bridges them explicitly
/// instead of making the bridge depend on either side's construction details.
pub struct CoreQuadraticBackend {
    solver: Arc<dyn ospf_rust_core::solver::QuadraticSolver>,
    name: String,
}

impl CoreQuadraticBackend {
    /// 包装一个 core 二次求解器；名称取自求解器自身。 / Wrap a core quadratic solver; the name comes from the solver itself.
    pub fn new(solver: Arc<dyn ospf_rust_core::solver::QuadraticSolver>) -> Self {
        let name = solver.name().to_owned();
        Self { solver, name }
    }
}

impl ospf_rust_framework::QuadraticSolver for CoreQuadraticBackend {
    fn name(&self) -> &str { &self.name }

    fn solve(&self, model: &QuadraticTetradModel) -> ospf_rust_core::Result<ospf_rust_framework::FeasibleSolution> {
        let report = self.solver.solve_quadratic_report(model)?;
        ospf_rust_framework::FeasibleSolution::try_from_report(&report)
    }

    fn solve_report_with_options(&self, model: &QuadraticTetradModel, options: ospf_rust_framework::FrameworkSolveOptions)
        -> ospf_rust_core::Result<SolveReport<f64>> {
        let core_options = SolveOptions {
            time_limit: options.time_limit,
            cancellation_handle: options.cancellation_handle.as_ref(),
            ..Default::default()
        };
        self.solver.solve_quadratic_report_with_options(model, &core_options)
    }
}

/// 把 core 二次求解器包成编排可用的后端。 / Wrap a core quadratic solver into an orchestration-ready backend.
pub fn core_backend(solver: Arc<dyn ospf_rust_core::solver::QuadraticSolver>) -> Arc<dyn ospf_rust_framework::QuadraticSolver> {
    Arc::new(CoreQuadraticBackend::new(solver))
}

/// 求解一次二次组合编排；串行与并行都走 framework 的同步实现。
/// Run one quadratic orchestration; both serial and parallel use the framework's synchronous implementation.
pub fn solve_quadratic_orchestration(
    model: &QuadraticTetradModel,
    backends: Vec<Arc<dyn ospf_rust_framework::QuadraticSolver>>,
    options: &QuadraticOrchestrationOptions,
    time_limit: Option<std::time::Duration>,
    cancellation: Option<ospf_rust_core::solver::SolveHandle>,
) -> Result<QuadraticOrchestrationReport> {
    use ospf_rust_framework::QuadraticSolver as _;
    options.validate()?;
    let framework_options = ospf_rust_framework::FrameworkSolveOptions {
        time_limit,
        cancellation_handle: cancellation,
        ..Default::default()
    };
    let aggregate = match options.orchestration {
        QuadraticOrchestration::Serial => {
            let solver = ospf_rust_framework::SerialCombinatorialQuadraticSolver::new(backends);
            let fallback_policy = match options.fallback_policy.unwrap_or_default() {
                SerialFallbackPolicy::StopOnCertifiedTerminal
                    => ospf_rust_framework::solver::column_generation_solver::CombinatorialFallbackPolicy::StopOnCertifiedTerminal,
                SerialFallbackPolicy::ContinueOnTerminal
                    => ospf_rust_framework::solver::column_generation_solver::CombinatorialFallbackPolicy::ContinueOnTerminal,
            };
            solver.solve_combinatorial_report_with_options(model, framework_options.with_fallback_policy(fallback_policy))
        }
        QuadraticOrchestration::Parallel => {
            let mode = match options.mode.unwrap_or_default() {
                ParallelCombinatorialMode::First => ospf_rust_framework::ParallelCombinatorialMode::First,
                ParallelCombinatorialMode::Best => ospf_rust_framework::ParallelCombinatorialMode::Best,
            };
            let solver = ospf_rust_framework::ParallelCombinatorialQuadraticSolver::new(backends, mode);
            solver.solve_combinatorial_report_with_options(model, framework_options)
        }
    }
    .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
    encode_quadratic_orchestration_report(&aggregate, model)
}

/// 供调用方复用的模型身份。 / Model identity callers can reuse.
pub fn quadratic_identity(model: &QuadraticTetradModel) -> Result<String> {
    ospf_rust_core::solver::quadratic_model_fingerprint(model)
        .map(|fingerprint| fingerprint.value).map_err(|error| BridgeError::new("InvalidInput", &error))
}
