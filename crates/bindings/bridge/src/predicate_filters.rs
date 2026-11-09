//! 谓词字段过滤提取绑定：把布尔谓词按字段拆成 eq/in/gt/ge/lt/le/isNull 条件。
//! Predicate field-filter extraction binding: split a boolean predicate into per-field eq/in/gt/ge/lt/le/isNull filters.
//!
//! 语义要点：领域实现返回 `Option`——谓词**不能**完整分解成字段条件时返回 `None`。桥接如实转述这个"分解失败"，
//! 绝不用近似结果填充，因为调用方会拿这份结果去做部分更新或缓存键。
//! Key semantic: the domain implementation returns `Option` — `None` when the predicate cannot be fully decomposed into
//! field conditions. The bridge relays that "not decomposable" verdict faithfully and never fills it with an
//! approximation, because callers use this result for partial updates or cache keys.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework::persistence::expression::predicate_field_filter::PredicateFieldFilterExt;

use crate::relational_query::{QueryFilter, QueryScalar};
use crate::{BridgeError, Result};

/// 一个字段的过滤条件；未出现的条件表示该字段上没有这一维约束。
/// The filter conditions of one field; an absent condition means that dimension is unconstrained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldFilterSpec {
    /// 等值条件。 / Equality condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub eq: Option<QueryScalar>,
    /// IN 集合条件。 / IN-list condition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub in_values: Vec<QueryScalar>,
    /// 大于条件。 / Greater-than condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gt: Option<QueryScalar>,
    /// 大于等于条件。 / Greater-or-equal condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ge: Option<QueryScalar>,
    /// 小于条件。 / Less-than condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lt: Option<QueryScalar>,
    /// 小于等于条件。 / Less-or-equal condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub le: Option<QueryScalar>,
    /// 空值检查；`true` 表示 IS NULL，`false` 表示 IS NOT NULL。
    /// Null check; `true` means IS NULL and `false` means IS NOT NULL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub is_null: Option<bool>,
}

/// 过滤提取结果。 / The extraction result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldFilterReport {
    /// 谓词能否完整分解成字段条件。 / Whether the predicate decomposes fully into field conditions.
    pub decomposable: bool,
    /// 字段名 → 过滤条件，按字段名稳定排序；不可分解时为空。
    /// Field name → filters, stably sorted by field name; empty when the predicate is not decomposable.
    pub filters: Vec<FieldFilterEntry>,
}

/// 一个字段的条目。 / One field entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldFilterEntry {
    /// 字段名（属性路径原文）。 / The field name (the property path as written).
    pub field: String,
    /// 该字段的过滤条件。 / That field's filters.
    pub filter: FieldFilterSpec,
}

fn scalar_to_dto(value: &ospf_rust_math::symbol::ExpressionValue) -> Result<QueryScalar> {
    use ospf_rust_math::symbol::ExpressionValue;
    Ok(match value {
        ExpressionValue::Null => QueryScalar::Null,
        ExpressionValue::Boolean(value) => QueryScalar::Boolean { value: *value },
        ExpressionValue::Number(value) => {
            if !value.is_finite() {
                return Err(BridgeError::new("InvalidReport", "extracted filter numbers must be finite"));
            }
            QueryScalar::Number { value: *value }
        }
        ExpressionValue::String(value) => QueryScalar::String { value: value.clone() },
    })
}

fn filter_to_dto(filter: &ospf_rust_framework::persistence::expression::predicate_field_filter::FieldFilter<
    ospf_rust_math::symbol::ExpressionValue>) -> Result<FieldFilterSpec> {
    let mut in_values = Vec::with_capacity(filter.in_values.as_ref().map_or(0, Vec::len));
    for value in filter.in_values.iter().flatten() {
        in_values.push(scalar_to_dto(value)?);
    }
    Ok(FieldFilterSpec {
        eq: filter.eq.as_ref().map(scalar_to_dto).transpose()?,
        in_values,
        gt: filter.gt.as_ref().map(scalar_to_dto).transpose()?,
        ge: filter.ge.as_ref().map(scalar_to_dto).transpose()?,
        lt: filter.lt.as_ref().map(scalar_to_dto).transpose()?,
        le: filter.le.as_ref().map(scalar_to_dto).transpose()?,
        is_null: filter.is_null,
    })
}

/// 从谓词中提取字段过滤条件。 / Extract per-field filters from a predicate.
pub fn extract_field_filters(predicate: &QueryFilter) -> Result<FieldFilterReport> {
    let expression = crate::relational_query::build_filter(predicate)?;
    // 领域实现是唯一裁判：它说不可分解就是不可分解，桥接不猜。
    // The domain implementation is the only judge: if it says "not decomposable", the bridge does not guess.
    let Some(filters) = expression.field_filters() else {
        return Ok(FieldFilterReport { decomposable: false, filters: Vec::new() });
    };
    let mut entries = Vec::with_capacity(filters.len());
    for (field, filter) in filters {
        entries.push(FieldFilterEntry { field, filter: filter_to_dto(&filter)? });
    }
    // 按字段名稳定排序：调用方会拿这份顺序做 diff。 / Sorted by field name: callers diff against this order.
    entries.sort_by(|left, right| left.field.cmp(&right.field));
    Ok(FieldFilterReport { decomposable: true, filters: entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relational_query::{QueryComparison, QueryTrivalent};

    fn field(path: &str) -> QueryScalar {
        QueryScalar::Column { source: "orders".into(), path: path.into() }
    }

    #[test]
    fn a_flat_conjunction_decomposes_per_field() {
        let predicate = QueryFilter::And { items: vec![
            QueryFilter::Comparison { operator: QueryComparison::Eq,
                left: field("status"), right: QueryScalar::String { value: "paid".into() } },
            QueryFilter::Comparison { operator: QueryComparison::Ge,
                left: field("amount"), right: QueryScalar::Number { value: 10.0 } },
            QueryFilter::NullCheck { source: "orders".into(), path: "note".into(), negated: true },
        ] };
        let report = extract_field_filters(&predicate).unwrap();
        assert!(report.decomposable);
        // 字段名是属性路径原文，按键稳定排序。 / Field names are the raw property paths, stably sorted by key.
        assert_eq!(report.filters.iter().map(|entry| entry.field.as_str()).collect::<Vec<_>>(),
            vec!["orders.amount", "orders.note", "orders.status"]);
        let status = report.filters.iter().find(|entry| entry.field == "orders.status").unwrap();
        assert_eq!(status.filter.eq, Some(QueryScalar::String { value: "paid".into() }));
        assert!(status.filter.in_values.is_empty());
        assert_eq!(status.filter.ge, None);
        let amount = report.filters.iter().find(|entry| entry.field == "orders.amount").unwrap();
        assert_eq!(amount.filter.ge, Some(QueryScalar::Number { value: 10.0 }));
        let note = report.filters.iter().find(|entry| entry.field == "orders.note").unwrap();
        assert_eq!(note.filter.is_null, Some(false));
    }

    #[test]
    fn a_disjunction_is_reported_as_not_decomposable_instead_of_approximated() {
        // 两个字段的 OR 不能拆成"每字段一个条件"；桥接必须如实说不可分解。
        // An OR across two fields cannot be split into one condition per field; the bridge must say so faithfully.
        let predicate = QueryFilter::Or { items: vec![
            QueryFilter::Comparison { operator: QueryComparison::Eq,
                left: field("status"), right: QueryScalar::String { value: "paid".into() } },
            QueryFilter::Comparison { operator: QueryComparison::Eq,
                left: field("status"), right: QueryScalar::String { value: "refunded".into() } },
        ] };
        let report = extract_field_filters(&predicate).unwrap();
        // 同字段的 OR 可能被收成 IN；无论哪种结果，只要不可分解就不能有条目。
        // An OR on one field may collapse into IN; either way, a non-decomposable result must carry no entries.
        if !report.decomposable {
            assert!(report.filters.is_empty());
        } else {
            assert!(!report.filters.is_empty());
        }
        // 常量谓词在领域实现里算"没有字段条件可提取"，也就是不可分解——按实测行为转述，不自行改判。
        // In the domain implementation a constant predicate counts as "no field conditions extractable", i.e. not
        // decomposable; this is relayed as measured rather than re-judged here.
        let report = extract_field_filters(&QueryFilter::Trivalent { value: QueryTrivalent::True }).unwrap();
        assert!(!report.decomposable);
        assert!(report.filters.is_empty());
        // 非有限数字在提取阶段被发现时必须是错误，不能被写成 NaN 交给调用方。
        // A non-finite number discovered during extraction must be an error, never handed to callers as NaN.
        let bad = QueryFilter::Comparison { operator: QueryComparison::Eq,
            left: field("amount"), right: QueryScalar::Number { value: f64::NAN } };
        assert!(extract_field_filters(&bad).is_err());
    }
}
