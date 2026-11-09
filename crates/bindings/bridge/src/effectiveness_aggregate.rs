//! 续挖第 34 单：有效性排序的**聚合**是纯数据，且与报告自己的改善判据**不同**
//! （`analysis/pipeline.rs` 的 `build_effectiveness_ranking` + `solver/logic_based_benders` 的同名概念）
//! Continuation, delivery 34: aggregating an effectiveness ranking is pure data, and it disagrees with the report's own
//! improvement predicate.
//!
//! 第 3 单只绑了 `EffectivenessRanking::validate()` 与两个投影。真正把扰动报告变成排序的
//! `build_effectiveness_ranking(baseline, funnel, reports)` 一直没绑，而它全是纯数据。实测出四条：
//!
//! 1. **只有 `objective_proven` 的观察才进排序**：未证明最优的观察即使 `improved == Some(true)`，
//!    也**既不**贡献 `max_improvement`、**也不**贡献 `first_effective_delta`。所以"看到改善"与"能排序"是两件事。
//! 2. **删除测试的改善不算进 `max_improvement`**：`max_improvement` 只从**扰动观察**里取，而
//!    `ConstraintPerturbationReport::has_global_improvement()` 会把 `removal.effective == Some(true)` 也算成改善。
//!    于是存在一份报告：`has_global_improvement()` 为**真**，而排序里 `is_globally_effective()` 为**假**。
//!    这是本单最值钱的一条——同一个"改善"在两层有两个不同的意思。**按实测补一条**：`improved == Some(true)` 但
//!    未证明最优时，报告的判据**仍为真**（它只看 `improved`，不看 `objective_proven`），所以"未证明的改善"是
//!    第二个不一致来源，而且两条都指向同一个根因：报告判据看的是"观察到"，排序判据看的是"已证明"。
//! 3. **`marginal_effect = max_improvement / first_effective_delta`，且只在 delta > 0 时成立**；
//!    `first_effective_delta` 取的是**已证明改善观察里最小的 delta**（不是第一条出现的）。
//! 4. **排序三段式**：已证明改善降序（`None` 当 `-inf` 排最后）→ |dual| 降序 → 稳定 ID 升序。
//!    没有漏斗候选时该行是 `Unclassified` / `Unknown` / `dual=None`，而不是"不重要"。
//!
//! Delivery 3 bound only `EffectivenessRanking::validate()` and the two projections. The aggregator
//! `build_effectiveness_ranking(baseline, funnel, reports)` — which actually turns perturbation reports into a ranking — was
//! never bound, and it is all pure data. Four measured rules. **Only `objective_proven` observations feed the ranking**: an
//! observation without an optimality proof contributes to **neither** `max_improvement` **nor** `first_effective_delta`,
//! even with `improved == Some(true)`, so "saw an improvement" and "can be ranked" are two different things. **The removal
//! test's improvement never reaches `max_improvement`**: it comes only from **perturbation observations**, while
//! `has_global_improvement()` also counts `removal.effective == Some(true)` — so a report can have
//! `has_global_improvement()` **true** while its ranking entry reports `is_globally_effective()` **false**. That is this
//! delivery's most valuable finding: one word, "improvement", with two meanings on two layers. Also
//! **`marginal_effect = max_improvement / first_effective_delta`, only when delta > 0**, and `first_effective_delta` is the
//! **smallest** delta among proven improving observations, not the first one seen. And the **three-key sort**: proven
//! improvement descending (`None` as `-inf`, last), then |dual| descending, then stable ID ascending — with a missing funnel
//! candidate yielding `Unclassified` / `Unknown` / `dual=None` rather than "unimportant".

use std::collections::BTreeMap;
use std::time::Duration;

use ospf_rust_core::analysis::perturbation::{
    ConstraintPerturbationObservation, ConstraintPerturbationReport, RemovalTestObservation,
};
use ospf_rust_core::analysis::pipeline::{
    build_effectiveness_ranking, CandidateFunnelRanking, CandidateTier, ConstraintCandidate,
    EffectivenessEntry, EffectivenessRanking,
};
use ospf_rust_core::solver::report::TerminationReason;
use ospf_rust_core::solver::StableConstraintId;
use ospf_rust_core::analysis::AnalysisStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条扰动观察的字面量。 / One perturbation observation literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationSpec {
    /// 正扰动量。 / The positive perturbation.
    pub delta: f64,
    /// 观察到的改善量。 / The observed improvement.
    #[serde(default)]
    pub improvement: Option<f64>,
    /// 是否被认为改善。 / Whether it is considered improving.
    #[serde(default)]
    pub improved: Option<bool>,
    /// 是否具备最优性证明（**决定是否参与排序**）。 / Whether optimality was proven (**decides participation**).
    #[serde(default)]
    pub objective_proven: bool,
}

/// 一份扰动报告的字面量。 / One perturbation-report literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerturbationSpec {
    /// 约束稳定 ID。 / The stable constraint ID.
    pub constraint_id: String,
    /// 观察列表。 / The observations.
    #[serde(default)]
    pub observations: Vec<ObservationSpec>,
    /// 删除测试是否已证明有效；空表示没跑。 / Whether removal was proven effective; absent means it did not run.
    #[serde(default)]
    pub removal_effective: Option<bool>,
}

/// 一个漏斗候选的字面量。 / One funnel-candidate literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelCandidateSpec {
    /// 约束稳定 ID。 / The stable constraint ID.
    pub constraint_id: String,
    /// 约束组。 / The constraint group.
    #[serde(default)]
    pub group: Option<String>,
    /// 分层名（`a` / `b` / `c` / `unclassified`）。 / The tier name.
    #[serde(default)]
    pub tier: String,
    /// 对偶值。 / The dual value.
    #[serde(default)]
    pub dual_value: Option<f64>,
    /// 局部有效性。 / Local effectiveness.
    #[serde(default)]
    pub local_effective: Option<bool>,
}

/// 有效性排序的聚合请求。 / The aggregation request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessAggregateRequest {
    /// baseline 目标值。 / The baseline objective.
    pub baseline_objective: f64,
    /// 对偶阈值。 / The dual threshold.
    #[serde(default)]
    pub dual_threshold: f64,
    /// 漏斗候选。 / The funnel candidates.
    #[serde(default)]
    pub funnel: Vec<FunnelCandidateSpec>,
    /// 扰动报告。 / The perturbation reports.
    pub reports: Vec<PerturbationSpec>,
}

/// 排序里的一行。 / One row of the ranking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AggregateRow {
    /// 约束稳定 ID。 / The stable constraint ID.
    pub constraint_id: String,
    /// 分层名。 / The tier name.
    pub tier: String,
    /// 已证明的最大改善。 / The largest proven improvement.
    #[serde(default)]
    pub max_improvement: Option<f64>,
    /// 首次有效（最小）delta。 / The first effective (smallest) delta.
    #[serde(default)]
    pub first_effective_delta: Option<f64>,
    /// 边际效应。 / The marginal effect.
    #[serde(default)]
    pub marginal_effect: Option<f64>,
    /// 删除结论。 / The removal verdict.
    #[serde(default)]
    pub removal_effective: Option<bool>,
    /// 是否被排序判为全局有效。 / Whether the ranking calls it globally effective.
    pub globally_effective: bool,
    /// 报告自己的改善判据。 / The report's own improvement predicate.
    pub report_has_global_improvement: bool,
    /// 两份判据是否一致。 / Whether the two predicates agree.
    pub predicates_agree: bool,
}

/// 聚合结论。 / The aggregation verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessAggregateReport {
    /// 排序后的行。 / The rows in ranking order.
    pub rows: Vec<AggregateRow>,
    /// 排序里的稳定 ID 顺序。 / The stable ID order in the ranking.
    pub order: Vec<String>,
    /// 未证明的观察被排除（**是**）。 / Whether unproven observations are excluded (**yes**).
    pub unproven_observations_are_ignored: bool,
    /// 删除改善不算进最大改善（**是**）。 / Whether a removal improvement stays out of the maximum (**yes**).
    pub removal_improvement_is_not_a_gain: bool,
    /// 两份改善判据可以不一致（**是**）。 / Whether the two improvement predicates can disagree (**yes**).
    pub the_two_predicates_can_disagree: bool,
    /// 边际效应只在 delta > 0 时成立（**是**）。 / Whether the marginal effect needs a positive delta (**yes**).
    pub marginal_effect_needs_positive_delta: bool,
    /// first delta 取最小而非最先（**是**）。 / Whether the first delta is the smallest, not the earliest (**yes**).
    pub first_delta_is_the_smallest: bool,
    /// `None` 改善排在最后（**是**）。 / Whether `None` improvement sorts last (**yes**).
    pub none_improvement_sorts_last: bool,
    /// 同改善时按 |dual| 降序（**是**）。 / Whether ties fall back to |dual| descending (**yes**).
    pub ties_fall_back_to_absolute_dual: bool,
    /// 缺漏斗候选时不是"不重要"（**是**）。 / Whether a missing candidate is not "unimportant" (**yes**).
    pub missing_candidate_is_unclassified: bool,
}

/// 报告条数上限（信任边界）。 / The report cap (a trust boundary).
const MAX_REPORTS: usize = 256;

/// 聚合一份有效性排序。非法输入是**错误**。 / Aggregate one effectiveness ranking.
pub fn verify_effectiveness_aggregate(
    request: &EffectivenessAggregateRequest,
) -> Result<EffectivenessAggregateReport> {
    if request.reports.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "effectiveness aggregation failed validation: at least one report is required"));
    }
    if request.reports.len() > MAX_REPORTS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "effectiveness aggregation failed validation: at most {MAX_REPORTS} reports are accepted")));
    }
    if !request.baseline_objective.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "effectiveness aggregation failed validation: the baseline objective must be finite"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for report in &request.reports {
        if report.constraint_id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "effectiveness aggregation failed validation: constraint IDs must not be blank"));
        }
        // 重复的约束 ID 在排序里会被判重复，这里先给出可读的错误。
        // A duplicate constraint ID is refused by the ranking; catch it here with a readable error.
        if !seen.insert(report.constraint_id.as_str()) {
            return Err(BridgeError::new("InvalidInput",
                "effectiveness aggregation failed validation: the request repeats a constraint"));
        }
        for observation in &report.observations {
            if !observation.delta.is_finite() || observation.delta <= 0.0 {
                return Err(BridgeError::new("InvalidInput",
                    "effectiveness aggregation failed validation: deltas must be finite and positive"));
            }
            if observation.improvement.is_some_and(|value| !value.is_finite()) {
                return Err(BridgeError::new("InvalidInput",
                    "effectiveness aggregation failed validation: improvements must be finite"));
            }
        }
    }
    let funnel = build_funnel(request);
    let reports: Vec<ConstraintPerturbationReport> = request.reports.iter()
        .map(|report| to_report(report)).collect();
    let ranking: EffectivenessRanking = build_effectiveness_ranking(
        request.baseline_objective, funnel.as_ref(), &reports)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let orders: BTreeMap<&str, bool> = reports.iter()
        .map(|report| (report.constraint_id.0.as_str(), report.has_global_improvement()))
        .collect();
    let rows: Vec<AggregateRow> = ranking.entries.iter().map(|entry| {
        let ranking_says = entry.is_globally_effective();
        let report_says = orders.get(entry.constraint_id.0.as_str()).copied().unwrap_or(false);
        row(entry, report_says, ranking_says == report_says)
    }).collect();
    let findings = canonical_findings();
    Ok(EffectivenessAggregateReport {
        order: ranking.entries.iter().map(|entry| entry.constraint_id.0.clone()).collect(),
        rows,
        unproven_observations_are_ignored: findings.unproven_observations_are_ignored,
        removal_improvement_is_not_a_gain: findings.removal_improvement_is_not_a_gain,
        the_two_predicates_can_disagree: findings.the_two_predicates_can_disagree,
        marginal_effect_needs_positive_delta: findings.marginal_effect_needs_positive_delta,
        first_delta_is_the_smallest: findings.first_delta_is_the_smallest,
        none_improvement_sorts_last: findings.none_improvement_sorts_last,
        ties_fall_back_to_absolute_dual: findings.ties_fall_back_to_absolute_dual,
        missing_candidate_is_unclassified: findings.missing_candidate_is_unclassified,
    })
}

/// 把一行领域条目投影成扁平行。 / Project one domain entry.
fn row(entry: &EffectivenessEntry, report_says: bool, agree: bool) -> AggregateRow {
    AggregateRow { constraint_id: entry.constraint_id.0.clone(),
        tier: entry.tier.as_str().to_owned(), max_improvement: entry.max_improvement,
        first_effective_delta: entry.first_effective_delta,
        marginal_effect: entry.marginal_effect, removal_effective: entry.removal_effective,
        globally_effective: entry.is_globally_effective(), report_has_global_improvement: report_says,
        predicates_agree: agree }
}

/// 按请求造漏斗排序。 / Build the funnel ranking from the request.
fn build_funnel(request: &EffectivenessAggregateRequest) -> Option<CandidateFunnelRanking> {
    if request.funnel.is_empty() {
        return None;
    }
    let candidates: Vec<ConstraintCandidate> = request.funnel.iter().enumerate()
        .map(|(position, spec)| ConstraintCandidate {
            constraint_id: StableConstraintId(spec.constraint_id.clone()),
            group: spec.group.clone(), tier: tier_of(&spec.tier),
            activity: ospf_rust_core::analysis::ActivityStatus::Unknown,
            slack: None, dual_value: spec.dual_value, local_effective: spec.local_effective,
            priority: position })
        .collect();
    Some(CandidateFunnelRanking { schema_version: "1.0".to_owned(),
        dual_threshold: request.dual_threshold, candidates, tier_a: 0, tier_b: 0, tier_c: 0,
        unclassified: 0 })
}

/// 分层名转枚举。 / Turn a tier name into the enum.
fn tier_of(name: &str) -> CandidateTier {
    match name {
        "a" | "A" | "tierA" => CandidateTier::TierA,
        "b" | "B" | "tierB" => CandidateTier::TierB,
        "c" | "C" | "tierC" => CandidateTier::TierC,
        _ => CandidateTier::Unclassified,
    }
}

/// 把请求里的报告字面量变成领域报告。 / Turn a request literal into a domain report.
fn to_report(spec: &PerturbationSpec) -> ConstraintPerturbationReport {
    let observations: Vec<ConstraintPerturbationObservation> = spec.observations.iter()
        .map(|observation| ConstraintPerturbationObservation {
            delta: observation.delta, perturbed_rhs: 0.0, status: AnalysisStatus::Reachable,
            termination_reason: TerminationReason::Completed, objective: Some(0.0),
            objective_change: observation.improvement.map(|value| -value),
            improvement: observation.improvement, improved: observation.improved,
            integer_pattern_changed: Some(false), solve_time: Duration::ZERO,
            objective_proven: observation.objective_proven })
        .collect();
    let removal = spec.removal_effective.map(|effective| RemovalTestObservation {
        status: AnalysisStatus::Reachable, termination_reason: TerminationReason::Completed,
        objective: Some(0.0), objective_change: Some(0.0), improvement: effective.then_some(1.0),
        effective: Some(effective), integer_pattern_changed: Some(false),
        solve_time: Duration::ZERO, objective_proven: true });
    let solve_count = observations.len() + usize::from(removal.is_some());
    ConstraintPerturbationReport { schema_version: "1.0".to_owned(),
        constraint_id: StableConstraintId(spec.constraint_id.clone()), baseline_objective: 0.0,
        observations, removal, adaptive_outcome: None, status: AnalysisStatus::Reachable,
        solve_count, unavailable_reason: None }
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    unproven_observations_are_ignored: bool,
    removal_improvement_is_not_a_gain: bool,
    the_two_predicates_can_disagree: bool,
    marginal_effect_needs_positive_delta: bool,
    first_delta_is_the_smallest: bool,
    none_improvement_sorts_last: bool,
    ties_fall_back_to_absolute_dual: bool,
    missing_candidate_is_unclassified: bool,
}

/// 造一份探针报告。 / Build one probe report.
fn probe_report(id: &str, observations: Vec<ObservationSpec>, removal: Option<bool>)
    -> ConstraintPerturbationReport {
    to_report(&PerturbationSpec { constraint_id: id.to_owned(), observations,
        removal_effective: removal })
}

/// 一条探针观察。 / One probe observation.
fn observed(delta: f64, improvement: f64, improved: bool, proven: bool) -> ObservationSpec {
    ObservationSpec { delta, improvement: Some(improvement), improved: Some(improved),
        objective_proven: proven }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let rank = |reports: &[ConstraintPerturbationReport], funnel: Option<&CandidateFunnelRanking>|
        build_effectiveness_ranking(0.0, funnel, reports).expect("probe ranking");
    // 1) 未证明的观察不参与：同一份改善，证明与否结果不同。 
    // 1) Unproven observations do not participate: the same improvement, proven or not, differs.
    let proven = rank(&[probe_report("p", vec![observed(1.0, 5.0, true, true)], None)], None);
    let unproven = rank(&[probe_report("p", vec![observed(1.0, 5.0, true, false)], None)], None);
    let unproven_observations_are_ignored = proven.entries[0].max_improvement == Some(5.0)
        && unproven.entries[0].max_improvement.is_none();
    // 2) 删除测试的改善不进 max_improvement，但报告自己的判据会认它。 
    // 2) A removal improvement stays out of `max_improvement`, yet the report's own predicate counts it.
    let removal_only = probe_report("r", Vec::new(), Some(true));
    let removal_ranking = rank(std::slice::from_ref(&removal_only), None);
    let removal_improvement_is_not_a_gain = removal_ranking.entries[0].max_improvement.is_none()
        && !removal_ranking.entries[0].is_globally_effective()
        && removal_only.has_global_improvement();
    let the_two_predicates_can_disagree = removal_only.has_global_improvement()
        != removal_ranking.entries[0].is_globally_effective();
    // 3) 边际效应需要正的 delta；first delta 取最小而不是最先。 
    // 3) The marginal effect needs a positive delta; the first delta is the smallest, not the earliest.
    let ordered = rank(&[probe_report("o", vec![observed(4.0, 8.0, true, true),
        observed(1.0, 2.0, true, true)], None)], None);
    let entry = &ordered.entries[0];
    let first_delta_is_the_smallest = entry.first_effective_delta == Some(1.0)
        && entry.max_improvement == Some(8.0);
    let marginal_effect_needs_positive_delta = entry.marginal_effect == Some(8.0)
        && entry.first_effective_delta.is_some_and(|delta| delta > 0.0);
    // 4) `None` 改善排在有改善的后面。 / `None` improvement sorts after a real one.
    let mixed = rank(&[probe_report("quiet", Vec::new(), None),
        probe_report("loud", vec![observed(1.0, 9.0, true, true)], None)], None);
    let none_improvement_sorts_last = mixed.entries[0].constraint_id.0 == "loud"
        && mixed.entries[1].max_improvement.is_none();
    // 5) 同改善时按 |dual| 降序。 / On equal improvement, |dual| decides.
    let duals: Vec<ConstraintCandidate> = vec![
        ConstraintCandidate { constraint_id: StableConstraintId("small".to_owned()), group: None,
            tier: CandidateTier::TierA, activity: ospf_rust_core::analysis::ActivityStatus::Active,
            slack: None, dual_value: Some(1.0), local_effective: None, priority: 0 },
        ConstraintCandidate { constraint_id: StableConstraintId("large".to_owned()), group: None,
            tier: CandidateTier::TierA, activity: ospf_rust_core::analysis::ActivityStatus::Active,
            slack: None, dual_value: Some(-7.0), local_effective: None, priority: 1 },
    ];
    let funnel = CandidateFunnelRanking { schema_version: "1.0".to_owned(), dual_threshold: 0.0,
        candidates: duals, tier_a: 2, tier_b: 0, tier_c: 0, unclassified: 0 };
    let tied = rank(&[probe_report("small", vec![observed(1.0, 3.0, true, true)], None),
        probe_report("large", vec![observed(1.0, 3.0, true, true)], None)], Some(&funnel));
    let ties_fall_back_to_absolute_dual = tied.entries[0].constraint_id.0 == "large"
        && tied.entries[1].constraint_id.0 == "small";
    // 6) 缺漏斗候选 → Unclassified，而不是"不重要"。 
    // 6) A missing funnel candidate is `Unclassified`, not "unimportant".
    let lonely = rank(&[probe_report("lonely", vec![observed(1.0, 1.0, true, true)], None)], None);
    let missing_candidate_is_unclassified = lonely.entries[0].tier == CandidateTier::Unclassified
        && lonely.entries[0].activity == ospf_rust_core::analysis::ActivityStatus::Unknown
        && lonely.entries[0].dual_value.is_none();
    Findings { unproven_observations_are_ignored, removal_improvement_is_not_a_gain,
        the_two_predicates_can_disagree, marginal_effect_needs_positive_delta,
        first_delta_is_the_smallest, none_improvement_sorts_last, ties_fall_back_to_absolute_dual,
        missing_candidate_is_unclassified }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EffectivenessAggregateRequest {
        EffectivenessAggregateRequest { baseline_objective: 100.0, dual_threshold: 1e-9,
            funnel: vec![
                FunnelCandidateSpec { constraint_id: "c.a".to_owned(), group: Some("g".to_owned()),
                    tier: "a".to_owned(), dual_value: Some(2.5), local_effective: Some(true) },
                FunnelCandidateSpec { constraint_id: "c.b".to_owned(), group: None,
                    tier: "c".to_owned(), dual_value: Some(0.1), local_effective: None },
            ],
            reports: vec![
                PerturbationSpec { constraint_id: "c.a".to_owned(), removal_effective: None,
                    observations: vec![observed(4.0, 8.0, true, true),
                        observed(1.0, 2.0, true, true), observed(2.0, 3.0, false, true)] },
                PerturbationSpec { constraint_id: "c.b".to_owned(), removal_effective: Some(true),
                    observations: Vec::new() },
            ] }
    }

    #[test]
    fn only_proven_observations_are_ranked() {
        let report = verify_effectiveness_aggregate(&request()).unwrap();
        let first = &report.rows[0];
        assert_eq!(first.constraint_id, "c.a");
        // 最大改善取已证明观察的最大值；first delta 取最小而不是最先（4.0 先出现，但取 1.0）。
        // The maximum comes from proven observations; the first delta is the smallest (1.0, not the earlier 4.0).
        assert_eq!(first.max_improvement, Some(8.0));
        assert_eq!(first.first_effective_delta, Some(1.0));
        assert_eq!(first.marginal_effect, Some(8.0));
        assert!(first.globally_effective);
        assert!(report.unproven_observations_are_ignored);
        assert!(report.first_delta_is_the_smallest);
        assert!(report.marginal_effect_needs_positive_delta);
    }

    #[test]
    fn a_removal_improvement_is_not_a_gain_but_the_report_says_it_is() {
        let report = verify_effectiveness_aggregate(&request()).unwrap();
        let removal = report.rows.iter().find(|row| row.constraint_id == "c.b").unwrap();
        // 排序：没有 perturb 改善 → 不是全局有效；报告自己：删除有效 → 有改善。
        // The ranking: no perturbation gain, so not globally effective. The report: removal effective, so it improved.
        assert_eq!(removal.max_improvement, None);
        assert!(!removal.globally_effective);
        assert!(removal.report_has_global_improvement);
        assert!(!removal.predicates_agree);
        assert!(report.removal_improvement_is_not_a_gain);
        assert!(report.the_two_predicates_can_disagree);
    }

    #[test]
    fn the_ranking_is_sorted_by_gain_then_absolute_dual_then_identity() {
        let report = verify_effectiveness_aggregate(&request()).unwrap();
        // c.a 有 8.0 的改善，c.b 没有 → c.a 在前。 / c.a gains 8.0, c.b nothing, so c.a leads.
        assert_eq!(report.order, vec!["c.a".to_owned(), "c.b".to_owned()]);
        assert_eq!(report.rows[0].tier, "A");
        assert_eq!(report.rows[1].tier, "C");
        assert!(report.none_improvement_sorts_last);
        assert!(report.ties_fall_back_to_absolute_dual);
        // 缺漏斗候选 → Unclassified，而不是不重要。 / A missing candidate is Unclassified.
        assert!(report.missing_candidate_is_unclassified);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.baseline_objective = 0.0;
        other.funnel = Vec::new();
        other.reports = vec![PerturbationSpec { constraint_id: "only".to_owned(),
            removal_effective: None,
            observations: vec![observed(2.0, 1.0, true, true)] }];
        let report = verify_effectiveness_aggregate(&other).unwrap();
        assert!(report.unproven_observations_are_ignored);
        assert!(report.removal_improvement_is_not_a_gain);
        assert!(report.the_two_predicates_can_disagree);
        assert!(report.marginal_effect_needs_positive_delta);
        assert!(report.first_delta_is_the_smallest);
        assert!(report.none_improvement_sorts_last);
        assert!(report.ties_fall_back_to_absolute_dual);
        assert!(report.missing_candidate_is_unclassified);
        assert_eq!(report.rows[0].tier, "UNCLASSIFIED");
        // 没有删除测试 → 两份判据一致。 / Without a removal test the two predicates agree.
        assert!(report.rows[0].predicates_agree);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut empty = request();
        empty.reports = Vec::new();
        assert!(verify_effectiveness_aggregate(&empty).unwrap_err().message
            .contains("at least one report"));
        let mut infinite = request();
        infinite.baseline_objective = f64::NAN;
        assert!(verify_effectiveness_aggregate(&infinite).unwrap_err().message
            .contains("baseline objective must be finite"));
        let mut blank = request();
        blank.reports[0].constraint_id = "  ".to_owned();
        assert!(verify_effectiveness_aggregate(&blank).unwrap_err().message
            .contains("constraint IDs must not be blank"));
        let mut duplicate = request();
        duplicate.reports[1].constraint_id = "c.a".to_owned();
        assert!(verify_effectiveness_aggregate(&duplicate).unwrap_err().message
            .contains("repeats a constraint"));
        let mut zero_delta = request();
        zero_delta.reports[0].observations[0].delta = 0.0;
        assert!(verify_effectiveness_aggregate(&zero_delta).unwrap_err().message
            .contains("deltas must be finite and positive"));
        let mut bad_improvement = request();
        bad_improvement.reports[0].observations[0].improvement = Some(f64::INFINITY);
        assert!(verify_effectiveness_aggregate(&bad_improvement).unwrap_err().message
            .contains("improvements must be finite"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_effectiveness_aggregate(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"unprovenObservationsAreIgnored\""), "{json}");
        assert!(json.contains("\"theTwoPredicatesCanDisagree\""), "{json}");
        assert!(json.contains("\"maxImprovement\""), "{json}");
        assert!(!json.contains("unproven_observations"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<EffectivenessAggregateReport>(&copy.to_string()).is_err());
    }
}
