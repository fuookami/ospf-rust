//! 将实际整数子报告、源方案列及物理产出绑定；不签发全模式空间证明。
//! Bind actual integer child reports to source plan columns and physical production, without full-pattern-space proofs.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::{MetaModel, intermediate::LinearTriadModel};
use ospf_rust_core::solver::{ProblemStatus, evaluate_linear_solution, require_infeasibility_certificate_for_model};
use ospf_rust_framework_csp1d as domain;
use domain::Csp1dModelContext;
use crate::{BridgeError, Csp1dPattern, Csp1dProduction, Csp1dStageKind, Csp1dStageReportArtifact, Csp1dUsage, ModelArtifact, Result,
    decode_csp1d_pattern, decode_csp1d_problem, decode_csp1d_stage_report, encode_csp1d_pattern, encode_csp1d_stage_report, invalid, verify_csp1d_usages};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const TOLERANCE: f64 = 1e-7;
const MAX_SAFE_COUNT: f64 = 9_007_199_254_740_991.0;

/// 原始整数阶段及完整列池；生产量只能由复验后的真实向量导出。
/// Original integer stage and complete column pool; production can only be derived from its revalidated actual vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dProductionReportArtifact {
    /// 独立产出报告协议版本。 / Independent production-report protocol version.
    pub schema_version: String,
    /// 外部源问题的 canonical 指纹。 / Canonical fingerprint of the external source problem.
    pub source_fingerprint: String,
    /// 实际整数模型的完整候选池，含零使用列。 / Complete candidate pool of the actual integer model, including unused columns.
    pub plans: Vec<Csp1dPattern>,
    /// 保留原始模型身份的实际整数子报告。 / Actual integer child report retaining its original model identity.
    pub child: Csp1dStageReportArtifact,
}

/// Rust 复验后派生的摘要，绝不接受载荷自称全局最优或全局不可行。
/// Rust-derived revalidated summary, never accepting payload claims of global optimality or infeasibility.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Csp1dProductionReportSummary {
    /// 源问题指纹。 / Source-problem fingerprint.
    pub source_fingerprint: String,
    /// 原始整数子模型指纹。 / Original integer-child fingerprint.
    pub child_model_fingerprint: String,
    /// 普通或最终整数阶段。 / Ordinary or final integer stage.
    pub stage: Csp1dStageKind,
    /// 原始子问题状态。 / Original child problem status.
    pub problem_status: String,
    /// 原始终止原因，包括限时和取消。 / Original termination reason, including timeout and cancellation.
    pub termination_reason: String,
    /// 完整整数候选池大小，不是模式空间大小。 / Complete integer candidate-pool size, not the pattern-space size.
    pub column_count: u32,
    /// 当前绑定始终 false。 / Always false for the current binding.
    pub candidate_complete: bool,
    /// 仅当前池内、由原始子报告携带的最优性语义。 / Original child report's optimality semantics over this pool only.
    pub restricted_optimal: bool,
    /// 仅当前池内且通过原 core 不可行证明门禁。 / Infeasibility over this pool only, gated by the original core certificate check.
    pub restricted_infeasible: bool,
    /// 是否从真实完整整数向量导出了源约束可行生产。 / Whether source-feasible production was derived from the actual full integer vector.
    pub has_production: bool,
    /// 子模型目标，不暗中加入目标常数。 / Child-model objective, without silently adding the objective constant.
    pub objective_value: Option<f64>,
}

/// 已将源模型、实际报告和离散生产共同复验的结果；仍不是来源认证。
/// Jointly revalidated source model, actual report and discrete production; still not provenance authentication.
#[derive(Debug, Clone)]
pub struct Csp1dProductionReport {
    /// 保留原始字节的子模型和报告封装。 / Child-model and report artifact retaining original inner bytes.
    pub artifact: Csp1dProductionReportArtifact,
    /// 只由验证器派生的摘要。 / Summary derived only by the validator.
    pub summary: Csp1dProductionReportSummary,
    /// 有 incumbent 时才有生产结果。 / Production exists only when there is an incumbent.
    pub production: Option<Csp1dProduction>,
}

fn columns(model: &LinearTriadModel) -> Result<BTreeMap<String, usize>> {
    let mut columns = BTreeMap::new();
    for (column, token) in model.variables.iter().enumerate() {
        if columns.insert(token.name().to_owned(), column).is_some() { return Err(invalid("duplicate CSP1D child column name")); }
    }
    Ok(columns)
}

fn compare_source_model(actual: &LinearTriadModel, expected: &LinearTriadModel) -> Result<Vec<usize>> {
    crate::source_model::compare_source_model(actual, expected)
}

/// 编码实际整数阶段和其完整源候选池；不把任意独立有效生产附到报告上。
/// Encode an actual integer stage and its complete source candidate pool, never attaching independently valid unrelated production.
pub fn encode_csp1d_production_report(
    source: &ModelArtifact,
    plans: &[domain::CuttingPlan<f64>],
    record: &domain::Csp1dStageReport,
) -> Result<String> {
    if plans.is_empty() || plans.len() > 512 || record.stage == domain::Csp1dSolveStage::Lp {
        return Err(invalid("CSP1D production report requires a bounded nonempty integer column pool"));
    }
    let (problem, _) = decode_csp1d_problem(source)?;
    let artifact = Csp1dProductionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        plans: plans.iter().map(|plan| encode_csp1d_pattern(plan, problem.length_unit)).collect::<Result<_>>()?, child: encode_csp1d_stage_report(record)? };
    let text = serde_json::to_string(&artifact).map_err(invalid)?;
    decode_csp1d_production_report(&text, source)?;
    Ok(text)
}

/// 复建受限原领域 context，对照实际列、完整行/目标/类型，再从真实报告向量导出精确生产。
/// Rebuild the restricted original-domain context, compare actual columns and complete rows/objective/types, then derive exact production from the actual report vector.
pub fn decode_csp1d_production_report(text: &str, source: &ModelArtifact) -> Result<Csp1dProductionReport> {
    if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "CSP1D production report exceeds 8 MiB")); }
    let artifact: Csp1dProductionReportArtifact = serde_json::from_str(text).map_err(invalid)?;
    if artifact.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported CSP1D production-report schema")); }
    let (problem, domain) = decode_csp1d_problem(source)?;
    if artifact.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "CSP1D production report belongs to a different source problem")); }
    if artifact.child.stage == Csp1dStageKind::Lp || artifact.plans.is_empty() || artifact.plans.len() > 512 { return Err(invalid("CSP1D production report requires a bounded nonempty integer column pool")); }
    let plans = artifact.plans.iter().map(|plan| decode_csp1d_pattern(&domain, plan, problem.length_unit)).collect::<Result<Vec<_>>>()?;
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    if plans.iter().any(|plan| !ids.insert(plan.id.clone()) || !keys.insert(plan.canonical_key())) { return Err(invalid("duplicate CSP1D pool identity or physical pattern")); }
    let (actual, report) = decode_csp1d_stage_report(&artifact.child)?;
    let mut builder = domain::Csp1dProduceContextBuilder::new(domain::ProduceInput { cutting_plans: plans.clone(), demands: domain.demands,
        materials: domain.materials, machines: domain.machines, warm_start_plan_usages: Vec::new() });
    builder.mode(domain::Csp1dModelingMode::MILP).is_final_milp(artifact.child.stage == Csp1dStageKind::FinalMilp);
    let mut context = builder.build().map_err(invalid)?;
    let mut model = MetaModel::new("csp1d_source_report_check");
    context.register(&mut model).map_err(invalid)?;
    let snapshot = model.linear_model_snapshot().map_err(invalid)?;
    let compiled = snapshot.compile_flt64().map_err(invalid)?;
    if actual.num_variables() != plans.len() || compiled.objective_constant != artifact.child.objective_constant { return Err(invalid("CSP1D child has unbound auxiliary columns or a different objective constant")); }
    let permutation = compare_source_model(&actual, &compiled.model)?;
    let production = if let Some(solution) = &report.solution {
        let rounded = solution.values.iter().map(|&value| {
            let integer = value.round();
            if !(0.0..=MAX_SAFE_COUNT).contains(&integer) || (integer - value).abs() > TOLERANCE { return Err(invalid("CSP1D child usage is not an exact supported integer")); }
            Ok(integer)
        }).collect::<Result<Vec<_>>>()?;
        let diagnostics = evaluate_linear_solution(&actual, &rounded, TOLERANCE).map_err(invalid)?;
        if diagnostics.constraint_evaluations.iter().any(|row| !row.satisfied) || diagnostics.variable_bound_evaluations.iter().any(|bound| !bound.satisfied) {
            return Err(invalid("rounded CSP1D child vector violates its complete actual model"));
        }
        let objective = ospf_rust_core::model::LinearObjectiveEvaluationFlt64::new(actual.c.iter().copied().zip(rounded.iter().copied()), 0.0).map_err(invalid)?;
        objective.validate_reported(solution.objective_value.or(solution.objective), TOLERANCE).map_err(invalid)?;
        if solution.objective.is_some() { objective.validate_reported(solution.objective, TOLERANCE).map_err(invalid)?; }
        let expected_columns = columns(&compiled.model)?;
        let usages = artifact.plans.iter().map(|plan| {
            let expected_column = expected_columns.get(&format!("csp1d_batch_{}", plan.id)).ok_or_else(|| invalid("source CSP1D plan lost its variable"))?;
            let amount = rounded[permutation[*expected_column]] as u64;
            Ok((amount > 0).then(|| Csp1dUsage { plan: plan.clone(), count: amount.to_string() }))
        }).collect::<Result<Vec<_>>>()?.into_iter().flatten().collect::<Vec<_>>();
        Some(verify_csp1d_usages(&problem, &usages)?)
    } else { None };
    let summary = Csp1dProductionReportSummary { source_fingerprint: source.fingerprint.clone(), child_model_fingerprint: artifact.child.model.model_fingerprint.value.clone(),
        stage: artifact.child.stage, problem_status: serde_json::to_value(report.problem_status).map_err(invalid)?.as_str().ok_or_else(|| invalid("invalid CSP1D child status"))?.into(),
        termination_reason: serde_json::to_value(report.termination_reason).map_err(invalid)?.as_str().ok_or_else(|| invalid("invalid CSP1D termination reason"))?.into(),
        column_count: plans.len() as u32, candidate_complete: false, restricted_optimal: report.is_optimal(),
        restricted_infeasible: report.problem_status == ProblemStatus::Infeasible
            && require_infeasibility_certificate_for_model(&report, &artifact.child.model.model_fingerprint).is_ok(),
        has_production: production.is_some(), objective_value: report.solution.as_ref().and_then(|solution| solution.objective_value.or(solution.objective)) };
    Ok(Csp1dProductionReport { artifact, summary, production })
}

#[cfg(test)]
mod tests;
