//! 续挖第 2 单：跨阶段约束组汇总（`analysis/pipeline.rs` 的 `AnalysisGroupSummary` + `summarize_groups`）
//! Continuation, delivery 2: the cross-stage constraint-group aggregation.
//!
//! `analysis/pipeline.rs` 剩下 9 个未引用类型，其中 7 个是 builder / adapter / pipeline（带生命周期、借 CP 快照，
//! 属真边界）；真正可数据化的只有 `AnalysisGroupSummary` 与产出它的 `summarize_groups`。这一单把汇总能力交出去：
//! **TS 把它已经拿到的那几份报告交进来，换一份跨阶段的组视**。
//!
//! 汇总语义（领域实现，桥接一条不重写）里有四条是只看签名看不出来的：
//!
//! 1. **组的来源有三个**：活动性报告里出现的组、有效性排名里**带改善值**的组、冲突报告里的成员所属组。只被有效性提到
//!    但整条 `max_improvement` 为空时，那一组**不会**因此materialise。
//! 2. **阻塞计数按活动性报告归组**：冲突报告刻意只保存稳定身份，所以成员落在哪一组要去活动性报告里查。查不到的成员
//!    归入**未分组**桶——这条防的是"把阻塞算到错的组上"。
//! 3. **`max_improvement` 取最大而不是最后**（含负值，负改善也算"观察到"）。
//! 4. **排序是确定性的**：阻塞数降序 → 活跃数降序 → 组名升序；`None`（未分组）在 `Option` 序里排在 `Some` **之前**。
//!
//! Of the nine unreferenced types there, seven are builders, adapters and pipelines (lifetime-bound, borrowing a CP
//! snapshot — a true boundary); only `AnalysisGroupSummary` and the `summarize_groups` that produces it are datafiable. This
//! delivery hands over the aggregation: **TypeScript submits the reports it already has and receives a cross-stage group
//! view**. Four rules are invisible from the signatures: groups materialise from **three** sources (an effectiveness entry
//! without an improvement value does **not** bring its group into existence); **blocking counts are grouped via the activity
//! report**, because the conflict report deliberately stores only stable identities, so an unknown member falls into the
//! **ungrouped** bucket — which prevents attributing blockers to the wrong group; `max_improvement` takes the **maximum**,
//! negatives included; and the ordering is deterministic (blocking desc, active desc, group name asc) with `None` (ungrouped)
//! sorting **before** `Some` per the `Option` ordering.

use ospf_rust_core::analysis::pipeline::{
    summarize_groups, AnalysisGroupSummary, EffectivenessRanking,
};
use ospf_rust_core::analysis::{ConflictExplanation, ConstraintActivityReport};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 跨阶段汇总请求：三份报告的 JSON 文本（都可缺省）。 / The aggregation request: three optional report JSON texts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisGroupAggregateRequest {
    /// 约束活动性报告。 / The constraint activity report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub activity_json: Option<String>,
    /// 有效性排名。 / The effectiveness ranking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effectiveness_json: Option<String>,
    /// 冲突解释。 / The conflict explanation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conflict_json: Option<String>,
}

/// 一组的汇总。 / One group's summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisGroupRow {
    /// 组名；未分组时为 `undefined`。 / The group name; `undefined` when ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<String>,
    /// 该组涉及的约束实例数。 / Involved constraint instances.
    pub involved_instances: u32,
    /// 其中活跃的实例数。 / Active instances.
    pub active_instances: u32,
    /// 其中进入当前 MUS 的实例数。 / Instances in the current MUS.
    pub blocking_instances: u32,
    /// 组内已证明的最大改善。 / The largest proven improvement in this group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_improvement: Option<f64>,
}

/// 跨阶段汇总结论。 / The aggregation verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisGroupAggregateReport {
    /// 组数。 / The number of groups.
    pub group_count: u32,
    /// 各行按领域的确定序给出。 / The rows in the domain's deterministic order.
    pub groups: Vec<AnalysisGroupRow>,
    /// 是否存在未分组桶。 / Whether an ungrouped bucket exists.
    pub ungrouped_present: bool,
    /// 涉及实例总数。 / Total involved instances.
    pub total_involved: u32,
    /// 活跃实例总数。 / Total active instances.
    pub total_active: u32,
    /// 阻塞实例总数。 / Total blocking instances.
    pub total_blocking: u32,
    /// 哪几份报告实际参与（`activity` / `effectiveness` / `conflict`）。 / Which reports took part.
    pub sources: Vec<String>,
    /// 排序规则说明（照领域的序写出，供 TS 依赖顺序）。 / The ordering rule, for callers relying on order.
    pub ordering_rule: String,
}

/// 单份报告的上限：信任边界。 / The per-report cap: a trust boundary.
const MAX_REPORT_BYTES: usize = 8 * 1024 * 1024;

/// 汇总三份（可选）报告里的约束组。非法输入是**错误**，原因原样带出。
/// Aggregate the constraint groups across three optional reports. Illegal input is an **error** carrying the reason.
pub fn aggregate_analysis_groups(request: &AnalysisGroupAggregateRequest)
    -> Result<AnalysisGroupAggregateReport>
{
    let mut sources = Vec::with_capacity(3);
    let activity = decode::<ConstraintActivityReport>(request.activity_json.as_deref(), "activity")?;
    if activity.is_some() { sources.push("activity".to_owned()); }
    let effectiveness = decode::<EffectivenessRanking>(request.effectiveness_json.as_deref(),
        "effectiveness")?;
    if effectiveness.is_some() { sources.push("effectiveness".to_owned()); }
    let conflict = decode::<ConflictExplanation>(request.conflict_json.as_deref(), "conflict")?;
    if conflict.is_some() { sources.push("conflict".to_owned()); }
    // 领域汇总：组来源、阻塞归组、最大改善与排序全在这里，桥接不重写。
    // The domain aggregation: group sources, blocking attribution, maximum improvement and ordering all live here.
    let summaries = summarize_groups(activity.as_ref(), effectiveness.as_ref(), conflict.as_ref());
    let rows = summaries.iter().map(project).collect::<Vec<_>>();
    let total_involved = summaries.iter()
        .try_fold(0u32, |total, summary| total.checked_add(count(summary.involved_instances)))
        .unwrap_or(u32::MAX);
    let total_active = summaries.iter()
        .try_fold(0u32, |total, summary| total.checked_add(count(summary.active_instances)))
        .unwrap_or(u32::MAX);
    let total_blocking = summaries.iter()
        .try_fold(0u32, |total, summary| total.checked_add(count(summary.blocking_instances)))
        .unwrap_or(u32::MAX);
    Ok(AnalysisGroupAggregateReport {
        group_count: count(summaries.len()),
        groups: rows,
        ungrouped_present: summaries.iter().any(|summary| summary.group.is_none()),
        total_involved,
        total_active,
        total_blocking,
        sources,
        ordering_rule: "blocking desc, active desc, group name asc; the ungrouped bucket sorts first"
            .to_owned(),
    })
}

/// 解码一份可选报告；体积与格式都在这里拦。 / Decode one optional report, checking size and shape here.
fn decode<T: serde::de::DeserializeOwned>(json: Option<&str>, label: &str) -> Result<Option<T>> {
    let Some(json) = json else { return Ok(None); };
    if json.len() > MAX_REPORT_BYTES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "analysis group aggregation failed validation: the {label} report exceeds {MAX_REPORT_BYTES} bytes")));
    }
    if json.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput", &format!(
            "analysis group aggregation failed validation: the {label} report is empty")));
    }
    serde_json::from_str(json).map(Some).map_err(|error| BridgeError::new("InvalidInput", &format!(
        "analysis group aggregation failed validation: the {label} report could not be decoded: {error}")))
}

/// 投影一组汇总。 / Project one group summary.
fn project(summary: &AnalysisGroupSummary) -> AnalysisGroupRow {
    AnalysisGroupRow {
        group: summary.group.clone(),
        involved_instances: count(summary.involved_instances),
        active_instances: count(summary.active_instances),
        blocking_instances: count(summary.blocking_instances),
        max_improvement: summary.max_improvement,
    }
}

/// 计数字段的安全转换。 / A safe cast for count fields.
fn count(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use ospf_rust_core::analysis::{
        ActivityEvidence, ActivityGroupSummary, ActivitySummary, AnalysisStatus, CandidateTier,
        ConflictExtractionTier, ConflictGroupSummary, ConflictMinimality, ConflictValidity,
        ActivityStatus, ConstraintActivity, DiagnosticSource, EffectivenessEntry,
        ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};

    const CONFLICT_SCHEMA: &str = ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION;

    fn record(id: &str, group: Option<&str>, status: ActivityStatus) -> ConstraintActivity {
        ConstraintActivity { constraint_id: id.into(), group: group.map(str::to_owned), status,
            lhs: Some(1.0), rhs: Some(1.0), slack: Some(0.0), normalized_slack: Some(0.0),
            tolerance: Some(1e-7), satisfied: Some(true),
            evidence: ActivityEvidence::LinearConstraint { lhs: 1.0, rhs: 1.0 } }
    }

    /// 一份自洽的活动性报告（汇总字段由记录算出来，不手填）。 / A self-consistent activity report.
    fn activity(records: Vec<ConstraintActivity>) -> ConstraintActivityReport {
        let mut summary = ActivitySummary::default();
        let mut grouped = BTreeMap::<Option<String>, ActivitySummary>::new();
        for item in &records {
            summary.add(item.status);
            grouped.entry(item.group.clone()).or_default().add(item.status);
        }
        let groups = grouped.into_iter()
            .map(|(group, summary)| ActivityGroupSummary { group, summary }).collect();
        ConstraintActivityReport { schema_version: "1.0".to_owned(), constraints: records,
            variable_bounds: Vec::new(), summary, variable_bound_summary: ActivitySummary::default(),
            groups, tolerance: 1e-7, nearly_active_tolerance: 1e-5 }
    }

    fn entry(id: &str, group: Option<&str>, improvement: Option<f64>) -> EffectivenessEntry {
        EffectivenessEntry { constraint_id: id.into(), group: group.map(str::to_owned),
            tier: CandidateTier::TierA, activity: ActivityStatus::Active, dual_value: None,
            local_effective: None, removal_effective: None, first_effective_delta: None,
            max_improvement: improvement, marginal_effect: None, status: AnalysisStatus::Reachable,
            unavailable_reason: None }
    }

    fn ranking(entries: Vec<EffectivenessEntry>) -> EffectivenessRanking {
        EffectivenessRanking { schema_version: "1.0".to_owned(), baseline_objective: 10.0, entries }
    }

    /// 一份**自洽的最小冲突解释**（形状与 `analysis_conflict` 的测试 helper 一致）。
    /// A **self-consistent minimal conflict explanation**, mirroring the helper in `analysis_conflict`'s tests.
    fn explanation(member_ids: &[&str]) -> ConflictExplanation {
        let target = ObjectiveTarget::at_least("objective", 5.0).expect("target");
        ConflictExplanation {
            schema_version: CONFLICT_SCHEMA.to_owned(),
            target: target.clone(),
            target_feasibility: TargetFeasibilityReport {
                schema_version: CONFLICT_SCHEMA.to_owned(),
                target: target.clone(),
                effective_integer_bound: None,
                source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
                status: AnalysisStatus::Unreachable,
                solution: None,
                exact_objective: None,
                objective_value: None,
                proof_status: ProofStatus::Verified,
                termination_reason: None,
                solve_time: None,
                target_fixed: true,
                message: None,
            },
            status: AnalysisStatus::Unreachable,
            available_evidence: Vec::new(),
            fixed_background: Vec::new(),
            members: member_ids.iter().map(|id| DiagnosticSource::Constraint {
                id: StableConstraintId((*id).to_owned()) }).collect(),
            target_source: DiagnosticSource::ObjectiveTarget { target },
            validity: ConflictValidity::Verified,
            minimality: ConflictMinimality::Irreducible,
            verifications: Vec::new(),
            group_summaries: Vec::new(),
            resolve_count: 0,
            memoized_checks: 0,
            extraction_tier: ConflictExtractionTier::AssumptionExtraction,
            unavailable_reason: None,
            background_blocking: false,
        }
    }

    fn request(activity_report: Option<&ConstraintActivityReport>,
        ranking_report: Option<&EffectivenessRanking>, conflict_report: Option<&ConflictExplanation>)
        -> AnalysisGroupAggregateRequest
    {
        AnalysisGroupAggregateRequest {
            activity_json: activity_report.map(|report| serde_json::to_string(report).expect("encode")),
            effectiveness_json: ranking_report.map(|report| serde_json::to_string(report).expect("encode")),
            conflict_json: conflict_report.map(|report| serde_json::to_string(report).expect("encode")),
        }
    }

    #[test]
    fn groups_materialise_from_all_three_sources_and_blockers_are_grouped_by_activity() {
        let activity_report = activity(vec![
            record("c1", Some("g1"), ActivityStatus::Active),
            record("c2", Some("g2"), ActivityStatus::Inactive),
        ]);
        // 只有带改善值的条目才把组拉进来；`g3` 的条目没有改善值，不该因此出现。
        // Only an entry with an improvement value materialises its group, so `g3` must not appear.
        let ranking_report = ranking(vec![
            entry("c1", Some("g1"), Some(0.25)),
            entry("c2", Some("g2"), Some(0.5)),
            entry("c3", Some("g3"), None),
        ]);
        // `c2` 在活动性报告里属于 `g2`；`ghost` 不在任何组里 → 落进未分组桶。
        // `c2` belongs to `g2` per the activity report; `ghost` is unknown, so it lands in the ungrouped bucket.
        let conflict_report = explanation(&["c2", "ghost"]);
        let report = aggregate_analysis_groups(&request(Some(&activity_report), Some(&ranking_report),
            Some(&conflict_report))).unwrap();
        assert_eq!(report.sources, vec!["activity".to_owned(), "effectiveness".to_owned(),
            "conflict".to_owned()]);
        // 三组：`g2`（1 阻塞、1 涉及、1 活跃？否），未分组（1 阻塞），`g1`（无阻塞）。
        // Three groups: `g2` (one blocker), the ungrouped bucket (one blocker), `g1` (no blockers).
        assert_eq!(report.group_count, 3);
        let names: Vec<Option<&str>> = report.groups.iter()
            .map(|row| row.group.as_deref()).collect();
        // 排序：阻塞降序 → 活跃降序 → 组名升序，未分组（None）在同分时排最前。
        // Ordering: blocking desc, active desc, then name asc, with the ungrouped bucket first on ties.
        assert_eq!(names, vec![None, Some("g2"), Some("g1")]);
        assert!(report.ungrouped_present);
        assert_eq!(report.groups[0].blocking_instances, 1);
        assert_eq!(report.groups[0].involved_instances, 0);
        assert_eq!(report.groups[1].blocking_instances, 1);
        assert_eq!(report.groups[1].involved_instances, 1);
        assert_eq!(report.groups[1].max_improvement, Some(0.5));
        assert_eq!(report.groups[2].involved_instances, 1);
        assert_eq!(report.groups[2].active_instances, 1);
        assert_eq!(report.groups[2].blocking_instances, 0);
        assert_eq!(report.groups[2].max_improvement, Some(0.25));
        assert_eq!(report.total_involved, 2);
        assert_eq!(report.total_active, 1);
        assert_eq!(report.total_blocking, 2);
        assert!(report.ordering_rule.contains("blocking desc"));
    }

    #[test]
    fn maximum_improvement_takes_the_maximum_including_negatives() {
        let ranking_report = ranking(vec![
            entry("c1", Some("g"), Some(0.1)),
            entry("c2", Some("g"), Some(0.9)),
            entry("c3", Some("g"), Some(-0.5)),
        ]);
        let report = aggregate_analysis_groups(&request(None, Some(&ranking_report), None)).unwrap();
        assert_eq!(report.group_count, 1);
        assert_eq!(report.groups[0].max_improvement, Some(0.9));
        assert_eq!(report.sources, vec!["effectiveness".to_owned()]);
        // 全是负改善时也保留（那是"观察到的最大改善"，不是"最大正值"）。
        // All-negative improvements are kept: it is the maximum improvement observed, not the maximum positive one.
        let negative = ranking(vec![entry("c1", Some("g"), Some(-0.1)),
            entry("c2", Some("g"), Some(-0.9))]);
        let report = aggregate_analysis_groups(&request(None, Some(&negative), None)).unwrap();
        assert_eq!(report.groups[0].max_improvement, Some(-0.1));
    }

    #[test]
    fn empty_and_broken_reports_are_handled() {
        // 三份都不给：没有组，也没有来源。 / No report at all: no groups and no sources.
        let report = aggregate_analysis_groups(&AnalysisGroupAggregateRequest {
            activity_json: None, effectiveness_json: None, conflict_json: None }).unwrap();
        assert_eq!(report.group_count, 0);
        assert!(report.groups.is_empty());
        assert!(!report.ungrouped_present);
        assert!(report.sources.is_empty());
        assert_eq!((report.total_involved, report.total_active, report.total_blocking), (0, 0, 0));
        // 空文本 / 非 JSON / 错的报告类型：都拒绝，并指出是哪一份。
        // Empty text, non-JSON and a wrong report shape are refused while naming the document.
        for (mutate, label) in [
            (Box::new(|request: &mut AnalysisGroupAggregateRequest| request.activity_json = Some(String::new()))
                as Box<dyn Fn(&mut AnalysisGroupAggregateRequest)>, "activity"),
            (Box::new(|request: &mut AnalysisGroupAggregateRequest| request.activity_json = Some("not json".to_owned())),
                "activity"),
            (Box::new(|request: &mut AnalysisGroupAggregateRequest| request.conflict_json = Some("{}".to_owned())),
                "conflict"),
        ] {
            let mut document = AnalysisGroupAggregateRequest { activity_json: None,
                effectiveness_json: None, conflict_json: None };
            mutate(&mut document);
            let error = aggregate_analysis_groups(&document).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains(label), "{}: {}", label, error.message);
        }
    }

    #[test]
    fn blockers_without_an_activity_report_land_in_the_ungrouped_bucket() {
        // 只有冲突报告：所有成员都归入未分组桶，而不是凭空出现在某个组里。
        // A conflict report alone: every member lands in the ungrouped bucket rather than appearing in some group.
        let conflict_report = explanation(&["a", "b"]);
        let report = aggregate_analysis_groups(&request(None, None, Some(&conflict_report))).unwrap();
        assert_eq!(report.group_count, 1);
        assert_eq!(report.groups[0].group, None);
        assert_eq!(report.groups[0].blocking_instances, 2);
        assert_eq!(report.groups[0].involved_instances, 0);
        assert_eq!(report.total_blocking, 2);
        assert_eq!(report.sources, vec!["conflict".to_owned()]);
    }
}
