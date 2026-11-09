//! 有界 CSP1D 原生执行契约及完整阶段/trace 报告；不提升受限池证明。
//! Bounded CSP1D native execution contracts and complete stage/trace reports, without promoting restricted-pool proofs.

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_framework_csp1d as domain;
use crate::{BridgeError, Csp1dGenerationOptions, Csp1dPattern, Csp1dProduction, Csp1dProductionReport, Csp1dProductionReportArtifact,
    Csp1dProductionReportSummary, Csp1dStageKind, Csp1dStageReportArtifact, Csp1dUsage, ModelArtifact, NativeBackend, Result,
    decode_csp1d_pattern, decode_csp1d_problem, decode_csp1d_production_report, decode_csp1d_stage_report, encode_csp1d_pattern,
    encode_csp1d_stage_report, invalid};

/// 原领域编排，不在 JS 重实现求解。 / Original-domain orchestration, without solver reimplementation in JS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp1dAlgorithm {
    /// 有界候选整数模型。 / Bounded-candidate integer model.
    Milp,
    /// 原 LP/定价/最终整数模型流程。 / Original LP/pricing/final-integer workflow.
    ColumnGeneration,
}

/// 宿主预注册默认 Roll/Sheet 规则，禁止任意回调或隐式后端替换。
/// Host-registered default Roll/Sheet rules, forbidding arbitrary callbacks and implicit backend substitution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dOptions {
    /// 显式原生后端。 / Explicit native backend.
    pub backend: NativeBackend,
    /// 原领域流程选择。 / Original-domain workflow selection.
    pub algorithm: Csp1dAlgorithm,
    /// 初始候选选项；定价复用同策略/刀数，在总池预算内继续枚举。 / Initial candidates; pricing reuses the strategy/knife count within the total pool budget.
    #[ts(optional)]
    pub generation: Option<Csp1dGenerationOptions>,
    /// 默认 8，范围 1..64。 / Default 8, range 1..64.
    #[ts(optional)]
    pub iteration_limit: Option<u32>,
    /// 每轮接纳列数，默认 64，范围 1..512。 / Admitted columns per round, default 64, range 1..512.
    #[ts(optional)]
    pub columns_per_iteration: Option<u32>,
    /// 完整池上限，默认 512；不得小于初始候选预算。 / Total pool cap, default 512; cannot be below the initial candidate budget.
    #[ts(optional)]
    pub pool_limit: Option<u32>,
}

impl Csp1dOptions {
    /// 校验工作预算并复用原配置；不调整原 Rust 默认入口。 / Validate work budgets and reuse original configuration without changing default Rust entries.
    pub fn config(&self) -> Result<domain::Csp1dConfiguration> {
        let generation = self.generation.clone().unwrap_or_default();
        generation.generator()?;
        let initial = generation.max_plans.unwrap_or(128);
        let iterations = self.iteration_limit.unwrap_or(8);
        let columns = self.columns_per_iteration.unwrap_or(64);
        let pool = self.pool_limit.unwrap_or(512);
        if !(1..=64).contains(&iterations) || !(1..=512).contains(&columns) || !(1..=512).contains(&pool) || initial > pool {
            return Err(invalid("invalid CSP1D execution budgets or initial pool exceeds poolLimit"));
        }
        Ok(domain::Csp1dConfiguration { max_initial_plans: u64::from(initial), max_pricing_plans: u64::from(columns), iteration_limit: u64::from(iterations) })
    }
}

/// 原领域业务状态，和求解器/执行停止原因分开。 / Original-domain business status, separate from solver/execution stop reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp1dBusinessStatus {
    /// 已提取最终生产。 / Final production extracted.
    Feasible,
    /// 保留原领域部分结果语义。 / Original partial-result semantics.
    Partial,
    /// 没有初始候选。 / No initial candidates.
    NoInitialPlans,
    /// 原流程失败，无伪造产出。 / Original workflow failed, without fabricated production.
    Failed,
}

/// 合作执行停止原因，不改写任何子报告。 / Cooperative execution stop reason, without rewriting child reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp1dExecutionStop {
    /// 流程已返回，未观察到预算/取消停止。 / Workflow returned without an observed budget/cancellation stop.
    Completed,
    /// 观察到合作取消。 / Cooperative cancellation observed.
    Cancelled,
    /// 观察到总预算耗尽。 / Total budget exhaustion observed.
    TimeLimit,
}

/// 完整阶段和原 trace 的无损封装，内部整数只由 Rust 读取。
/// Lossless envelope for every stage and the original trace, with inner integers read only by Rust.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dExecutionReportArtifact {
    /// 独立执行报告版本。 / Independent execution-report version.
    pub schema_version: String,
    /// 源问题身份。 / Source-problem identity.
    pub source_fingerprint: String,
    /// 实际配置。 / Actual configuration.
    pub configuration: Csp1dOptions,
    /// 原业务状态。 / Original business status.
    pub business_status: Csp1dBusinessStatus,
    /// 执行停止语义。 / Execution stop semantics.
    pub stop: Csp1dExecutionStop,
    /// 完整最终候选池。 / Complete final candidate pool.
    pub plans: Vec<Csp1dPattern>,
    /// 按实际执行顺序保留每个子模型及报告。 / Every actual child model/report in execution order.
    pub stages: Vec<Csp1dStageReportArtifact>,
    /// 原领域 trace 的 canonical 文本，保留全部计数。 / Canonical original-domain trace text retaining every counter.
    pub trace_json: String,
}

/// Rust 复验派生的安全摘要，不在 JS 复制领域协议字段。
/// Safe Rust-revalidated derived summary, without copying domain protocol fields in JS.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Csp1dExecutionSummary {
    /// 原生执行方式。 / Native execution mode.
    pub algorithm: Csp1dAlgorithm,
    /// 原业务状态。 / Original business status.
    pub business_status: Csp1dBusinessStatus,
    /// 合作停止原因。 / Cooperative stop reason.
    pub stop: Csp1dExecutionStop,
    /// 始终 false；受限定价不穷举混合模式空间。 / Always false; restricted pricing does not exhaust mixed patterns.
    pub candidate_complete: bool,
    /// 最终候选池大小。 / Final candidate-pool size.
    pub column_count: u32,
    /// 实际阶段数。 / Actual stage count.
    pub stage_count: u32,
    /// 原定价 trace 迭代记录数。 / Original pricing-trace iteration count.
    pub iteration_count: u32,
    /// 最终整数子问题的源绑定摘要，不是全局证明。 / Source-bound final integer-child summary, not a global proof.
    pub integer: Option<Csp1dProductionReportSummary>,
}

/// 已复验完整流程，生产量仅从真实整数报告导出。 / Revalidated complete workflow, deriving production only from the actual integer report.
#[derive(Debug, Clone)]
pub struct Csp1dExecutionReport {
    /// 保留的原始封装。 / Retained original artifact.
    pub artifact: Csp1dExecutionReportArtifact,
    /// 派生摘要。 / Derived summary.
    pub summary: Csp1dExecutionSummary,
    /// 独立整数报告；前置失败时可以不存在。 / Independent integer report, possibly absent on an earlier failure.
    pub integer: Option<Csp1dProductionReport>,
    /// 从整数报告导出的离散生产。 / Discrete production derived from the integer report.
    pub production: Option<Csp1dProduction>,
}

/// 编码原流程并核对原领域提取与报告列完全一致。 / Encode the original workflow and require its domain extraction to match report columns exactly.
pub fn encode_csp1d_execution_report(
    source: &ModelArtifact,
    configuration: Csp1dOptions,
    result: &domain::Csp1dColumnGenerationResult<f64>,
    records: &[domain::Csp1dStageReport],
    stop: Csp1dExecutionStop,
) -> Result<String> {
    configuration.config()?;
    let (problem, _) = decode_csp1d_problem(source)?;
    let artifact = Csp1dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration,
        business_status: match result.solution.status { domain::Csp1dSolutionStatus::Feasible => Csp1dBusinessStatus::Feasible,
            domain::Csp1dSolutionStatus::Partial => Csp1dBusinessStatus::Partial, domain::Csp1dSolutionStatus::NoInitialPlans => Csp1dBusinessStatus::NoInitialPlans,
            domain::Csp1dSolutionStatus::Failed => Csp1dBusinessStatus::Failed }, stop,
        plans: result.solution.generated_plans.iter().map(|plan| encode_csp1d_pattern(plan, problem.length_unit)).collect::<Result<_>>()?,
        stages: records.iter().map(encode_csp1d_stage_report).collect::<Result<_>>()?, trace_json: serde_json::to_string(&result.trace).map_err(invalid)? };
    let text = serde_json::to_string(&artifact).map_err(invalid)?;
    let verified = decode_csp1d_execution_report(&text, source)?;
    let expected = result.solution.produce.cutting_plans.iter().map(|usage| Ok(Csp1dUsage {
        plan: encode_csp1d_pattern(&usage.plan, problem.length_unit)?, count: usage.amount.to_string(),
    })).collect::<Result<Vec<_>>>()?;
    if verified.production.as_ref().map_or(!expected.is_empty(), |production| production.usages != expected) {
        return Err(invalid("CSP1D original domain extraction disagrees with the actual integer report"));
    }
    Ok(text)
}

/// 复验全部真实阶段与 trace；只有最终整数子报告可产生源绑定生产。
/// Revalidate every actual stage and trace; only the final integer child can produce source-bound production.
pub fn decode_csp1d_execution_report(text: &str, source: &ModelArtifact) -> Result<Csp1dExecutionReport> {
    if text.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "CSP1D execution report exceeds 8 MiB")); }
    let artifact: Csp1dExecutionReportArtifact = serde_json::from_str(text).map_err(invalid)?;
    if artifact.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported CSP1D execution-report schema")); }
    let (problem, domain) = decode_csp1d_problem(source)?;
    if artifact.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "CSP1D workflow belongs to a different source problem")); }
    let config = artifact.configuration.config()?;
    let trace: domain::Csp1dColumnGenerationTrace = serde_json::from_str(&artifact.trace_json).map_err(invalid)?;
    if serde_json::to_string(&trace).map_err(invalid)? != artifact.trace_json || artifact.plans.len() > artifact.configuration.pool_limit.unwrap_or(512) as usize
        || artifact.stages.len() > 3 * config.iteration_limit as usize + 1 || trace.iterations.len() > config.iteration_limit as usize
        || trace.final_plan_count != artifact.plans.len() as u64 || trace.initial_plan_count > config.max_initial_plans
        || trace.initial_plan_count > trace.final_plan_count || trace.priced_plan_count.len() != trace.iterations.len() {
        return Err(invalid("invalid CSP1D trace shape, noncanonical trace or exceeded workflow budget"));
    }
    let mut ids = BTreeSet::new(); let mut keys = BTreeSet::new();
    for plan in &artifact.plans {
        let plan = decode_csp1d_pattern(&domain, plan, problem.length_unit)?;
        if !ids.insert(plan.id.clone()) || !keys.insert(plan.canonical_key()) { return Err(invalid("duplicate CSP1D workflow plan")); }
    }
    let mut pool_count = trace.initial_plan_count;
    for (iteration, record) in trace.iterations.iter().enumerate() {
        if record.iteration != iteration as i64 || !record.lp_objective.is_finite() || record.plan_count_before < 0 || record.plan_count_after < 0
            || record.plan_count_before as u64 != pool_count || record.priced_plan_count > config.max_pricing_plans
            || record.priced_plan_count != trace.priced_plan_count[iteration] || record.plan_count_after as u64 != pool_count + record.priced_plan_count {
            return Err(invalid("inconsistent CSP1D pricing trace"));
        }
        pool_count += record.priced_plan_count;
    }
    if pool_count != trace.final_plan_count { return Err(invalid("CSP1D final pool disagrees with the pricing trace")); }
    let mut integer_index = None;
    for (index, stage) in artifact.stages.iter().enumerate() {
        decode_csp1d_stage_report(stage)?;
        if stage.stage != Csp1dStageKind::Lp {
            if integer_index.is_some() || index + 1 != artifact.stages.len()
                || stage.stage != if artifact.configuration.algorithm == Csp1dAlgorithm::Milp { Csp1dStageKind::Milp } else { Csp1dStageKind::FinalMilp } {
                return Err(invalid("invalid CSP1D workflow integer-stage order"));
            }
            integer_index = Some(index);
        } else if artifact.configuration.algorithm == Csp1dAlgorithm::Milp { return Err(invalid("ordinary CSP1D MILP cannot contain LP stages")); }
    }
    let integer = if let Some(index) = integer_index {
        let integer_artifact = Csp1dProductionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
            plans: artifact.plans.clone(), child: artifact.stages[index].clone() };
        Some(decode_csp1d_production_report(&serde_json::to_string(&integer_artifact).map_err(invalid)?, source)?)
    } else { None };
    let production = integer.as_ref().and_then(|report| report.production.clone());
    let no_initial = artifact.business_status == Csp1dBusinessStatus::NoInitialPlans;
    if no_initial != artifact.plans.is_empty() || (no_initial && (!artifact.stages.is_empty() || trace.final_milp_status != domain::Csp1dFinalMilpStatus::NotAttempted
        || trace.termination_reason != domain::Csp1dTerminationReason::NoInitialPlans))
        || (production.is_some() != (artifact.business_status == Csp1dBusinessStatus::Feasible))
        || (production.is_some() != (trace.final_milp_status == domain::Csp1dFinalMilpStatus::Solved))
        || trace.partial_solution_available || artifact.business_status == Csp1dBusinessStatus::Partial {
        return Err(invalid("CSP1D workflow business status or production availability disagrees with its actual integer child"));
    }
    if let Some(child) = artifact.stages.last() {
        let (_, report) = decode_csp1d_stage_report(child)?;
        use ospf_rust_core::solver::TerminationReason;
        let required_stop = match report.termination_reason { TerminationReason::Cancelled | TerminationReason::Interrupted => Some(Csp1dExecutionStop::Cancelled),
            TerminationReason::TimeLimit => Some(Csp1dExecutionStop::TimeLimit), _ => None };
        if required_stop.is_some_and(|stop| artifact.stop != stop) { return Err(invalid("CSP1D workflow stop conflicts with the actual terminal child")); }
    }
    let summary = Csp1dExecutionSummary { algorithm: artifact.configuration.algorithm, business_status: artifact.business_status, stop: artifact.stop,
        candidate_complete: false, column_count: artifact.plans.len() as u32, stage_count: artifact.stages.len() as u32,
        iteration_count: trace.iterations.len() as u32, integer: integer.as_ref().map(|report| report.summary.clone()) };
    Ok(Csp1dExecutionReport { artifact, summary, integer, production })
}
