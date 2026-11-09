//! 批次 B 第 9 单：多目标报告与跨目标 criticality 校验 / Batch B, delivery 9: multi-target report and cross-target criticality.
//!
//! `MultiTargetAnalysisReport::validate(policy)` 是这一层最值钱的能力：它不只是"字段自洽"，而是**把跨目标结论重算一遍
//! 再比对**——
//!
//! * 各 result 的 target 身份必须与各自的 feasibility / conflict / improvement plan 完全一致；
//! * 没有已验证冲突就不许带 correction 建议；`correctionSets` 与 `improvementPlans` 必须等长、逐项相等、rank 稠密；
//! * `criticalityProfile.observations` 必须由各 result 的（target, feasibility.status, 冲突约束 ID）**精确重建**；
//! * `criticalityProfile.classifications` 必须与 `build_criticality_profile` 的重算结果完全一致。
//!
//! 最后一条把 `criticality` 的分类规则也一起跨线了：某约束在**恰好一个**不可达目标里出现 → `LOCAL_BOTTLENECK`；
//! 在**多于一个**里出现 → `PERSISTENT_BOTTLENECK`；在所有（多于一个）不可达目标里都出现 → `STRUCTURAL_BOTTLENECK`。
//!
//! `MultiTargetAnalysisReport::validate(policy)` is this layer's most valuable capability: it does not merely check field
//! consistency, it **recomputes the cross-target conclusions and compares**. Every result's target identity must match its
//! own feasibility / conflict / improvement plan; correction recommendations require a verified conflict, with equal-length,
//! item-equal, densely ranked plans; the criticality observations must be exactly rebuilt from the results; and the
//! classifications must match `build_criticality_profile`'s recomputation. That last rule brings criticality's
//! classification rules across the wire too.

use std::collections::BTreeSet;

use ospf_rust_core::analysis::correction::RelaxabilityPolicy;
use ospf_rust_core::analysis::criticality::{CriticalityKind, CriticalityProfile};
use ospf_rust_core::analysis::multi_target::MultiTargetAnalysisReport;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::analysis_correction::RelaxationPolicyInput;
use crate::{BridgeError, Result};

/// 一个目标的结果投影。 / One target's result projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultiTargetResultRow {
    /// 目标身份。 / The objective identity.
    pub objective_id: String,
    /// 方向（`AtLeast`/`AtMost`）。 / The direction.
    pub relation: String,
    /// 目标值。 / The target value.
    pub value: f64,
    /// 稳定 target 身份。 / The stable target identity.
    pub stable_id: String,
    /// 可行性状态。 / The feasibility status.
    pub status: String,
    /// 三值可达性。 / The ternary reachability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reachable: Option<bool>,
    /// 是否含已验证的不可约冲突。 / Whether it carries a verified irreducible conflict.
    pub has_verified_conflict: bool,
    /// 冲突的约束稳定 ID。 / The conflict's stable constraint IDs.
    pub blocking_constraint_ids: Vec<String>,
    /// correction 建议数。 / The correction-recommendation count.
    pub correction_set_count: u32,
    /// 改进方案数。 / The improvement-plan count.
    pub plan_count: u32,
    /// 最便宜建议的带权成本。 / The weighted cost of the cheapest recommendation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cheapest_plan_cost: Option<f64>,
}

/// 一类瓶颈的约束集合。 / The constraint set of one bottleneck kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BottleneckKindRow {
    /// 类别名（`LOCAL_BOTTLENECK`/`PERSISTENT_BOTTLENECK`/`STRUCTURAL_BOTTLENECK`）。
    /// The kind name.
    pub kind: String,
    /// 该类别下的约束稳定 ID。 / The stable constraint IDs in this kind.
    pub constraint_ids: Vec<String>,
}

/// 多目标报告的校验结论与跨目标摘要。 / The multi-target verdict and cross-target summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultiTargetReportSummary {
    /// 报告 schema。 / The report schema.
    pub schema_version: String,
    /// 结果条数。 / The result count.
    pub result_count: u32,
    /// 含已验证冲突的结果数。 / The number of results with a verified conflict.
    pub verified_conflict_count: u32,
    /// 全部改进方案数。 / The total improvement-plan count.
    pub plan_count: u32,
    /// 已证明不可达的目标数（`proven_target_count`）。 / The number of proven-unreachable targets.
    pub proven_target_count: u32,
    /// 被至少一个已验证冲突点名的约束数。 / The number of constraints named by at least one verified conflict.
    pub classified_constraint_count: u32,
    /// 逐目标结果。 / The per-target results.
    pub results: Vec<MultiTargetResultRow>,
    /// 三类瓶颈的约束集合（`LOCAL`/`PERSISTENT`/`STRUCTURAL`，固定顺序）。
    /// The three bottleneck kinds in a fixed order.
    pub bottlenecks: Vec<BottleneckKindRow>,
}

/// 校验一份多目标报告（含跨目标 criticality 重算核对）并给出摘要。非法输入是**错误**，原因原样带出。
/// Verify a multi-target report (including the cross-target criticality recomputation) and summarize it. An illegal input
/// is an **error** carrying the reason verbatim.
pub fn verify_multi_target_report(
    json: &str,
    policy: Option<&RelaxationPolicyInput>,
) -> Result<MultiTargetReportSummary> {
    if json.len() > 32 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the multi-target report exceeds 32 MiB"));
    }
    let policy = match policy {
        None => RelaxabilityPolicy::default(),
        Some(input) => RelaxabilityPolicy {
            max_candidates: usize::try_from(input.max_candidates).unwrap_or(usize::MAX),
            max_plans: usize::try_from(input.max_plans).unwrap_or(usize::MAX),
            require_positive_weight: input.require_positive_weight,
        },
    };
    let report: MultiTargetAnalysisReport = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("multi-target report is not decodable: {error}"))
    })?;
    report.validate(&policy).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("multi-target report failed validation: {error}"))
    })?;
    let profile = &report.criticality_profile;
    Ok(MultiTargetReportSummary {
        schema_version: report.schema_version.clone(),
        result_count: u32::try_from(report.results.len()).unwrap_or(u32::MAX),
        verified_conflict_count: u32::try_from(report.verified_conflicts().len()).unwrap_or(u32::MAX),
        plan_count: u32::try_from(report.improvement_plans().len()).unwrap_or(u32::MAX),
        proven_target_count: u32::try_from(profile.proven_target_count()).unwrap_or(u32::MAX),
        classified_constraint_count: u32::try_from(profile.classifications.len()).unwrap_or(u32::MAX),
        results: report.results.iter().map(|result| MultiTargetResultRow {
            objective_id: result.target.objective_id().as_str().to_owned(),
            relation: format!("{:?}", result.target.relation()),
            value: result.target.value(),
            stable_id: result.target.stable_id(),
            status: status_name(result.feasibility.status).to_owned(),
            reachable: result.feasibility.reachable(),
            has_verified_conflict: result.has_verified_conflict(),
            blocking_constraint_ids: if result.has_verified_conflict() {
                result.conflict.constraint_ids().into_iter().map(|id| id.0).collect()
            } else {
                Vec::new()
            },
            correction_set_count: u32::try_from(result.correction_sets.len()).unwrap_or(u32::MAX),
            plan_count: u32::try_from(result.improvement_plans.len()).unwrap_or(u32::MAX),
            cheapest_plan_cost: result
                .improvement_plans
                .iter()
                .map(|plan| plan.correction_set.total_cost)
                .min_by(f64::total_cmp),
        }).collect(),
        // 三类瓶颈的集合直接来自领域的 `constraints(kind)`，顺序固定，方便调用方按位置读。
        // The three kind sets come straight from the domain's `constraints(kind)` in a fixed order.
        bottlenecks: [CriticalityKind::LocalBottleneck,
            CriticalityKind::PersistentBottleneck,
            CriticalityKind::StructuralBottleneck]
            .into_iter()
            .map(|kind| BottleneckKindRow {
                kind: kind_name(kind),
                constraint_ids: profile.constraints(kind).into_iter().map(|id| id.0).collect(),
            })
            .collect(),
    })
}

/// 瓶颈类别名，走领域的 serde 表示（`SCREAMING_SNAKE_CASE`）。
/// The bottleneck kind name, through the domain's serde representation.
fn kind_name(kind: CriticalityKind) -> String {
    serde_json::to_value(kind).ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{kind:?}"))
}

/// 分析状态名。 / The analysis status name.
fn status_name(status: ospf_rust_core::analysis::AnalysisStatus) -> &'static str {
    use ospf_rust_core::analysis::AnalysisStatus;
    match status {
        AnalysisStatus::Reachable => "reachable",
        AnalysisStatus::Unreachable => "unreachable",
        AnalysisStatus::Unknown => "unknown",
        AnalysisStatus::Unsupported => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::analysis::correction::{
        alternative_improvement_plans_from_conflict, weighted_correction_sets_from_conflict,
        CorrectionSet,
    };
    use ospf_rust_core::analysis::criticality::{build_criticality_profile, CriticalityObservation};
    use ospf_rust_core::analysis::multi_target::{MultiTargetAnalysisResult, MULTI_TARGET_ANALYSIS_REPORT_SCHEMA_VERSION};
    use ospf_rust_core::analysis::{
        AnalysisStatus, ConflictExplanation, ConflictExtractionTier, ConflictGroupSummary,
        ConflictMinimality, ConflictValidity, DiagnosticSource, ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};

    fn target(id: &str, value: f64) -> ObjectiveTarget {
        ObjectiveTarget::at_least(id, value).expect("target")
    }

    fn feasibility(target: &ObjectiveTarget, status: AnalysisStatus) -> TargetFeasibilityReport {
        TargetFeasibilityReport {
            schema_version: "1.0".to_owned(),
            target: target.clone(),
            effective_integer_bound: None,
            source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
            status,
            solution: None,
            exact_objective: None,
            objective_value: None,
            proof_status: ProofStatus::Verified,
            termination_reason: None,
            solve_time: None,
            target_fixed: true,
            message: None,
        }
    }

    /// 构造一个不可约、已验证、不可达的冲突。 / Build an irreducible, verified, unreachable conflict.
    fn conflict(target: &ObjectiveTarget, members: &[&str]) -> ConflictExplanation {
        ConflictExplanation {
            schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
            target: target.clone(),
            target_feasibility: feasibility(target, AnalysisStatus::Unreachable),
            status: AnalysisStatus::Unreachable,
            available_evidence: Vec::new(),
            fixed_background: Vec::new(),
            members: members.iter()
                .map(|id| DiagnosticSource::Constraint { id: StableConstraintId((*id).to_owned()) })
                .collect(),
            target_source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
            validity: ConflictValidity::Verified,
            minimality: ConflictMinimality::Irreducible,
            verifications: Vec::new(),
            group_summaries: vec![ConflictGroupSummary { group: None, count: members.len() }],
            resolve_count: 2,
            memoized_checks: 0,
            extraction_tier: ConflictExtractionTier::AssumptionExtraction,
            unavailable_reason: None,
            background_blocking: false,
        }
    }

    /// 一条完整结果：走领域的真实路径生成 correction 与 plan。
    /// One complete result, with corrections and plans produced through the real domain path.
    fn result(target: ObjectiveTarget, members: &[&str]) -> MultiTargetAnalysisResult {
        let conflict = conflict(&target, members);
        let policy = RelaxabilityPolicy::default();
        let costs = std::collections::BTreeMap::new();
        let correction_sets = weighted_correction_sets_from_conflict(&conflict, &costs, &policy)
            .expect("correction sets");
        let improvement_plans = alternative_improvement_plans_from_conflict(&conflict, &costs, &policy)
            .expect("improvement plans");
        MultiTargetAnalysisResult {
            feasibility: conflict.target_feasibility.clone(),
            target,
            conflict,
            correction_sets,
            improvement_plans,
        }
    }

    /// 一条没有冲突的结果（可行性未知，因此不得带任何建议）。
    /// A result without a conflict (unknown feasibility, so it must carry no recommendations).
    fn empty_result(target: ObjectiveTarget) -> MultiTargetAnalysisResult {
        let feasibility = feasibility(&target, AnalysisStatus::Unknown);
        let conflict = ConflictExplanation {
            schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
            target: target.clone(),
            target_feasibility: feasibility.clone(),
            status: AnalysisStatus::Unknown,
            available_evidence: Vec::new(),
            fixed_background: Vec::new(),
            members: Vec::new(),
            target_source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
            validity: ConflictValidity::Unknown,
            minimality: ConflictMinimality::NotChecked,
            verifications: Vec::new(),
            group_summaries: Vec::new(),
            resolve_count: 0,
            memoized_checks: 0,
            extraction_tier: ConflictExtractionTier::Unavailable,
            unavailable_reason: Some("no conflict analysis".to_owned()),
            background_blocking: false,
        };
        MultiTargetAnalysisResult {
            target,
            feasibility,
            conflict,
            correction_sets: Vec::<CorrectionSet>::new(),
            improvement_plans: Vec::new(),
        }
    }

    fn profile_for(results: &[MultiTargetAnalysisResult]) -> CriticalityProfile {
        build_criticality_profile(results.iter().map(|result| CriticalityObservation {
            target: result.target.clone(),
            status: result.feasibility.status,
            blocking_constraint_ids: if result.has_verified_conflict() {
                result.conflict.constraint_ids()
            } else {
                BTreeSet::new()
            },
        }).collect()).expect("criticality profile")
    }

    fn report() -> MultiTargetAnalysisReport {
        let results = vec![
            result(target("throughput", 10.0), &["capacity", "shift"]),
            result(target("cost", 5.0), &["capacity"]),
            empty_result(target("latency", 3.0)),
        ];
        let criticality_profile = profile_for(&results);
        MultiTargetAnalysisReport {
            schema_version: MULTI_TARGET_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
            results,
            criticality_profile,
        }
    }

    fn encode(report: &MultiTargetAnalysisReport) -> String {
        serde_json::to_string(report).expect("serialize multi-target report")
    }

    #[test]
    fn report_is_verified_and_cross_target_bottlenecks_are_recomputed() {
        let summary = verify_multi_target_report(&encode(&report()), None).unwrap();
        assert_eq!(summary.result_count, 3);
        assert_eq!(summary.verified_conflict_count, 2);
        assert_eq!(summary.plan_count, 3);
        // 两个目标被证明不可达，第三个状态未知。 / Two targets are proven unreachable, the third is unknown.
        assert_eq!(summary.proven_target_count, 2);
        assert_eq!(summary.classified_constraint_count, 2);
        assert_eq!(summary.results[0].objective_id, "throughput");
        assert_eq!(summary.results[0].relation, "AtLeast");
        assert_eq!(summary.results[0].status, "unreachable");
        assert_eq!(summary.results[0].reachable, Some(false));
        assert!(summary.results[0].has_verified_conflict);
        assert_eq!(summary.results[0].blocking_constraint_ids, vec!["capacity", "shift"]);
        assert_eq!(summary.results[0].correction_set_count, 2);
        assert_eq!(summary.results[0].plan_count, 2);
        assert_eq!(summary.results[0].cheapest_plan_cost, Some(1.0));
        // 第三个目标未知：没有冲突、没有建议、可达性为 `None`。
        // The third target is unknown: no conflict, no recommendations and `None` reachability.
        assert!(!summary.results[2].has_verified_conflict);
        assert_eq!(summary.results[2].status, "unknown");
        assert_eq!(summary.results[2].reachable, None);
        assert_eq!(summary.results[2].plan_count, 0);
        assert_eq!(summary.results[2].cheapest_plan_cost, None);

        // 分类规则实测：`capacity` 在两个不可达目标里都出现 → 持久瓶颈 **且**（因为不可达目标共 2 个）结构瓶颈；
        // `shift` 只在一个里出现 → 局部瓶颈。
        // Measured classification: `capacity` appears in both unreachable targets, so it is a persistent bottleneck and
        // (since there are two unreachable targets) a structural one; `shift` appears in only one, so it is local.
        assert_eq!(summary.bottlenecks[0].kind, "LOCAL_BOTTLENECK");
        assert_eq!(summary.bottlenecks[0].constraint_ids, vec!["shift"]);
        assert_eq!(summary.bottlenecks[1].kind, "PERSISTENT_BOTTLENECK");
        assert_eq!(summary.bottlenecks[1].constraint_ids, vec!["capacity"]);
        assert_eq!(summary.bottlenecks[2].kind, "STRUCTURAL_BOTTLENECK");
        assert_eq!(summary.bottlenecks[2].constraint_ids, vec!["capacity"]);
    }

    #[test]
    fn recomputed_criticality_evidence_is_enforced() {
        // 篡改分类：领域会把 observations 重算一遍，分类对不上就拒绝。
        // Tampering with a classification: the domain recomputes the observations, so a mismatch is refused.
        let mut broken = report();
        broken.criticality_profile.classifications
            .get_mut(&StableConstraintId("shift".to_owned()))
            .expect("shift classification")
            .insert(CriticalityKind::StructuralBottleneck);
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        // 实测：这一条先被 `CriticalityProfile::validate()` 的更早一道自洽校验挡住（分类必须由 observations 重算而来），
        // 而不是多目标报告那一层。层次顺序值得记下来。
        // Measured: this is caught earlier by `CriticalityProfile::validate()`'s own self-consistency check (the
        // classifications must be recomputed from the observations), not by the multi-target layer. The layering is worth
        // recording.
        assert!(error.message.contains("classifications do not match observations"), "{}", error.message);

        // 篡改观察：约束 ID 与结果里的冲突不一致。 / Tampering with an observation: its IDs disagree with the conflict.
        let mut broken = report();
        broken.criticality_profile.observations[0].blocking_constraint_ids =
            [StableConstraintId("ghost".to_owned())].into_iter().collect();
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        // 同样是先被 profile 自身的重算校验挡住（改了 observation，分类就对不上了）。
        // Again the profile's own recomputation check fires first: changing an observation invalidates its classifications.
        assert!(error.message.contains("classifications do not match observations"), "{}", error.message);

        // 重复 target：拒绝。 / A duplicate target: refused.
        let mut broken = report();
        let clone = broken.results[0].clone();
        broken.results.push(clone);
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert!(error.message.contains("duplicate targets"), "{}", error.message);

        // 空结果集：拒绝。 / An empty result set: refused.
        let mut broken = report();
        broken.results.clear();
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert!(error.message.contains("at least one result"), "{}", error.message);

        // 空白 schema：拒绝。 / A blank schema: refused.
        let mut broken = report();
        broken.schema_version = "  ".to_owned();
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert!(error.message.contains("schema version"), "{}", error.message);
    }

    #[test]
    fn recommendations_require_a_verified_conflict_and_dense_ranking() {
        // 没有已验证冲突却带建议：拒绝。 / Recommendations without a verified conflict: refused.
        let mut broken = report();
        let stolen = broken.results[0].correction_sets[0].clone();
        let plan = broken.results[0].improvement_plans[0].clone();
        broken.results[2].correction_sets.push(stolen);
        broken.results[2].improvement_plans.push(plan);
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert!(error.message.contains("require a verified irreducible conflict"), "{}", error.message);

        // 建议集合与方案不等长：拒绝。 / Unequal correction-set and plan counts: refused.
        let mut broken = report();
        broken.results[0].correction_sets.pop();
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert!(error.message.contains("same length"), "{}", error.message);

        // rank 不稠密：拒绝。 / A non-dense rank: refused.
        let mut broken = report();
        broken.results[0].improvement_plans[0].rank = 5;
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert!(error.message.contains("dense and ordered"), "{}", error.message);

        // feasibility 与冲突报告里的 target 报告不一致：拒绝。
        // A feasibility that disagrees with the conflict's own target report: refused.
        let mut broken = report();
        // 用 `unsupported` 而不是 `reachable`：后者还要求见证解，会先被那条规则挡住。
        // Use `unsupported` rather than `reachable`: the latter also demands a witness, which would trip that rule first.
        broken.results[0].feasibility = feasibility(&target("throughput", 10.0), AnalysisStatus::Unsupported);
        let error = verify_multi_target_report(&encode(&broken), None).unwrap_err();
        assert!(error.message.contains("does not match conflict report"), "{}", error.message);

        // 策略收紧到 `maxPlans = 1`：原来的 2 条建议超出策略，被拒绝。
        // Tightening the policy to one plan: the two existing recommendations exceed it and are refused.
        let tight = RelaxationPolicyInput {
            max_candidates: 32, max_plans: 1, require_positive_weight: true };
        let error = verify_multi_target_report(&encode(&report()), Some(&tight)).unwrap_err();
        // 实测：先撞上的是"方案序号超出策略"（rank 1 ≥ maxPlans 1），而不是"建议集合大小超出策略"。
        // Measured: the plan-rank check fires first (rank 1 ≥ maxPlans 1), before the correction-set size check.
        assert!(error.message.contains("exceeds policy"), "{}", error.message);

        assert_eq!(verify_multi_target_report("not json", None).unwrap_err().code, "InvalidInput");
    }
}
