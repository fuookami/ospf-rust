//! 续挖第 110 单：**候选漏斗的分层、排序与短名单口径**——`classify_candidate` 的 A/B 分界是
//! `|dual| > threshold`（**严格大于**，正好等于阈值落到 B），`Active` 与 `NearlyActive` **同权**，
//! 只有 `Inactive` 与 `SatisfiedWithoutSlackMetric` 落到 Tier C（`Violated`/`Unknown` 是未分层）；
//! **没有灵敏度报告就没有任何分层可声称**（C 除外，它只由活动性给出）；排序是**层级 → |dual| 降序 → 身份升序**
//! 且 `priority` 就是位置；短名单**先放 A/B，再用 C/未分层补满名额**；`local_effective` 由漏斗**自己重算**、
//! 不读灵敏度报告里的同名字段；而配置里的 `default_candidate_limit` **在漏斗里根本不生效**
//! （`analysis/pipeline.rs` 的 `build_candidate_funnel` / `classify_candidate` / `CandidateFunnelRanking`）
//! Continuation, delivery 110: **the candidate funnel's tiers, ordering and shortlist** — `classify_candidate` splits A
//! from B on `|dual| > threshold` (**strictly greater**, so exactly the threshold lands in B), `Active` and `NearlyActive`
//! **share the rule**, and only `Inactive` and `SatisfiedWithoutSlackMetric` reach Tier C while `Violated`/`Unknown` stay
//! unclassified; **without a sensitivity report no tier may be claimed** (except C, which comes from activity alone); the
//! order is **tier, then |dual| descending, then identity ascending**, with `priority` equal to the position; the shortlist
//! **takes A/B first and then fills the remaining slots with C and unclassified**; `local_effective` is **recomputed by the
//! funnel** rather than read from the sensitivity report; and the configuration's `default_candidate_limit` **is not enforced
//! by the funnel at all** (`build_candidate_funnel` / `classify_candidate` / `CandidateFunnelRanking` in
//! `analysis/pipeline.rs`).
//!
//! 第 109 单量的是灵敏度，这一单量**候选漏斗**。八条实测：
//!
//! 1. **A/B 分界是严格大于**（`the_tier_boundary_is_strictly_greater_than_the_threshold()`）。
//! 2. **`Active` 与 `NearlyActive` 同权**（`both_active_states_share_the_dual_rule()`）。
//! 3. **只有 `Inactive` 与 `SatisfiedWithoutSlackMetric` 到 C**（`only_the_slackless_states_reach_tier_c()`）。
//! 4. **没有灵敏度报告就没有分层**（`no_sensitivity_means_no_tier_claim()`），C 除外。
//! 5. **排序是层级 → |dual| 降序 → 身份升序**（`the_ordering_is_tier_then_dual_then_identity()`）。
//! 6. **短名单先放 A/B**（`the_shortlist_prefers_the_strong_tiers()`）。
//! 7. **`local_effective` 由漏斗自己重算**（`the_local_effectiveness_is_recomputed_here()`）。
//! 8. **配置的候选上限不被强制**（`the_candidate_limit_is_not_enforced()`）。
//!
//! Delivery 109 measured the sensitivity; this one measures **the candidate funnel**. **The tier boundary is strictly
//! greater.** **Both active states share the dual rule.** **Only the slackless states reach Tier C.** **No sensitivity means
//! no tier claim** apart from C. **The ordering is tier, then dual, then identity.** **The shortlist prefers the strong
//! tiers.** **The local effectiveness is recomputed here.** And **the candidate limit is not enforced.**

use ospf_rust_core::analysis::activity::{
    ActivityEvidence, ActivityGroupSummary, ActivitySummary, ConstraintActivity,
    ConstraintActivityReport,
};
use ospf_rust_core::analysis::fixed_integer::{
    LocalConstraintSensitivity, LocalConstraintSensitivityReport, SensitivityScope,
};
use ospf_rust_core::analysis::pipeline::{
    build_candidate_funnel, CandidateFunnelConfig, CandidateTier, ConstraintCandidate,
};
use ospf_rust_core::analysis::{ActivityStatus, AnalysisStatus};
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个候选输入的描述。 / The description of one candidate input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelSortCandidateSpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 活动性状态名。 / The activity status name.
    pub status: String,
    /// 约束组（缺省为未分组）。 / The constraint group.
    #[serde(default)]
    pub group: Option<String>,
    /// 固定整数 LP 对偶值（缺省表示没有）。 / The dual value.
    #[serde(default)]
    pub dual: Option<f64>,
}

/// 候选漏斗探针请求。 / The candidate funnel probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelProbeRequest {
    /// |dual| 阈值（默认 1e-9）。 / The dual threshold.
    #[serde(default)]
    pub dual_threshold: Option<f64>,
    /// 昂贵分析的候选上限（默认 32）。 / The candidate limit for expensive analysis.
    #[serde(default)]
    pub default_candidate_limit: Option<u32>,
    /// 候选列表。 / The candidate list.
    #[serde(default)]
    pub candidates: Vec<FunnelSortCandidateSpec>,
    /// 短名单名额（缺省不取短名单）。 / The shortlist size; absent means no shortlist.
    #[serde(default)]
    pub shortlist: Option<u32>,
    /// 是否传入灵敏度报告（默认真，当有对偶时）。 / Whether the sensitivity report is supplied.
    #[serde(default)]
    pub with_sensitivity: Option<bool>,
}

/// 一条排序后的候选读数。 / One ordered candidate reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelCandidateRow {
    /// 稳定身份。 / The stable identity.
    pub id: String,
    /// 层级名（`A` / `B` / `C` / `UNCLASSIFIED`）。 / The tier name.
    pub tier: String,
    /// 排序位置。 / The position.
    pub priority: u32,
    /// 局部有效性（`null` 表示没有对偶）。 / The local effectiveness.
    #[serde(default)]
    pub local_effective: Option<bool>,
}

/// 候选漏斗探针结论。 / The candidate funnel probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelProbeReport {
    /// 排序后的候选。 / The ordered candidates.
    pub candidates: Vec<FunnelCandidateRow>,
    /// 四个层级的计数。 / The four tier counts.
    pub tier_a: u32,
    pub tier_b: u32,
    pub tier_c: u32,
    pub unclassified: u32,
    /// 短名单里的身份。 / The shortlisted identities.
    pub shortlist: Vec<String>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_tier_boundary_is_strictly_greater_than_the_threshold: bool,
    pub both_active_states_share_the_dual_rule: bool,
    pub only_the_slackless_states_reach_tier_c: bool,
    pub no_sensitivity_means_no_tier_claim: bool,
    pub the_ordering_is_tier_then_dual_then_identity: bool,
    pub the_shortlist_prefers_the_strong_tiers: bool,
    pub the_local_effectiveness_is_recomputed_here: bool,
    pub the_candidate_limit_is_not_enforced: bool,
}

/// 按名字取活动性状态。 / Resolve an activity status by name.
fn status_named(name: &str) -> Option<ActivityStatus> {
    match name {
        "Active" => Some(ActivityStatus::Active),
        "NearlyActive" => Some(ActivityStatus::NearlyActive),
        "Inactive" => Some(ActivityStatus::Inactive),
        "SatisfiedWithoutSlackMetric" => Some(ActivityStatus::SatisfiedWithoutSlackMetric),
        "Violated" => Some(ActivityStatus::Violated),
        "Unknown" => Some(ActivityStatus::Unknown),
        _ => None,
    }
}

/// 层级名。 / The tier name.
fn tier_name(tier: CandidateTier) -> String {
    tier.as_str().to_owned()
}

/// 造一条约束活动性记录。 / Build one constraint activity record.
fn activity_of(spec: &FunnelSortCandidateSpec, status: ActivityStatus) -> ConstraintActivity {
    ConstraintActivity {
        constraint_id: StableConstraintId(spec.id.clone()),
        group: spec.group.clone(),
        status,
        lhs: Some(0.0),
        rhs: Some(1.0),
        slack: Some(1.0),
        normalized_slack: Some(1.0),
        tolerance: Some(1e-9),
        satisfied: Some(true),
        evidence: ActivityEvidence::LinearConstraint { lhs: 0.0, rhs: 1.0 },
    }
}

/// 造一份活动性报告（计数与分组都按记录重算）。 / Build an activity report whose counts match its records.
fn activity_report_of(records: Vec<ConstraintActivity>) -> ConstraintActivityReport {
    let mut summary = ActivitySummary::default();
    let mut groups: std::collections::BTreeMap<Option<String>, ActivitySummary> =
        std::collections::BTreeMap::new();
    for record in &records {
        summary.add(record.status);
        groups.entry(record.group.clone()).or_default().add(record.status);
    }
    ConstraintActivityReport {
        schema_version: "critical-analysis".to_owned(),
        constraints: records,
        variable_bounds: Vec::new(),
        summary,
        variable_bound_summary: ActivitySummary::default(),
        groups: groups.into_iter()
            .map(|(group, summary)| ActivityGroupSummary { group, summary })
            .collect(),
        tolerance: 1e-9,
        nearly_active_tolerance: 1e-6,
    }
}

/// 造一份灵敏度报告。`local_effective` 故意写反，用来证明漏斗自己重算。
/// Build a sensitivity report; `local_effective` is deliberately inverted to prove the funnel recomputes it.
fn sensitivity_report_of(specs: &[FunnelSortCandidateSpec]) -> LocalConstraintSensitivityReport {
    LocalConstraintSensitivityReport {
        schema_version: "sensitivity".to_owned(),
        scope: SensitivityScope::FixedIntegerIncumbent,
        baseline_objective: 0.0,
        lp_objective: None,
        fixed_integer_count: 0,
        constraints: specs.iter().map(|spec| LocalConstraintSensitivity {
            constraint_id: StableConstraintId(spec.id.clone()),
            dual_value: spec.dual,
            // 故意与阈值判断相反。 / Deliberately opposite to the threshold verdict.
            local_effective: Some(false),
            status: AnalysisStatus::Unknown,
            scope: SensitivityScope::FixedIntegerIncumbent,
            activity: None,
        }).collect(),
        status: AnalysisStatus::Unknown,
        termination_reason: None,
        unavailable_reason: None,
    }
}

/// 走一遍候选漏斗探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_funnel_probe(request: &FunnelProbeRequest) -> Result<FunnelProbeReport> {
    if request.candidates.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "funnel probe failed validation: \
            at most eight candidates are accepted"));
    }
    let findings = canonical_findings();
    let config = CandidateFunnelConfig {
        dual_threshold: request.dual_threshold.unwrap_or(1e-9),
        default_candidate_limit: usize::try_from(request.default_candidate_limit.unwrap_or(32))
            .map_err(|_| BridgeError::new("InvalidInput", "candidate limit overflow"))?,
    };
    let mut records = Vec::with_capacity(request.candidates.len());
    for spec in &request.candidates {
        let status = status_named(&spec.status).ok_or_else(|| BridgeError::new("InvalidInput",
            "funnel probe failed validation: unknown activity status"))?;
        records.push(activity_of(spec, status));
    }
    let activity = activity_report_of(records);
    let sensitivity = sensitivity_report_of(&request.candidates);
    let supplied = request.with_sensitivity.unwrap_or(true);
    let ranking = build_candidate_funnel(&activity, supplied.then_some(&sensitivity), &config)
        .map_err(|error| BridgeError::new("InvalidInput", error))?;
    let shortlist = request.shortlist.map(|limit| {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        ranking.shortlist(limit).into_iter()
            .map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>()
    }).unwrap_or_default();
    Ok(FunnelProbeReport {
        candidates: ranking.candidates.iter().map(|candidate| FunnelCandidateRow {
            id: candidate.constraint_id.0.clone(),
            tier: tier_name(candidate.tier),
            priority: u32::try_from(candidate.priority).unwrap_or(u32::MAX),
            local_effective: candidate.local_effective,
        }).collect(),
        tier_a: u32::try_from(ranking.tier_a).unwrap_or(u32::MAX),
        tier_b: u32::try_from(ranking.tier_b).unwrap_or(u32::MAX),
        tier_c: u32::try_from(ranking.tier_c).unwrap_or(u32::MAX),
        unclassified: u32::try_from(ranking.unclassified).unwrap_or(u32::MAX),
        shortlist,
        the_tier_boundary_is_strictly_greater_than_the_threshold:
            findings.the_tier_boundary_is_strictly_greater_than_the_threshold,
        both_active_states_share_the_dual_rule: findings.both_active_states_share_the_dual_rule,
        only_the_slackless_states_reach_tier_c: findings.only_the_slackless_states_reach_tier_c,
        no_sensitivity_means_no_tier_claim: findings.no_sensitivity_means_no_tier_claim,
        the_ordering_is_tier_then_dual_then_identity:
            findings.the_ordering_is_tier_then_dual_then_identity,
        the_shortlist_prefers_the_strong_tiers: findings.the_shortlist_prefers_the_strong_tiers,
        the_local_effectiveness_is_recomputed_here:
            findings.the_local_effectiveness_is_recomputed_here,
        the_candidate_limit_is_not_enforced: findings.the_candidate_limit_is_not_enforced,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_tier_boundary_is_strictly_greater_than_the_threshold: bool,
    both_active_states_share_the_dual_rule: bool,
    only_the_slackless_states_reach_tier_c: bool,
    no_sensitivity_means_no_tier_claim: bool,
    the_ordering_is_tier_then_dual_then_identity: bool,
    the_shortlist_prefers_the_strong_tiers: bool,
    the_local_effectiveness_is_recomputed_here: bool,
    the_candidate_limit_is_not_enforced: bool,
}

/// 造一个候选输入。 / Build one candidate input.
fn spec(id: &str, status: &str, dual: Option<f64>) -> FunnelSortCandidateSpec {
    FunnelSortCandidateSpec { id: id.to_owned(), status: status.to_owned(), group: None, dual }
}

/// 跑一次漏斗。 / Run the funnel once.
fn funnel(specs: &[FunnelSortCandidateSpec], threshold: f64, limit: usize,
    with_sensitivity: bool) -> Vec<ConstraintCandidate>
{
    let records = specs.iter().map(|spec| {
        activity_of(spec, status_named(&spec.status).expect("status"))
    }).collect();
    let activity = activity_report_of(records);
    let sensitivity = sensitivity_report_of(specs);
    let config = CandidateFunnelConfig { dual_threshold: threshold, default_candidate_limit: limit };
    build_candidate_funnel(&activity, with_sensitivity.then_some(&sensitivity), &config)
        .expect("funnel builds").candidates
}

/// 取身份序列。 / Take the identity sequence.
fn ids(candidates: &[ConstraintCandidate]) -> Vec<String> {
    candidates.iter().map(|candidate| candidate.constraint_id.0.clone()).collect()
}

/// 默认阈值。 / The default threshold.
const THRESHOLD: f64 = 1e-9;

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) A/B 分界严格大于。 / The A/B boundary is strictly greater.
        let the_tier_boundary_is_strictly_greater_than_the_threshold = {
            let at = funnel(&[spec("c1", "Active", Some(THRESHOLD))], THRESHOLD, 32, true);
            let above = funnel(&[spec("c1", "Active", Some(THRESHOLD * 2.0))], THRESHOLD, 32, true);
            let below = funnel(&[spec("c1", "Active", Some(THRESHOLD / 2.0))], THRESHOLD, 32, true);
            // 负对偶按绝对值判。 / A negative dual is judged by magnitude.
            let negative = funnel(&[spec("c1", "Active", Some(-THRESHOLD * 2.0))], THRESHOLD, 32,
                true);
            at[0].tier == CandidateTier::TierB && above[0].tier == CandidateTier::TierA
                && below[0].tier == CandidateTier::TierB
                && negative[0].tier == CandidateTier::TierA
        };
        // 2) 两个 active 状态同权。 / Both active states share the rule.
        let both_active_states_share_the_dual_rule = {
            let strong = THRESHOLD * 2.0;
            let active = funnel(&[spec("c1", "Active", Some(strong))], THRESHOLD, 32, true);
            let nearly = funnel(&[spec("c1", "NearlyActive", Some(strong))], THRESHOLD, 32, true);
            let weak_active = funnel(&[spec("c1", "Active", Some(0.0))], THRESHOLD, 32, true);
            let weak_nearly = funnel(&[spec("c1", "NearlyActive", Some(0.0))], THRESHOLD, 32, true);
            active[0].tier == nearly[0].tier && active[0].tier == CandidateTier::TierA
                && weak_active[0].tier == weak_nearly[0].tier
                && weak_active[0].tier == CandidateTier::TierB
        };
        // 3) 只有无 slack 的两个状态到 C。 / Only the two slackless states reach C.
        let only_the_slackless_states_reach_tier_c = {
            let of = |status: &str| funnel(&[spec("c1", status, None)], THRESHOLD, 32, false)[0]
                .tier;
            of("Inactive") == CandidateTier::TierC
                && of("SatisfiedWithoutSlackMetric") == CandidateTier::TierC
                && of("Violated") == CandidateTier::Unclassified
                && of("Unknown") == CandidateTier::Unclassified
        };
        // 4) 没有灵敏度就没有分层（C 除外）。 / No sensitivity means no tier claim, C excepted.
        let no_sensitivity_means_no_tier_claim = {
            let bare = funnel(&[spec("c1", "Active", Some(1.0)), spec("c2", "Active", Some(0.0)),
                spec("c3", "Inactive", None)], THRESHOLD, 32, false);
            // 有对偶也没用：报告缺失时一律未分层。 / Duals do not help: without the report all stay unclassified.
            let tiers: Vec<CandidateTier> = bare.iter().map(|candidate| candidate.tier).collect();
            tiers == vec![CandidateTier::TierC, CandidateTier::Unclassified,
                CandidateTier::Unclassified]
                && bare.iter().all(|candidate| candidate.local_effective.is_none())
        };
        // 5) 排序：层级 → |dual| 降序 → 身份升序。 / Tier, then dual descending, then identity.
        let the_ordering_is_tier_then_dual_then_identity = {
            let ordered = funnel(&[
                spec("z", "Inactive", None),
                spec("a", "Active", Some(1.0)),
                spec("b", "Active", Some(5.0)),
                // 同层级同对偶 → 身份升序。 / Same tier and dual: identity ascending.
                spec("c", "Active", Some(1.0)),
                spec("d", "Active", Some(0.0)),
            ], THRESHOLD, 32, true);
            ids(&ordered) == vec!["b".to_owned(), "a".to_owned(), "c".to_owned(),
                "d".to_owned(), "z".to_owned()]
                && ordered.iter().enumerate()
                    .all(|(position, candidate)| candidate.priority == position)
        };
        // 6) 短名单先放 A/B。 / The shortlist prefers A/B.
        let the_shortlist_prefers_the_strong_tiers = {
            let records: Vec<_> = [spec("a", "Active", Some(1.0)), spec("b", "Inactive", None),
                spec("c", "Violated", None), spec("d", "Active", Some(0.0))]
                .into_iter().map(|spec| {
                    activity_of(&spec, status_named(&spec.status).expect("status")) })
                .collect();
            let activity = activity_report_of(records);
            let specs = [spec("a", "Active", Some(1.0)), spec("b", "Inactive", None),
                spec("c", "Violated", None), spec("d", "Active", Some(0.0))];
            let sensitivity = sensitivity_report_of(&specs);
            let config = CandidateFunnelConfig { dual_threshold: THRESHOLD,
                default_candidate_limit: 32 };
            let ranking = build_candidate_funnel(&activity, Some(&sensitivity), &config)
                .expect("funnel builds");
            let short = |limit: usize| ranking.shortlist(limit).into_iter()
                .map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>();
            // 名额 2 → 只放 A/B；名额 4 → 再用 C/未分层补满。 
            // Two slots take A/B only; four slots fill up with C and unclassified.
            short(2) == vec!["a".to_owned(), "d".to_owned()]
                && short(4).len() == 4
                && short(0).is_empty()
                && CandidateTier::TierA.is_shortlist_default()
                && CandidateTier::TierB.is_shortlist_default()
                && !CandidateTier::TierC.is_shortlist_default()
                && !CandidateTier::Unclassified.is_shortlist_default()
        };
        // 7) local_effective 由漏斗重算。 / The funnel recomputes local effectiveness.
        let the_local_effectiveness_is_recomputed_here = {
            // 灵敏度报告里写的是 `Some(false)`，漏斗按阈值给出 `Some(true)`。 
            // The report says `Some(false)` while the funnel says `Some(true)`.
            let candidates = funnel(&[spec("c1", "Active", Some(1.0)),
                spec("c2", "Active", Some(0.0)), spec("c3", "Inactive", None)],
                THRESHOLD, 32, true);
            let report = sensitivity_report_of(&[spec("c1", "Active", Some(1.0))]);
            candidates[0].local_effective == Some(true)
                && candidates[1].local_effective == Some(false)
                && candidates[2].local_effective.is_none()
                && report.constraints[0].local_effective == Some(false)
        };
        // 8) 配置的候选上限不被强制。 / The candidate limit is not enforced.
        let the_candidate_limit_is_not_enforced = {
            let many: Vec<FunnelSortCandidateSpec> = (0..5)
                .map(|index| spec(&format!("c{index}"), "Active", Some(1.0))).collect();
            // 上限设 1，漏斗仍返回全部 5 条。 / A limit of one still yields all five candidates.
            let capped = funnel(&many, THRESHOLD, 1, true);
            capped.len() == 5
                && CandidateFunnelConfig::default().default_candidate_limit == 32
                && CandidateFunnelConfig::default().validate().is_ok()
                && CandidateFunnelConfig { dual_threshold: -1.0,
                    default_candidate_limit: 32 }.validate().is_err()
        };
        Findings {
            the_tier_boundary_is_strictly_greater_than_the_threshold,
            both_active_states_share_the_dual_rule,
            only_the_slackless_states_reach_tier_c,
            no_sensitivity_means_no_tier_claim,
            the_ordering_is_tier_then_dual_then_identity,
            the_shortlist_prefers_the_strong_tiers,
            the_local_effectiveness_is_recomputed_here,
            the_candidate_limit_is_not_enforced,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: FunnelProbeRequest) -> FunnelProbeReport {
        verify_funnel_probe(&request).unwrap()
    }

    fn spec(id: &str, status: &str, dual: Option<f64>) -> FunnelSortCandidateSpec {
        FunnelSortCandidateSpec { id: id.to_owned(), status: status.to_owned(), group: None,
            dual }
    }

    fn request() -> FunnelProbeRequest {
        FunnelProbeRequest { dual_threshold: None, default_candidate_limit: None,
            candidates: vec![spec("a", "Active", Some(1.0)), spec("b", "Active", Some(0.0))],
            shortlist: None, with_sensitivity: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert!(report.the_tier_boundary_is_strictly_greater_than_the_threshold);
        assert!(report.both_active_states_share_the_dual_rule);
        assert!(report.only_the_slackless_states_reach_tier_c);
        assert!(report.no_sensitivity_means_no_tier_claim);
        assert!(report.the_ordering_is_tier_then_dual_then_identity);
        assert!(report.the_shortlist_prefers_the_strong_tiers);
        assert!(report.the_local_effectiveness_is_recomputed_here);
        assert!(report.the_candidate_limit_is_not_enforced);
        // 对偶 1 落 A、0 落 B；排序把 A 放前面。 / A dual of one is tier A, zero is tier B.
        assert_eq!(report.tier_a, 1);
        assert_eq!(report.tier_b, 1);
        assert_eq!(report.candidates[0].id, "a");
        assert_eq!(report.candidates[0].tier, "A");
        assert_eq!(report.candidates[1].tier, "B");
    }

    #[test]
    fn the_tier_boundary_is_strict() {
        let at = ask(FunnelProbeRequest { dual_threshold: Some(1.0),
            candidates: vec![spec("c1", "Active", Some(1.0))], ..request() });
        assert_eq!(at.candidates[0].tier, "B");
        let above = ask(FunnelProbeRequest { dual_threshold: Some(1.0),
            candidates: vec![spec("c1", "Active", Some(2.0))], ..request() });
        assert_eq!(above.candidates[0].tier, "A");
        // 负对偶按绝对值。 / A negative dual is judged by magnitude.
        let negative = ask(FunnelProbeRequest { dual_threshold: Some(1.0),
            candidates: vec![spec("c1", "NearlyActive", Some(-2.0))], ..request() });
        assert_eq!(negative.candidates[0].tier, "A");
    }

    #[test]
    fn the_slackless_states_alone_reach_tier_c() {
        let report = ask(FunnelProbeRequest { candidates: vec![
            spec("a", "Inactive", None), spec("b", "SatisfiedWithoutSlackMetric", None),
            spec("c", "Violated", None), spec("d", "Unknown", None)],
            with_sensitivity: Some(false), ..request() });
        assert_eq!(report.tier_c, 2);
        assert_eq!(report.unclassified, 2);
    }

    #[test]
    fn no_sensitivity_means_no_tier_claim() {
        let report = ask(FunnelProbeRequest { candidates: vec![
            spec("a", "Active", Some(9.0)), spec("b", "Inactive", None)],
            with_sensitivity: Some(false), ..request() });
        assert_eq!(report.tier_a, 0);
        assert_eq!(report.tier_b, 0);
        // C 仍按活动性给出，其余一律未分层。 / C still comes from activity; the rest stay unclassified.
        assert_eq!(report.tier_c, 1);
        assert_eq!(report.unclassified, 1);
        assert!(report.candidates.iter().all(|row| row.local_effective.is_none()));
    }

    #[test]
    fn the_ordering_and_the_shortlist_are_deterministic() {
        let report = ask(FunnelProbeRequest { shortlist: Some(2), candidates: vec![
            spec("z", "Inactive", None), spec("a", "Active", Some(1.0)),
            spec("b", "Active", Some(5.0)), spec("c", "Active", Some(1.0)),
            spec("d", "Active", Some(0.0))], ..request() });
        assert_eq!(report.candidates.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["b", "a", "c", "d", "z"]);
        assert_eq!(report.candidates.iter().map(|row| row.priority).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]);
        // 名额 2 → 只放 A/B。 / Two slots take A/B only.
        assert_eq!(report.shortlist, vec!["b".to_owned(), "a".to_owned()]);
        let wide = ask(FunnelProbeRequest { shortlist: Some(5), candidates: vec![
            spec("a", "Active", Some(1.0)), spec("z", "Inactive", None)], ..request() });
        assert_eq!(wide.shortlist.len(), 2);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_funnel_probe(&FunnelProbeRequest { candidates: vec![
            spec("c1", "Nearly", None)], ..request() }).unwrap_err()
            .message.contains("unknown activity status"));
        assert!(verify_funnel_probe(&FunnelProbeRequest {
            candidates: (0..9).map(|index| spec(&format!("c{index}"), "Active", None)).collect(),
            ..request() }).unwrap_err().message.contains("at most eight candidates"));
        // 负阈值被配置校验拒。 / A negative threshold is refused by the configuration.
        assert!(verify_funnel_probe(&FunnelProbeRequest {
            dual_threshold: Some(-1.0), ..request() }).unwrap_err()
            .message.contains("finite and non-negative"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"tierA\""), "{json}");
        assert!(json.contains("\"theTierBoundaryIsStrictlyGreaterThanTheThreshold\""), "{json}");
        assert!(!json.contains("tier_a"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<FunnelProbeReport>(&copy.to_string()).is_err());
    }
}
