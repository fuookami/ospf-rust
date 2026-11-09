//! 批次 B 第 5 单：从已验证冲突生成带权松弛建议 / Batch B, delivery 5: weighted relaxation recommendations from a verified conflict.
//!
//! 这是 `core/analysis` 里少见的**纯函数**组合能力：给定一份已验证的冲突解释 + 调用方给的业务成本，
//! 直接产出排序好的松弛建议，全程不触碰求解器。领域在类型上刻意保留两条保守语义，本桥接把它们作为**字段**暴露
//! 而不是藏起来：
//!
//! * `is_complete_mcs()` **恒为 false**——建议来自单个 conflict，不是完整模型范围的 MCS；
//! * `requires_revalidation()` **恒为 true**——数值松弛幅度只是建议，应用后必须由调用方重新验证 target 可达性。
//!
//! This is one of the few **pure-function** compositions in `core/analysis`: given a verified conflict explanation plus
//! caller-supplied business costs it produces ranked relaxation recommendations without ever touching a solver. The
//! domain deliberately keeps two conservative properties on the types, and this bridge exposes them as **fields** rather
//! than hiding them: `is_complete_mcs()` is always false (the recommendation comes from a single conflict and is not a
//! model-wide MCS) and `requires_revalidation()` is always true (numeric amounts are suggestions only).

use std::collections::BTreeMap;

use ospf_rust_core::analysis::correction::{
    AlternativeImprovementPlan, RelaxabilityPolicy, RelaxationCost, relaxation_recommendations_from_conflict,
};
use ospf_rust_core::analysis::ConflictExplanation;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个证据来源的业务成本。省略时领域默认 `weight = 1.0, relaxation = 1.0`。
/// The business cost of one evidence source. Absent entries default to `weight = 1.0, relaxation = 1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaxationCostInput {
    /// 业务成本权重。 / The business cost weight.
    pub weight: f64,
    /// 建议的正松弛幅度。 / The suggested positive relaxation amount.
    pub relaxation: f64,
}

/// 建议排序策略。省略时用领域默认（`maxCandidates = 32`、`maxPlans = 8`、要求正权重）。
/// The recommendation policy. Absent means the domain defaults (32 candidates, 8 plans, positive weights required).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaxationPolicyInput {
    /// 单个建议集合允许的最大来源数。 / The maximum sources in one recommendation set.
    pub max_candidates: u32,
    /// 允许返回的最大排序建议数。 / The maximum number of ranked recommendations.
    pub max_plans: u32,
    /// 是否要求权重严格为正。 / Whether weights must be strictly positive.
    pub require_positive_weight: bool,
}

/// 建议请求：冲突解释 + 业务成本 + 策略。 / The request: a conflict explanation, business costs and a policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaxationRequest {
    /// 已验证冲突解释的 JSON 文本。 / The verified conflict-explanation JSON text.
    pub conflict: String,
    /// 按 `source` 的稳定 ID 索引的业务成本（例如 `constraint:capacity`）。
    /// Business costs keyed by the source's stable ID (for example `constraint:capacity`).
    #[serde(default)]
    pub costs: BTreeMap<String, RelaxationCostInput>,
    /// 排序策略；省略用领域默认。 / The ranking policy; absent means domain defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub policy: Option<RelaxationPolicyInput>,
}

/// 一个建议来源。 / One recommended source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaxationMember {
    /// 证据来源的稳定 ID（可直接作为成本表的键）。 / The source's stable ID (usable as a key in the cost map).
    pub source: String,
    /// 业务成本权重。 / The business cost weight.
    pub weight: f64,
    /// 建议的松弛幅度（建议值，非已应用值）。 / The suggested relaxation amount (a suggestion, not applied).
    pub relaxation: f64,
}

/// 一条排序好的松弛建议。 / One ranked relaxation recommendation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaxationPlan {
    /// 确定性的从零开始名次。 / The deterministic zero-based rank.
    pub rank: u32,
    /// 带权建议成本（各成员 `weight × relaxation` 之和）。 / The weighted cost (the sum of `weight × relaxation`).
    pub total_cost: f64,
    /// 是否已验证删除层面最小性。 / Whether deletion minimality was verified.
    pub minimal: bool,
    /// **恒为 `false`**：本协议从不宣称完整模型范围的 MCS。 / **Always `false`**: never a model-wide MCS.
    pub is_complete_mcs: bool,
    /// **恒为 `true`**：应用数值松弛后必须由调用方重新验证 target 可达性。 / **Always `true`**: revalidate after applying.
    pub requires_revalidation: bool,
    /// 关联 target 的目标身份。 / The associated target's objective identity.
    pub target_objective_id: String,
    /// 关联 target 的方向（`AtLeast`/`AtMost`）。 / The associated target's direction.
    pub target_relation: String,
    /// 关联 target 的边界值。 / The associated target's bound value.
    pub target_value: f64,
    /// 建议的来源。 / The recommended sources.
    pub members: Vec<RelaxationMember>,
}

/// 松弛建议的生成结论。 / The outcome of recommendation generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaxationReport {
    /// 建议条数。 / The number of recommendations.
    pub plan_count: u32,
    /// 建议（按成本升序，成本相同按来源稳定 ID 升序）。 / Recommendations (ascending cost, ties by source ID).
    pub plans: Vec<RelaxationPlan>,
    /// 冲突是否通过了建议生成所需的全部证明门槛。为 `false` 时建议数为 0，且**这不是错误**。
    /// Whether the conflict passed every proof gate recommendation generation requires. When `false` there are zero
    /// recommendations, and that is **not** an error.
    pub conflict_proven: bool,
}

/// 从一份已验证冲突生成带权松弛建议。请求本身非法才是**错误**；冲突未通过证明门槛只是返回 0 条建议。
/// Generate weighted relaxation recommendations from a verified conflict. Only an invalid *request* is an **error**; a
/// conflict that fails the proof gates simply yields zero recommendations.
pub fn generate_relaxation_recommendations(request: &RelaxationRequest) -> Result<RelaxationReport> {
    if request.conflict.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the conflict explanation exceeds 8 MiB"));
    }
    let conflict: ConflictExplanation = serde_json::from_str(&request.conflict).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("conflict explanation is not decodable: {error}"))
    })?;
    // 冲突本身也必须自洽——否则"已验证"这个前提就是空的。
    // The conflict itself must be self-consistent, otherwise "verified" is an empty premise.
    conflict.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("conflict explanation failed validation: {error}"))
    })?;
    let policy = match request.policy {
        None => RelaxabilityPolicy::default(),
        Some(input) => RelaxabilityPolicy {
            max_candidates: usize::try_from(input.max_candidates).unwrap_or(usize::MAX),
            max_plans: usize::try_from(input.max_plans).unwrap_or(usize::MAX),
            require_positive_weight: input.require_positive_weight,
        },
    };
    let costs: BTreeMap<String, RelaxationCost> = request
        .costs
        .iter()
        .map(|(source, cost)| {
            (source.clone(), RelaxationCost { weight: cost.weight, relaxation: cost.relaxation })
        })
        .collect();
    // 领域自己的入口：内部先判证明门槛，再按成本升序排序并按 `max_plans` 截断。
    // The domain's own entry point: it checks the proof gates first, then sorts by ascending cost and truncates to
    // `max_plans`.
    let plans = relaxation_recommendations_from_conflict(&conflict, &costs, &policy).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("relaxation recommendations were refused: {error}"))
    })?;
    let conflict_proven = !plans.is_empty() || conflict_proven_directly(&conflict);
    Ok(RelaxationReport {
        plan_count: u32::try_from(plans.len()).unwrap_or(u32::MAX),
        plans: plans.iter().map(plan).collect(),
        conflict_proven,
    })
}

/// 独立判断证明门槛，用于在"0 条建议"时区分"没通过门槛"与"通过了但没有成员"。
/// Judge the proof gates independently so a zero-recommendation outcome can distinguish "gates failed" from "gates
/// passed but there were no members".
fn conflict_proven_directly(conflict: &ConflictExplanation) -> bool {
    use ospf_rust_core::analysis::{AnalysisStatus, ConflictValidity};
    conflict.status == AnalysisStatus::Unreachable
        && conflict.validity == ConflictValidity::Verified
        && conflict.minimality_verified()
        && !conflict.background_blocking
}

/// 把一条领域方案投影成扁平建议。 / Project one domain plan into a flat recommendation.
fn plan(plan: &AlternativeImprovementPlan) -> RelaxationPlan {
    RelaxationPlan {
        rank: u32::try_from(plan.rank).unwrap_or(u32::MAX),
        total_cost: plan.correction_set.total_cost,
        minimal: plan.correction_set.minimal,
        // 直接调用领域的恒定结论，而不是在这里硬编码 `false`/`true`：如果哪天领域改了语义，这里会跟着变。
        // Call the domain's constant verdicts instead of hard-coding `false`/`true` here: if the semantics ever change,
        // this follows.
        is_complete_mcs: plan.is_complete_mcs(),
        requires_revalidation: plan.requires_revalidation(),
        target_objective_id: plan.target.objective_id().as_str().to_owned(),
        target_relation: format!("{:?}", plan.target.relation()),
        target_value: plan.target.value(),
        members: plan.correction_set.members.iter().map(|member| RelaxationMember {
            source: member.source.stable_id(),
            weight: member.weight,
            relaxation: member.relaxation,
        }).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::analysis::{
        AnalysisStatus, ConflictExtractionTier, ConflictGroupSummary, ConflictMinimality,
        ConflictValidity, DiagnosticSource, ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};

    fn target() -> ObjectiveTarget {
        ObjectiveTarget::at_least("objective", 5.0).expect("target")
    }

    fn conflict_with(members: &[&str], minimality: ConflictMinimality) -> ConflictExplanation {
        let target = target();
        ConflictExplanation {
            schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
            target: target.clone(),
            target_feasibility: TargetFeasibilityReport {
                schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
                target: target.clone(),
                effective_integer_bound: None,
                source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
                status: AnalysisStatus::Unreachable, solution: None, exact_objective: None,
                objective_value: None, proof_status: ProofStatus::Verified, termination_reason: None,
                solve_time: None, target_fixed: true, message: None,
            },
            status: AnalysisStatus::Unreachable,
            available_evidence: Vec::new(),
            fixed_background: Vec::new(),
            members: members.iter()
                .map(|id| DiagnosticSource::Constraint { id: StableConstraintId((*id).to_owned()) })
                .collect(),
            target_source: DiagnosticSource::ObjectiveTarget { target },
            validity: ConflictValidity::Verified,
            minimality,
            verifications: Vec::new(),
            group_summaries: if members.is_empty() {
                Vec::new()
            } else {
                vec![ConflictGroupSummary { group: Some("g".to_owned()), count: members.len() }]
            },
            resolve_count: 2, memoized_checks: 0,
            extraction_tier: ConflictExtractionTier::AssumptionExtraction,
            unavailable_reason: None,
            background_blocking: false,
        }
    }

    fn encode(conflict: &ConflictExplanation) -> String {
        serde_json::to_string(conflict).expect("serialize conflict")
    }

    fn request(conflict: &ConflictExplanation) -> RelaxationRequest {
        RelaxationRequest { conflict: encode(conflict), costs: BTreeMap::new(), policy: None }
    }

    #[test]
    fn irreducible_conflict_yields_one_minimal_plan_per_member() {
        let report = generate_relaxation_recommendations(&request(&conflict_with(
            &["capacity", "shift"], ConflictMinimality::Irreducible))).unwrap();
        assert_eq!(report.plan_count, 2);
        assert!(report.conflict_proven);
        // 默认策略下每个成员一个 singleton 建议，成本都是 1.0，因此按来源稳定 ID 升序打破平局。
        // Under the default policy each member gets one singleton plan, all at cost 1.0, so ties break on the source ID.
        assert_eq!(report.plans.iter().map(|plan| plan.members[0].source.as_str()).collect::<Vec<_>>(),
            vec!["constraint:capacity", "constraint:shift"]);
        for (position, plan) in report.plans.iter().enumerate() {
            assert_eq!(plan.rank, position as u32);
            assert_eq!(plan.total_cost, 1.0);
            assert!(plan.minimal);
            // 两条保守结论以字段形式暴露，调用方无法误读成"已解决"。
            // The two conservative properties are exposed as fields so a caller cannot read them as "solved".
            assert!(!plan.is_complete_mcs, "a single-conflict plan is never a model-wide MCS");
            assert!(plan.requires_revalidation, "numeric relaxation always needs revalidation");
            assert_eq!(plan.members[0].weight, 1.0);
            assert_eq!(plan.members[0].relaxation, 1.0);
            assert_eq!(plan.target_objective_id, "objective");
            assert_eq!(plan.target_relation, "AtLeast");
            assert_eq!(plan.target_value, 5.0);
        }
    }

    #[test]
    fn caller_costs_rank_the_plans_and_policy_truncates_them() {
        let conflict = conflict_with(&["capacity", "shift", "budget"], ConflictMinimality::Irreducible);
        let mut with_costs = request(&conflict);
        with_costs.costs.insert("constraint:capacity".to_owned(),
            RelaxationCostInput { weight: 5.0, relaxation: 2.0 });
        with_costs.costs.insert("constraint:shift".to_owned(),
            RelaxationCostInput { weight: 1.0, relaxation: 1.0 });
        // `budget` 不在成本表里 → 领域默认 1.0/1.0，和 `shift` 同价同权重。
        // `budget` is absent from the cost map → the domain default 1.0/1.0, equal to `shift`.
        let report = generate_relaxation_recommendations(&with_costs).unwrap();
        assert_eq!(report.plans.iter().map(|plan| plan.members[0].source.as_str()).collect::<Vec<_>>(),
            vec!["constraint:budget", "constraint:shift", "constraint:capacity"]);
        assert_eq!(report.plans[2].total_cost, 10.0);

        // `maxPlans = 2`：按成本升序截断，最贵的那条被丢掉。
        // `maxPlans = 2`: truncate by ascending cost, dropping the priciest.
        with_costs.policy = Some(RelaxationPolicyInput {
            max_candidates: 32, max_plans: 2, require_positive_weight: true });
        let report = generate_relaxation_recommendations(&with_costs).unwrap();
        assert_eq!(report.plan_count, 2);
        assert_eq!(report.plans.iter().map(|plan| plan.members[0].source.as_str()).collect::<Vec<_>>(),
            vec!["constraint:budget", "constraint:shift"]);

        // `maxPlans = 1`：只剩最便宜的一条，名次仍是 0。
        // `maxPlans = 1`: only the cheapest survives, still ranked 0.
        with_costs.policy = Some(RelaxationPolicyInput {
            max_candidates: 32, max_plans: 1, require_positive_weight: true });
        let report = generate_relaxation_recommendations(&with_costs).unwrap();
        assert_eq!(report.plan_count, 1);
        assert_eq!(report.plans[0].rank, 0);
    }

    #[test]
    fn unproven_conflict_yields_no_recommendation_without_being_an_error() {
        // 未做删除复验 → 证明门槛不过 → 0 条建议，而且是 `Ok`。把"未证明"当成错误会诱导调用方重试或吞掉结论。
        // No deletion verification → the proof gate fails → zero recommendations, as `Ok`. Treating "unproven" as an
        // error would push callers to retry or swallow the conclusion.
        let report = generate_relaxation_recommendations(&request(&conflict_with(
            &["capacity"], ConflictMinimality::NotChecked))).unwrap();
        assert_eq!(report.plan_count, 0);
        assert!(!report.conflict_proven);
        assert!(report.plans.is_empty());

        // 空成员的不可约冲突在领域里本身非法，要作为**错误**回绝，而不是静默给 0 条。
        // An irreducible conflict with no members is itself invalid, so it is refused as an **error** rather than
        // silently yielding zero.
        let error = generate_relaxation_recommendations(&request(&conflict_with(
            &[], ConflictMinimality::Irreducible))).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("at least one blocking"), "{}", error.message);
    }

    #[test]
    fn illegal_requests_are_refused_with_the_domain_reason() {
        let conflict = conflict_with(&["capacity"], ConflictMinimality::Irreducible);

        // 策略上限为 0：领域拒绝。 / A policy limit of zero: refused by the domain.
        let mut zero_policy = request(&conflict);
        zero_policy.policy = Some(RelaxationPolicyInput {
            max_candidates: 0, max_plans: 8, require_positive_weight: true });
        let error = generate_relaxation_recommendations(&zero_policy).unwrap_err();
        assert!(error.message.contains("must be positive"), "{}", error.message);

        // 非正松弛幅度：领域拒绝。 / A non-positive relaxation amount: refused by the domain.
        let mut zero_relaxation = request(&conflict);
        zero_relaxation.costs.insert("constraint:capacity".to_owned(),
            RelaxationCostInput { weight: 1.0, relaxation: 0.0 });
        let error = generate_relaxation_recommendations(&zero_relaxation).unwrap_err();
        assert!(error.message.contains("finite and positive"), "{}", error.message);

        // 要求正权重却给了 0：领域拒绝。 / A zero weight while positive weights are required: refused.
        let mut zero_weight = request(&conflict);
        zero_weight.costs.insert("constraint:capacity".to_owned(),
            RelaxationCostInput { weight: 0.0, relaxation: 1.0 });
        let error = generate_relaxation_recommendations(&zero_weight).unwrap_err();
        assert!(error.message.contains("weight must be positive"), "{}", error.message);

        // 放开正权重要求后，0 权重是合法的（成本为 0，排在最前）。
        // With the positive-weight requirement relaxed, a zero weight is legal (cost 0, ranked first).
        let mut free_weight = zero_weight;
        free_weight.policy = Some(RelaxationPolicyInput {
            max_candidates: 32, max_plans: 8, require_positive_weight: false });
        let report = generate_relaxation_recommendations(&free_weight).unwrap();
        assert_eq!(report.plan_count, 1);
        assert_eq!(report.plans[0].total_cost, 0.0);

        // 冲突本身非法（重复成员）：先被冲突校验挡住。 / An invalid conflict (duplicate members) is caught first.
        let mut duplicate = conflict_with(&["capacity", "capacity"], ConflictMinimality::Irreducible);
        duplicate.group_summaries = Vec::new();
        let error = generate_relaxation_recommendations(&request(&duplicate)).unwrap_err();
        assert!(error.message.contains("duplicate"), "{}", error.message);

        // 非 JSON / 超限。 / Non-JSON and over the limit.
        let mut bad = request(&conflict);
        bad.conflict = "not json".to_owned();
        assert_eq!(generate_relaxation_recommendations(&bad).unwrap_err().code, "InvalidInput");
        bad.conflict = "x".repeat(9 * 1024 * 1024);
        assert_eq!(generate_relaxation_recommendations(&bad).unwrap_err().code, "InvalidInput");
    }
}
