//! 续挖第 111 单：**有效性排序的两个视角互不蕴含，而且"已排序"只是文档承诺**——`is_globally_effective()`
//! 只看 `max_improvement > 0`（`Some(0.0)`、负数、`None` 都不算），`globally_effective()` 与
//! `locally_effective()` 都只是**按原顺序过滤**（`locally_effective()` 只认恰好 `Some(true)`），于是
//! **局部有效与全局有效可以一真一假两个方向都出现**；`validate()` 拒重复身份、空身份、非有限的对偶/改善/
//! 边际效应，`first_effective_delta` 还必须是**正**的，`local_effective` **必须有对偶**，而
//! `removal_effective` **只在 `Unsupported` 状态下被拒**（`Unknown` 配 `Some` 判定照样通过）；最扎眼的是：
//! **`validate()` 从不检查条目是否真按最大改善降序**（`analysis/pipeline.rs` 的 `EffectivenessRanking` /
//! `EffectivenessEntry`）
//! Continuation, delivery 111: **the effectiveness ranking's two views are mutually non-implying, and "sorted" is only a
//! documentation promise** — `is_globally_effective()` reads `max_improvement > 0` only, so `Some(0.0)`, a negative value and
//! `None` all fail, while `globally_effective()` and `locally_effective()` merely **filter in the original order** and
//! `locally_effective()` accepts exactly `Some(true)` — so **a local verdict and a global verdict can disagree in both
//! directions**; `validate()` refuses duplicate identities, blank identities and non-finite dual, improvement or marginal
//! values, `first_effective_delta` must be **positive**, `local_effective` **requires a dual**, and `removal_effective` is
//! refused **only when the status is `Unsupported`**, so `Unknown` with a `Some` verdict passes; most strikingly,
//! **`validate()` never checks that entries really descend by maximum improvement**
//! (`EffectivenessRanking` / `EffectivenessEntry` in `analysis/pipeline.rs`).
//!
//! 第 110 单量的是候选漏斗，这一单量**有效性排序的两个视角与校验口径**。八条实测：
//!
//! 1. **全局有效只看 `max_improvement > 0`**（`global_effective_needs_a_strictly_positive_improvement()`）。
//! 2. **两个视角都按原顺序过滤，且从不检查排序**（`the_views_filter_in_place_and_the_order_is_unchecked()`）。
//! 3. **局部有效只认恰好 `Some(true)`**（`local_effective_accepts_exactly_true()`）。
//! 4. **两个视角互不蕴含**（`the_two_views_do_not_imply_each_other()`）。
//! 5. **校验拒重复身份、空身份与非有限数值**（`validation_refuses_duplicates_and_non_finite_values()`）。
//! 6. **`first_effective_delta` 必须是正数**（`the_first_delta_must_be_positive()`）。
//! 7. **`local_effective` 必须有对偶**（`the_local_verdict_requires_a_dual()`）。
//! 8. **`removal_effective` 只在 `Unsupported` 时被拒**（`the_removal_verdict_only_rejects_unsupported()`）。
//!
//! Delivery 110 measured the candidate funnel; this one measures **the effectiveness ranking's two views and its validation
//! readings**. **A global verdict needs a strictly positive improvement.** **Both views filter in place and the order is
//! unchecked.** **A local verdict accepts exactly `true`.** **The two views do not imply each other.** **Validation refuses
//! duplicates and non-finite values.** **The first delta must be positive.** **The local verdict requires a dual.** And **the
//! removal verdict only rejects unsupported statuses.**

use ospf_rust_core::analysis::pipeline::{
    CandidateTier, EffectivenessEntry, EffectivenessRanking,
};
use ospf_rust_core::analysis::{ActivityStatus, AnalysisStatus};
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条有效性条目的描述。 / The description of one effectiveness entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessEntrySpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 对偶值（`null` 表示没有）。 / The dual value.
    #[serde(default)]
    pub dual: Option<f64>,
    /// 局部有效判定。 / The local verdict.
    #[serde(default)]
    pub local_effective: Option<bool>,
    /// 单删判定。 / The removal verdict.
    #[serde(default)]
    pub removal_effective: Option<bool>,
    /// 首次改善的 delta。 / The first effective delta.
    #[serde(default)]
    pub first_effective_delta: Option<f64>,
    /// 最大已证明改善。 / The largest proven improvement.
    #[serde(default)]
    pub max_improvement: Option<f64>,
    /// 每单位 delta 的边际效应。 / The marginal effect.
    #[serde(default)]
    pub marginal_effect: Option<f64>,
    /// 状态名（默认 `Unknown`）。 / The status name.
    #[serde(default)]
    pub status: Option<String>,
}

/// 有效性排序探针请求。 / The effectiveness ranking probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessRankingRequest {
    /// 报告 schema（默认 `effectiveness`）。 / The report schema.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// baseline 目标值（默认 0）。 / The baseline objective.
    #[serde(default)]
    pub baseline_objective: Option<f64>,
    /// 条目。 / The entries.
    #[serde(default)]
    pub entries: Vec<EffectivenessEntrySpec>,
}

/// 一条条目的读数。 / One entry reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessEntryRow {
    /// 稳定身份。 / The stable identity.
    pub id: String,
    /// 是否全局有效。 / Whether it is globally effective.
    pub global_effective: bool,
    /// 是否出现在局部有效视图里。 / Whether it is in the local view.
    pub in_local_view: bool,
}

/// 有效性排序探针结论。 / The effectiveness ranking probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessRankingReport {
    /// 排序是否合法。 / Whether the ranking validates.
    pub ranking_valid: bool,
    /// 校验的错误文本。 / The validation error text.
    pub ranking_error: String,
    /// 每条条目的读数（按原顺序）。 / Every entry, in the original order.
    pub rows: Vec<EffectivenessEntryRow>,
    /// 全局有效条数。 / The global count.
    pub global_count: u32,
    /// 局部有效条数。 / The local count.
    pub local_count: u32,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub global_effective_needs_a_strictly_positive_improvement: bool,
    pub the_views_filter_in_place_and_the_order_is_unchecked: bool,
    pub local_effective_accepts_exactly_true: bool,
    pub the_two_views_do_not_imply_each_other: bool,
    pub validation_refuses_duplicates_and_non_finite_values: bool,
    pub the_first_delta_must_be_positive: bool,
    pub the_local_verdict_requires_a_dual: bool,
    pub the_removal_verdict_only_rejects_unsupported: bool,
}

/// 按名字取状态。 / Resolve a status by name.
fn status_named(name: &str) -> Option<AnalysisStatus> {
    match name {
        "Reachable" => Some(AnalysisStatus::Reachable),
        "Unreachable" => Some(AnalysisStatus::Unreachable),
        "Unknown" => Some(AnalysisStatus::Unknown),
        "Unsupported" => Some(AnalysisStatus::Unsupported),
        _ => None,
    }
}

/// 造一条条目。 / Build one entry.
fn entry_of(spec: &EffectivenessEntrySpec) -> Result<EffectivenessEntry> {
    let status = match spec.status.as_deref() {
        None => AnalysisStatus::Unknown,
        Some(name) => status_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "effectiveness probe failed validation: unknown entry status"))?,
    };
    Ok(EffectivenessEntry {
        constraint_id: StableConstraintId(spec.id.clone()),
        group: None,
        tier: CandidateTier::Unclassified,
        activity: ActivityStatus::Unknown,
        dual_value: spec.dual,
        local_effective: spec.local_effective,
        removal_effective: spec.removal_effective,
        first_effective_delta: spec.first_effective_delta,
        max_improvement: spec.max_improvement,
        marginal_effect: spec.marginal_effect,
        status,
        unavailable_reason: None,
    })
}

/// 走一遍有效性排序探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_effectiveness_ranking_probe(
    request: &EffectivenessRankingRequest,
) -> Result<EffectivenessRankingReport> {
    if request.entries.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "effectiveness probe failed validation: \
            at most eight entries are accepted"));
    }
    let findings = canonical_findings();
    let ranking = EffectivenessRanking {
        schema_version: request.schema_version.clone()
            .unwrap_or_else(|| "effectiveness".to_owned()),
        baseline_objective: request.baseline_objective.unwrap_or(0.0),
        entries: request.entries.iter().map(entry_of).collect::<Result<Vec<_>>>()?,
    };
    let validated = ranking.validate();
    let globals = ranking.globally_effective();
    let locals = ranking.locally_effective();
    Ok(EffectivenessRankingReport {
        ranking_valid: validated.is_ok(),
        ranking_error: validated.err().map(|error| error.to_string()).unwrap_or_default(),
        rows: ranking.entries.iter().map(|entry| EffectivenessEntryRow {
            id: entry.constraint_id.0.clone(),
            global_effective: entry.is_globally_effective(),
            in_local_view: entry.local_effective == Some(true),
        }).collect(),
        global_count: u32::try_from(globals.len()).unwrap_or(u32::MAX),
        local_count: u32::try_from(locals.len()).unwrap_or(u32::MAX),
        global_effective_needs_a_strictly_positive_improvement:
            findings.global_effective_needs_a_strictly_positive_improvement,
        the_views_filter_in_place_and_the_order_is_unchecked:
            findings.the_views_filter_in_place_and_the_order_is_unchecked,
        local_effective_accepts_exactly_true: findings.local_effective_accepts_exactly_true,
        the_two_views_do_not_imply_each_other: findings.the_two_views_do_not_imply_each_other,
        validation_refuses_duplicates_and_non_finite_values:
            findings.validation_refuses_duplicates_and_non_finite_values,
        the_first_delta_must_be_positive: findings.the_first_delta_must_be_positive,
        the_local_verdict_requires_a_dual: findings.the_local_verdict_requires_a_dual,
        the_removal_verdict_only_rejects_unsupported:
            findings.the_removal_verdict_only_rejects_unsupported,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    global_effective_needs_a_strictly_positive_improvement: bool,
    the_views_filter_in_place_and_the_order_is_unchecked: bool,
    local_effective_accepts_exactly_true: bool,
    the_two_views_do_not_imply_each_other: bool,
    validation_refuses_duplicates_and_non_finite_values: bool,
    the_first_delta_must_be_positive: bool,
    the_local_verdict_requires_a_dual: bool,
    the_removal_verdict_only_rejects_unsupported: bool,
}

/// 造一条条目（内部用）。 / Build one entry (internal).
fn entry(id: &str, dual: Option<f64>, local: Option<bool>, improvement: Option<f64>)
    -> EffectivenessEntry
{
    EffectivenessEntry {
        constraint_id: StableConstraintId(id.to_owned()),
        group: None,
        tier: CandidateTier::Unclassified,
        activity: ActivityStatus::Unknown,
        dual_value: dual,
        local_effective: local,
        removal_effective: None,
        first_effective_delta: None,
        max_improvement: improvement,
        marginal_effect: None,
        status: AnalysisStatus::Unknown,
        unavailable_reason: None,
    }
}

/// 造一份排序。 / Build one ranking.
fn ranking_of(entries: Vec<EffectivenessEntry>) -> EffectivenessRanking {
    EffectivenessRanking {
        schema_version: "effectiveness".to_owned(),
        baseline_objective: 0.0,
        entries,
    }
}

/// 取条目身份序列。 / Take the entry identity sequence.
fn ids_of(entries: Vec<&EffectivenessEntry>) -> Vec<String> {
    entries.into_iter().map(|entry| entry.constraint_id.0.clone()).collect()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 全局有效只看严格正的改善。 / A global verdict needs a strictly positive improvement.
        let global_effective_needs_a_strictly_positive_improvement = {
            entry("c1", None, None, Some(1.0)).is_globally_effective()
                && !entry("c1", None, None, Some(0.0)).is_globally_effective()
                && !entry("c1", None, None, Some(-1.0)).is_globally_effective()
                && !entry("c1", None, None, None).is_globally_effective()
                // 与状态、对偶、局部判定都无关。 / Independent of status, dual and the local verdict.
                && entry("c1", Some(9.0), Some(true), None).is_globally_effective() == false
        };
        // 2) 两个视角按原顺序过滤，而且不检查排序。 / The views filter in place and never check the order.
        let the_views_filter_in_place_and_the_order_is_unchecked = {
            // 故意乱序：小的在前。 / Deliberately unsorted: the smaller improvement first.
            let unsorted = ranking_of(vec![
                entry("a", Some(1.0), Some(true), Some(1.0)),
                entry("b", Some(1.0), Some(true), Some(5.0)),
            ]);
            let projected = ids_of(unsorted.globally_effective());
            // 视图保持原顺序（a 在前），而文档说"按最大改善降序"。 
            // The view keeps the original order with `a` first, while the doc says "ordered by maximum improvement".
            projected == vec!["a".to_owned(), "b".to_owned()]
                // 乱序的排序照样通过校验。 / An unsorted ranking still validates.
                && unsorted.validate().is_ok()
        };
        // 3) 局部有效只认恰好 Some(true)。 / A local verdict accepts exactly true.
        let local_effective_accepts_exactly_true = {
            let mixed = ranking_of(vec![
                entry("a", Some(1.0), Some(true), None),
                entry("b", Some(1.0), Some(false), None),
                entry("c", Some(1.0), None, None),
            ]);
            ids_of(mixed.locally_effective()) == vec!["a".to_owned()]
        };
        // 4) 两个视角互不蕴含。 / The two views do not imply each other.
        let the_two_views_do_not_imply_each_other = {
            // 局部真、全局假。 / Local true, global false.
            let local_only = entry("a", Some(1.0), Some(true), None);
            // 全局真、局部假。 / Global true, local false.
            let global_only = entry("b", Some(1.0), Some(false), Some(3.0));
            let mixed = ranking_of(vec![local_only.clone(), global_only.clone()]);
            local_only.is_globally_effective() == false
                && global_only.is_globally_effective()
                && ids_of(mixed.locally_effective()) == vec!["a".to_owned()]
                && ids_of(mixed.globally_effective()) == vec!["b".to_owned()]
        };
        // 5) 校验拒重复身份、空身份与非有限数值。 
        // Validation refuses duplicates, blank identities and non-finite values.
        let validation_refuses_duplicates_and_non_finite_values = {
            let honest = ranking_of(vec![entry("a", Some(1.0), None, None)]);
            honest.validate().is_ok()
                && ranking_of(vec![entry("a", None, None, None), entry("a", None, None, None)])
                    .validate().is_err()
                && ranking_of(vec![entry("  ", None, None, None)]).validate().is_err()
                && ranking_of(vec![entry("a", Some(f64::NAN), None, None)]).validate().is_err()
                && ranking_of(vec![entry("a", None, None, Some(f64::INFINITY))])
                    .validate().is_err()
                && { let mut broken = honest.clone();
                    broken.baseline_objective = f64::NAN; broken.validate().is_err() }
                && { let mut broken = honest.clone();
                    broken.schema_version = " ".to_owned(); broken.validate().is_err() }
        };
        // 6) 首次改善 delta 必须是正数。 / The first delta must be positive.
        let the_first_delta_must_be_positive = {
            let with_delta = |delta: f64| {
                let mut sample = entry("a", None, None, None);
                sample.first_effective_delta = Some(delta);
                ranking_of(vec![sample]).validate()
            };
            with_delta(1.0).is_ok() && with_delta(0.0).is_err() && with_delta(-1.0).is_err()
                && with_delta(f64::NAN).is_err()
        };
        // 7) 局部判定必须有对偶。 / A local verdict requires a dual.
        let the_local_verdict_requires_a_dual = {
            ranking_of(vec![entry("a", None, Some(true), None)]).validate().is_err()
                && ranking_of(vec![entry("a", None, Some(false), None)]).validate().is_err()
                && ranking_of(vec![entry("a", Some(1.0), Some(true), None)]).validate().is_ok()
                && ranking_of(vec![entry("a", None, None, None)]).validate().is_ok()
        };
        // 8) 单删判定只在 Unsupported 时被拒。 / The removal verdict is refused only for unsupported.
        let the_removal_verdict_only_rejects_unsupported = {
            let with_status = |status: AnalysisStatus| {
                let mut sample = entry("a", None, None, None);
                sample.removal_effective = Some(true);
                sample.status = status;
                ranking_of(vec![sample]).validate()
            };
            // Unsupported 被拒。 / Unsupported is refused.
            with_status(AnalysisStatus::Unsupported).is_err()
                // Unknown 与 Reachable 都放行。 / Unknown and reachable both pass.
                && with_status(AnalysisStatus::Unknown).is_ok()
                && with_status(AnalysisStatus::Reachable).is_ok()
        };
        Findings {
            global_effective_needs_a_strictly_positive_improvement,
            the_views_filter_in_place_and_the_order_is_unchecked,
            local_effective_accepts_exactly_true,
            the_two_views_do_not_imply_each_other,
            validation_refuses_duplicates_and_non_finite_values,
            the_first_delta_must_be_positive,
            the_local_verdict_requires_a_dual,
            the_removal_verdict_only_rejects_unsupported,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: EffectivenessRankingRequest) -> EffectivenessRankingReport {
        verify_effectiveness_ranking_probe(&request).unwrap()
    }

    fn spec(id: &str) -> EffectivenessEntrySpec {
        EffectivenessEntrySpec { id: id.to_owned(), dual: None, local_effective: None,
            removal_effective: None, first_effective_delta: None, max_improvement: None,
            marginal_effect: None, status: None }
    }

    fn request() -> EffectivenessRankingRequest {
        EffectivenessRankingRequest { schema_version: None, baseline_objective: None,
            entries: vec![EffectivenessEntrySpec { max_improvement: Some(2.0),
                dual: Some(1.0), local_effective: Some(true), ..spec("a") }] }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert!(report.ranking_valid, "{}", report.ranking_error);
        assert_eq!(report.global_count, 1);
        assert_eq!(report.local_count, 1);
        assert!(report.rows[0].global_effective);
        assert!(report.rows[0].in_local_view);
        assert!(report.global_effective_needs_a_strictly_positive_improvement);
        assert!(report.the_views_filter_in_place_and_the_order_is_unchecked);
        assert!(report.local_effective_accepts_exactly_true);
        assert!(report.the_two_views_do_not_imply_each_other);
        assert!(report.validation_refuses_duplicates_and_non_finite_values);
        assert!(report.the_first_delta_must_be_positive);
        assert!(report.the_local_verdict_requires_a_dual);
        assert!(report.the_removal_verdict_only_rejects_unsupported);
    }

    #[test]
    fn a_global_verdict_needs_a_positive_improvement() {
        for improvement in [Some(1.0), Some(0.0), Some(-1.0), None] {
            let report = ask(EffectivenessRankingRequest {
                entries: vec![EffectivenessEntrySpec { max_improvement: improvement,
                    ..spec("a") }], ..request() });
            assert_eq!(report.rows[0].global_effective,
                improvement.is_some_and(|value| value > 0.0), "{improvement:?}");
        }
    }

    #[test]
    fn the_two_views_disagree_in_both_directions() {
        // 局部真、全局假。 / Local true, global false.
        let local = ask(EffectivenessRankingRequest { entries: vec![
            EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(true),
                ..spec("a") }], ..request() });
        assert!(local.rows[0].in_local_view);
        assert!(!local.rows[0].global_effective);
        // 全局真、局部假。 / Global true, local false.
        let global = ask(EffectivenessRankingRequest { entries: vec![
            EffectivenessEntrySpec { dual: Some(1.0), local_effective: Some(false),
                max_improvement: Some(3.0), ..spec("a") }], ..request() });
        assert!(!global.rows[0].in_local_view);
        assert!(global.rows[0].global_effective);
        assert_eq!(global.global_count, 1);
        assert_eq!(global.local_count, 0);
    }

    #[test]
    fn an_unsorted_ranking_still_validates() {
        let report = ask(EffectivenessRankingRequest { entries: vec![
            EffectivenessEntrySpec { max_improvement: Some(1.0), ..spec("a") },
            EffectivenessEntrySpec { max_improvement: Some(5.0), ..spec("b") }], ..request() });
        assert!(report.ranking_valid, "{}", report.ranking_error);
        // 视图保持原顺序。 / The view keeps the original order.
        assert_eq!(report.rows[0].id, "a");
        assert_eq!(report.rows[1].id, "b");
        assert_eq!(report.global_count, 2);
    }

    #[test]
    fn the_validation_refusals_are_specific() {
        // 重复身份。 / Duplicates.
        assert!(!ask(EffectivenessRankingRequest { entries: vec![spec("a"), spec("a")],
            ..request() }).ranking_valid);
        // 空身份。 / A blank identity.
        assert!(!ask(EffectivenessRankingRequest { entries: vec![spec("  ")], ..request() })
            .ranking_valid);
        // 非有限数值。 / A non-finite value.
        assert!(!ask(EffectivenessRankingRequest { entries: vec![
            EffectivenessEntrySpec { dual: Some(f64::NAN), ..spec("a") }], ..request() })
            .ranking_valid);
        let nan_baseline = ask(EffectivenessRankingRequest {
            baseline_objective: Some(f64::NAN), ..request() });
        assert!(!nan_baseline.ranking_valid);
        assert!(nan_baseline.ranking_error.contains("invalid"), "{}", nan_baseline.ranking_error);
        // delta 必须为正。 / The delta must be positive.
        for delta in [Some(1.0), Some(0.0), Some(-1.0)] {
            let report = ask(EffectivenessRankingRequest { entries: vec![
                EffectivenessEntrySpec { first_effective_delta: delta, ..spec("a") }],
                ..request() });
            assert_eq!(report.ranking_valid, delta.is_some_and(|value| value > 0.0),
                "{delta:?}");
        }
        // 局部判定必须有对偶。 / The local verdict requires a dual.
        assert!(!ask(EffectivenessRankingRequest { entries: vec![
            EffectivenessEntrySpec { local_effective: Some(true), ..spec("a") }],
            ..request() }).ranking_valid);
    }

    #[test]
    fn the_removal_verdict_only_rejects_unsupported() {
        for (status, expected) in [("Unsupported", false), ("Unknown", true),
            ("Reachable", true)] {
            let report = ask(EffectivenessRankingRequest { entries: vec![
                EffectivenessEntrySpec { removal_effective: Some(true),
                    status: Some(status.to_owned()), ..spec("a") }], ..request() });
            assert_eq!(report.ranking_valid, expected, "{status}");
        }
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_effectiveness_ranking_probe(&EffectivenessRankingRequest {
            entries: (0..9).map(|index| spec(&format!("c{index}"))).collect(),
            ..request() }).unwrap_err().message.contains("at most eight entries"));
        assert!(verify_effectiveness_ranking_probe(&EffectivenessRankingRequest {
            entries: vec![EffectivenessEntrySpec { status: Some("Nearly".to_owned()),
                ..spec("a") }], ..request() }).unwrap_err()
            .message.contains("unknown entry status"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"rankingValid\""), "{json}");
        assert!(json.contains("\"globalCount\""), "{json}");
        assert!(json.contains("\"theTwoViewsDoNotImplyEachOther\""), "{json}");
        assert!(!json.contains("ranking_valid"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<EffectivenessRankingReport>(&copy.to_string()).is_err());
    }
}
