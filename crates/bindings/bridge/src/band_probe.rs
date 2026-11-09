//! 续挖第 101 单：**活动性分档的边界全是"包含式"的**——`slack < -active` 才算违反，于是**恰好 `-active` 落在
//! `Active` 档**；容差随尺度**只增不减**（`tolerance.max(relative * |scale|.max(1.0))`，scale 0 也不缩小）；
//! `nearly` 档再被 `.max(active)` 抬一次，所以"nearly 小于 active"这种非法配置**仍能分类**（只是 `validate()` 会拒）；
//! 而非有限的 slack 或尺度直接给 `Unknown`；报告的 `validate()` 则**要求逐项合并计数与组计数都等于重算结果**
//! （`analysis/activity.rs` 的 `ActivityConfig` / `ConstraintActivityReport`）
//! Continuation, delivery 101: **every activity-band edge is inclusive in the loosest sense** — only `slack < -active`
//! counts as violated, so **exactly `-active` lands in `Active`**; the tolerance **only grows with scale**
//! (`tolerance.max(relative * |scale|.max(1.0))`, and a scale of zero shrinks nothing); the nearly band is lifted again by
//! `.max(active)`, so an illegal "nearly below active" configuration **still classifies** while `validate()` refuses it;
//! a non-finite slack or scale yields `Unknown`; and the report's `validate()` **demands both the merged counts and the
//! per-group counts to equal a recount** (`ActivityConfig` / `ConstraintActivityReport` in `analysis/activity.rs`).
//!
//! 第 100 单量的是组汇总，这一单量**活动性分档本身**。八条实测：
//!
//! 1. **非有限的 slack 或尺度直接给 `Unknown`**（`a_non_finite_slack_or_scale_is_unknown()`）。
//! 2. **恰好 `-active` 落在 `Active` 档而不是 `Violated`**（`minus_the_tolerance_is_active_not_violated()`）：
//!    `slack < -active` 是严格不等。
//! 3. **越过 active 带才违反**（`beyond_the_active_band_is_violated()`）。
//! 4. **尺度会放大容差**（`the_scale_widens_the_tolerance()`）。
//! 5. **尺度永不缩小容差**（`the_scale_never_shrinks_the_tolerance()`）：`|scale|.max(1.0)`。
//! 6. **nearly 档永不低于 active 档**（`the_nearly_band_never_sits_below_active()`）：
//!    `.max(active)` 让非法配置也能分类，而 `validate()` 会拒。
//! 7. **合并计数必须等于重算结果**（`the_summary_must_match_the_records()`）。
//! 8. **组计数必须等于重算结果**（`the_group_summary_must_match_the_records()`）。
//!
//! Delivery 100 measured the group summary; this one measures **the activity banding itself**. **A non-finite slack or
//! scale yields `Unknown`.** **Exactly `-active` lands in `Active`** rather than `Violated`, because the comparison is
//! strict. **Only beyond the active band is violated.** **The scale widens the tolerance.** **The scale never shrinks
//! it.** **The nearly band never sits below active**, which lets an illegal configuration still classify while
//! `validate()` refuses it. And the report's **merged and per-group counts must both equal a recount**.

use ospf_rust_core::analysis::activity::{
    ActivityConfig, ActivityEvidence, ActivityGroupSummary, ActivitySummary, ConstraintActivity,
    ConstraintActivityReport,
};
use ospf_rust_core::analysis::ActivityStatus;
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个分档探针。 / One banding probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BandProbeSpec {
    /// signed slack。 / The signed slack.
    pub slack: f64,
    /// 尺度。 / The scale.
    pub scale: f64,
}

/// 活动性分档探针请求。 / The activity-banding probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BandProbeRequest {
    /// active 容差（默认 1e-7）。 / The active tolerance.
    #[serde(default)]
    pub tolerance: Option<f64>,
    /// nearly-active 容差（默认 1e-5）。 / The nearly-active tolerance.
    #[serde(default)]
    pub nearly_active_tolerance: Option<f64>,
    /// 相对容差（默认 0）。 / The relative tolerance.
    #[serde(default)]
    pub relative_tolerance: Option<f64>,
    /// 要分类的 `(slack, scale)` 列表。 / The `(slack, scale)` pairs to classify.
    #[serde(default)]
    pub probes: Vec<BandProbeSpec>,
    /// 是否故意把报告的合并计数改坏。 / Whether to deliberately break the report's merged counts.
    #[serde(default)]
    pub break_summary: Option<bool>,
}

/// 一条分档读数。 / One banding reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BandProbeRow {
    /// 状态名。 / The status name.
    pub status: String,
    /// 该尺度下的 active 容差。 / The active tolerance at this scale.
    pub active_tolerance: f64,
    /// 该尺度下的 nearly 容差。 / The nearly tolerance at this scale.
    pub nearly_tolerance: f64,
}

/// 活动性分档探针结论。 / The banding probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BandProbeReport {
    /// 每条 `(slack, scale)` 的读数。 / The reading of every `(slack, scale)` pair.
    pub rows: Vec<BandProbeRow>,
    /// 这份配置是否通过 `ActivityConfig::validate()`。 / Whether the configuration validates.
    pub config_valid: bool,
    /// 报告本身是否通过 `validate()`。 / Whether the report validates.
    pub report_valid: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub a_non_finite_slack_or_scale_is_unknown: bool,
    pub minus_the_tolerance_is_active_not_violated: bool,
    pub beyond_the_active_band_is_violated: bool,
    pub the_scale_widens_the_tolerance: bool,
    pub the_scale_never_shrinks_the_tolerance: bool,
    pub the_nearly_band_never_sits_below_active: bool,
    pub the_summary_must_match_the_records: bool,
    pub the_group_summary_must_match_the_records: bool,
}

/// 状态名。 / The status name.
fn status_name(status: ActivityStatus) -> String {
    match status {
        ActivityStatus::Active => "Active",
        ActivityStatus::NearlyActive => "NearlyActive",
        ActivityStatus::Inactive => "Inactive",
        ActivityStatus::SatisfiedWithoutSlackMetric => "SatisfiedWithoutSlackMetric",
        ActivityStatus::Violated => "Violated",
        ActivityStatus::Unknown => "Unknown",
    }.to_owned()
}

/// 造一条约束记录。 / Build one constraint record.
fn record(id: &str, group: Option<&str>, status: ActivityStatus) -> ConstraintActivity {
    ConstraintActivity {
        constraint_id: StableConstraintId(id.to_owned()),
        group: group.map(str::to_owned),
        status,
        lhs: None,
        rhs: None,
        slack: None,
        normalized_slack: None,
        tolerance: None,
        satisfied: None,
        evidence: ActivityEvidence::LinearConstraint { lhs: 0.0, rhs: 0.0 },
    }
}

/// 按记录重算合并计数与组计数。 / Recount both the merged and the per-group counts.
fn recount(records: &[ConstraintActivity])
    -> (ActivitySummary, Vec<ActivityGroupSummary>, Vec<ConstraintActivity>)
{
    let mut summary = ActivitySummary::default();
    let mut by_group: std::collections::BTreeMap<Option<String>, ActivitySummary> =
        std::collections::BTreeMap::new();
    for record in records {
        summary.add(record.status);
        by_group.entry(record.group.clone()).or_default().add(record.status);
    }
    let groups = by_group.into_iter().map(|(group, summary)| ActivityGroupSummary {
        group, summary }).collect();
    (summary, groups, records.to_vec())
}

/// 造一份报告。 / Build one report.
fn report_of(config: &ActivityConfig, records: Vec<ConstraintActivity>,
    break_summary: bool) -> ConstraintActivityReport
{
    let (mut summary, groups, constraints) = recount(&records);
    if break_summary { summary.active = summary.active.saturating_add(1); }
    ConstraintActivityReport {
        schema_version: "band-probe".to_owned(),
        constraints,
        variable_bounds: Vec::new(),
        summary,
        variable_bound_summary: ActivitySummary::default(),
        groups,
        tolerance: config.tolerance,
        nearly_active_tolerance: config.nearly_active_tolerance,
    }
}

/// 走一遍活动性分档探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_band_probe(request: &BandProbeRequest) -> Result<BandProbeReport> {
    if request.probes.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "band probe failed validation: \
            at most eight probes are accepted"));
    }
    let config = ActivityConfig {
        tolerance: request.tolerance.unwrap_or(1e-7),
        nearly_active_tolerance: request.nearly_active_tolerance.unwrap_or(1e-5),
        relative_tolerance: request.relative_tolerance.unwrap_or(0.0),
        include_variable_bounds: false,
    };
    let findings = canonical_findings();
    let rows = request.probes.iter().map(|probe| {
        let active = config.effective_tolerance(probe.scale);
        let nearly = config.effective_nearly_active_tolerance(probe.scale).max(active);
        BandProbeRow {
            status: status_name(config.classify(probe.slack, probe.scale)),
            active_tolerance: active,
            nearly_tolerance: nearly,
        }
    }).collect();
    // 报告只用来量两条计数口径：记录按分类结果给出。 
    // The report only exercises the two counting rules, with records derived from the classifications.
    let records: Vec<ConstraintActivity> = request.probes.iter().enumerate().map(|(index, probe)| {
        record(&format!("c{index}"), if index % 2 == 0 { Some("even") } else { None },
            config.classify(probe.slack, probe.scale))
    }).collect();
    let report = report_of(&config, records, request.break_summary.unwrap_or(false));
    Ok(BandProbeReport {
        rows,
        config_valid: config.validate().is_ok(),
        report_valid: report.validate().is_ok(),
        a_non_finite_slack_or_scale_is_unknown: findings.a_non_finite_slack_or_scale_is_unknown,
        minus_the_tolerance_is_active_not_violated:
            findings.minus_the_tolerance_is_active_not_violated,
        beyond_the_active_band_is_violated: findings.beyond_the_active_band_is_violated,
        the_scale_widens_the_tolerance: findings.the_scale_widens_the_tolerance,
        the_scale_never_shrinks_the_tolerance: findings.the_scale_never_shrinks_the_tolerance,
        the_nearly_band_never_sits_below_active: findings.the_nearly_band_never_sits_below_active,
        the_summary_must_match_the_records: findings.the_summary_must_match_the_records,
        the_group_summary_must_match_the_records:
            findings.the_group_summary_must_match_the_records,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    a_non_finite_slack_or_scale_is_unknown: bool,
    minus_the_tolerance_is_active_not_violated: bool,
    beyond_the_active_band_is_violated: bool,
    the_scale_widens_the_tolerance: bool,
    the_scale_never_shrinks_the_tolerance: bool,
    the_nearly_band_never_sits_below_active: bool,
    the_summary_must_match_the_records: bool,
    the_group_summary_must_match_the_records: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 默认配置：active 1e-7、nearly 1e-5、相对 0。 / The default configuration.
        let config = ActivityConfig::new(1e-7, 1e-5);
        // 1) 非有限的 slack 或尺度给 Unknown。 / A non-finite slack or scale yields Unknown.
        let a_non_finite_slack_or_scale_is_unknown = {
            config.classify(f64::NAN, 1.0) == ActivityStatus::Unknown
                && config.classify(1.0, f64::INFINITY) == ActivityStatus::Unknown
                && config.classify(f64::NEG_INFINITY, 1.0) == ActivityStatus::Unknown
        };
        // 2) 恰好 `-active` 落在 Active 档。 / Exactly `-active` lands in Active.
        let minus_the_tolerance_is_active_not_violated = {
            // `slack < -active` 是严格不等，所以恰好等于边界不算违反。 
            // The comparison `slack < -active` is strict, so exactly the edge is not a violation.
            config.classify(-1e-7, 1.0) == ActivityStatus::Active
                // 再往前一点点才违反。 / A hair beyond it violates.
                && config.classify(-(1e-7 + 1e-12), 1.0) == ActivityStatus::Violated
        };
        // 3) 越过 active 带才违反，且 nearly 带只对"正侧"分档。 
        // Only beyond the active band is violated, and the nearly band only subdivides non-violations.
        let beyond_the_active_band_is_violated = {
            config.classify(-2e-6, 1.0) == ActivityStatus::Violated
                // 正侧的 2e-6 落在 nearly 带里。 / On the positive side 2e-6 sits in the nearly band.
                && config.classify(2e-6, 1.0) == ActivityStatus::NearlyActive
                // 更远就是 inactive。 / Further out is inactive.
                && config.classify(2e-5, 1.0) == ActivityStatus::Inactive
        };
        // 4) 尺度放大容差。 / The scale widens the tolerance.
        let the_scale_widens_the_tolerance = {
            let scaled = ActivityConfig { relative_tolerance: 0.1, ..config };
            // `max(1e-7, 0.1 * 100) = 10`。 / `max(1e-7, 0.1 * 100)` is ten.
            scaled.effective_tolerance(100.0) == 10.0
                && scaled.effective_nearly_active_tolerance(100.0).max(
                    scaled.effective_tolerance(100.0)) == 10.0
                // 于是 slack 5 在这个尺度下算 Active。 / So a slack of five reads as Active at that scale.
                && scaled.classify(5.0, 100.0) == ActivityStatus::Active
        };
        // 5) 尺度永不缩小容差。 / The scale never shrinks the tolerance.
        let the_scale_never_shrinks_the_tolerance = {
            let scaled = ActivityConfig { relative_tolerance: 0.1, ..config };
            // `|scale|.max(1.0)` 让 scale 0 与 0.5 都回落到 1 倍相对容差，也就是 0.1（大于 active 容差）。 
            // `|scale|.max(1.0)` makes both zero and a half fall back to one unit of relative tolerance, i.e. 0.1.
            scaled.effective_tolerance(0.0) == 0.1_f64
                && scaled.effective_tolerance(0.5) == 0.1_f64
                && scaled.effective_tolerance(-100.0) == 10.0
        };
        // 6) nearly 档永不低于 active 档。 / The nearly band never sits below active.
        let the_nearly_band_never_sits_below_active = {
            // 非法配置（nearly 小于 active）照样能分类，因为 `classify` 用 `.max(active)`。 
            // An illegal configuration still classifies, since `classify` applies `.max(active)`.
            let illegal = ActivityConfig::new(1e-5, 1e-7);
            illegal.validate().is_err()
                && illegal.classify(5e-6, 1.0) == ActivityStatus::Active
                // 合法配置下两条带是递增的。 / With a legal configuration the two bands are increasing.
                && config.effective_nearly_active_tolerance(1.0)
                    >= config.effective_tolerance(1.0)
        };
        // 7) 合并计数必须等于重算结果。 / The merged counts must equal a recount.
        let the_summary_must_match_the_records = {
            let records = vec![record("a", Some("g"), ActivityStatus::Active),
                record("b", Some("g"), ActivityStatus::Inactive)];
            report_of(&config, records.clone(), false).validate().is_ok()
                && report_of(&config, records, true).validate().is_err()
        };
        // 8) 组计数必须等于重算结果。 / The per-group counts must equal a recount.
        let the_group_summary_must_match_the_records = {
            let records = vec![record("a", Some("g"), ActivityStatus::Active),
                record("b", None, ActivityStatus::Active)];
            // 把组汇总整段换成一个错的：`g` 记成 0 个 active。 
            // Replace the group summaries with a wrong one that counts zero actives in `g`.
            let mut broken = report_of(&config, records.clone(), false);
            broken.groups = vec![ActivityGroupSummary { group: Some("g".to_owned()),
                summary: ActivitySummary::default() }];
            let honest = report_of(&config, records, false);
            broken.validate().is_err() && honest.validate().is_ok()
                // 缺一组也算错。 / A missing group is wrong too.
                && { let mut missing = honest.clone();
                    missing.groups.clear();
                    missing.validate().is_err() }
        };
        Findings {
            a_non_finite_slack_or_scale_is_unknown,
            minus_the_tolerance_is_active_not_violated,
            beyond_the_active_band_is_violated,
            the_scale_widens_the_tolerance,
            the_scale_never_shrinks_the_tolerance,
            the_nearly_band_never_sits_below_active,
            the_summary_must_match_the_records,
            the_group_summary_must_match_the_records,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(slack: f64, scale: f64) -> BandProbeSpec {
        BandProbeSpec { slack, scale }
    }

    fn request() -> BandProbeRequest {
        BandProbeRequest { tolerance: Some(1e-7), nearly_active_tolerance: Some(1e-5),
            relative_tolerance: Some(0.0),
            probes: vec![probe(0.0, 1.0), probe(-1e-7, 1.0), probe(2e-6, 1.0),
                probe(2e-5, 1.0), probe(-2e-6, 1.0)],
            break_summary: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_band_probe(&request()).unwrap();
        assert!(report.a_non_finite_slack_or_scale_is_unknown);
        assert!(report.minus_the_tolerance_is_active_not_violated);
        assert!(report.beyond_the_active_band_is_violated);
        assert!(report.the_scale_widens_the_tolerance);
        assert!(report.the_scale_never_shrinks_the_tolerance);
        assert!(report.the_nearly_band_never_sits_below_active);
        assert!(report.the_summary_must_match_the_records);
        assert!(report.the_group_summary_must_match_the_records);
        assert!(report.config_valid);
        assert!(report.report_valid);
        // 五条探针的分类结果。 / The five classifications.
        assert_eq!(report.rows.iter().map(|row| row.status.as_str()).collect::<Vec<_>>(),
            vec!["Active", "Active", "NearlyActive", "Inactive", "Violated"]);
        assert_eq!(report.rows[0].active_tolerance, 1e-7);
        assert_eq!(report.rows[0].nearly_tolerance, 1e-5);
    }

    #[test]
    fn a_non_finite_probe_is_unknown() {
        let report = verify_band_probe(&BandProbeRequest {
            probes: vec![probe(f64::NAN, 1.0), probe(1.0, f64::NAN)], ..request() }).unwrap();
        assert_eq!(report.rows[0].status, "Unknown");
        assert_eq!(report.rows[1].status, "Unknown");
    }

    #[test]
    fn the_scale_widens_and_never_shrinks() {
        let report = verify_band_probe(&BandProbeRequest { relative_tolerance: Some(0.1),
            probes: vec![probe(5.0, 100.0), probe(5.0, 0.0)], ..request() }).unwrap();
        // 尺度 100 → 容差 10，slack 5 算 Active。 / Scale one hundred widens to ten, so five is active.
        assert_eq!(report.rows[0].status, "Active");
        assert_eq!(report.rows[0].active_tolerance, 10.0);
        // 尺度 0 → 容差 0.1，slack 5 算 Inactive。 / Scale zero keeps 0.1, so five is inactive.
        assert_eq!(report.rows[1].status, "Inactive");
        assert_eq!(report.rows[1].active_tolerance, 0.1);
    }

    #[test]
    fn an_illegal_band_configuration_still_classifies_but_does_not_validate() {
        let report = verify_band_probe(&BandProbeRequest { tolerance: Some(1e-5),
            nearly_active_tolerance: Some(1e-7), probes: vec![probe(5e-6, 1.0)],
            ..request() }).unwrap();
        assert!(!report.config_valid);
        // `classify` 用 `.max(active)`，于是 5e-6 仍落在 Active。 
        // `classify` applies `.max(active)`, so 5e-6 still reads as active.
        assert_eq!(report.rows[0].status, "Active");
    }

    #[test]
    fn a_broken_summary_is_refused() {
        let broken = verify_band_probe(&BandProbeRequest { break_summary: Some(true),
            ..request() }).unwrap();
        assert!(!broken.report_valid);
        let honest = verify_band_probe(&BandProbeRequest { break_summary: Some(false),
            ..request() }).unwrap();
        assert!(honest.report_valid);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_band_probe(&BandProbeRequest { probes: (0..9).map(|index|
            probe(index as f64, 1.0)).collect(), ..request() }).unwrap_err()
            .message.contains("at most eight probes"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_band_probe(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"activeTolerance\""), "{json}");
        assert!(json.contains("\"configValid\""), "{json}");
        assert!(json.contains("\"minusTheToleranceIsActiveNotViolated\""), "{json}");
        assert!(!json.contains("active_tolerance"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BandProbeReport>(&copy.to_string()).is_err());
    }
}
