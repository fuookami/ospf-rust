//! 续挖第 97 单：**候选漏斗的分层语义全在"活动性 + 对偶"的组合里**——`NearlyActive` 与 `Active` 共用一档，
//! **`NaN` 对偶会静静落进 Tier B**（`NaN > t` 为假），而"active 但没有对偶"只能是 `Unclassified`；
//! **`Inactive` 带巨大对偶也还是 Tier C**，`Violated` 却落到 `Unclassified`；`as_str()` 给的是 `A`/`UNCLASSIFIED`，
//! 而线上序列化名是 `TIER_A`；分类器**不校验阈值**（负阈值让一切都成 Tier A），配置**不校验上限**（0 也照收）
//! （`analysis/pipeline.rs` 的 `CandidateTier` / `CandidateFunnelConfig` / `ConstraintCandidate` /
//! `CandidateFunnelRanking` / `classify_candidate`）
//! Continuation, delivery 97: **the funnel's tiering lives entirely in the activity + dual combination** —
//! `NearlyActive` shares a bucket with `Active`, **a `NaN` dual quietly lands in Tier B** (`NaN > t` is false) while
//! "active without a dual" can only be `Unclassified`; **`Inactive` stays Tier C even with a huge dual** whereas
//! `Violated` falls through to `Unclassified`; `as_str()` says `A`/`UNCLASSIFIED` while the wire name is `TIER_A`; the
//! classifier **never checks the threshold** (a negative one makes everything Tier A) and the config **never checks the
//! limit** (zero is accepted) (`CandidateTier` / `CandidateFunnelConfig` / `ConstraintCandidate` /
//! `CandidateFunnelRanking` / `classify_candidate` in `analysis/pipeline.rs`).
//!
//! 第 96 单量的是带状态解，这一单量**候选漏斗**。八条实测：
//!
//! 1. **短层名与线上名不一致**（`the_short_tier_names_differ_from_the_wire_names()`）：`as_str()` 是
//!    `A`/`B`/`C`/`UNCLASSIFIED`，序列化是 `TIER_A`/`TIER_B`/`TIER_C`/`UNCLASSIFIED`。
//! 2. **`NearlyActive` 与 `Active` 共用一档**（`nearly_active_shares_the_active_bucket()`）。
//! 3. **active 但没有对偶只能是 `Unclassified`**（`an_active_candidate_without_a_dual_is_unclassified()`）：
//!    不是 Tier B。
//! 4. **`Inactive` 带巨大对偶也还是 Tier C**（`an_inactive_candidate_is_tier_c_whatever_its_dual()`）。
//! 5. **`NaN` 对偶落进 Tier B**（`a_nan_dual_lands_in_tier_b()`）：`NaN > 阈值` 为假。本单最值钱的一条。
//!    注意这条**只能由内部固定探针观察**——`NaN` 过不了 JSON，线上会被降成 `null`（于是变成"没有对偶"→
//!    未分层），SDK 侧的用例实测的正是这个降级。
//! 6. **分类器不校验阈值、配置不校验上限**（`the_classifier_ignores_the_threshold_and_the_config_the_limit()`）：
//!    负阈值让 active 候选一律进 Tier A（**分类器不查**，但做出来的排序会被 `validate()` 拒绝），
//!    而 `default_candidate_limit = 0` 照样通过配置校验。
//! 7. **短名单只按既有顺序取，不重排 tier**（`the_shortlist_keeps_the_existing_order()`）：先 Tier A/B（按
//!    `candidates` 既有顺序），名额还有余才补 Tier C/未分层，且永不超过上限。
//! 8. **`Violated` 落到 `Unclassified` 而不是 Tier C**（`a_violated_candidate_is_unclassified()`）。
//!
//! Delivery 96 measured the status-carrying solution; this one measures **the candidate funnel**. **The short tier names
//! differ from the wire names.** **`NearlyActive` shares the active bucket.** **Active without a dual can only be
//! `Unclassified`.** **`Inactive` stays Tier C even with a huge dual.** **A `NaN` dual lands in Tier B** — the most
//! valuable finding here, though it is only observable by the internal fixed probe because JSON degrades `NaN` to `null`
//! and the candidate then reads as having no dual. **The classifier never checks the threshold** (a negative one makes everything Tier A, though
//! the resulting ranking is refused) **and the config never checks the limit.** **The shortlist keeps the existing
//! order** rather than re-sorting by tier, and **`Violated` falls through to `Unclassified`**.

use ospf_rust_core::analysis::pipeline::{
    classify_candidate, CandidateFunnelConfig, CandidateFunnelRanking, CandidateTier,
    ConstraintCandidate,
};
use ospf_rust_core::analysis::ActivityStatus;
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条候选的输入。 / One candidate's input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelProbeSpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 活动性名（六个之一）。 / The activity name, one of six.
    pub activity: String,
    /// 对偶值（`null` 表示没有证据）。 / The dual value, `null` when there is no evidence.
    #[serde(default)]
    pub dual: Option<f64>,
}

/// 一条候选的分层读数。 / One candidate's tier reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelRow {
    /// 稳定身份。 / The stable identity.
    pub id: String,
    /// `as_str()` 的短名。 / The short name from `as_str()`.
    pub tier: String,
    /// 序列化后的线上名。 / The serialised wire name.
    pub wire: String,
    /// 排序位置。 / The rank position.
    pub priority: u32,
    /// 是否进了短名单。 / Whether it made the shortlist.
    pub shortlisted: bool,
}

/// 候选漏斗探针请求。 / The funnel probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelRequest {
    /// 候选列表（顺序即优先级）。 / The candidates, in priority order.
    #[serde(default)]
    pub candidates: Vec<FunnelProbeSpec>,
    /// 对偶阈值（默认 1e-9）。 / The dual threshold (default 1e-9).
    #[serde(default)]
    pub dual_threshold: Option<f64>,
    /// 短名单上限（默认 2）。 / The shortlist limit (default 2).
    #[serde(default)]
    pub shortlist_limit: Option<u32>,
}

/// 候选漏斗探针结论。 / The funnel probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunnelReport {
    /// 每条候选的读数。 / Every candidate's reading.
    pub rows: Vec<FunnelRow>,
    /// 四个层级计数。 / The four tier counts.
    pub tier_a: u32,
    pub tier_b: u32,
    pub tier_c: u32,
    pub unclassified: u32,
    /// 短名单的身份列表。 / The shortlist identities.
    pub shortlist: Vec<String>,
    /// 构建出来的排序是否通过 `validate()`。 / Whether the built ranking passes `validate()`.
    pub ranking_valid: bool,
    /// 计数被改坏之后是否被 `validate()` 拒绝。 / Whether a mangled count is refused by `validate()`.
    pub bad_counts_refused: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_short_tier_names_differ_from_the_wire_names: bool,
    pub nearly_active_shares_the_active_bucket: bool,
    pub an_active_candidate_without_a_dual_is_unclassified: bool,
    pub an_inactive_candidate_is_tier_c_whatever_its_dual: bool,
    pub a_nan_dual_lands_in_tier_b: bool,
    pub the_classifier_ignores_the_threshold_and_the_config_the_limit: bool,
    pub the_shortlist_keeps_the_existing_order: bool,
    pub a_violated_candidate_is_unclassified: bool,
}

/// 按名字取活动性。 / Resolve an activity by name.
fn activity_named(name: &str) -> Option<ActivityStatus> {
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

/// 把一条候选铺成漏斗条目。 / Lay one candidate out as a funnel entry.
fn candidate_of(spec: &FunnelProbeSpec, priority: usize, threshold: f64)
    -> Result<ConstraintCandidate>
{
    let activity = activity_named(&spec.activity).ok_or_else(|| BridgeError::new("InvalidInput",
        "funnel probe failed validation: unknown activity name"))?;
    Ok(ConstraintCandidate {
        constraint_id: StableConstraintId(spec.id.clone()),
        group: None,
        tier: classify_candidate(activity, spec.dual, threshold),
        activity,
        slack: None,
        dual_value: spec.dual,
        local_effective: None,
        priority,
    })
}

/// 上线名字（序列化）。 / The wire name, taken through serialisation.
fn wire_name(tier: CandidateTier) -> String {
    serde_json::to_value(tier).ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// 走一遍漏斗探针。非法输入是**错误**。 / Walk the funnel probe. An illegal input is an error.
pub fn verify_funnel(request: &FunnelRequest) -> Result<FunnelReport> {
    if request.candidates.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "funnel probe failed validation: \
            at most eight candidates are accepted"));
    }
    for spec in &request.candidates {
        if spec.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "funnel probe failed validation: \
                a candidate identity may not be blank"));
        }
    }
    let dual_threshold = request.dual_threshold.unwrap_or(1e-9);
    if !dual_threshold.is_finite() {
        return Err(BridgeError::new("InvalidInput", "funnel probe failed validation: \
            the dual threshold must be finite"));
    }
    let shortlist_limit = usize::try_from(request.shortlist_limit.unwrap_or(2))
        .map_err(|_| BridgeError::new("InvalidInput",
            "funnel probe failed validation: the limit overflowed"))?;
    if shortlist_limit > 8 {
        return Err(BridgeError::new("InvalidInput", "funnel probe failed validation: \
            the limit stays at eight"));
    }
    let findings = canonical_findings();
    let candidates = request.candidates.iter().enumerate()
        .map(|(priority, spec)| candidate_of(spec, priority, dual_threshold))
        .collect::<Result<Vec<_>>>()?;
    let count = |tier: CandidateTier| u32::try_from(candidates.iter()
        .filter(|candidate| candidate.tier == tier).count()).unwrap_or(u32::MAX);
    let ranking = CandidateFunnelRanking {
        schema_version: "funnel-probe".to_owned(),
        dual_threshold,
        candidates: candidates.clone(),
        tier_a: usize::try_from(count(CandidateTier::TierA)).unwrap_or(0),
        tier_b: usize::try_from(count(CandidateTier::TierB)).unwrap_or(0),
        tier_c: usize::try_from(count(CandidateTier::TierC)).unwrap_or(0),
        unclassified: usize::try_from(count(CandidateTier::Unclassified)).unwrap_or(0),
    };
    let ranking_valid = ranking.validate().is_ok();
    // 把计数改坏一位：`validate()` 必须拒绝。 / Mangle one count; `validate()` must refuse it.
    let mut mangled = ranking.clone();
    mangled.tier_a = mangled.tier_a.saturating_add(1);
    let bad_counts_refused = mangled.validate().is_err();
    let shortlisted: Vec<String> = ranking.shortlist(shortlist_limit).iter()
        .map(|candidate| candidate.constraint_id.0.clone()).collect();
    let rows = candidates.iter().map(|candidate| FunnelRow {
        id: candidate.constraint_id.0.clone(),
        tier: candidate.tier.as_str().to_owned(),
        wire: wire_name(candidate.tier),
        priority: u32::try_from(candidate.priority).unwrap_or(u32::MAX),
        shortlisted: shortlisted.contains(&candidate.constraint_id.0),
    }).collect();
    Ok(FunnelReport {
        rows,
        tier_a: count(CandidateTier::TierA),
        tier_b: count(CandidateTier::TierB),
        tier_c: count(CandidateTier::TierC),
        unclassified: count(CandidateTier::Unclassified),
        shortlist: shortlisted,
        ranking_valid,
        bad_counts_refused,
        the_short_tier_names_differ_from_the_wire_names:
            findings.the_short_tier_names_differ_from_the_wire_names,
        nearly_active_shares_the_active_bucket:
            findings.nearly_active_shares_the_active_bucket,
        an_active_candidate_without_a_dual_is_unclassified:
            findings.an_active_candidate_without_a_dual_is_unclassified,
        an_inactive_candidate_is_tier_c_whatever_its_dual:
            findings.an_inactive_candidate_is_tier_c_whatever_its_dual,
        a_nan_dual_lands_in_tier_b: findings.a_nan_dual_lands_in_tier_b,
        the_classifier_ignores_the_threshold_and_the_config_the_limit:
            findings.the_classifier_ignores_the_threshold_and_the_config_the_limit,
        the_shortlist_keeps_the_existing_order: findings.the_shortlist_keeps_the_existing_order,
        a_violated_candidate_is_unclassified: findings.a_violated_candidate_is_unclassified,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_short_tier_names_differ_from_the_wire_names: bool,
    nearly_active_shares_the_active_bucket: bool,
    an_active_candidate_without_a_dual_is_unclassified: bool,
    an_inactive_candidate_is_tier_c_whatever_its_dual: bool,
    a_nan_dual_lands_in_tier_b: bool,
    the_classifier_ignores_the_threshold_and_the_config_the_limit: bool,
    the_shortlist_keeps_the_existing_order: bool,
    a_violated_candidate_is_unclassified: bool,
}

/// 造一条漏斗条目。 / Build one funnel entry.
fn entry(id: &str, activity: ActivityStatus, dual: Option<f64>, priority: usize)
    -> ConstraintCandidate
{
    ConstraintCandidate {
        constraint_id: StableConstraintId(id.to_owned()),
        group: None,
        tier: classify_candidate(activity, dual, 1e-9),
        activity,
        slack: None,
        dual_value: dual,
        local_effective: None,
        priority,
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 短名与线上名不一致。 / The short names differ from the wire names.
        let the_short_tier_names_differ_from_the_wire_names = {
            CandidateTier::TierA.as_str() == "A"
                && CandidateTier::TierB.as_str() == "B"
                && CandidateTier::TierC.as_str() == "C"
                && CandidateTier::Unclassified.as_str() == "UNCLASSIFIED"
                && wire_name(CandidateTier::TierA) == "TIER_A"
                && wire_name(CandidateTier::TierC) == "TIER_C"
                // 只有未分层两边一致。 / Only the unclassified tier agrees.
                && wire_name(CandidateTier::Unclassified) == "UNCLASSIFIED"
        };
        // 2) `NearlyActive` 与 `Active` 共用一档。 / `NearlyActive` shares the active bucket.
        let nearly_active_shares_the_active_bucket = {
            classify_candidate(ActivityStatus::NearlyActive, Some(1.0), 1e-9)
                == CandidateTier::TierA
                && classify_candidate(ActivityStatus::Active, Some(1.0), 1e-9)
                    == CandidateTier::TierA
                // 对偶在阈值内两边都落 Tier B。 / Inside the threshold both land in Tier B.
                && classify_candidate(ActivityStatus::NearlyActive, Some(0.0), 1e-9)
                    == CandidateTier::TierB
        };
        // 3) active 但没有对偶只能是 Unclassified。 / Active without a dual is only ever Unclassified.
        let an_active_candidate_without_a_dual_is_unclassified = {
            classify_candidate(ActivityStatus::Active, None, 1e-9)
                == CandidateTier::Unclassified
                && classify_candidate(ActivityStatus::NearlyActive, None, 1e-9)
                    == CandidateTier::Unclassified
        };
        // 4) `Inactive` 带巨大对偶也还是 Tier C。 / `Inactive` stays Tier C with a huge dual.
        let an_inactive_candidate_is_tier_c_whatever_its_dual = {
            classify_candidate(ActivityStatus::Inactive, Some(1.0e9), 1e-9)
                == CandidateTier::TierC
                && classify_candidate(ActivityStatus::SatisfiedWithoutSlackMetric, Some(-1.0e9),
                    1e-9) == CandidateTier::TierC
        };
        // 5) `NaN` 对偶落进 Tier B。 / A `NaN` dual lands in Tier B.
        let a_nan_dual_lands_in_tier_b = {
            // `NaN > 阈值` 为假 → 走 Tier B 分支，而不是"证据不足"。 
            // `NaN > threshold` is false, so the Tier B branch is taken instead of "insufficient evidence".
            classify_candidate(ActivityStatus::Active, Some(f64::NAN), 1e-9)
                == CandidateTier::TierB
                // 对照：没有对偶才是 Unclassified。 / Control: only a missing dual is Unclassified.
                && classify_candidate(ActivityStatus::Active, None, 1e-9)
                    == CandidateTier::Unclassified
        };
        // 6) 分类器不校验阈值、配置不校验上限。 / The classifier skips the threshold, the config the limit.
        let the_classifier_ignores_the_threshold_and_the_config_the_limit = {
            // 负阈值：`|对偶| > 负数` 恒真 → 一切都成 Tier A。 
            // A negative threshold makes `|dual| > negative` always true, so everything becomes Tier A.
            let negative = classify_candidate(ActivityStatus::Active, Some(0.0), -1.0);
            // 但配置自己会拒绝负阈值。 / The config itself refuses a negative threshold.
            let refused = CandidateFunnelConfig { dual_threshold: -1.0, default_candidate_limit: 32 }
                .validate().is_err();
            // 而上限 0（乃至巨值）照样通过 —— 没有人检查它。 
            // A limit of zero (or an enormous one) still passes, since nobody checks it.
            let zero_limit = CandidateFunnelConfig { dual_threshold: 1e-9, default_candidate_limit: 0 }
                .validate().is_ok();
            // 负阈值做出来的排序自己会被 `validate()` 拒绝 —— 分类器不查、排序查。 
            // A ranking built with a negative threshold is refused by `validate()`, so the classifier skips the check
            // while the ranking keeps it.
            let ranking = CandidateFunnelRanking {
                schema_version: "probe".to_owned(),
                dual_threshold: -1.0,
                candidates: vec![entry("a", ActivityStatus::Active, Some(0.0), 0)],
                tier_a: 1, tier_b: 0, tier_c: 0, unclassified: 0,
            };
            negative == CandidateTier::TierA && refused && zero_limit
                && ranking.validate().is_err()
        };
        // 7) 短名单只按既有顺序取。 / The shortlist keeps the existing order.
        let the_shortlist_keeps_the_existing_order = {
            // 顺序故意是 C、A、B：短名单先收 A/B（保持既有顺序），再补 C。 
            // The order is deliberately C, A, B: the shortlist takes A/B in their existing order, then fills with C.
            let ranking = CandidateFunnelRanking {
                schema_version: "probe".to_owned(),
                dual_threshold: 1e-9,
                candidates: vec![
                    entry("c", ActivityStatus::Inactive, None, 0),
                    entry("a", ActivityStatus::Active, Some(1.0), 1),
                    entry("b", ActivityStatus::Active, Some(0.0), 2),
                ],
                tier_a: 1, tier_b: 1, tier_c: 1, unclassified: 0,
            };
            let ids = |limit: usize| ranking.shortlist(limit).iter()
                .map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>();
            // 上限 2：只收 A/B，顺序是 a 然后 b（Tier B 可以排在 Tier A 前面？不——既有顺序里 a 在 b 前）。 
            // A limit of two takes A and B in their existing order, and a limit of three appends C.
            ids(0).is_empty()
                && ids(1) == vec!["a".to_owned()]
                && ids(2) == vec!["a".to_owned(), "b".to_owned()]
                && ids(3) == vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
                // 上限超过全部候选时不会重复。 / A limit beyond every candidate repeats nothing.
                && ids(9).len() == 3
        };
        // 8) `Violated` 落到 Unclassified。 / `Violated` falls through to Unclassified.
        let a_violated_candidate_is_unclassified = {
            classify_candidate(ActivityStatus::Violated, Some(1.0), 1e-9)
                == CandidateTier::Unclassified
                && classify_candidate(ActivityStatus::Violated, None, 1e-9)
                    == CandidateTier::Unclassified
                && classify_candidate(ActivityStatus::Unknown, Some(1.0), 1e-9)
                    == CandidateTier::Unclassified
                // 对照：`Inactive` 才是 Tier C。 / Control: `Inactive` is the Tier C case.
                && classify_candidate(ActivityStatus::Inactive, Some(1.0), 1e-9)
                    == CandidateTier::TierC
        };
        Findings {
            the_short_tier_names_differ_from_the_wire_names,
            nearly_active_shares_the_active_bucket,
            an_active_candidate_without_a_dual_is_unclassified,
            an_inactive_candidate_is_tier_c_whatever_its_dual,
            a_nan_dual_lands_in_tier_b,
            the_classifier_ignores_the_threshold_and_the_config_the_limit,
            the_shortlist_keeps_the_existing_order,
            a_violated_candidate_is_unclassified,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FunnelRequest {
        FunnelRequest { candidates: vec![
            FunnelProbeSpec { id: "c".to_owned(), activity: "Inactive".to_owned(), dual: None },
            FunnelProbeSpec { id: "a".to_owned(), activity: "Active".to_owned(), dual: Some(1.0) },
            FunnelProbeSpec { id: "b".to_owned(), activity: "Active".to_owned(), dual: Some(0.0) },
        ], dual_threshold: Some(1e-9), shortlist_limit: Some(2) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_funnel(&request()).unwrap();
        assert!(report.the_short_tier_names_differ_from_the_wire_names);
        assert!(report.nearly_active_shares_the_active_bucket);
        assert!(report.an_active_candidate_without_a_dual_is_unclassified);
        assert!(report.an_inactive_candidate_is_tier_c_whatever_its_dual);
        assert!(report.a_nan_dual_lands_in_tier_b);
        assert!(report.the_classifier_ignores_the_threshold_and_the_config_the_limit);
        assert!(report.the_shortlist_keeps_the_existing_order);
        assert!(report.a_violated_candidate_is_unclassified);
        assert_eq!(report.tier_a, 1);
        assert_eq!(report.tier_b, 1);
        assert_eq!(report.tier_c, 1);
        assert_eq!(report.unclassified, 0);
        // 短名单先收 A/B，保持既有顺序。 / The shortlist takes A and B in their existing order.
        assert_eq!(report.shortlist, vec!["a".to_owned(), "b".to_owned()]);
        assert!(report.ranking_valid);
        assert!(report.bad_counts_refused);
        // 短名与线上名同时给出。 / Both the short and the wire names are present.
        assert_eq!(report.rows[1].tier, "A");
        assert_eq!(report.rows[1].wire, "TIER_A");
        assert_eq!(report.rows[0].tier, "C");
    }

    #[test]
    fn a_nan_dual_is_tier_b() {
        let report = verify_funnel(&FunnelRequest { candidates: vec![FunnelProbeSpec {
            id: "n".to_owned(), activity: "Active".to_owned(), dual: Some(f64::NAN) }],
            dual_threshold: Some(1e-9), shortlist_limit: Some(1) }).unwrap();
        assert_eq!(report.rows[0].tier, "B");
        assert_eq!(report.tier_b, 1);
        // `NaN` 对偶也让排序通过校验（Tier B 只要求 `|对偶| <= 阈值`，`NaN` 比较为假）。 
        // The NaN dual also validates, since Tier B merely requires `|dual| <= threshold` and NaN compares false.
        assert!(report.ranking_valid);
    }

    #[test]
    fn a_missing_dual_is_unclassified_not_tier_b() {
        let report = verify_funnel(&FunnelRequest { candidates: vec![FunnelProbeSpec {
            id: "a".to_owned(), activity: "Active".to_owned(), dual: None }],
            dual_threshold: Some(1e-9), shortlist_limit: Some(2) }).unwrap();
        assert_eq!(report.rows[0].tier, "UNCLASSIFIED");
        assert_eq!(report.unclassified, 1);
        // 未分层只在名额剩时才进短名单。 / Unclassified only enters while slots remain.
        assert_eq!(report.shortlist, vec!["a".to_owned()]);
    }

    #[test]
    fn a_negative_threshold_makes_every_active_candidate_tier_a() {
        let report = verify_funnel(&FunnelRequest { candidates: vec![FunnelProbeSpec {
            id: "a".to_owned(), activity: "Active".to_owned(), dual: Some(0.0) }],
            dual_threshold: Some(-1.0), shortlist_limit: Some(1) }).unwrap();
        // 分类器不校验阈值：`|0| > -1` 为真。 / The classifier checks nothing: `|0| > -1` holds.
        assert_eq!(report.rows[0].tier, "A");
        // 但排序自己会拒绝负阈值。 / The ranking itself refuses a negative threshold.
        assert!(!report.ranking_valid);
    }

    #[test]
    fn the_shortlist_respects_the_limit() {
        let report = verify_funnel(&FunnelRequest { shortlist_limit: Some(0), ..request() })
            .unwrap();
        assert!(report.shortlist.is_empty());
        let wide = verify_funnel(&FunnelRequest { shortlist_limit: Some(8), ..request() })
            .unwrap();
        assert_eq!(wide.shortlist, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn bad_input_is_refused() {
        let spec = |id: &str| FunnelProbeSpec { id: id.to_owned(), activity: "Active".to_owned(),
            dual: Some(1.0) };
        assert!(verify_funnel(&FunnelRequest { candidates: (0..9).map(|index|
            spec(&format!("c{index}"))).collect(), ..request() }).unwrap_err()
            .message.contains("at most eight candidates"));
        assert!(verify_funnel(&FunnelRequest { candidates: vec![FunnelProbeSpec {
            id: "  ".to_owned(), activity: "Active".to_owned(), dual: Some(1.0) }], ..request() })
            .unwrap_err().message.contains("may not be blank"));
        assert!(verify_funnel(&FunnelRequest { candidates: vec![FunnelProbeSpec {
            id: "a".to_owned(), activity: "Nearly".to_owned(), dual: Some(1.0) }], ..request() })
            .unwrap_err().message.contains("unknown activity name"));
        assert!(verify_funnel(&FunnelRequest { dual_threshold: Some(f64::NAN), ..request() })
            .unwrap_err().message.contains("must be finite"));
        assert!(verify_funnel(&FunnelRequest { shortlist_limit: Some(9), ..request() })
            .unwrap_err().message.contains("the limit stays at eight"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_funnel(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"tierA\""), "{json}");
        assert!(json.contains("\"shortlist\""), "{json}");
        assert!(json.contains("\"aNanDualLandsInTierB\""), "{json}");
        assert!(!json.contains("tier_a"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<FunnelReport>(&copy.to_string()).is_err());
    }
}
