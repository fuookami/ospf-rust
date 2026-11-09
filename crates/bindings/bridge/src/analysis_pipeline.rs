//! 候选漏斗的只读协议：只委托 core 的 `build_candidate_funnel`，不重算分层。
//! Read-only candidate-funnel protocol: delegates to core's `build_candidate_funnel` without recomputing tiers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::analysis::{
    ActivityStatus as CoreActivityStatus, CandidateFunnelConfig, CandidateFunnelRanking, CandidateTier,
    ConstraintActivityReport, build_candidate_funnel,
};
use crate::{ActivityStatus, BridgeError, Result};

/// 线性解池选项；请求数量必须 ≥2，因为原 core 对 ≤1 视为"未请求多解"。
/// Linear solution-pool options; the requested amount must be ≥2 because the original core treats ≤1 as "no pool".
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearSolutionPoolOptions {
    /// 期望解数量，2..=256。 / Requested solution count, 2..=256.
    pub amount: u32,
}

impl LinearSolutionPoolOptions {
    /// 上界与后端能力无关，只限制请求规模。 / The bound is independent of backend capability and only limits request size.
    pub const MAX_AMOUNT: u32 = 256;

    /// 校验请求数量。 / Validate the requested amount.
    pub fn validate(&self) -> Result<()> {
        if self.amount < 2 || self.amount > Self::MAX_AMOUNT {
            return Err(crate::invalid("linear solution pool amount must be between 2 and 256"));
        }
        Ok(())
    }
}

/// 线性解池报告：解向量按**稳定变量身份**给出，不依赖 solver 列号。
/// Linear solution-pool report: solution vectors are given against **stable variable identities**, not solver columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearSolutionPoolReport {
    /// 模型指纹，报告只对自己的模型有效。 / Model fingerprint; the report is only valid for its own model.
    pub model_fingerprint: String,
    /// 按解向量顺序排列的稳定变量身份。 / Stable variable identities in solution-vector order.
    pub variable_ids: Vec<String>,
    /// 每个解一个完整变量向量。 / One complete variable vector per solution.
    pub solutions: Vec<Vec<f64>>,
    ///  incumbent 目标值（若有）。 / Incumbent objective value when the backend provides one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub best_objective: Option<f64>,
}

/// IIS 算法选择，委托原 core 的两种过滤算法。 / IIS algorithm selection, delegating to the original core filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LinearIisAlgorithm {
    /// 弹性过滤，core 默认。 / Elastic filtering, the core default.
    Elastic,
    /// 删除过滤。 / Deletion filtering.
    Deletion,
}

/// IIS 计算选项；所有字段缺省即沿用 core 默认。 / IIS options; every default defers to the original core defaults.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearIisOptions {
    /// 默认弹性过滤。 / Defaults to elastic filtering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub algorithm: Option<LinearIisAlgorithm>,
    /// 是否把变量边界纳入 IIS，默认沿用 core。 / Whether variable bounds join the IIS; defaults to core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub include_bounds: Option<bool>,
    /// 最大迭代次数，必须为正。 / Maximum iterations; must be positive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_iterations: Option<u32>,
}

/// 不可约不可行子系统（IIS）结果，全部按**稳定身份**给出，不使用 solver 行号。
/// Irreducible infeasible subsystem (IIS) result, always in **stable identities** rather than solver row numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearIisReport {
    /// 实际使用的算法。 / Algorithm actually used.
    pub algorithm: LinearIisAlgorithm,
    /// IIS 内的稳定约束身份，按原模型顺序。 / Stable constraint identities inside the IIS, in original model order.
    pub constraint_ids: Vec<String>,
    /// IIS 内取到下界的稳定变量身份。 / Stable variables whose lower bound is inside the IIS.
    pub lower_bound_variables: Vec<String>,
    /// IIS 内取到上界的稳定变量身份。 / Stable variables whose upper bound is inside the IIS.
    pub upper_bound_variables: Vec<String>,
    /// 原模型约束总数。 / Original model constraint count.
    pub original_constraint_count: u32,
    /// 原模型变量总数。 / Original model variable count.
    pub original_variable_count: u32,
}

/// 漏斗配置，默认对偶阈值 1e-9、短名单上限 32。 / Funnel configuration; defaults are a 1e-9 dual threshold and a 32-candidate shortlist.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisFunnelOptions {
    /// |dual| 超过该值才算强候选；有限且非负，默认 1e-9。 / A dual above this value marks a strong candidate; finite and
    /// nonnegative, default 1e-9.
    #[ts(optional)]
    pub dual_threshold: Option<f64>,
    /// 短名单期望上限，1..=1024，默认 32。 / Requested shortlist limit, 1..=1024, default 32.
    #[ts(optional)]
    pub candidate_limit: Option<u32>,
}

impl AnalysisFunnelOptions {
    /// 校验并套用 core 默认值。 / Validate and apply the core defaults.
    pub fn config(&self) -> Result<CandidateFunnelConfig> {
        let defaults = CandidateFunnelConfig::default();
        let config = CandidateFunnelConfig {
            dual_threshold: self.dual_threshold.unwrap_or(defaults.dual_threshold),
            default_candidate_limit: self.candidate_limit.map_or(defaults.default_candidate_limit, |value| value as usize),
        };
        if let Some(limit) = self.candidate_limit {
            if limit == 0 || limit > 1024 { return Err(BridgeError::new("InvalidInput", "candidate limit must be in [1, 1024]")); }
        }
        config.validate().map_err(|error| BridgeError::new("InvalidInput", error.to_string()))?;
        Ok(config)
    }
}

/// 候选分层，取值由 core 的 `CandidateTier` 决定。 / Candidate tier, taken from core's `CandidateTier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisCandidateTier {
    /// active 且 |dual| 超过阈值。 / Active with a dual above the threshold.
    TierA,
    /// active 但对偶约为 0。 / Active with an approximately zero dual.
    TierB,
    /// 非 active。 / Not active.
    TierC,
    /// 证据不足。 / Insufficient evidence.
    Unclassified,
}

/// 单个候选。 / One candidate.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisCandidate {
    /// 原始稳定约束身份。 / Stable original-constraint identity.
    pub constraint_id: String,
    /// 约束组。 / Constraint group.
    pub group: Option<String>,
    /// 分层。 / Tier.
    pub tier: AnalysisCandidateTier,
    /// 活动性状态。 / Activity status.
    pub activity: ActivityStatus,
    /// signed slack。 / Signed slack.
    pub slack: Option<f64>,
    /// 固定整数 LP 对偶值。 / Fixed-integer LP dual value.
    pub dual_value: Option<f64>,
    /// 局部有效性。 / Local effectiveness.
    pub local_effective: Option<bool>,
    /// 排序位置，0 最高。 / Rank position, 0 being highest.
    pub priority: u32,
}

/// 漏斗排名。 / Funnel ranking.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisCandidateFunnel {
    /// 报告 schema。 / Report schema.
    pub schema_version: String,
    /// 实际使用的对偶阈值。 / Dual threshold actually used.
    pub dual_threshold: f64,
    /// 全部候选，按优先级排序。 / Every candidate, ordered by priority.
    pub candidates: Vec<AnalysisCandidate>,
    /// Tier A 数量。 / Tier A count.
    pub tier_a: u32,
    /// Tier B 数量。 / Tier B count.
    pub tier_b: u32,
    /// Tier C 数量。 / Tier C count.
    pub tier_c: u32,
    /// 无法分层的数量。 / Unclassified count.
    pub unclassified: u32,
    /// 短名单：优先 A/B，名额未满时按优先级补 C/Unclassified，因此无对偶时仍是"按活动性排序的前 N 条"。
    /// Shortlist: A/B first, then C/Unclassified in priority order while slots remain, so without duals it is still
    /// the top N by activity.
    pub shortlist: Vec<String>,
    /// 本次是否带有对偶证据。 / Whether dual evidence was available.
    pub has_dual_evidence: bool,
}

/// 由活动性报告构建候选漏斗；对偶证据恒为缺失，因此只给出 Tier C 与 Unclassified。
/// Build the candidate funnel from an activity report; dual evidence is always absent here, so only Tier C and
/// Unclassified can be assigned.
/// 在 Rust 内对真实线性模型计算 IIS，并把 solver 索引换回稳定身份；
/// 不暴露墙上时钟耗时，避免同一输入的输出随机器性能漂移。
/// Compute the IIS inside Rust over a real linear model and map solver indices back to stable identities; wall-clock
/// timing is deliberately not exposed so identical inputs never drift with machine speed.
pub fn compute_linear_iis(artifact: &crate::ModelArtifact, options: &LinearIisOptions, time_limit: Option<std::time::Duration>) -> Result<LinearIisReport> {
    let model = crate::decode_model(artifact)?;
    let algorithm = options.algorithm.unwrap_or(LinearIisAlgorithm::Elastic);
    let mut config = ospf_rust_core::solver::iis::IISConfig { algorithm: match algorithm {
        LinearIisAlgorithm::Elastic => ospf_rust_core::solver::iis::IISAlgorithm::ElasticFiltering,
        LinearIisAlgorithm::Deletion => ospf_rust_core::solver::iis::IISAlgorithm::DeletionFiltering },
        ..Default::default() };
    if let Some(include_bounds) = options.include_bounds { config.include_bounds = include_bounds; }
    if let Some(max_iterations) = options.max_iterations {
        config.max_iterations = usize::try_from(max_iterations).map_err(|_| BridgeError::new("InvalidInput", "IIS max iterations overflow"))?;
    }
    if config.max_iterations == 0 { return Err(crate::invalid("IIS max iterations must be positive")); }
    // 共享预算直接进原 core 的时间限制；不额外伪造可中断性。
    // The shared budget goes straight into core's time limit without faking interruptibility.
    config.time_limit = time_limit;
    let iis = ospf_rust_core::solver::iis::compute_iis(&model, &config).map_err(|error| BridgeError::new("ExecutionFailure", error.to_string()))?;
    let mapping = ospf_rust_core::solver::linear_model_mapping(&model).map_err(crate::invalid)?;
    let constraints = |indices: &[usize]| indices.iter().map(|index| mapping.constraints_by_row.get(*index).cloned()
        .ok_or_else(|| BridgeError::new("ExecutionFailure", "IIS names a constraint row outside the mapping"))).collect::<Result<Vec<_>>>();
    let variables = |indices: &[usize]| indices.iter().map(|index| mapping.variables_by_column.get(*index).map(|id| id.0.clone())
        .ok_or_else(|| BridgeError::new("ExecutionFailure", "IIS names a variable column outside the mapping"))).collect::<Result<Vec<_>>>();
    let mut constraint_ids = constraints(&iis.constraint_indices)?;
    // core 给出的是无序集合语义，这里按原模型顺序稳定输出以便复现。
    // Core exposes set semantics; emit in original model order so results reproduce.
    constraint_ids.sort_by_key(|id| mapping.rows_by_constraint.get(id).copied().unwrap_or(usize::MAX));
    Ok(LinearIisReport { algorithm, constraint_ids,
        lower_bound_variables: variables(&iis.lower_bound_indices)?, upper_bound_variables: variables(&iis.upper_bound_indices)?,
        original_constraint_count: u32::try_from(iis.original_num_constraints).map_err(|_| BridgeError::new("ExecutionFailure", "constraint count overflow"))?,
        original_variable_count: u32::try_from(iis.original_num_variables).map_err(|_| BridgeError::new("ExecutionFailure", "variable count overflow"))? })
}

pub fn build_analysis_candidate_funnel(activity: &ConstraintActivityReport, options: &AnalysisFunnelOptions) -> Result<AnalysisCandidateFunnel> {
    let config = options.config()?;
    activity.validate().map_err(|error| BridgeError::new("InvalidInput", error.to_string()))?;
    let ranking: CandidateFunnelRanking = build_candidate_funnel(activity, None, &config)
        .map_err(|error| BridgeError::new("InvalidInput", error.to_string()))?;
    let shortlist = ranking.shortlist(config.default_candidate_limit).iter().map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>();
    Ok(AnalysisCandidateFunnel {
        schema_version: ranking.schema_version,
        dual_threshold: ranking.dual_threshold,
        candidates: ranking.candidates.iter().map(|candidate| Ok(AnalysisCandidate {
            constraint_id: candidate.constraint_id.0.clone(), group: candidate.group.clone(), tier: match candidate.tier {
                CandidateTier::TierA => AnalysisCandidateTier::TierA, CandidateTier::TierB => AnalysisCandidateTier::TierB,
                CandidateTier::TierC => AnalysisCandidateTier::TierC, CandidateTier::Unclassified => AnalysisCandidateTier::Unclassified },
            activity: candidate.activity.into(), slack: candidate.slack, dual_value: candidate.dual_value,
            local_effective: candidate.local_effective, priority: u32::try_from(candidate.priority).map_err(|_| BridgeError::new("ExecutionFailure", "candidate priority overflow"))?,
        })).collect::<Result<Vec<_>>>()?,
        tier_a: u32::try_from(ranking.tier_a).map_err(|_| BridgeError::new("ExecutionFailure", "tier count overflow"))?,
        tier_b: u32::try_from(ranking.tier_b).map_err(|_| BridgeError::new("ExecutionFailure", "tier count overflow"))?,
        tier_c: u32::try_from(ranking.tier_c).map_err(|_| BridgeError::new("ExecutionFailure", "tier count overflow"))?,
        unclassified: u32::try_from(ranking.unclassified).map_err(|_| BridgeError::new("ExecutionFailure", "tier count overflow"))?,
        shortlist, has_dual_evidence: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use ospf_rust_core::analysis::{ActivityEvidence, ActivityGroupSummary, ActivitySummary, ConstraintActivity};

    fn record(id: &str, group: Option<&str>, status: CoreActivityStatus) -> ConstraintActivity {
        ConstraintActivity { constraint_id: id.into(), group: group.map(str::to_owned), status,
            lhs: Some(1.0), rhs: Some(1.0), slack: Some(0.0), normalized_slack: Some(0.0), tolerance: Some(1e-7),
            satisfied: Some(true),
            evidence: ActivityEvidence::LinearConstraint { lhs: 1.0, rhs: 1.0 } }
    }

    fn activity() -> ConstraintActivityReport {
        let records = vec![record("c1", Some("g"), CoreActivityStatus::Active), record("c2", None, CoreActivityStatus::Inactive),
            record("c3", Some("g"), CoreActivityStatus::Active)];
        let mut summary = ActivitySummary::default();
        let mut grouped = BTreeMap::<Option<String>, ActivitySummary>::new();
        for item in &records { summary.add(item.status); grouped.entry(item.group.clone()).or_default().add(item.status); }
        let groups = grouped.into_iter().map(|(group, summary)| ActivityGroupSummary { group, summary }).collect();
        ConstraintActivityReport { schema_version: "1.0".to_owned(), constraints: records, variable_bounds: Vec::new(),
            summary, variable_bound_summary: ActivitySummary::default(), groups, tolerance: 1e-7, nearly_active_tolerance: 1e-5 }
    }

    /// IIS 必须由真实不可行模型算出，身份换回稳定 ID；可行模型不得凭空给出子系统。
    /// The IIS must come from a real infeasible model with stable identities, and a feasible model must never yield a
    /// fabricated subsystem.
    #[test]
    fn linear_iis_names_real_stable_identities_of_an_infeasible_model() {
        use ospf_rust_framework::solver::remote::{SerializedConstraint, SerializedConstraintCell, SerializedConstraintSign, SerializedVariable};
        let build = |rhs_loose: f64| {
            let mut input = crate::SerializedLinearModel::empty("iis-probe");
            input.variables.push(SerializedVariable { stable_id: "x".into(), index: 0, name: "x".into(),
                lower_bound: 0.0, upper_bound: 10.0, variable_type: crate::SerializedVariableType::Continuous });
            // x >= 5 与 x <= 1 互相矛盾，两条都必须出现在 IIS 里。
            // `x >= 5` and `x <= 1` contradict each other, so both must appear in the IIS.
            input.constraints.push(SerializedConstraint { stable_id: "at_least_five".into(), name: "at_least_five".into(), rhs: 5.0,
                sign: SerializedConstraintSign::GreaterEqual,
                cells: vec![SerializedConstraintCell { row_index: 0, col_index: 0, coefficient: 1.0 }] });
            input.constraints.push(SerializedConstraint { stable_id: "at_most_one".into(), name: "at_most_one".into(), rhs: rhs_loose,
                sign: SerializedConstraintSign::LessEqual,
                cells: vec![SerializedConstraintCell { row_index: 1, col_index: 0, coefficient: 1.0 }] });
            input
        };
        let artifact = crate::encode_model(&crate::compile_linear(&build(1.0)).unwrap()).unwrap();
        let options = LinearIisOptions { algorithm: None, include_bounds: None, max_iterations: None };
        // 迭代次数为零必须拒绝，这一条与后端无关。 / Zero iterations must be rejected regardless of the backend.
        assert_eq!(compute_linear_iis(&artifact, &LinearIisOptions { algorithm: None, include_bounds: None, max_iterations: Some(0) }, None).unwrap_err().code, "InvalidInput");
        // 无后端编译时必须给出清晰的能力错误，而不是 panic 或空子系统。
        // Without a compiled backend the call must fail with a clear capability error, not a panic or an empty subsystem.
        #[cfg(not(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12")))]
        {
            let error = compute_linear_iis(&artifact, &options, None).unwrap_err();
            assert!(matches!(error.code.as_str(), "ExecutionFailure" | "Unsupported"), "unexpected error: {error:?}");
            return;
        }
        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
        let report = compute_linear_iis(&artifact, &options, None).unwrap();
        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
        {
        assert_eq!(report.algorithm, LinearIisAlgorithm::Elastic);
        assert_eq!(report.original_constraint_count, 2);
        assert_eq!(report.original_variable_count, 1);
        // 稳定身份是原模型的长编码形式（与活动性分析同一格式），按原模型顺序输出。
        // Stable identities use the original model's long encoded form (the same format as the activity analysis) and
        // are emitted in original model order.
        assert_eq!(report.constraint_ids.len(), 2);
        assert!(report.constraint_ids[0].contains("at_least_five"), "got {:?}", report.constraint_ids);
        assert!(report.constraint_ids[1].contains("at_most_one"), "got {:?}", report.constraint_ids);
        // 删除过滤给出同一子系统，且不暴露耗时字段。 / Deletion filtering yields the same subsystem and no timing field.
        let deletion = compute_linear_iis(&artifact, &LinearIisOptions { algorithm: Some(LinearIisAlgorithm::Deletion),
            include_bounds: None, max_iterations: None }, None).unwrap();
        assert_eq!(deletion.constraint_ids, report.constraint_ids);
        assert_eq!(deletion.algorithm, LinearIisAlgorithm::Deletion);
        }
    }

    #[test]
    fn funnel_without_duals_never_claims_a_shortlist() {
        let report = activity();
        let funnel = build_analysis_candidate_funnel(&report, &AnalysisFunnelOptions { dual_threshold: None, candidate_limit: None }).unwrap();
        // 无对偶证据时不允许出现 A/B，短名单必须为空。
        // Without dual evidence no A/B tier may appear and the shortlist must stay empty.
        assert_eq!((funnel.tier_a, funnel.tier_b), (0, 0));
        assert!(funnel.candidates.iter().all(|candidate| matches!(candidate.tier, AnalysisCandidateTier::TierC | AnalysisCandidateTier::Unclassified)));
        // 无对偶时短名单仍是活动性排序的前 N 条（不是空表），但其中不允许出现 A/B。
        // Without duals the shortlist is still the top N by activity rather than empty, yet it must not contain A/B.
        assert_eq!(funnel.shortlist.len(), 3);
        for id in &funnel.shortlist {
            let candidate = funnel.candidates.iter().find(|candidate| &candidate.constraint_id == id).unwrap();
            assert!(matches!(candidate.tier, AnalysisCandidateTier::TierC | AnalysisCandidateTier::Unclassified));
        }
        let limited = build_analysis_candidate_funnel(&report, &AnalysisFunnelOptions { dual_threshold: None, candidate_limit: Some(2) }).unwrap();
        assert_eq!(limited.shortlist.len(), 2);
        assert!(!funnel.has_dual_evidence);
        assert_eq!(funnel.candidates.len(), 3);
        // 优先级必须是 0..n 的排列，不重复 / Priorities must be a permutation of 0..n without duplicates.
        let mut priorities = funnel.candidates.iter().map(|candidate| candidate.priority).collect::<Vec<_>>();
        priorities.sort_unstable();
        assert_eq!(priorities, vec![0, 1, 2]);
        // 阈值与上限越界必须拒绝，不静默夹取 / Out-of-range threshold and limit must be rejected without silent clamping.
        assert_eq!(build_analysis_candidate_funnel(&report, &AnalysisFunnelOptions { dual_threshold: Some(-1.0), candidate_limit: None }).unwrap_err().code, "InvalidInput");
        assert_eq!(build_analysis_candidate_funnel(&report, &AnalysisFunnelOptions { dual_threshold: None, candidate_limit: Some(0) }).unwrap_err().code, "InvalidInput");
        assert_eq!(build_analysis_candidate_funnel(&report, &AnalysisFunnelOptions { dual_threshold: None, candidate_limit: Some(1025) }).unwrap_err().code, "InvalidInput");
        assert_eq!(build_analysis_candidate_funnel(&report, &AnalysisFunnelOptions { dual_threshold: Some(f64::NAN), candidate_limit: None }).unwrap_err().code, "InvalidInput");
    }
}
