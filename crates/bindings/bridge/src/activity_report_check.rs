//! 续挖第 46 单：活动性报告的 `validate()` 是**聚合严查、数值只看有限性**
//! （`analysis/activity.rs` 的 `ConstraintActivityReport::validate`）
//! Continuation, delivery 46: `ConstraintActivityReport::validate()` is **strict about aggregates and only checks finiteness
//! for numbers** (`ConstraintActivityReport::validate`).
//!
//! 第 44/45 单量的是判定带子与证据谓词；这一单量**报告自洽性校验的边界**。六条实测：
//!
//! 1. **聚合字段被重算比对**（`aggregates_are_recomputed()`）：`summary` 必须等于按 `constraints` 重算的计数，
//!    `variable_bound_summary` 同理，`groups` 必须等于按 `group` 分组的聚合（含 `None` 组）。
//!    改一位计数就报 "summary does not match its records"。
//! 2. **`groups` 的条数也被查**（`duplicate_groups_are_refused()`）：`actual_groups.len() != self.groups.len()`
//!    专门用来抓"同一个组被列了两次"。
//! 3. **数值字段只查有限性、不查彼此关系**（`numbers_only_check_finiteness()`）：
//!    `slack` / `normalized_slack` / `tolerance` 各自 `is_finite` 通过就算过——`normalized_slack` 可以
//!    与 `slack / |rhs|` 完全不符，`tolerance` 可以与记录外的配置无关。
//! 4. **`status` 不与 `slack`/`tolerance` 交叉校验**（`status_is_not_cross_checked()`）：
//!    声明 `Inactive` 而 slack 为 0、或声明 `Active` 而 slack 巨大，都能通过。
//! 5. **`satisfied` 也不与 slack 交叉校验**（`satisfied_is_not_cross_checked()`）：
//!    `satisfied: true` 配一个很负的 slack 照样通过。
//! 6. **重复的 `constraint_id` / 空白 ID / 空白 group / 空白 `schema_version` 被拒**
//!    （`identities_are_checked()`）。
//!
//! Delivery 44 and 45 measured the classification bands and the evidence predicates; this one measures **how far the report's
//! self-consistency check reaches**. **Aggregate fields are recomputed and compared** (`aggregates_are_recomputed()`):
//! `summary` must equal the counts recomputed from `constraints`, `variable_bound_summary` likewise, and `groups` must equal
//! the per-group aggregation (including the `None` group) — changing one count reports "summary does not match its records".
//! **The number of groups is checked too** (`duplicate_groups_are_refused()`): `actual_groups.len() != self.groups.len()`
//! exists precisely to catch a group listed twice. **Numeric fields are only checked for finiteness, never against each other**
//! (`numbers_only_check_finiteness()`): `slack`, `normalized_slack` and `tolerance` each pass on `is_finite` alone, so
//! `normalized_slack` may disagree wildly with `slack / |rhs|` and `tolerance` need not relate to any configuration.
//! **`status` is not cross-checked against `slack`/`tolerance`** (`status_is_not_cross_checked()`): claiming `Inactive` with a
//! zero slack, or `Active` with a huge one, both pass. **`satisfied` is not cross-checked against the slack either**
//! (`satisfied_is_not_cross_checked()`): `satisfied: true` beside a very negative slack passes. And **duplicate
//! `constraint_id`s, blank IDs, blank groups and a blank `schema_version` are refused** (`identities_are_checked()`).

use ospf_rust_core::analysis::activity::{
    ActivityEvidence, ActivityGroupSummary, ActivityStatus, ActivitySummary, ConstraintActivity,
    ConstraintActivityReport, VariableBoundActivity,
};
use ospf_rust_core::analysis::BoundSide;
use ospf_rust_core::solver::report::{StableConstraintId, StableVariableId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条约束记录的字面量。 / One constraint record literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintRecordSpec {
    /// 约束 ID。 / The constraint ID.
    pub id: String,
    /// 组名。 / The group name.
    #[serde(default)]
    pub group: Option<String>,
    /// 状态：`ACTIVE` / `NEARLY_ACTIVE` / `INACTIVE` / `SATISFIED_WITHOUT_SLACK_METRIC` / `VIOLATED` / `UNKNOWN`。
    /// The status.
    pub status: String,
    /// signed slack。 / The signed slack.
    #[serde(default)]
    pub slack: Option<f64>,
    /// 归一化 slack。 / The normalized slack.
    #[serde(default)]
    pub normalized_slack: Option<f64>,
    /// 使用的容差。 / The tolerance used.
    #[serde(default)]
    pub tolerance: Option<f64>,
    /// 语义满足性。 / The semantic satisfaction.
    #[serde(default)]
    pub satisfied: Option<bool>,
}

/// 一条变量边界记录的字面量。 / One variable-bound record literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundRecordSpec {
    /// 变量 ID。 / The variable ID.
    pub variable: String,
    /// `lower` / `upper`。 / `lower` or `upper`.
    pub side: String,
    pub value: f64,
    pub bound: f64,
    pub slack: f64,
    pub normalized_slack: f64,
    pub tolerance: f64,
    /// 是否满足。 / Whether it is satisfied.
    pub satisfied: bool,
    /// 状态名。 / The status name.
    pub status: String,
}

/// 一个组汇总的字面量。 / One group summary literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupSummarySpec {
    /// 组名；`null` 表示未分组。 / The group name; `null` means ungrouped.
    #[serde(default)]
    pub group: Option<String>,
    /// 组内计数（按状态名给，缺省为 0）。 / The counts by status name (zero when absent).
    #[serde(default)]
    pub counts: Vec<StatusCountSpec>,
}

/// 一个状态的计数。 / One status count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusCountSpec {
    /// 状态名。 / The status name.
    pub status: String,
    /// 计数。 / The count.
    pub count: u64,
}

/// 报告请求。 / The report request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityReportRequest {
    /// `schemaVersion`。 / The `schemaVersion`.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 配置的 active 容差。 / The configured active tolerance.
    #[serde(default)]
    pub tolerance: Option<f64>,
    /// 配置的 nearly 容差。 / The configured nearly tolerance.
    #[serde(default)]
    pub nearly_active_tolerance: Option<f64>,
    /// 约束记录。 / The constraint records.
    #[serde(default)]
    pub constraints: Vec<ConstraintRecordSpec>,
    /// 变量边界记录。 / The variable-bound records.
    #[serde(default)]
    pub variable_bounds: Vec<BoundRecordSpec>,
    /// `summary`（缺省则按记录重算）。 / `summary` (recomputed from the records when absent).
    #[serde(default)]
    pub summary: Option<Vec<StatusCountSpec>>,
    /// `variableBoundSummary`（缺省则按记录重算）。 / `variableBoundSummary` (recomputed when absent).
    #[serde(default)]
    pub variable_bound_summary: Option<Vec<StatusCountSpec>>,
    /// `groups`（缺省则按记录重算）。 / `groups` (recomputed from the records when absent).
    #[serde(default)]
    pub groups: Option<Vec<GroupSummarySpec>>,
}

/// 报告自洽性结论。 / The report self-consistency verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityReportVerdict {
    /// `validate()` 是否通过。 / Whether `validate()` passed.
    pub accepted: bool,
    /// 失败消息。 / The failure message.
    #[serde(default)]
    pub error: Option<String>,
    /// 聚合字段被重算比对（**是**）。 / Whether the aggregates are recomputed (**yes**).
    pub aggregates_are_recomputed: bool,
    /// 重复的组被拒（**是**）。 / Whether duplicate groups are refused (**yes**).
    pub duplicate_groups_are_refused: bool,
    /// 数值只查有限性（**是**）。 / Whether numbers are only checked for finiteness (**yes**).
    pub numbers_only_check_finiteness: bool,
    /// `status` 不交叉校验（**是**）。 / Whether `status` is not cross-checked (**yes**).
    pub status_is_not_cross_checked: bool,
    /// `satisfied` 不交叉校验（**是**）。 / Whether `satisfied` is not cross-checked (**yes**).
    pub satisfied_is_not_cross_checked: bool,
    /// 身份被校验（**是**）。 / Whether identities are checked (**yes**).
    pub identities_are_checked: bool,
}

/// 记录条数上限（信任边界）。 / The record cap (a trust boundary).
const MAX_RECORDS: usize = 128;

/// 状态名转枚举。 / Turn a status name into the enum.
fn status(name: &str) -> Result<ActivityStatus> {
    match name {
        "ACTIVE" => Ok(ActivityStatus::Active),
        "NEARLY_ACTIVE" => Ok(ActivityStatus::NearlyActive),
        "INACTIVE" => Ok(ActivityStatus::Inactive),
        "SATISFIED_WITHOUT_SLACK_METRIC" => Ok(ActivityStatus::SatisfiedWithoutSlackMetric),
        "VIOLATED" => Ok(ActivityStatus::Violated),
        "UNKNOWN" => Ok(ActivityStatus::Unknown),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "activity report failed validation: unknown status `{other}`"))),
    }
}

/// 按状态名列表造计数。 / Build counts from named entries.
fn summary_of(counts: &[StatusCountSpec]) -> Result<ActivitySummary> {
    let mut summary = ActivitySummary::default();
    for entry in counts {
        let status = status(&entry.status)?;
        for _ in 0..entry.count {
            summary.add(status);
        }
    }
    Ok(summary)
}

/// 按记录重算约束计数。 / Recompute the constraint counts.
fn recompute(constraints: &[ConstraintActivity]) -> ActivitySummary {
    let mut summary = ActivitySummary::default();
    for activity in constraints {
        summary.add(activity.status);
    }
    summary
}

/// 按记录重算边界计数。 / Recompute the bound counts.
fn recompute_bounds(bounds: &[VariableBoundActivity]) -> ActivitySummary {
    let mut summary = ActivitySummary::default();
    for activity in bounds {
        summary.add(activity.status);
    }
    summary
}

/// 走一遍报告自洽性校验。非法输入是**错误**。 / Walk the report self-consistency check.
pub fn verify_activity_report(request: &ActivityReportRequest) -> Result<ActivityReportVerdict> {
    if request.constraints.len() > MAX_RECORDS || request.variable_bounds.len() > MAX_RECORDS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "activity report failed validation: at most {MAX_RECORDS} records are accepted")));
    }
    let constraints: Vec<ConstraintActivity> = request.constraints.iter().map(|record| {
        Ok(ConstraintActivity { constraint_id: StableConstraintId(record.id.clone()),
            group: record.group.clone(), status: status(&record.status)?, lhs: None, rhs: None,
            slack: record.slack, normalized_slack: record.normalized_slack,
            tolerance: record.tolerance, satisfied: record.satisfied,
            evidence: ActivityEvidence::LinearConstraint { lhs: record.slack.unwrap_or(0.0),
                rhs: 0.0 } })
    }).collect::<Result<Vec<_>>>()?;
    let bounds: Vec<VariableBoundActivity> = request.variable_bounds.iter().map(|record| {
        Ok(VariableBoundActivity { variable_id: StableVariableId(record.variable.clone()),
            side: match record.side.as_str() {
                "lower" => BoundSide::Lower,
                "upper" => BoundSide::Upper,
                other => return Err(BridgeError::new("InvalidInput", &format!(
                    "activity report failed validation: unknown bound side `{other}`"))),
            },
            value: record.value, bound: record.bound, slack: record.slack,
            normalized_slack: record.normalized_slack, tolerance: record.tolerance,
            satisfied: record.satisfied, status: status(&record.status)?,
            evidence: ActivityEvidence::VariableBound { variable_id: StableVariableId(
                record.variable.clone()), side: BoundSide::Lower, value: record.value,
                bound: record.bound } })
    }).collect::<Result<Vec<_>>>()?;
    let summary = match &request.summary {
        Some(counts) => summary_of(counts)?,
        None => recompute(&constraints),
    };
    let variable_bound_summary = match &request.variable_bound_summary {
        Some(counts) => summary_of(counts)?,
        None => recompute_bounds(&bounds),
    };
    let groups: Vec<ActivityGroupSummary> = match &request.groups {
        Some(specs) => specs.iter().map(|spec| Ok(ActivityGroupSummary {
            group: spec.group.clone(), summary: summary_of(&spec.counts)? }))
            .collect::<Result<Vec<_>>>()?,
        None => {
            let mut by_group: std::collections::BTreeMap<Option<String>, ActivitySummary> =
                std::collections::BTreeMap::new();
            for activity in &constraints {
                by_group.entry(activity.group.clone()).or_default().add(activity.status);
            }
            by_group.into_iter().map(|(group, summary)| ActivityGroupSummary { group, summary })
                .collect()
        }
    };
    let report = ConstraintActivityReport {
        schema_version: request.schema_version.clone().unwrap_or_else(|| "activity/v1".to_owned()),
        constraints, variable_bounds: bounds, summary, variable_bound_summary, groups,
        tolerance: request.tolerance.unwrap_or(1e-7),
        nearly_active_tolerance: request.nearly_active_tolerance.unwrap_or(1e-5),
    };
    let outcome = report.validate();
    let (accepted, error) = match &outcome {
        Ok(()) => (true, None),
        Err(failure) => (false, Some(failure.to_string())),
    };
    let findings = canonical_findings();
    Ok(ActivityReportVerdict { accepted, error,
        aggregates_are_recomputed: findings.aggregates_are_recomputed,
        duplicate_groups_are_refused: findings.duplicate_groups_are_refused,
        numbers_only_check_finiteness: findings.numbers_only_check_finiteness,
        status_is_not_cross_checked: findings.status_is_not_cross_checked,
        satisfied_is_not_cross_checked: findings.satisfied_is_not_cross_checked,
        identities_are_checked: findings.identities_are_checked })
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    aggregates_are_recomputed: bool,
    duplicate_groups_are_refused: bool,
    numbers_only_check_finiteness: bool,
    status_is_not_cross_checked: bool,
    satisfied_is_not_cross_checked: bool,
    identities_are_checked: bool,
}

/// 造一条约束记录。 / Build one constraint record.
fn record(id: &str, group: Option<&str>, status: ActivityStatus) -> ConstraintActivity {
    ConstraintActivity { constraint_id: StableConstraintId(id.to_owned()),
        group: group.map(|group| group.to_owned()), status, lhs: None, rhs: None,
        slack: Some(-1.0), normalized_slack: Some(-99.0), tolerance: Some(1e-9),
        satisfied: Some(true),
        evidence: ActivityEvidence::LinearConstraint { lhs: -1.0, rhs: 0.0 } }
}

/// 造一个只含一条记录的报告。 / Build a report holding one record.
fn report_of(constraints: Vec<ConstraintActivity>) -> ConstraintActivityReport {
    let summary = recompute(&constraints);
    let mut by_group: std::collections::BTreeMap<Option<String>, ActivitySummary> =
        std::collections::BTreeMap::new();
    for activity in &constraints {
        by_group.entry(activity.group.clone()).or_default().add(activity.status);
    }
    ConstraintActivityReport {
        schema_version: "activity/v1".to_owned(),
        constraints,
        variable_bounds: Vec::new(),
        summary,
        variable_bound_summary: ActivitySummary::default(),
        groups: by_group.into_iter().map(|(group, summary)| ActivityGroupSummary { group, summary })
            .collect(),
        tolerance: 1e-7, nearly_active_tolerance: 1e-5,
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let base = report_of(vec![record("c1", Some("g"), ActivityStatus::Inactive),
        record("c2", None, ActivityStatus::Active)]);
    // 1) 聚合被重算比对：改一位计数就失败。 / Aggregates are recomputed: one wrong count fails.
    let mut wrong_summary = base.clone();
    wrong_summary.summary.inactive += 1;
    let aggregates_are_recomputed = base.validate().is_ok()
        && wrong_summary.validate().is_err()
        && {
            let mut wrong_groups = base.clone();
            if let Some(first) = wrong_groups.groups.first_mut() { first.summary.active += 1; }
            wrong_groups.validate().is_err()
        };
    // 2) 重复的组被拒。 / A duplicated group is refused.
    let mut duplicated = base.clone();
    duplicated.groups.push(duplicated.groups[0].clone());
    let duplicate_groups_are_refused = base.validate().is_ok() && duplicated.validate().is_err();
    // 3) 数值只查有限性：`normalized_slack` 与 slack 完全不符也过。 
    // 3) Numbers only check finiteness: a wildly inconsistent `normalized_slack` passes.
    let numbers_only_check_finiteness = {
        let mut inconsistent = base.clone();
        for activity in &mut inconsistent.constraints {
            activity.slack = Some(5.0);
            activity.normalized_slack = Some(-1234.5);
            activity.tolerance = Some(0.0);
        }
        inconsistent.validate().is_ok()
    };
    // 4) `status` 不交叉校验。 / `status` is not cross-checked.
    let status_is_not_cross_checked = {
        let mut mismatched = base.clone();
        for activity in &mut mismatched.constraints {
            activity.status = ActivityStatus::Inactive;
            activity.slack = Some(0.0);
        }
        let summary = recompute(&mismatched.constraints);
        mismatched.summary = summary;
        mismatched.groups.clear();
        let mut by_group: std::collections::BTreeMap<Option<String>, ActivitySummary> =
            std::collections::BTreeMap::new();
        for activity in &mismatched.constraints {
            by_group.entry(activity.group.clone()).or_default().add(activity.status);
        }
        mismatched.groups = by_group.into_iter()
            .map(|(group, summary)| ActivityGroupSummary { group, summary }).collect();
        mismatched.validate().is_ok()
    };
    // 5) `satisfied` 不交叉校验。 / `satisfied` is not cross-checked.
    let satisfied_is_not_cross_checked = {
        let mut inconsistent = base.clone();
        for activity in &mut inconsistent.constraints {
            activity.slack = Some(-1e9);
            activity.satisfied = Some(true);
        }
        inconsistent.validate().is_ok()
    };
    // 6) 身份被校验：重复 ID / 空白 ID / 空白组 / 空白 schema 都被拒。 
    // 6) Identities are checked: duplicate IDs, blank IDs, blank groups and a blank schema all fail.
    let identities_are_checked = {
        let duplicated = report_of(vec![record("c1", None, ActivityStatus::Active),
            record("c1", None, ActivityStatus::Active)]);
        let blank_id = report_of(vec![record("  ", None, ActivityStatus::Active)]);
        let blank_group = report_of(vec![record("c1", Some("   "), ActivityStatus::Active)]);
        let mut blank_schema = report_of(vec![record("c1", None, ActivityStatus::Active)]);
        blank_schema.schema_version = String::new();
        duplicated.validate().is_err() && blank_id.validate().is_err()
            && blank_group.validate().is_err() && blank_schema.validate().is_err()
    };
    Findings { aggregates_are_recomputed, duplicate_groups_are_refused,
        numbers_only_check_finiteness, status_is_not_cross_checked,
        satisfied_is_not_cross_checked, identities_are_checked }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ActivityReportRequest {
        ActivityReportRequest { schema_version: Some("activity/v1".to_owned()),
            tolerance: Some(1e-7), nearly_active_tolerance: Some(1e-5),
            constraints: vec![
                ConstraintRecordSpec { id: "c1".to_owned(), group: Some("g".to_owned()),
                    status: "INACTIVE".to_owned(), slack: Some(1.0),
                    normalized_slack: Some(1.0), tolerance: Some(1e-7), satisfied: Some(true) },
                ConstraintRecordSpec { id: "c2".to_owned(), group: None,
                    status: "ACTIVE".to_owned(), slack: Some(0.0), normalized_slack: Some(0.0),
                    tolerance: Some(1e-7), satisfied: Some(true) },
            ],
            variable_bounds: Vec::new(), summary: None, variable_bound_summary: None,
            groups: None }
    }

    #[test]
    fn a_consistent_report_passes() {
        let verdict = verify_activity_report(&request()).unwrap();
        assert!(verdict.accepted, "{:?}", verdict.error);
        assert!(verdict.aggregates_are_recomputed);
        assert!(verdict.duplicate_groups_are_refused);
        assert!(verdict.numbers_only_check_finiteness);
        assert!(verdict.status_is_not_cross_checked);
        assert!(verdict.satisfied_is_not_cross_checked);
        assert!(verdict.identities_are_checked);
    }

    #[test]
    fn a_wrong_summary_is_refused() {
        let mut document = request();
        document.summary = Some(vec![StatusCountSpec { status: "INACTIVE".to_owned(),
            count: 2 }]);
        let verdict = verify_activity_report(&document).unwrap();
        assert!(!verdict.accepted);
        assert!(verdict.error.as_deref().unwrap_or_default()
            .contains("summary does not match its records"));
    }

    #[test]
    fn a_wrong_group_summary_is_refused() {
        let mut document = request();
        document.groups = Some(vec![GroupSummarySpec { group: Some("g".to_owned()),
            counts: vec![StatusCountSpec { status: "ACTIVE".to_owned(), count: 1 }] }]);
        let verdict = verify_activity_report(&document).unwrap();
        assert!(!verdict.accepted);
        assert!(verdict.error.as_deref().unwrap_or_default()
            .contains("group summary does not match its records"));
    }

    #[test]
    fn inconsistent_numbers_still_pass() {
        let mut document = request();
        document.constraints[0].slack = Some(5.0);
        document.constraints[0].normalized_slack = Some(-1234.5);
        document.constraints[0].tolerance = Some(0.0);
        // `status` 与 `satisfied` 也不一致。 / `status` and `satisfied` disagree as well.
        document.constraints[0].status = "INACTIVE".to_owned();
        document.constraints[0].satisfied = Some(false);
        let verdict = verify_activity_report(&document).unwrap();
        assert!(verdict.accepted, "{:?}", verdict.error);
    }

    #[test]
    fn identities_are_checked() {
        let mut duplicated = request();
        duplicated.constraints[1].id = "c1".to_owned();
        assert!(verify_activity_report(&duplicated).unwrap().error.as_deref()
            .unwrap_or_default().contains("duplicate constraint IDs"));
        let mut blank_id = request();
        blank_id.constraints[0].id = "  ".to_owned();
        assert!(verify_activity_report(&blank_id).unwrap().error.as_deref()
            .unwrap_or_default().contains("constraint ID must not be blank"));
        let mut blank_group = request();
        blank_group.constraints[0].group = Some("   ".to_owned());
        assert!(verify_activity_report(&blank_group).unwrap().error.as_deref()
            .unwrap_or_default().contains("group must not be blank"));
        let mut blank_schema = request();
        blank_schema.schema_version = Some("  ".to_owned());
        assert!(!verify_activity_report(&blank_schema).unwrap().accepted);
    }

    #[test]
    fn variable_bounds_are_counted_separately() {
        let mut document = request();
        document.variable_bounds = vec![BoundRecordSpec { variable: "x".to_owned(),
            side: "upper".to_owned(), value: 3.0, bound: 5.0, slack: 2.0,
            normalized_slack: 0.4, tolerance: 1e-7, satisfied: true,
            status: "INACTIVE".to_owned() }];
        // 边界汇总缺省时按记录重算 → 通过。 / The bound summary is recomputed when absent → it passes.
        assert!(verify_activity_report(&document).unwrap().accepted);
        // 给一个错的边界汇总 → 拒绝。 / A wrong bound summary is refused.
        document.variable_bound_summary = Some(vec![StatusCountSpec { status: "ACTIVE".to_owned(),
            count: 1 }]);
        let verdict = verify_activity_report(&document).unwrap();
        assert!(!verdict.accepted);
        assert!(verdict.error.as_deref().unwrap_or_default()
            .contains("summary does not match its records"));
    }

    #[test]
    fn bad_input_is_refused() {
        let mut unknown = request();
        unknown.constraints[0].status = "active".to_owned();
        assert!(verify_activity_report(&unknown).unwrap_err().message
            .contains("unknown status"));
        let mut side = request();
        side.variable_bounds = vec![BoundRecordSpec { variable: "x".to_owned(),
            side: "middle".to_owned(), value: 0.0, bound: 0.0, slack: 0.0,
            normalized_slack: 0.0, tolerance: 0.0, satisfied: true,
            status: "ACTIVE".to_owned() }];
        assert!(verify_activity_report(&side).unwrap_err().message
            .contains("unknown bound side"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let verdict = verify_activity_report(&request()).unwrap();
        let json = serde_json::to_string(&verdict).unwrap();
        assert!(json.contains("\"aggregatesAreRecomputed\""), "{json}");
        assert!(json.contains("\"numbersOnlyCheckFiniteness\""), "{json}");
        assert!(json.contains("\"statusIsNotCrossChecked\""), "{json}");
        assert!(!json.contains("aggregates_are"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ActivityReportVerdict>(&copy.to_string()).is_err());
    }
}
