//! persistence 的两块可移植编辑语义：字段赋值集合与排序空值支持判定。
//! Two portable editing semantics from `persistence`: assignment sets and sort null-ordering support.
//!
//! 这两块都不是数据库能力，而是**发送给后端之前**的纯数据判定：
//! - 赋值集合：领域实现允许同一字段被赋值多次（`then` 直接追加，语义是"后者覆盖"）。桥接如实标出重复字段，
//!   而不是替调用方挑一个赢家——静默选一个正是最难查的那类 bug。
//! - 排序空值支持：后端能不能表达 `NULLS FIRST/LAST` 是纯谓词，提前判定就能避免一条注定失败的查询。
//! Neither is a database capability; both are pure data decisions taken **before** anything is sent to a backend:
//! - Assignment sets: the domain allows the same field to be assigned more than once (`then` simply appends, meaning
//!   "last one wins"). The bridge reports duplicated fields instead of picking a winner for the caller — silently
//!   choosing one is exactly the kind of bug that is hardest to find.
//! - Sort null ordering: whether a backend can express `NULLS FIRST/LAST` is a pure predicate, so deciding early
//!   avoids issuing a query that is bound to fail.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework::persistence::expression::sort_by::{
    NullsOrder as DomainNullsOrder, NullsOrderSupport, SortDirection as DomainSortDirection, SortItem,
};
use ospf_rust_math::symbol::{ExpressionValue, PropertyPath};

use crate::relational_query::{QueryNullsOrder, QueryOrderSpec, QuerySortDirection};
use crate::{BridgeError, Result};

/// 一条赋值。 / One assignment.
///
/// `kind: "expression"` 需要把标量表达式树当数据传输，超出当前可数据化子集，因此**明确拒绝**而不是近似。
/// `kind: "expression"` would require transporting a scalar expression tree, which is outside the current
/// data-expressible subset, so it is refused explicitly instead of approximated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentSpec {
    /// 目标字段路径。 / The target field path.
    pub path: String,
    /// 赋值种类。 / The assignment kind.
    pub kind: AssignmentKind,
    /// `kind: "value"` 时的值；其它种类必须省略。 / The value for `kind: "value"`; omitted for other kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<AssignmentValue>,
}

/// 赋值种类。 / Assignment kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AssignmentKind {
    /// 设置为常量。 / Set to a constant.
    Value,
    /// 设置为空值。 / Set to null.
    Null,
}

/// 可赋的常量值。 / A constant that can be assigned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
pub enum AssignmentValue {
    /// 布尔值。 / Boolean.
    Boolean { value: bool },
    /// 数字值。 / Number.
    Number { value: f64 },
    /// 字符串值。 / String.
    String { value: String },
}

impl AssignmentValue {
    fn to_domain(&self) -> Result<ExpressionValue> {
        Ok(match self {
            Self::Boolean { value } => ExpressionValue::Boolean(*value),
            Self::Number { value } => {
                if !value.is_finite() {
                    return Err(BridgeError::new("InvalidInput", "assignment numbers must be finite"));
                }
                ExpressionValue::Number(*value)
            }
            Self::String { value } => ExpressionValue::String(value.clone()),
        })
    }
}

/// 一条规范化的赋值。 / One normalised assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentEntry {
    /// 目标字段路径。 / The target field path.
    pub path: String,
    /// 赋值种类。 / The assignment kind.
    pub kind: AssignmentKind,
    /// `kind: "value"` 时的值。 / The value for `kind: "value"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<AssignmentValue>,
}

/// 赋值集合判定报告。 / The assignment-set verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentReport {
    /// 规范化后的赋值序列（保持调用方给出的顺序）。 / The normalised assignment sequence in caller order.
    pub entries: Vec<AssignmentEntry>,
    /// 是否为空集合。 / Whether the set is empty.
    pub is_empty: bool,
    /// 被重复赋值的字段路径，按稳定升序；空表示没有歧义。
    /// Field paths assigned more than once, stably ascending; empty means no ambiguity.
    pub duplicate_paths: Vec<String>,
}

/// 校验并规范化一个赋值集合。 / Validate and normalise one assignment set.
pub fn validate_assignments(assignments: &[AssignmentSpec]) -> Result<AssignmentReport> {
    let mut entries = Vec::with_capacity(assignments.len());
    let mut seen: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    for assignment in assignments {
        if assignment.path.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "assignment paths must not be blank"));
        }
        // 路径必须能被解析成属性路径，否则后端拿到的是一个永远匹配不上的字段。
        // The path must parse as a property path; otherwise the backend receives a field that can never match.
        let _: PropertyPath = PropertyPath::from(assignment.path.as_str());
        let value = match assignment.kind {
            AssignmentKind::Value => Some(assignment.value.as_ref()
                .ok_or_else(|| BridgeError::new("InvalidInput", "a value assignment requires a value"))?
                .to_domain()?),
            AssignmentKind::Null => {
                if assignment.value.is_some() {
                    return Err(BridgeError::new("InvalidInput", "a null assignment must not carry a value"));
                }
                None
            }
        };
        // 真正走一遍领域构造器，确认这份序列在领域侧是合法的。
        // Actually build the domain collection so the sequence is confirmed legal on the domain side.
        let domain = match (&value, assignment.kind) {
            (Some(value), AssignmentKind::Value) => {
                ospf_rust_framework::persistence::expression::update_assignment::UpdateAssignments::<
                    ExpressionValue>::set(PropertyPath::from(assignment.path.as_str()), value.clone())
            }
            _ => ospf_rust_framework::persistence::expression::update_assignment::UpdateAssignments::<
                ExpressionValue>::set_null(PropertyPath::from(assignment.path.as_str())),
        };
        if domain.is_empty() {
            return Err(BridgeError::new("InvalidReport", "domain produced an empty assignment set"));
        }
        *seen.entry(assignment.path.as_str()).or_insert(0) += 1;
        entries.push(AssignmentEntry { path: assignment.path.clone(), kind: assignment.kind,
            value: assignment.value.clone() });
    }
    let duplicate_paths = seen.into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(path, _)| path.to_owned())
        .collect();
    Ok(AssignmentReport { is_empty: entries.is_empty(), entries, duplicate_paths })
}

/// 后端的空值排序支持级别。 / The backend's null-ordering support level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NullsSupportLevel {
    /// 由后端决定。 / Decided by the backend.
    Auto,
    /// 始终支持。 / Always supported.
    Always,
    /// 从不支持。 / Never supported.
    Never,
    /// 仅升序支持。 / Supported only for ascending order.
    OnlyAsc,
}

impl NullsSupportLevel {
    fn to_domain(self) -> NullsOrderSupport {
        match self {
            Self::Auto => NullsOrderSupport::Auto,
            Self::Always => NullsOrderSupport::Always,
            Self::Never => NullsOrderSupport::Never,
            Self::OnlyAsc => NullsOrderSupport::OnlyAsc,
        }
    }
}

/// 一个排序项的空值排序判定。 / The null-ordering verdict for one sort item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SortItemVerdict {
    /// 字段路径原文。 / The raw field path.
    pub path: String,
    /// 是否声明了空值排序。 / Whether a null ordering was declared.
    pub declares_nulls_order: bool,
    /// 后端是否支持该排序项的空值排序语法。 / Whether the backend supports null ordering for this item.
    pub nulls_supported: bool,
}

/// 排序计划判定报告。 / The sort-plan verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SortPlanReport {
    /// 每个排序项的判定，保持输入顺序。 / The verdict per item, in input order.
    pub items: Vec<SortItemVerdict>,
    /// 会被后端拒绝的下标（声明了空值排序但后端不支持），升序。
    /// Indices the backend would reject (null ordering declared but unsupported), ascending.
    pub rejected_indices: Vec<u32>,
    /// 整份计划能否被该后端接受。 / Whether the whole plan is acceptable to that backend.
    pub accepted: bool,
}

/// 一份排序计划：后端支持级别 + 排序项。 / A sort plan: the backend's support level plus the sort items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SortPlanSpec {
    /// 后端的空值排序支持级别。 / The backend's null-ordering support level.
    pub support: NullsSupportLevel,
    /// 排序项，按执行顺序。 / The sort items in execution order.
    pub items: Vec<QueryOrderSpec>,
}

/// 判定一份排序计划能否被指定后端接受。 / Decide whether a sort plan is acceptable to the given backend.
pub fn validate_sort_plan(level: NullsSupportLevel, items: &[QueryOrderSpec]) -> Result<SortPlanReport> {
    let support = level.to_domain();
    let mut verdicts = Vec::with_capacity(items.len());
    let mut rejected = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if item.column.source.trim().is_empty() || item.column.path.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "sort columns require a source and a path"));
        }
        let nulls = item.nulls.and_then(|nulls| match nulls {
            QueryNullsOrder::Unspecified => None,
            QueryNullsOrder::First => Some(DomainNullsOrder::NullsFirst),
            QueryNullsOrder::Last => Some(DomainNullsOrder::NullsLast),
        });
        let declares_nulls_order = nulls.is_some();
        let domain = SortItem::new(
            PropertyPath::from(format!("{}.{}", item.column.source, item.column.path).as_str()),
            match item.direction {
                QuerySortDirection::Ascending => DomainSortDirection::Asc,
                QuerySortDirection::Descending => DomainSortDirection::Desc,
            }, nulls);
        let nulls_supported = support.is_supported(&domain);
        // 只有真的声明了空值排序时，"不支持"才会让后端拒绝这条查询。
        // Only when a null ordering is actually declared does "unsupported" make the backend reject the query.
        if declares_nulls_order && !nulls_supported {
            rejected.push(u32::try_from(index).unwrap_or(u32::MAX));
        }
        verdicts.push(SortItemVerdict {
            path: format!("{}.{}", item.column.source, item.column.path),
            declares_nulls_order,
            nulls_supported,
        });
    }
    Ok(SortPlanReport { accepted: rejected.is_empty(), items: verdicts, rejected_indices: rejected })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relational_query::QueryColumn;

    fn value(path: &str, number: f64) -> AssignmentSpec {
        AssignmentSpec { path: path.into(), kind: AssignmentKind::Value,
            value: Some(AssignmentValue::Number { value: number }) }
    }

    fn order(path: &str, direction: QuerySortDirection, nulls: Option<QueryNullsOrder>) -> QueryOrderSpec {
        QueryOrderSpec { column: QueryColumn { source: "orders".into(), path: path.into() }, direction, nulls }
    }

    #[test]
    fn assignments_preserve_order_and_report_duplicates_instead_of_picking_a_winner() {
        let report = validate_assignments(&[value("orders.status", 1.0),
            AssignmentSpec { path: "orders.note".into(), kind: AssignmentKind::Null, value: None },
            value("orders.status", 2.0)]).unwrap();
        assert!(!report.is_empty);
        assert_eq!(report.entries.len(), 3);
        // 顺序原样保留：领域语义是"后者覆盖"。 / Order is preserved: the domain semantics are "last one wins".
        assert_eq!(report.entries[0].path, "orders.status");
        assert_eq!(report.entries[2].path, "orders.status");
        // 重复字段被标出来，但不替调用方选一个。 / Duplicates are reported without picking one for the caller.
        assert_eq!(report.duplicate_paths, vec!["orders.status".to_owned()]);
        // 空集合是合法的，但不是"有赋值"。 / An empty set is legal but is not "has assignments".
        let empty = validate_assignments(&[]).unwrap();
        assert!(empty.is_empty);
        assert!(empty.duplicate_paths.is_empty());
        // 非法形态必须被拒。 / Malformed shapes must be refused.
        let broken: Vec<(&str, Vec<AssignmentSpec>)> = vec![
            ("blank path", vec![AssignmentSpec { path: " ".into(), kind: AssignmentKind::Null, value: None }]),
            ("value without value", vec![AssignmentSpec { path: "a".into(), kind: AssignmentKind::Value, value: None }]),
            ("null with value", vec![AssignmentSpec { path: "a".into(), kind: AssignmentKind::Null,
                value: Some(AssignmentValue::Number { value: 1.0 }) }]),
            ("non-finite number", vec![AssignmentSpec { path: "a".into(), kind: AssignmentKind::Value,
                value: Some(AssignmentValue::Number { value: f64::NAN }) }]),
        ];
        for (label, case) in broken {
            assert!(validate_assignments(&case).is_err(), "{label} must be refused");
        }
    }

    #[test]
    fn sort_plan_reports_only_declared_but_unsupported_null_orderings() {
        // 后端从不支持空值排序：只有声明了的那一项被拒。 / A backend that never supports null ordering rejects only
        // the item that declared it.
        let report = validate_sort_plan(NullsSupportLevel::Never, &[
            order("id", QuerySortDirection::Ascending, None),
            order("amount", QuerySortDirection::Descending, Some(QueryNullsOrder::Last)),
        ]).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.rejected_indices, vec![1]);
        assert!(!report.items[0].declares_nulls_order);
        // `nulls_supported` 回答的是"这个后端能不能表达该排序项的空值排序"，与"有没有声明"是两件事：
        // 在 `Never` 下它恒为 false，但没声明空值排序的项不会因此被拒。
        // `nulls_supported` answers "can this backend express null ordering for this item", which is separate from
        // "was one declared": under `Never` it is always false, yet an item without a declared null ordering is not
        // rejected because of it.
        assert!(!report.items[0].nulls_supported);
        assert!(report.items[1].declares_nulls_order);
        assert!(!report.items[1].nulls_supported);
        // 仅升序支持：降序声明空值排序被拒，升序放行。 / Asc-only support rejects descending, allows ascending.
        let report = validate_sort_plan(NullsSupportLevel::OnlyAsc, &[
            order("id", QuerySortDirection::Ascending, Some(QueryNullsOrder::First)),
            order("amount", QuerySortDirection::Descending, Some(QueryNullsOrder::First)),
        ]).unwrap();
        assert_eq!(report.rejected_indices, vec![1]);
        assert!(report.items[0].nulls_supported);
        // 始终支持 / 自动：都放行。 / Always and auto accept everything.
        for level in [NullsSupportLevel::Always, NullsSupportLevel::Auto] {
            let report = validate_sort_plan(level, &[
                order("id", QuerySortDirection::Descending, Some(QueryNullsOrder::Last))]).unwrap();
            assert!(report.accepted, "{level:?}");
            assert!(report.rejected_indices.is_empty());
        }
        // 未指定空值排序不算声明，因此 Never 也不会拒。 / Unspecified nulls is not a declaration, so Never allows it.
        let report = validate_sort_plan(NullsSupportLevel::Never, &[
            order("id", QuerySortDirection::Ascending, Some(QueryNullsOrder::Unspecified))]).unwrap();
        assert!(report.accepted);
        // 空列引用被拒。 / Blank column references are refused.
        assert!(validate_sort_plan(NullsSupportLevel::Always, &[QueryOrderSpec {
            column: QueryColumn { source: " ".into(), path: "id".into() },
            direction: QuerySortDirection::Ascending, nulls: None }]).is_err());
    }
}
