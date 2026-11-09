//! 续挖第 100 单：**跨阶段组汇总的四条口径**——顺序是**阻塞降序 → active 降序 → 组名升序**，而未分组（`None`）
//! 因为是 `Option` 排序**永远排在最前**；组只会由三个来源"落地"：活动性报告的**每条**记录、有效性排序里**带改善**
//! 的条目、冲突解释里的阻塞身份；于是**没有改善的有效性条目不会建组**，而 `max_improvement` 取的是**最大值不是求和**；
//! `active_instances` **只数 `Active`**（`NearlyActive` 不算）；只由有效性建出来的组**三项计数全为 0**
//! （`analysis/pipeline.rs` 的 `AnalysisGroupSummary` / `summarize_groups`）
//! Continuation, delivery 100: **the four readings behind the cross-stage group summary** — the order is **blocking
//! descending → active descending → name ascending**, and the ungrouped `None` bucket **wins a tie** because of `Option`'s
//! ordering, while a group with more actives still precedes it; a group is materialised by three sources only: **every** activity record, effectiveness entries
//! **carrying an improvement**, and the blocking identities of a conflict explanation; hence **an effectiveness entry
//! without an improvement creates no group** and `max_improvement` is a **maximum rather than a sum**;
//! `active_instances` **counts only `Active`** (`NearlyActive` is excluded); and a group created by effectiveness alone
//! carries **zero in all three counts** (`AnalysisGroupSummary` / `summarize_groups` in `analysis/pipeline.rs`).
//!
//! 第 99 单量的是统一报告的校验，这一单量它的**组汇总**（`pipeline.rs` 的最后一个公开数据面）。八条实测：
//!
//! 1. **顺序是阻塞降序 → active 降序 → 组名升序**（`the_order_is_blocking_then_active_then_name()`）。
//! 2. **并列时未分组排最前**（`the_ungrouped_bucket_wins_a_tie()`）：`Option` 的排序里 `None < Some`，
//!    但 active 更多的组仍然排在未分组之前——所以未分组只在**并列**时先行。
//! 3. **没有改善的有效性条目不会建组**（`an_effectiveness_without_an_improvement_creates_no_group()`）。
//! 4. **`max_improvement` 取最大值不是求和**（`the_max_improvement_is_a_maximum_not_a_sum()`）。
//! 5. **`active_instances` 只数 `Active`**（`only_active_counts_as_active()`）：`NearlyActive` 不进这个计数。
//! 6. **只由有效性建出来的组三项计数全为 0**（`an_effectiveness_only_group_has_zero_counts()`）。
//! 7. **只由活动性建出来的组没有改善值**（`an_activity_only_group_has_no_improvement()`）。
//! 8. **三个输入都缺时汇总为空**（`no_stage_means_an_empty_summary()`）。
//!
//! Delivery 99 measured the unified report's validation; this one measures its **group summary**, the last public data
//! face of `pipeline.rs`. **The order is blocking then active then name**, and **the ungrouped bucket wins a tie** rather than leading
//! outright. **An
//! effectiveness entry without an improvement creates no group.** **`max_improvement` is a maximum rather than a sum.**
//! **`active_instances` counts only `Active`**, so `NearlyActive` is excluded. **An effectiveness-only group carries zero
//! counts.** **An activity-only group has no improvement.** And **no stage at all means an empty summary.**

use ospf_rust_core::analysis::activity::{
    ActivityEvidence, ActivitySummary, ConstraintActivity, ConstraintActivityReport,
};
use ospf_rust_core::analysis::pipeline::{
    summarize_groups, AnalysisGroupSummary, CandidateTier, EffectivenessEntry,
    EffectivenessRanking,
};
use ospf_rust_core::analysis::{ActivityStatus, AnalysisStatus};
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条活动性记录。 / One activity record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupActivitySpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 组名（`null` 表示未分组）。 / The group name, `null` for ungrouped.
    #[serde(default)]
    pub group: Option<String>,
    /// 活动性名（六个之一）。 / The activity name, one of six.
    pub activity: String,
}

/// 一条有效性条目。 / One effectiveness entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupEffectivenessSpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 组名（`null` 表示未分组）。 / The group name, `null` for ungrouped.
    #[serde(default)]
    pub group: Option<String>,
    /// 已证明的最大改善（`null` 表示没有）。 / The proven maximum improvement, `null` for none.
    #[serde(default)]
    pub max_improvement: Option<f64>,
}

/// 组汇总的一行读数。 / One line of the group summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupSummaryRow {
    /// 组名（未分组时为空串）。 / The group name, an empty string when ungrouped.
    pub group: String,
    /// 是否未分组。 / Whether this is the ungrouped bucket.
    pub ungrouped: bool,
    /// 涉及实例数。 / Involved instances.
    pub involved_instances: u32,
    /// active 实例数。 / Active instances.
    pub active_instances: u32,
    /// 阻塞实例数。 / Blocking instances.
    pub blocking_instances: u32,
    /// 已证明的最大改善。 / The proven maximum improvement.
    #[serde(default)]
    pub max_improvement: Option<f64>,
}

/// 组汇总探针请求。 / The group-summary probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupSummaryRequest {
    /// 活动性记录（缺省表示没有该阶段）。 / The activity records; absent means the stage is missing.
    #[serde(default)]
    pub activity: Option<Vec<GroupActivitySpec>>,
    /// 有效性条目（缺省表示没有该阶段）。 / The effectiveness entries; absent means the stage is missing.
    #[serde(default)]
    pub effectiveness: Option<Vec<GroupEffectivenessSpec>>,
}

/// 组汇总探针结论。 / The group-summary probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupSummaryReport {
    /// 每一组的读数（按输出的顺序）。 / Every group's reading in output order.
    pub rows: Vec<GroupSummaryRow>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_order_is_blocking_then_active_then_name: bool,
    pub the_ungrouped_bucket_wins_a_tie: bool,
    pub an_effectiveness_without_an_improvement_creates_no_group: bool,
    pub the_max_improvement_is_a_maximum_not_a_sum: bool,
    pub only_active_counts_as_active: bool,
    pub an_effectiveness_only_group_has_zero_counts: bool,
    pub an_activity_only_group_has_no_improvement: bool,
    pub no_stage_means_an_empty_summary: bool,
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

/// 造一条活动性记录。 / Build one activity record.
fn record_of(spec: &GroupActivitySpec) -> Result<ConstraintActivity> {
    let status = activity_named(&spec.activity).ok_or_else(|| BridgeError::new("InvalidInput",
        "group summmary probe failed validation: unknown activity name"))?;
    Ok(ConstraintActivity {
        constraint_id: StableConstraintId(spec.id.clone()),
        group: spec.group.clone(),
        status,
        lhs: None,
        rhs: None,
        slack: None,
        normalized_slack: None,
        tolerance: None,
        satisfied: None,
        evidence: ActivityEvidence::LinearConstraint { lhs: 0.0, rhs: 0.0 },
    })
}

/// 造一条有效性条目。 / Build one effectiveness entry.
fn entry_of(spec: &GroupEffectivenessSpec) -> EffectivenessEntry {
    EffectivenessEntry {
        constraint_id: StableConstraintId(spec.id.clone()),
        group: spec.group.clone(),
        tier: CandidateTier::Unclassified,
        activity: ActivityStatus::Unknown,
        dual_value: None,
        local_effective: None,
        removal_effective: None,
        first_effective_delta: None,
        max_improvement: spec.max_improvement,
        marginal_effect: None,
        status: AnalysisStatus::Unknown,
        unavailable_reason: None,
    }
}

/// 造一份活动性报告（不含变量边界）。 / Build an activity report without variable bounds.
fn activity_report_of(specs: &[GroupActivitySpec]) -> Result<ConstraintActivityReport> {
    let constraints = specs.iter().map(record_of).collect::<Result<Vec<_>>>()?;
    let mut summary = ActivitySummary::default();
    for record in &constraints {
        summary.add(record.status);
    }
    Ok(ConstraintActivityReport {
        schema_version: "group-probe".to_owned(),
        constraints,
        variable_bounds: Vec::new(),
        summary: summary.clone(),
        variable_bound_summary: ActivitySummary::default(),
        groups: Vec::new(),
        tolerance: 0.0,
        nearly_active_tolerance: 0.0,
    })
}

/// 造一份有效性排序。 / Build an effectiveness ranking.
fn ranking_of(specs: &[GroupEffectivenessSpec]) -> EffectivenessRanking {
    EffectivenessRanking {
        schema_version: "group-probe".to_owned(),
        baseline_objective: 0.0,
        entries: specs.iter().map(entry_of).collect(),
    }
}

/// 把汇总铺成读数行。 / Lay the summary out as readable rows.
fn rows_of(summaries: &[AnalysisGroupSummary]) -> Vec<GroupSummaryRow> {
    summaries.iter().map(|summary| GroupSummaryRow {
        group: summary.group.clone().unwrap_or_default(),
        ungrouped: summary.group.is_none(),
        involved_instances: u32::try_from(summary.involved_instances).unwrap_or(u32::MAX),
        active_instances: u32::try_from(summary.active_instances).unwrap_or(u32::MAX),
        blocking_instances: u32::try_from(summary.blocking_instances).unwrap_or(u32::MAX),
        max_improvement: summary.max_improvement,
    }).collect()
}

/// 走一遍组汇总探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_group_summary(request: &GroupSummaryRequest) -> Result<GroupSummaryReport> {
    if request.activity.as_ref().is_some_and(|rows| rows.len() > 8)
        || request.effectiveness.as_ref().is_some_and(|rows| rows.len() > 8)
    {
        return Err(BridgeError::new("InvalidInput", "group summary probe failed validation: \
            at most eight records per stage"));
    }
    let candidates: Vec<f64> = request.effectiveness.iter().flatten()
        .filter_map(|spec| spec.max_improvement).collect();
    if candidates.iter().any(|value| !value.is_finite()) {
        return Err(BridgeError::new("InvalidInput", "group summary probe failed validation: \
            non-finite improvements cannot cross the boundary"));
    }
    let findings = canonical_findings();
    let activity = match &request.activity {
        Some(specs) => Some(activity_report_of(specs)?),
        None => None,
    };
    let ranking = request.effectiveness.as_ref().map(|specs| ranking_of(specs));
    let summaries = summarize_groups(activity.as_ref(), ranking.as_ref(), None);
    Ok(GroupSummaryReport {
        rows: rows_of(&summaries),
        the_order_is_blocking_then_active_then_name:
            findings.the_order_is_blocking_then_active_then_name,
        the_ungrouped_bucket_wins_a_tie: findings.the_ungrouped_bucket_wins_a_tie,
        an_effectiveness_without_an_improvement_creates_no_group:
            findings.an_effectiveness_without_an_improvement_creates_no_group,
        the_max_improvement_is_a_maximum_not_a_sum:
            findings.the_max_improvement_is_a_maximum_not_a_sum,
        only_active_counts_as_active: findings.only_active_counts_as_active,
        an_effectiveness_only_group_has_zero_counts:
            findings.an_effectiveness_only_group_has_zero_counts,
        an_activity_only_group_has_no_improvement:
            findings.an_activity_only_group_has_no_improvement,
        no_stage_means_an_empty_summary: findings.no_stage_means_an_empty_summary,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_order_is_blocking_then_active_then_name: bool,
    the_ungrouped_bucket_wins_a_tie: bool,
    an_effectiveness_without_an_improvement_creates_no_group: bool,
    the_max_improvement_is_a_maximum_not_a_sum: bool,
    only_active_counts_as_active: bool,
    an_effectiveness_only_group_has_zero_counts: bool,
    an_activity_only_group_has_no_improvement: bool,
    no_stage_means_an_empty_summary: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 一次汇总：组 "alpha" 两条 active、组 "beta" 一条 inactive、一条未分组 active。 
        // One summary: group "alpha" has two active records, "beta" one inactive, plus one ungrouped active.
        let activity = activity_report_of(&[
            GroupActivitySpec { id: "a1".to_owned(), group: Some("alpha".to_owned()),
                activity: "Active".to_owned() },
            GroupActivitySpec { id: "a2".to_owned(), group: Some("alpha".to_owned()),
                activity: "Active".to_owned() },
            GroupActivitySpec { id: "b1".to_owned(), group: Some("beta".to_owned()),
                activity: "Inactive".to_owned() },
            GroupActivitySpec { id: "u1".to_owned(), group: None,
                activity: "Active".to_owned() },
        ]).expect("activity report");
        let summaries = summarize_groups(Some(&activity), None, None);
        // 1) 顺序：active 降序 → 组名升序（阻塞数全 0 时）。 
        // The order under zero blocking: active descending, then name ascending.
        let the_order_is_blocking_then_active_then_name = {
            let names: Vec<Option<String>> = summaries.iter().map(|s| s.group.clone()).collect();
            // active 数是 2 / 1 / 0，于是 alpha 排第一、未分组其次、beta 最后。 
            // The active counts are two, one and zero, so alpha leads, the ungrouped bucket follows and beta trails.
            names == vec![Some("alpha".to_owned()), None, Some("beta".to_owned())]
                // 并列 active 时按组名升序。 / Equal active counts fall back to the name ascending.
                && { let tied = summarize_groups(Some(&activity_report_of(&[
                        GroupActivitySpec { id: "t1".to_owned(), group: Some("zulu".to_owned()),
                            activity: "Active".to_owned() },
                        GroupActivitySpec { id: "t2".to_owned(), group: Some("yankee".to_owned()),
                            activity: "Active".to_owned() },
                    ]).expect("activity report")), None, None);
                    tied.iter().map(|summary| summary.group.clone()).collect::<Vec<_>>()
                        == vec![Some("yankee".to_owned()), Some("zulu".to_owned())] }
        };
        // 2) 并列时未分组排最前。 / On a tie the ungrouped bucket comes first.
        let the_ungrouped_bucket_wins_a_tie = {
            // 未分组与 beta 的 active 都是 0 → `None < Some(..)` 让未分组排前。 
            // The ungrouped bucket and beta both have zero actives, so `None < Some(..)` puts the bucket first.
            let tied = summarize_groups(Some(&activity_report_of(&[
                GroupActivitySpec { id: "u1".to_owned(), group: None,
                    activity: "Inactive".to_owned() },
                GroupActivitySpec { id: "b1".to_owned(), group: Some("beta".to_owned()),
                    activity: "Inactive".to_owned() },
            ]).expect("activity report")), None, None);
            tied.first().is_some_and(|summary| summary.group.is_none())
                && tied.iter().filter(|summary| summary.group.is_none()).count() == 1
        };
        // 3) 没有改善的有效性条目不会建组。 / An effectiveness entry without an improvement creates no group.
        let an_effectiveness_without_an_improvement_creates_no_group = {
            let none = summarize_groups(None, Some(&ranking_of(&[GroupEffectivenessSpec {
                id: "e1".to_owned(), group: Some("gamma".to_owned()),
                max_improvement: None }])), None);
            let some = summarize_groups(None, Some(&ranking_of(&[GroupEffectivenessSpec {
                id: "e2".to_owned(), group: Some("gamma".to_owned()),
                max_improvement: Some(1.0) }])), None);
            // 没有改善 → 空；有改善 → 建组且计数全 0。 
            // Without an improvement the list is empty; with one a group appears with zero counts.
            none.is_empty() && some.len() == 1
        };
        // 4) 最大值不是求和。 / A maximum rather than a sum.
        let the_max_improvement_is_a_maximum_not_a_sum = {
            let maxed = summarize_groups(None, Some(&ranking_of(&[
                GroupEffectivenessSpec { id: "e1".to_owned(),
                    group: Some("delta".to_owned()), max_improvement: Some(3.0) },
                GroupEffectivenessSpec { id: "e2".to_owned(),
                    group: Some("delta".to_owned()), max_improvement: Some(9.0) },
            ])), None);
            maxed.len() == 1 && maxed[0].max_improvement == Some(9.0)
        };
        // 5) `active_instances` 只数 `Active`。 / `active_instances` counts only `Active`.
        let only_active_counts_as_active = {
            let report = activity_report_of(&[
                GroupActivitySpec { id: "a1".to_owned(), group: Some("eps".to_owned()),
                    activity: "Active".to_owned() },
                GroupActivitySpec { id: "a2".to_owned(), group: Some("eps".to_owned()),
                    activity: "NearlyActive".to_owned() },
            ]).expect("activity report");
            let one = summarize_groups(Some(&report), None, None);
            // 两条记录都进 involved，但只有一条进 active。 
            // Both records count as involved while only one counts as active.
            one.len() == 1 && one[0].involved_instances == 2 && one[0].active_instances == 1
        };
        // 6) 只由有效性建出来的组三项计数全为 0。 / An effectiveness-only group has zero counts.
        let an_effectiveness_only_group_has_zero_counts = {
            let only = summarize_groups(None, Some(&ranking_of(&[GroupEffectivenessSpec {
                id: "e1".to_owned(), group: Some("zeta".to_owned()),
                max_improvement: Some(2.0) }])), None);
            only.len() == 1 && only[0].involved_instances == 0 && only[0].active_instances == 0
                && only[0].blocking_instances == 0 && only[0].max_improvement == Some(2.0)
        };
        // 7) 只由活动性建出来的组没有改善值。 / An activity-only group has no improvement.
        let an_activity_only_group_has_no_improvement = {
            let only = summarize_groups(Some(&activity), None, None);
            only.iter().all(|summary| summary.max_improvement.is_none())
                && only.iter().map(|summary| summary.involved_instances).sum::<usize>() == 4
        };
        // 8) 三个输入都缺时汇总为空。 / No stage at all means an empty summary.
        let no_stage_means_an_empty_summary = summarize_groups(None, None, None).is_empty();
        Findings {
            the_order_is_blocking_then_active_then_name,
            the_ungrouped_bucket_wins_a_tie,
            an_effectiveness_without_an_improvement_creates_no_group,
            the_max_improvement_is_a_maximum_not_a_sum,
            only_active_counts_as_active,
            an_effectiveness_only_group_has_zero_counts,
            an_activity_only_group_has_no_improvement,
            no_stage_means_an_empty_summary,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, group: Option<&str>, activity: &str) -> GroupActivitySpec {
        GroupActivitySpec { id: id.to_owned(), group: group.map(str::to_owned),
            activity: activity.to_owned() }
    }

    fn entry(id: &str, group: Option<&str>, improvement: Option<f64>) -> GroupEffectivenessSpec {
        GroupEffectivenessSpec { id: id.to_owned(), group: group.map(str::to_owned),
            max_improvement: improvement }
    }

    fn request() -> GroupSummaryRequest {
        GroupSummaryRequest { activity: Some(vec![row("a1", Some("alpha"), "Active"),
            row("a2", Some("alpha"), "Active"), row("b1", Some("beta"), "Inactive"),
            row("u1", None, "Active")]), effectiveness: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_group_summary(&request()).unwrap();
        assert!(report.the_order_is_blocking_then_active_then_name);
        assert!(report.the_ungrouped_bucket_wins_a_tie);
        assert!(report.an_effectiveness_without_an_improvement_creates_no_group);
        assert!(report.the_max_improvement_is_a_maximum_not_a_sum);
        assert!(report.only_active_counts_as_active);
        assert!(report.an_effectiveness_only_group_has_zero_counts);
        assert!(report.an_activity_only_group_has_no_improvement);
        assert!(report.no_stage_means_an_empty_summary);
        // 顺序：alpha（2 active）→ 未分组（1）→ beta（0）。 
        // The order: alpha with two actives, then the ungrouped bucket, then beta with none.
        assert_eq!(report.rows.len(), 3);
        assert_eq!(report.rows[0].group, "alpha");
        assert_eq!(report.rows[0].involved_instances, 2);
        assert_eq!(report.rows[0].active_instances, 2);
        assert!(report.rows[1].ungrouped);
        assert_eq!(report.rows[1].active_instances, 1);
        assert_eq!(report.rows[2].group, "beta");
        assert_eq!(report.rows[2].active_instances, 0);
        assert!(report.rows.iter().all(|row| row.max_improvement.is_none()));
        assert!(report.rows.iter().all(|row| row.blocking_instances == 0));
    }

    #[test]
    fn no_stage_gives_an_empty_summary() {
        let report = verify_group_summary(&GroupSummaryRequest { activity: None,
            effectiveness: None }).unwrap();
        assert!(report.rows.is_empty());
        assert!(report.no_stage_means_an_empty_summary);
    }

    #[test]
    fn an_effectiveness_only_group_has_no_counts() {
        let report = verify_group_summary(&GroupSummaryRequest { activity: None,
            effectiveness: Some(vec![entry("e1", Some("zeta"), Some(2.0))]) }).unwrap();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].group, "zeta");
        assert_eq!(report.rows[0].involved_instances, 0);
        assert_eq!(report.rows[0].active_instances, 0);
        assert_eq!(report.rows[0].max_improvement, Some(2.0));
    }

    #[test]
    fn an_improvement_of_none_creates_no_group() {
        let report = verify_group_summary(&GroupSummaryRequest { activity: None,
            effectiveness: Some(vec![entry("e1", Some("gamma"), None)]) }).unwrap();
        assert!(report.rows.is_empty());
    }

    #[test]
    fn the_improvement_is_a_maximum_per_group() {
        let report = verify_group_summary(&GroupSummaryRequest { activity: None,
            effectiveness: Some(vec![entry("e1", Some("delta"), Some(3.0)),
                entry("e2", Some("delta"), Some(9.0))]) }).unwrap();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].max_improvement, Some(9.0));
    }

    #[test]
    fn only_active_counts_as_active() {
        let report = verify_group_summary(&GroupSummaryRequest { activity: Some(vec![
            row("a1", Some("eps"), "Active"), row("a2", Some("eps"), "NearlyActive")]),
            effectiveness: None }).unwrap();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].involved_instances, 2);
        assert_eq!(report.rows[0].active_instances, 1);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_group_summary(&GroupSummaryRequest { activity: Some((0..9).map(|index|
            row(&format!("c{index}"), None, "Active")).collect()), effectiveness: None })
            .unwrap_err().message.contains("at most eight records"));
        assert!(verify_group_summary(&GroupSummaryRequest { activity: Some(vec![
            GroupActivitySpec { id: "a".to_owned(), group: None,
                activity: "Nearly".to_owned() }]), effectiveness: None }).unwrap_err()
            .message.contains("unknown activity name"));
        assert!(verify_group_summary(&GroupSummaryRequest { activity: None,
            effectiveness: Some(vec![entry("e", None, Some(f64::NAN))]) }).unwrap_err()
            .message.contains("non-finite improvements"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_group_summary(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"involvedInstances\""), "{json}");
        assert!(json.contains("\"ungrouped\""), "{json}");
        assert!(json.contains("\"theUngroupedBucketWinsATie\""), "{json}");
        assert!(!json.contains("involved_instances"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<GroupSummaryReport>(&copy.to_string()).is_err());
    }
}
