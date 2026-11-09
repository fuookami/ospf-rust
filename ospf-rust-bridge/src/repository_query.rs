//! 仓储查询的整体入口：基础计划 + where 谓词 + 排序/分页 → 一份已校验的关系查询计划。
//! The whole repository-query entry: base plan plus a where predicate plus sort/paging → one validated relational plan.
//!
//! 这是 persistence 侧可移植语义的收束点：它把已经绑定的三块（查询计划 / 表达式子集 / 排序）合成一次判定，
//! 并复述领域实现里那条容易踩的规则——**有 `limit` 时 `offset` 缺省为 0；只有 `offset` 时走
//! `with_offset_without_limit`**，两者不是同一个东西。
//! This closes out the portable side of `persistence`: it composes the three already-bound pieces (query plan,
//! expression subset, ordering) into one verdict and relays a rule that is easy to get wrong — **with a `limit`, a
//! missing `offset` means 0; with only an `offset`, the plan goes through `with_offset_without_limit`**; the two are not
//! the same thing.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework::persistence::expression::repository_api::{
    build_relational_query_plan, RepositoryQuery,
};
use ospf_rust_framework::persistence::expression::sort_by::{
    NullsOrder as DomainNullsOrder, SortBy, SortDirection as DomainSortDirection, SortItem,
};
use ospf_rust_math::symbol::{ExpressionValue, PropertyPath};

use crate::relational_query::{
    plan_from_spec, QueryNullsOrder, QueryOrderSpec, QueryPlanReport, QueryPlanViolation, QuerySortDirection,
    RelationalQueryPlanSpec,
};
use crate::{BridgeError, Result};

/// 一份仓储查询请求。 / One repository-query request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryQuerySpec {
    /// 基础计划（其自身的谓词会与 `where` 做 AND）。 / The base plan; its own predicate is ANDed with `where`.
    pub plan: RelationalQueryPlanSpec,
    /// 附加的 where 谓词。 / The additional where predicate.
    pub predicate: crate::relational_query::QueryFilter,
    /// 排序项，按优先级。 / The sort items in priority order.
    #[serde(default)]
    pub sort: Vec<QueryOrderSpec>,
    /// 可选返回上限。 / Optional result limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
    /// 可选偏移量。 / Optional result offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub offset: Option<u32>,
}

fn sort_by(items: &[QueryOrderSpec]) -> Result<Option<SortBy>> {
    if items.is_empty() {
        return Ok(None);
    }
    let mut built = Vec::with_capacity(items.len());
    for item in items {
        if item.column.source.trim().is_empty() || item.column.path.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "sort columns require a source and a path"));
        }
        built.push(SortItem::new(
            PropertyPath::from(item.column.path.as_str()),
            match item.direction {
                QuerySortDirection::Ascending => DomainSortDirection::Asc,
                QuerySortDirection::Descending => DomainSortDirection::Desc,
            },
            item.nulls.and_then(|nulls| match nulls {
                QueryNullsOrder::Unspecified => None,
                QueryNullsOrder::First => Some(DomainNullsOrder::NullsFirst),
                QueryNullsOrder::Last => Some(DomainNullsOrder::NullsLast),
            })));
    }
    Ok(Some(SortBy { items: built }))
}

/// 构造并校验一份仓储查询计划。 / Build and validate one repository query plan.
pub fn build_repository_query(spec: &RepositoryQuerySpec) -> Result<QueryPlanReport> {
    let base = plan_from_spec(&spec.plan)?;
    let predicate = crate::relational_query::build_filter(&spec.predicate)?;
    let mut options = RepositoryQuery::new();
    if let Some(sort) = sort_by(&spec.sort)? {
        options = options.with_sort_by(sort);
    }
    if let Some(limit) = spec.limit {
        options = options.with_limit(usize::try_from(limit)
            .map_err(|_| BridgeError::new("InvalidInput", "repository limit overflow"))?);
    }
    if let Some(offset) = spec.offset {
        options = options.with_offset(usize::try_from(offset)
            .map_err(|_| BridgeError::new("InvalidInput", "repository offset overflow"))?);
    }
    let join_count = u32::try_from(base.joins().len()).unwrap_or(u32::MAX);
    let projection_count = u32::try_from(base.projections().len()).unwrap_or(u32::MAX);
    match build_relational_query_plan(base, &predicate, &options) {
        Ok(plan) => Ok(QueryPlanReport {
            valid: true,
            violations: Vec::new(),
            canonical: Some(plan.canonical()),
            canonical_hash: Some(plan.canonical_hash()),
            join_count,
            projection_count,
            depth: join_count.saturating_add(1),
        }),
        // 领域入口只报第一个错误，桥接照抄，不伪造"全部错误列表"。
        // The domain entry reports only the first error; the bridge copies that and never fabricates a full list.
        Err(error) => Ok(QueryPlanReport {
            valid: false,
            violations: vec![QueryPlanViolation { field: error.field, reason: error.reason }],
            canonical: None,
            canonical_hash: None,
            join_count,
            projection_count,
            depth: join_count.saturating_add(1),
        }),
    }
}

/// 让调用方确认分页语义：`limit` 存在时 `offset` 缺省就是 0。
/// Let callers confirm the paging semantics: when `limit` exists, a missing `offset` means 0.
pub fn effective_paging(spec: &RepositoryQuerySpec) -> Result<(Option<u32>, Option<u32>)> {
    match (spec.limit, spec.offset) {
        (Some(limit), None) => Ok((Some(limit), Some(0))),
        (limit, offset) => Ok((limit, offset)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relational_query::{QueryColumn, QueryComparison, QueryFilter, QueryScalar, QuerySourceSpec};

    fn spec() -> RepositoryQuerySpec {
        RepositoryQuerySpec {
            plan: RelationalQueryPlanSpec {
                root: QuerySourceSpec { name: "orders".into(), alias: None },
                joins: Vec::new(), predicate: None,
                projections: vec![QueryColumn { source: "orders".into(), path: "id".into() }],
                group_by: Vec::new(), order_by: Vec::new(), root_key: Vec::new(), page: None, limits: None,
            },
            predicate: QueryFilter::Comparison { operator: QueryComparison::Eq,
                left: QueryScalar::Column { source: "orders".into(), path: "status".into() },
                right: QueryScalar::String { value: "paid".into() } },
            sort: vec![QueryOrderSpec { column: QueryColumn { source: "orders".into(), path: "id".into() },
                direction: QuerySortDirection::Descending, nulls: Some(QueryNullsOrder::Last) }],
            limit: None, offset: None,
        }
    }

    #[test]
    fn a_well_formed_repository_query_is_merged_and_validated() {
        let report = build_repository_query(&spec()).unwrap();
        assert!(report.valid, "{:?}", report.violations);
        let canonical = report.canonical.unwrap();
        // 排序与分页确实进了计划：规范化结果随分页变化。 / Sorting and paging really enter the plan.
        let mut paged = spec();
        paged.limit = Some(10);
        paged.offset = Some(20);
        let paged_report = build_repository_query(&paged).unwrap();
        assert!(paged_report.valid);
        assert_ne!(paged_report.canonical.as_deref(), Some(canonical.as_str()));
        // 同一请求两次结果一致。 / Two runs of the same request agree.
        assert_eq!(build_repository_query(&spec()).unwrap().canonical_hash, report.canonical_hash);
    }

    #[test]
    fn paging_semantics_are_relayed_exactly() {
        // limit 存在而 offset 缺省 = 0。 / limit present with a missing offset means 0.
        let mut limited = spec();
        limited.limit = Some(5);
        assert_eq!(effective_paging(&limited).unwrap(), (Some(5), Some(0)));
        // 只有 offset：不是"limit=0"，两者必须分开。 / Only offset is not "limit=0"; the two stay separate.
        let mut offset_only = spec();
        offset_only.offset = Some(7);
        assert_eq!(effective_paging(&offset_only).unwrap(), (None, Some(7)));
        // 只有 offset 的计划仍然是合法计划。 / An offset-only plan is still a valid plan.
        assert!(build_repository_query(&offset_only).unwrap().valid);
    }

    #[test]
    fn invalid_parts_surface_as_violations_without_a_canonical_form() {
        // 计划校验**不**解析 where 谓词的来源（那是后端 translator 的职责），所以这条仍然"合法"——
        // 桥接如实转述，不自己加一层领域没有的规则。
        // Plan validation does **not** resolve where-predicate sources (that is the backend translator's job), so this
        // stays "valid"; the bridge relays that rather than inventing a rule the domain does not have.
        let mut unresolved_predicate = spec();
        unresolved_predicate.predicate = QueryFilter::Comparison { operator: QueryComparison::Eq,
            left: QueryScalar::Column { source: "ghost".into(), path: "id".into() },
            right: QueryScalar::Null };
        assert!(build_repository_query(&unresolved_predicate).unwrap().valid);
        // 领域校验器真正会拒绝的形态：页大小为 0。 / What the validator really rejects: a zero page limit.
        let mut bad_page = spec();
        bad_page.plan.page = Some(crate::relational_query::QueryPageSpec { limit: 0, offset: 0 });
        let report = build_repository_query(&bad_page).unwrap();
        assert!(!report.valid);
        assert_eq!(report.canonical, None);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].field, "page");
        // 空排序列引用在入口就被拒，而不是变成一条永远匹配不上的排序。
        // A blank sort column is refused at the entry rather than becoming an ordering that can never match.
        let mut bad_sort = spec();
        bad_sort.sort[0].column.path = " ".into();
        assert!(build_repository_query(&bad_sort).is_err());
        // 空 where 谓词列表被拒。 / An empty where item list is refused.
        let mut bad_where = spec();
        bad_where.predicate = QueryFilter::And { items: Vec::new() };
        assert!(build_repository_query(&bad_where).is_err());
    }
}
