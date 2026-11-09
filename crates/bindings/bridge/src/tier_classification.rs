//! 续挖第 36 单：候选分层的**推导规则**与"降权但不丢弃"的短名单（`analysis/pipeline.rs` 的
//! `classify_candidate` / `CandidateTier` / `CandidateFunnelRanking::shortlist`）
//! Continuation, delivery 36: how a candidate tier is derived, and the shortlist that deprioritises without dropping.
//!
//! 第 2 单绑了 `summarize_groups`、`analysis_funnel` 绑的是**从 JSON 读一份既有排序**，而把
//! "活动性 + 对偶证据"变成分层的 `classify_candidate` 一直没绑。实测出五条：
//!
//! 1. **active / nearly-active 必须有对偶证据才能分层**：有活动性但**没有**对偶 → `Unclassified`，而不是 TierB。
//!    "未知对偶"绝不能当成"对偶很小"。
//! 2. **TierA 与 TierB 是互补的一半**：有对偶时，`|dual| > threshold` → TierA，否则（**含恰好相等**）→ TierB。
//!    这个跨界用的是**严格大于**，所以 `|dual| == threshold` 落在 TierB——而 `CandidateFunnelRanking::validate`
//!    对 TierA 要求 `> threshold`、对 TierB 要求 `<= threshold`，两边严丝合缝。
//! 3. **TierC 只给两种活动性**：`Inactive` 与 `SatisfiedWithoutSlackMetric`。**`Violated` 不是 TierC**，
//!    而是 `Unclassified`——被违反的约束拿不到"可以做廉价分析"的分层，`Unknown` 同理。
//! 4. **短名单降权但不丢弃**（`shortlist(limit)`）：先放 TierA/TierB，名额还有就补 TierC/Unclassified。
//!    名额够时**全部返回**，所以"没进短名单"永远只表示"这次名额不够"，不表示"不重要"。
//! 5. **在线序化名字与枚举名不一致**：`CandidateTier::as_str()` 给 `"A"` / `"B"` / `"C"` / `"UNCLASSIFIED"`，
//!    而 serde（`SCREAMING_SNAKE_CASE`）给 `"TIER_A"` / `"TIER_B"` / `"TIER_C"` / `"UNCLASSIFIED"`。
//!    两个字段都在本单的读回里，别混用。
//!
//! Delivery 2 bound `summarize_groups` and `analysis_funnel` reads an **existing** ranking from JSON; the derivation
//! `classify_candidate` that turns activity plus dual evidence into a tier was never bound. Five measured rules. **Activity
//! alone cannot make a tier**: an active candidate **without** a dual is `Unclassified`, not TierB, so "unknown dual" is
//! never read as "small dual". **TierA and TierB are two complementary halves**: with a dual, `|dual| > threshold` gives
//! TierA and everything else (**including equality**) gives TierB, and the strict comparison lines up exactly with
//! `validate`'s "TierA needs above the threshold" / "TierB needs at or below". **TierC covers exactly two activity states**:
//! `Inactive` and `SatisfiedWithoutSlackMetric` — **`Violated` is not TierC** but `Unclassified`, as is `Unknown`, so a
//! violated constraint gets no cheap-analysis tier. And **the shortlist deprioritises without dropping**: `shortlist(limit)`
//! places TierA/TierB first and appends the rest while slots remain, so with enough slots **everything** comes back and "not in
//! the shortlist" only ever means "no slot this time". Note also that **the wire names disagree with the enum labels**:
//! `as_str()` yields `"A"` / `"B"` / `"C"` / `"UNCLASSIFIED"` while serde (`SCREAMING_SNAKE_CASE`) yields `"TIER_A"` /
//! `"TIER_B"` / `"TIER_C"` / `"UNCLASSIFIED"` — both are in this delivery's read-back, so do not mix them up.

use ospf_rust_core::analysis::activity::ActivityStatus;
use ospf_rust_core::analysis::pipeline::{
    classify_candidate, CandidateFunnelConfig, CandidateFunnelRanking, CandidateTier,
    ConstraintCandidate,
};
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个候选的字面量。 / One candidate literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSpec {
    /// 约束稳定 ID。 / The stable constraint ID.
    pub constraint_id: String,
    /// 活动性名（`active` / `nearlyActive` / `inactive` / `satisfiedWithoutSlackMetric` / `violated` /
    /// `unknown`）。 / The activity name.
    pub activity: String,
    /// 对偶值；缺省表示没有对偶证据。 / The dual value; absent means no dual evidence.
    #[serde(default)]
    pub dual_value: Option<f64>,
}

/// 分层请求。 / The classification request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TierClassificationRequest {
    /// 对偶阈值（默认 `1e-9`）。 / The dual threshold (default `1e-9`).
    #[serde(default)]
    pub dual_threshold: f64,
    /// 候选列表。 / The candidates.
    pub candidates: Vec<CandidateSpec>,
    /// 短名单名额。 / The shortlist size.
    #[serde(default)]
    pub shortlist_limit: u32,
}

/// 一行的分层结论。 / One row's classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TierRow {
    /// 约束稳定 ID。 / The stable constraint ID.
    pub constraint_id: String,
    /// 活动性名。 / The activity name.
    pub activity: String,
    /// 对偶值。 / The dual value.
    #[serde(default)]
    pub dual_value: Option<f64>,
    /// 分层名（`as_str`：`A` / `B` / `C` / `UNCLASSIFIED`）。 / The tier label.
    pub tier: String,
    /// 分层名（serde 线序：`TIER_A` / `TIER_B` / `TIER_C` / `UNCLASSIFIED`）。 / The tier wire name.
    pub tier_wire: String,
    /// 是否默认进短名单。 / Whether it is a shortlist default.
    pub shortlist_default: bool,
    /// 优先级（等于下标）。 / The priority (equal to the index).
    pub priority: u32,
}

/// 分层结论。 / The classification verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TierClassificationReport {
    /// 每一行。 / Every row.
    pub rows: Vec<TierRow>,
    /// Tier A / B / C / Unclassified 的计数。 / The tier counts.
    pub tier_a: u32,
    /// Tier B 计数。 / The Tier B count.
    pub tier_b: u32,
    /// Tier C 计数。 / The Tier C count.
    pub tier_c: u32,
    /// 未分层计数。 / The unclassified count.
    pub unclassified: u32,
    /// 短名单里的稳定 ID。 / The shortlist's stable IDs.
    pub shortlist: Vec<String>,
    /// 名额够时短名单返回全部（**是**）。 / Whether a large enough shortlist returns everything (**yes**).
    pub shortlist_returns_everything_when_wide: bool,
    /// 短名单先放 A/B（**是**）。 / Whether the shortlist prefers A/B first (**yes**).
    pub shortlist_prefers_defaults: bool,
    /// 活动性没有对偶 → 未分层（**是**）。 / Whether activity without a dual is unclassified (**yes**).
    pub activity_without_dual_is_unclassified: bool,
    /// 阈值上的对偶落在 Tier B（**是**）。 / Whether a dual exactly at the threshold is TierB (**yes**).
    pub threshold_equality_is_tier_b: bool,
    /// `violated` 不是 Tier C（**是**）。 / Whether `violated` is not TierC (**yes**).
    pub violated_is_not_tier_c: bool,
    /// `unknown` 是未分层（**是**）。 / Whether `unknown` is unclassified (**yes**).
    pub unknown_is_unclassified: bool,
    /// 未分层不是短名单默认（**是**）。 / Whether unclassified is not a shortlist default (**yes**).
    pub unclassified_is_not_a_shortlist_default: bool,
    /// 线序名与枚举名不同（**是**）。 / Whether the wire name differs from the label (**yes**).
    pub wire_name_differs_from_label: bool,
}

/// 候选条数上限（信任边界）。 / The candidate cap (a trust boundary).
const MAX_CANDIDATES: usize = 512;

/// 活动性名转枚举。 / Turn an activity name into the enum.
fn activity_of(name: &str) -> Result<ActivityStatus> {
    Ok(match name {
        "active" => ActivityStatus::Active,
        "nearlyActive" => ActivityStatus::NearlyActive,
        "inactive" => ActivityStatus::Inactive,
        "satisfiedWithoutSlackMetric" => ActivityStatus::SatisfiedWithoutSlackMetric,
        "violated" => ActivityStatus::Violated,
        "unknown" => ActivityStatus::Unknown,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "tier classification failed validation: unknown activity `{other}`"))),
    })
}

/// 分层名（`as_str`）。 / The tier label.
fn label(tier: CandidateTier) -> String {
    tier.as_str().to_owned()
}

/// 分层线序名（serde）。 / The tier wire name.
fn wire(tier: CandidateTier) -> String {
    serde_json::to_string(&tier).map(|text| text.trim_matches('"').to_owned())
        .unwrap_or_default()
}

/// 走一遍候选分层。非法输入是**错误**。 / Walk the candidate classification.
pub fn verify_tier_classification(
    request: &TierClassificationRequest,
) -> Result<TierClassificationReport> {
    if request.candidates.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "tier classification failed validation: at least one candidate is required"));
    }
    if request.candidates.len() > MAX_CANDIDATES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "tier classification failed validation: at most {MAX_CANDIDATES} candidates are accepted")));
    }
    let config = CandidateFunnelConfig { dual_threshold: request.dual_threshold,
        default_candidate_limit: usize::try_from(request.shortlist_limit).unwrap_or(usize::MAX) };
    config.validate().map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let mut seen = std::collections::BTreeSet::new();
    let mut entries = Vec::with_capacity(request.candidates.len());
    for spec in &request.candidates {
        if spec.constraint_id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "tier classification failed validation: constraint IDs must not be blank"));
        }
        if !seen.insert(spec.constraint_id.as_str()) {
            return Err(BridgeError::new("InvalidInput",
                "tier classification failed validation: the request repeats a constraint"));
        }
        if spec.dual_value.is_some_and(|value| !value.is_finite()) {
            return Err(BridgeError::new("InvalidInput",
                "tier classification failed validation: dual values must be finite"));
        }
        let activity = activity_of(&spec.activity)?;
        entries.push((spec, activity));
    }
    // 分层由领域自己的 `classify_candidate` 决定，桥接里不重写判据。
    // The tier comes from the domain's own `classify_candidate`; no predicate is re-written here.
    let candidates: Vec<ConstraintCandidate> = entries.iter().enumerate()
        .map(|(position, (spec, activity))| ConstraintCandidate {
            constraint_id: StableConstraintId(spec.constraint_id.clone()), group: None,
            tier: classify_candidate(*activity, spec.dual_value, request.dual_threshold),
            activity: *activity, slack: None, dual_value: spec.dual_value, local_effective: None,
            priority: position })
        .collect();
    let mut ranking = CandidateFunnelRanking { schema_version: "1.0".to_owned(),
        dual_threshold: request.dual_threshold, candidates, tier_a: 0, tier_b: 0, tier_c: 0,
        unclassified: 0 };
    for candidate in &ranking.candidates {
        match candidate.tier {
            CandidateTier::TierA => ranking.tier_a += 1,
            CandidateTier::TierB => ranking.tier_b += 1,
            CandidateTier::TierC => ranking.tier_c += 1,
            CandidateTier::Unclassified => ranking.unclassified += 1,
        }
    }
    // 计数补齐后必须通过领域自己的不变量校验——分层与计数是同一条规则的两种写法。
    // With the counts filled in the domain's own invariant check must pass: the tier and the counts are the same rule.
    ranking.validate().map_err(|error| BridgeError::new("InternalInvariant", &error))?;
    let shortlist: Vec<String> = ranking.shortlist(config.default_candidate_limit).iter()
        .map(|candidate| candidate.constraint_id.0.clone()).collect();
    let findings = canonical_findings(request.dual_threshold);
    Ok(TierClassificationReport {
        rows: ranking.candidates.iter().map(|candidate| TierRow {
            constraint_id: candidate.constraint_id.0.clone(),
            activity: activity_name(candidate.activity).to_owned(),
            dual_value: candidate.dual_value, tier: label(candidate.tier),
            tier_wire: wire(candidate.tier),
            shortlist_default: candidate.tier.is_shortlist_default(),
            priority: u32::try_from(candidate.priority).unwrap_or(u32::MAX) }).collect(),
        tier_a: u32::try_from(ranking.tier_a).unwrap_or(u32::MAX),
        tier_b: u32::try_from(ranking.tier_b).unwrap_or(u32::MAX),
        tier_c: u32::try_from(ranking.tier_c).unwrap_or(u32::MAX),
        unclassified: u32::try_from(ranking.unclassified).unwrap_or(u32::MAX),
        shortlist,
        shortlist_returns_everything_when_wide: findings.shortlist_returns_everything_when_wide,
        shortlist_prefers_defaults: findings.shortlist_prefers_defaults,
        activity_without_dual_is_unclassified: findings.activity_without_dual_is_unclassified,
        threshold_equality_is_tier_b: findings.threshold_equality_is_tier_b,
        violated_is_not_tier_c: findings.violated_is_not_tier_c,
        unknown_is_unclassified: findings.unknown_is_unclassified,
        unclassified_is_not_a_shortlist_default: findings.unclassified_is_not_a_shortlist_default,
        wire_name_differs_from_label: findings.wire_name_differs_from_label,
    })
}

/// 活动性名。 / The activity name.
fn activity_name(activity: ActivityStatus) -> &'static str {
    match activity {
        ActivityStatus::Active => "active",
        ActivityStatus::NearlyActive => "nearlyActive",
        ActivityStatus::Inactive => "inactive",
        ActivityStatus::SatisfiedWithoutSlackMetric => "satisfiedWithoutSlackMetric",
        ActivityStatus::Violated => "violated",
        ActivityStatus::Unknown => "unknown",
    }
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    shortlist_returns_everything_when_wide: bool,
    shortlist_prefers_defaults: bool,
    activity_without_dual_is_unclassified: bool,
    threshold_equality_is_tier_b: bool,
    violated_is_not_tier_c: bool,
    unknown_is_unclassified: bool,
    unclassified_is_not_a_shortlist_default: bool,
    wire_name_differs_from_label: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings(threshold: f64) -> Findings {
    let classify = |activity: ActivityStatus, dual: Option<f64>|
        classify_candidate(activity, dual, threshold);
    // 1) 活动性没有对偶 → 未分层。 / Activity without a dual is unclassified.
    let activity_without_dual_is_unclassified = classify(ActivityStatus::Active, None)
        == CandidateTier::Unclassified
        && classify(ActivityStatus::NearlyActive, None) == CandidateTier::Unclassified;
    // 2) 恰好等于阈值 → TierB（严格大于才 TierA）。 / Exactly at the threshold is TierB.
    let threshold_equality_is_tier_b = classify(ActivityStatus::Active, Some(threshold))
        == CandidateTier::TierB
        && classify(ActivityStatus::Active, Some(-threshold)) == CandidateTier::TierB
        && classify(ActivityStatus::Active, Some(threshold + 1.0)) == CandidateTier::TierA;
    // 3) `violated` 与 `unknown` 都不是 TierC。 / Neither `violated` nor `unknown` is TierC.
    let violated_is_not_tier_c = classify(ActivityStatus::Violated, None)
        == CandidateTier::Unclassified
        && classify(ActivityStatus::Inactive, None) == CandidateTier::TierC
        && classify(ActivityStatus::SatisfiedWithoutSlackMetric, None) == CandidateTier::TierC;
    let unknown_is_unclassified = classify(ActivityStatus::Unknown, None)
        == CandidateTier::Unclassified;
    // 4) 未分层不是短名单默认。 / Unclassified is not a shortlist default.
    let unclassified_is_not_a_shortlist_default = !CandidateTier::Unclassified.is_shortlist_default()
        && !CandidateTier::TierC.is_shortlist_default()
        && CandidateTier::TierA.is_shortlist_default()
        && CandidateTier::TierB.is_shortlist_default();
    // 5) 线序名与枚举名不同。 / The wire name differs from the label.
    let wire_name_differs_from_label = wire(CandidateTier::TierA) == "TIER_A"
        && label(CandidateTier::TierA) == "A"
        && wire(CandidateTier::Unclassified) == label(CandidateTier::Unclassified);
    // 6) 短名单：名额够时返回全部，且先放 A/B。 / The shortlist: everything with enough slots, A/B first.
    let probe = |tiers: &[CandidateTier]| {
        let candidates: Vec<ConstraintCandidate> = tiers.iter().enumerate()
            .map(|(position, tier)| ConstraintCandidate {
                constraint_id: StableConstraintId(format!("probe.{position}")), group: None,
                tier: *tier, activity: ActivityStatus::Unknown, slack: None,
                dual_value: Some(0.0), local_effective: None, priority: position })
            .collect();
        CandidateFunnelRanking { schema_version: "1.0".to_owned(), dual_threshold: threshold,
            candidates, tier_a: 0, tier_b: 0, tier_c: 0, unclassified: 0 }
    };
    let mixed = probe(&[CandidateTier::TierC, CandidateTier::TierA, CandidateTier::Unclassified,
        CandidateTier::TierB]);
    let shortlist_returns_everything_when_wide = mixed.shortlist(16).len() == 4;
    let shortlist_prefers_defaults = mixed.shortlist(2).iter()
        .all(|candidate| candidate.tier.is_shortlist_default())
        && mixed.shortlist(2).len() == 2
        && mixed.shortlist(0).is_empty();
    Findings { shortlist_returns_everything_when_wide, shortlist_prefers_defaults,
        activity_without_dual_is_unclassified, threshold_equality_is_tier_b, violated_is_not_tier_c,
        unknown_is_unclassified, unclassified_is_not_a_shortlist_default, wire_name_differs_from_label }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> TierClassificationRequest {
        TierClassificationRequest { dual_threshold: 1e-9, shortlist_limit: 2,
            candidates: vec![
                CandidateSpec { constraint_id: "c.hot".to_owned(), activity: "active".to_owned(),
                    dual_value: Some(3.0) },
                CandidateSpec { constraint_id: "c.warm".to_owned(), activity: "active".to_owned(),
                    dual_value: Some(0.0) },
                CandidateSpec { constraint_id: "c.blind".to_owned(),
                    activity: "nearlyActive".to_owned(), dual_value: None },
                CandidateSpec { constraint_id: "c.cold".to_owned(), activity: "inactive".to_owned(),
                    dual_value: None },
                CandidateSpec { constraint_id: "c.broken".to_owned(), activity: "violated".to_owned(),
                    dual_value: None },
            ] }
    }

    #[test]
    fn the_tiers_follow_the_evidence_and_nothing_else() {
        let report = verify_tier_classification(&request()).unwrap();
        assert_eq!(report.rows[0].tier, "A");
        assert_eq!(report.rows[1].tier, "B");
        // 有活动性但对偶缺失 → 未分层，而不是 B。 / Activity without a dual is unclassified, not B.
        assert_eq!(report.rows[2].tier, "UNCLASSIFIED");
        assert_eq!(report.rows[3].tier, "C");
        // 被违反的约束拿不到 TierC。 / A violated constraint gets no TierC.
        assert_eq!(report.rows[4].tier, "UNCLASSIFIED");
        assert_eq!(report.tier_a, 1);
        assert_eq!(report.tier_b, 1);
        assert_eq!(report.tier_c, 1);
        assert_eq!(report.unclassified, 2);
        assert!(report.activity_without_dual_is_unclassified);
        assert!(report.violated_is_not_tier_c);
        assert!(report.unknown_is_unclassified);
        assert!(report.threshold_equality_is_tier_b);
    }

    #[test]
    fn the_shortlist_deprioritises_without_dropping() {
        let report = verify_tier_classification(&request()).unwrap();
        // 名额 2 → 只放 A/B。 / Two slots hold A/B only.
        assert_eq!(report.shortlist, vec!["c.hot".to_owned(), "c.warm".to_owned()]);
        assert!(report.shortlist_prefers_defaults);
        assert!(report.shortlist_returns_everything_when_wide);
        assert!(report.unclassified_is_not_a_shortlist_default);
        // 名额够时全部返回。 / Enough slots return everything.
        let mut wide = request();
        wide.shortlist_limit = 99;
        assert_eq!(verify_tier_classification(&wide).unwrap().shortlist.len(), 5);
        // 名额 0 → 空。 / Zero slots give nothing.
        let mut none = request();
        none.shortlist_limit = 0;
        assert!(verify_tier_classification(&none).unwrap().shortlist.is_empty());
    }

    #[test]
    fn the_wire_name_differs_from_the_label() {
        let report = verify_tier_classification(&request()).unwrap();
        assert_eq!(report.rows[0].tier, "A");
        assert_eq!(report.rows[0].tier_wire, "TIER_A");
        assert_eq!(report.rows[2].tier, "UNCLASSIFIED");
        assert_eq!(report.rows[2].tier_wire, "UNCLASSIFIED");
        assert!(report.wire_name_differs_from_label);
        // 优先级必须等于下标（领域不变量在右侧替我们校验过）。 
        // The priority equals the index (the domain invariant already checked it).
        for (position, row) in report.rows.iter().enumerate() {
            assert_eq!(row.priority as usize, position);
        }
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.dual_threshold = 5.0;
        other.shortlist_limit = 1;
        other.candidates = vec![CandidateSpec { constraint_id: "only".to_owned(),
            activity: "active".to_owned(), dual_value: Some(5.0) }];
        let report = verify_tier_classification(&other).unwrap();
        // 恰好等于阈值 5.0 → B。 / Exactly at the threshold 5.0 gives B.
        assert_eq!(report.rows[0].tier, "B");
        assert!(report.shortlist_returns_everything_when_wide);
        assert!(report.shortlist_prefers_defaults);
        assert!(report.activity_without_dual_is_unclassified);
        assert!(report.threshold_equality_is_tier_b);
        assert!(report.violated_is_not_tier_c);
        assert!(report.unknown_is_unclassified);
        assert!(report.unclassified_is_not_a_shortlist_default);
        assert!(report.wire_name_differs_from_label);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut empty = request();
        empty.candidates = Vec::new();
        assert!(verify_tier_classification(&empty).unwrap_err().message
            .contains("at least one candidate"));
        let mut blank = request();
        blank.candidates[0].constraint_id = "  ".to_owned();
        assert!(verify_tier_classification(&blank).unwrap_err().message
            .contains("constraint IDs must not be blank"));
        let mut duplicate = request();
        duplicate.candidates[1].constraint_id = "c.hot".to_owned();
        assert!(verify_tier_classification(&duplicate).unwrap_err().message
            .contains("repeats a constraint"));
        let mut unknown = request();
        unknown.candidates[0].activity = "resting".to_owned();
        assert!(verify_tier_classification(&unknown).unwrap_err().message
            .contains("unknown activity"));
        let mut infinite = request();
        infinite.candidates[0].dual_value = Some(f64::INFINITY);
        assert!(verify_tier_classification(&infinite).unwrap_err().message
            .contains("dual values must be finite"));
        let mut negative = request();
        negative.dual_threshold = -1.0;
        assert!(verify_tier_classification(&negative).unwrap_err().message
            .contains("must be finite and non-negative"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_tier_classification(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"activityWithoutDualIsUnclassified\""), "{json}");
        assert!(json.contains("\"tierWire\""), "{json}");
        assert!(json.contains("\"tierA\""), "{json}");
        assert!(!json.contains("tier_a"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<TierClassificationReport>(&copy.to_string()).is_err());
    }
}
