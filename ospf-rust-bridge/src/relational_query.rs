//! 关系查询计划（`persistence::query`）的可移植子集：结构校验 + 规范化 + 稳定指纹。
//! Portable subset of the relational query plan (`persistence::query`): structural validation, canonicalisation and a
//! stable fingerprint.
//!
//! 为什么只绑这一层：`persistence/backend/*`（11 个数据库驱动）依赖进程内连接与驱动 crate，属真实不可移植边界；
//! 但查询计划本身是**纯数据**——校验、规范化与指纹都不需要任何数据库。调用方因此可以在把计划交给任一后端之前
//! 先用同一套规则自检，而不是各自复制一份校验逻辑。
//! Why only this layer: `persistence/backend/*` (11 database drivers) needs in-process connections and driver crates,
//! which is a genuine non-portable boundary. The plan itself is **pure data** — validation, canonicalisation and the
//! fingerprint need no database at all. Callers can therefore self-check a plan against the same rules before handing it
//! to any backend, instead of each copying its own validation logic.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework::persistence::query::{
    ColumnRef, JoinCardinality, JoinSpec, JoinType, NullsOrder, OrderSpec, PageSpec, ProjectionSpec,
    QuerySource, RelationalQueryLimits, RelationalQueryPlan, SortDirection,
};
use ospf_rust_math::Trivalent;
use ospf_rust_math::symbol::{BooleanExpression, ComparisonOperator, ExpressionValue, NullCheckType, PropertyPath,
    ScalarExpression};

use crate::{BridgeError, Result};

/// 一个列引用。 / One column reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryColumn {
    /// 数据源名称或别名。 / Source name or alias.
    pub source: String,
    /// 字段路径。 / Field path.
    pub path: String,
}

/// 标量值。 / A scalar value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum QueryScalar {
    /// 空值。 / Null.
    Null,
    /// 布尔值。 / Boolean.
    Boolean { value: bool },
    /// 数字值。 / Number.
    Number { value: f64 },
    /// 字符串值。 / String.
    String { value: String },
    /// 列引用。 / Column reference.
    Column { source: String, path: String },
}

/// 比较操作符。 / Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QueryComparison {
    /// 等于。 / Equal.
    Eq,
    /// 不等于。 / Not equal.
    Ne,
    /// 小于。 / Less than.
    Lt,
    /// 小于等于。 / Less than or equal.
    Le,
    /// 大于。 / Greater than.
    Gt,
    /// 大于等于。 / Greater than or equal.
    Ge,
}

impl QueryComparison {
    fn to_domain(self) -> ComparisonOperator {
        match self {
            Self::Eq => ComparisonOperator::Eq,
            Self::Ne => ComparisonOperator::Ne,
            Self::Lt => ComparisonOperator::Lt,
            Self::Le => ComparisonOperator::Le,
            Self::Gt => ComparisonOperator::Gt,
            Self::Ge => ComparisonOperator::Ge,
        }
    }
}

/// 布尔三值常量。 / Boolean trivalent constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QueryTrivalent {
    /// 真。 / True.
    True,
    /// 假。 / False.
    False,
    /// 未知。 / Unknown.
    Unknown,
}

impl QueryTrivalent {
    fn to_domain(self) -> Trivalent {
        match self {
            Self::True => Trivalent::True,
            Self::False => Trivalent::False,
            Self::Unknown => Trivalent::Unknown,
        }
    }
}

/// 布尔筛选表达式树。
///
/// 只覆盖比较、逻辑与/或/非、空值检查与布尔常量这些**可通过数据表达**的形态；
/// `In`、`PatternMatch`、`Custom`、函数调用与算术运算会被明确拒绝，而不是悄悄近似。
/// Boolean filter tree. It covers only the shapes expressible as data: comparisons, AND/OR/NOT, null checks and boolean
/// constants. `In`, `PatternMatch`, `Custom`, function calls and arithmetic are refused explicitly instead of being
/// silently approximated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum QueryFilter {
    /// 布尔常量。 / Boolean constant.
    Trivalent { value: QueryTrivalent },
    /// 比较。 / Comparison.
    Comparison { operator: QueryComparison, left: QueryScalar, right: QueryScalar },
    /// 逻辑与。 / Logical AND.
    And { items: Vec<QueryFilter> },
    /// 逻辑或。 / Logical OR.
    Or { items: Vec<QueryFilter> },
    /// 逻辑非。 / Logical NOT.
    Not { item: Box<QueryFilter> },
    /// 空值检查。 / Null check.
    NullCheck { source: String, path: String, negated: bool },
}

/// 嵌套深度上限：防止恶意筛选树把构造拖成栈溢出。
/// Nesting bound so a hostile filter tree cannot turn construction into a stack overflow.
const MAX_FILTER_DEPTH: usize = 32;

fn scalar(value: &QueryScalar, depth: usize) -> Result<ScalarExpression<ExpressionValue>> {
    if depth > MAX_FILTER_DEPTH {
        return Err(BridgeError::new("InvalidInput", "query filter nesting exceeds the supported depth"));
    }
    Ok(match value {
        QueryScalar::Null => ScalarExpression::Constant(ExpressionValue::Null),
        QueryScalar::Boolean { value } => ScalarExpression::Constant(ExpressionValue::Boolean(*value)),
        QueryScalar::Number { value } => {
            if !value.is_finite() {
                return Err(BridgeError::new("InvalidInput", "query scalar numbers must be finite"));
            }
            ScalarExpression::Constant(ExpressionValue::Number(*value))
        }
        QueryScalar::String { value } => ScalarExpression::Constant(ExpressionValue::String(value.clone())),
        QueryScalar::Column { source, path } => {
            ScalarExpression::Reference(column_path(source, path)?)
        }
    })
}

fn column_path(source: &str, path: &str) -> Result<PropertyPath> {
    if source.trim().is_empty() || path.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput", "query column references require a source and a path"));
    }
    // 领域校验器靠第一个 `.` 取数据源限定符，所以这里必须拼成 `<source>.<path>`。
    // The domain validator takes the source qualifier from the first `.`, so the path must be `<source>.<path>`.
    Ok(PropertyPath::from(format!("{source}.{path}").as_str()))
}

pub(crate) fn build_filter(expression: &QueryFilter) -> Result<BooleanExpression<ExpressionValue>> {
    filter(expression, 0)
}

fn filter(expression: &QueryFilter, depth: usize) -> Result<BooleanExpression<ExpressionValue>> {
    if depth > MAX_FILTER_DEPTH {
        return Err(BridgeError::new("InvalidInput", "query filter nesting exceeds the supported depth"));
    }
    Ok(match expression {
        QueryFilter::Trivalent { value } => BooleanExpression::Constant(value.to_domain()),
        QueryFilter::Comparison { operator, left, right } => BooleanExpression::Comparison {
            operator: operator.to_domain(),
            left: scalar(left, depth + 1)?,
            right: scalar(right, depth + 1)?,
        },
        QueryFilter::And { items } => {
            if items.is_empty() {
                return Err(BridgeError::new("InvalidInput", "a query AND requires at least one item"));
            }
            let mut built = Vec::with_capacity(items.len());
            for item in items {
                built.push(filter(item, depth + 1)?);
            }
            BooleanExpression::And(built)
        }
        QueryFilter::Or { items } => {
            if items.is_empty() {
                return Err(BridgeError::new("InvalidInput", "a query OR requires at least one item"));
            }
            let mut built = Vec::with_capacity(items.len());
            for item in items {
                built.push(filter(item, depth + 1)?);
            }
            BooleanExpression::Or(built)
        }
        QueryFilter::Not { item } => BooleanExpression::Not(Box::new(filter(item, depth + 1)?)),
        QueryFilter::NullCheck { source, path, negated } => BooleanExpression::NullCheck {
            path: column_path(source, path)?,
            null_check_type: if *negated { NullCheckType::IsNotNull } else { NullCheckType::IsNull },
        },
    })
}

/// Join 类型。 / Join types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QueryJoinType {
    /// 内连接。 / Inner join.
    Inner,
    /// 左连接。 / Left join.
    Left,
    /// 存在性连接。 / Exists join.
    Exists,
}

/// 数据源基数。 / Source cardinality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QueryJoinCardinality {
    /// 一对一。 / One to one.
    OneToOne,
    /// 一对多。 / One to many.
    OneToMany,
    /// 多对一。 / Many to one.
    ManyToOne,
}

/// 一个数据源。 / One query source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuerySourceSpec {
    /// 数据源名称。 / Source name.
    pub name: String,
    /// 可选别名。 / Optional alias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub alias: Option<String>,
}

impl QuerySourceSpec {
    fn to_domain(&self) -> QuerySource {
        let source = QuerySource::new(&self.name);
        match &self.alias {
            Some(alias) => source.with_alias(alias),
            None => source,
        }
    }
}

/// 一个 join。 / One join.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryJoinSpec {
    /// Join 类型。 / Join type.
    pub join_type: QueryJoinType,
    /// 被连接的数据源。 / The joined source.
    pub source: QuerySourceSpec,
    /// 关联条件。 / The correlation condition.
    pub condition: QueryFilter,
    /// 声明的基数。 / The declared cardinality.
    pub cardinality: QueryJoinCardinality,
}

/// 一条排序。 / One ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryOrderSpec {
    /// 排序列。 / The ordering column.
    pub column: QueryColumn,
    /// 方向。 / The direction.
    pub direction: QuerySortDirection,
    /// NULL 位置。 / NULL placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub nulls: Option<QueryNullsOrder>,
}

/// 排序方向。 / Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QuerySortDirection {
    /// 升序。 / Ascending.
    Ascending,
    /// 降序。 / Descending.
    Descending,
}

/// NULL 排序位置。 / NULL ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QueryNullsOrder {
    /// 不指定。 / Unspecified.
    Unspecified,
    /// 在前。 / First.
    First,
    /// 在后。 / Last.
    Last,
}

/// 分页。 / Paging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryPageSpec {
    /// 单页上限。 / Page limit.
    pub limit: u32,
    /// 跳过记录数。 / Records to skip.
    pub offset: u32,
}

/// 计划限制。 / Plan limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryPlanLimits {
    /// 最大 join 数。 / Maximum joins.
    pub max_joins: u32,
    /// 最大数据源深度。 / Maximum source depth.
    pub max_depth: u32,
    /// 最大投影字段数。 / Maximum projections.
    pub max_projections: u32,
}

/// 校验失败项。 / One validation violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryPlanViolation {
    /// 出错字段。 / The failing field.
    pub field: String,
    /// 失败原因。 / The reason.
    pub reason: String,
}

/// 查询计划校验报告。 / Query plan validation report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryPlanReport {
    /// 是否通过校验。 / Whether validation passed.
    pub valid: bool,
    /// 违规项；通过时为空。
    /// 领域校验器只报**第一个**错误，所以这里至多一项——不伪造"全部错误列表"。
    /// Violations; empty when valid. The domain validator reports only the **first** error, so there is at most one
    /// entry here — no fabricated "all errors" list.
    pub violations: Vec<QueryPlanViolation>,
    /// 规范化表示；仅在校验通过时给出。 / The canonical form, only when validation passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub canonical: Option<String>,
    /// 规范化指纹；仅在校验通过时给出。 / The canonical fingerprint, only when validation passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub canonical_hash: Option<String>,
    /// join 数量。 / The join count.
    pub join_count: u32,
    /// 投影数量。 / The projection count.
    pub projection_count: u32,
    /// 数据源深度（join 数 + 1）。 / The source depth (joins + 1).
    pub depth: u32,
}

/// 一份关系查询计划。 / One relational query plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationalQueryPlanSpec {
    /// 根数据源。 / The root source.
    pub root: QuerySourceSpec,
    /// join 列表。 / The joins.
    #[serde(default)]
    pub joins: Vec<QueryJoinSpec>,
    /// 可选筛选。 / The optional predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub predicate: Option<QueryFilter>,
    /// 投影字段。 / The projections.
    #[serde(default)]
    pub projections: Vec<QueryColumn>,
    /// 分组字段。 / The grouping columns.
    #[serde(default)]
    pub group_by: Vec<QueryColumn>,
    /// 排序。 / The orderings.
    #[serde(default)]
    pub order_by: Vec<QueryOrderSpec>,
    /// 稳定主键。 / The stable key.
    #[serde(default)]
    pub root_key: Vec<QueryColumn>,
    /// 可选分页。 / The optional page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub page: Option<QueryPageSpec>,
    /// 可选限制；省略时用领域默认值。 / Optional limits; the domain default applies when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limits: Option<QueryPlanLimits>,
}

fn column(column: &QueryColumn) -> Result<ColumnRef> {
    if column.source.trim().is_empty() || column.path.trim().is_empty() {
        // 空列引用交给领域校验器拒绝，而不是在这里另造一套规则。
        // Blank column references are left for the domain validator to refuse rather than inventing a second rule set.
        return Ok(ColumnRef::new(&column.source, &column.path));
    }
    Ok(ColumnRef::new(&column.source, &column.path))
}

/// 构建真实的查询计划。 / Build the real query plan.
fn plan(spec: &RelationalQueryPlanSpec) -> Result<RelationalQueryPlan> {
    let mut built = RelationalQueryPlan::new(spec.root.to_domain());
    if !spec.joins.is_empty() {
        let mut joins = Vec::with_capacity(spec.joins.len());
        for join in &spec.joins {
            joins.push(JoinSpec::new(match join.join_type {
                QueryJoinType::Inner => JoinType::Inner,
                QueryJoinType::Left => JoinType::Left,
                QueryJoinType::Exists => JoinType::Exists,
            }, join.source.to_domain(), filter(&join.condition, 0)?, match join.cardinality {
                QueryJoinCardinality::OneToOne => JoinCardinality::OneToOne,
                QueryJoinCardinality::OneToMany => JoinCardinality::OneToMany,
                QueryJoinCardinality::ManyToOne => JoinCardinality::ManyToOne,
            }));
        }
        built = built.with_joins(joins);
    }
    if let Some(predicate) = &spec.predicate {
        built = built.with_predicate(filter(predicate, 0)?);
    }
    if !spec.projections.is_empty() {
        let mut projections = Vec::with_capacity(spec.projections.len());
        for projection in &spec.projections {
            projections.push(ProjectionSpec::new(column(projection)?));
        }
        built = built.with_projections(projections);
    }
    if !spec.group_by.is_empty() {
        let mut group_by = Vec::with_capacity(spec.group_by.len());
        for entry in &spec.group_by {
            group_by.push(column(entry)?);
        }
        built = built.with_group_by(group_by);
    }
    if !spec.order_by.is_empty() {
        let mut order_by = Vec::with_capacity(spec.order_by.len());
        for entry in &spec.order_by {
            let mut order = OrderSpec::new(column(&entry.column)?).with_direction(match entry.direction {
                QuerySortDirection::Ascending => SortDirection::Ascending,
                QuerySortDirection::Descending => SortDirection::Descending,
            });
            if let Some(nulls) = entry.nulls {
                order = order.with_nulls(match nulls {
                    QueryNullsOrder::Unspecified => NullsOrder::Unspecified,
                    QueryNullsOrder::First => NullsOrder::First,
                    QueryNullsOrder::Last => NullsOrder::Last,
                });
            }
            order_by.push(order);
        }
        built = built.with_order_by(order_by);
    }
    if !spec.root_key.is_empty() {
        let mut root_key = Vec::with_capacity(spec.root_key.len());
        for entry in &spec.root_key {
            root_key.push(column(entry)?);
        }
        built = built.with_root_key(root_key);
    }
    if let Some(page) = spec.page {
        built = built.with_page(PageSpec::new(
            usize::try_from(page.limit).map_err(|_| BridgeError::new("InvalidInput", "page limit overflow"))?,
            usize::try_from(page.offset).map_err(|_| BridgeError::new("InvalidInput", "page offset overflow"))?));
    }
    Ok(built)
}

/// 从 DTO 构建计划（不校验），供仓储入口复用。 / Build the plan from a DTO without validating, for the repository entry.
pub(crate) fn plan_from_spec(spec: &RelationalQueryPlanSpec) -> Result<RelationalQueryPlan> {
    plan(spec)
}

/// 校验并规范化一份查询计划。 / Validate and canonicalise one query plan.
pub fn validate_query_plan(spec: &RelationalQueryPlanSpec) -> Result<QueryPlanReport> {
    let limits = match spec.limits {
        Some(limits) => RelationalQueryLimits {
            max_joins: usize::try_from(limits.max_joins)
                .map_err(|_| BridgeError::new("InvalidInput", "max joins overflow"))?,
            max_depth: usize::try_from(limits.max_depth)
                .map_err(|_| BridgeError::new("InvalidInput", "max depth overflow"))?,
            max_projections: usize::try_from(limits.max_projections)
                .map_err(|_| BridgeError::new("InvalidInput", "max projections overflow"))?,
        },
        None => RelationalQueryLimits::default(),
    };
    let built = plan(spec)?;
    let join_count = u32::try_from(built.joins().len()).unwrap_or(u32::MAX);
    let projection_count = u32::try_from(built.projections().len()).unwrap_or(u32::MAX);
    let outcome = built.validate_with_limits(limits);
    match outcome {
        Ok(()) => Ok(QueryPlanReport {
            valid: true,
            violations: Vec::new(),
            canonical: Some(built.canonical()),
            canonical_hash: Some(built.canonical_hash()),
            join_count,
            projection_count,
            depth: join_count.saturating_add(1),
        }),
        Err(error) => Ok(QueryPlanReport {
            valid: false,
            violations: vec![QueryPlanViolation { field: error.field, reason: error.reason }],
            // 非法计划不给出规范化结果：那会诱导调用方当成可用计划去执行。
            // An invalid plan gets no canonical form: presenting one would invite callers to execute it.
            canonical: None,
            canonical_hash: None,
            join_count,
            projection_count,
            depth: join_count.saturating_add(1),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> QuerySourceSpec {
        QuerySourceSpec { name: "orders".into(), alias: Some("o".into()) }
    }

    fn column_filter() -> QueryFilter {
        QueryFilter::Comparison { operator: QueryComparison::Eq,
            left: QueryScalar::Column { source: "o".into(), path: "customer_id".into() },
            right: QueryScalar::Column { source: "c".into(), path: "id".into() } }
    }

    fn plan_spec() -> RelationalQueryPlanSpec {
        RelationalQueryPlanSpec { root: root(),
            joins: vec![QueryJoinSpec { join_type: QueryJoinType::Inner,
                source: QuerySourceSpec { name: "customers".into(), alias: Some("c".into()) },
                condition: column_filter(), cardinality: QueryJoinCardinality::OneToOne }],
            predicate: None,
            projections: vec![QueryColumn { source: "o".into(), path: "id".into() }],
            group_by: Vec::new(), order_by: Vec::new(), root_key: Vec::new(), page: None, limits: None }
    }

    #[test]
    fn a_well_formed_plan_validates_and_is_canonicalised() {
        let report = validate_query_plan(&plan_spec()).unwrap();
        assert!(report.valid, "{:?}", report.violations);
        assert_eq!(report.join_count, 1);
        assert_eq!(report.projection_count, 1);
        assert_eq!(report.depth, 2);
        let canonical = report.canonical.unwrap();
        let hash = report.canonical_hash.unwrap();
        assert!(!canonical.is_empty());
        // 规范化与指纹必须稳定：同一计划两次调用结果一致。
        // Canonicalisation and the fingerprint must be stable: two calls on the same plan agree.
        let again = validate_query_plan(&plan_spec()).unwrap();
        assert_eq!(again.canonical.as_deref(), Some(canonical.as_str()));
        assert_eq!(again.canonical_hash.as_deref(), Some(hash.as_str()));
    }

    #[test]
    fn violations_are_reported_without_inventing_a_canonical_form() {
        // join 条件引用未绑定的数据源。 / The join condition references an unbound source.
        let mut spec = plan_spec();
        spec.joins[0].condition = QueryFilter::Comparison { operator: QueryComparison::Eq,
            left: QueryScalar::Column { source: "ghost".into(), path: "id".into() },
            right: QueryScalar::Column { source: "o".into(), path: "id".into() } };
        let report = validate_query_plan(&spec).unwrap();
        assert!(!report.valid);
        assert_eq!(report.violations.len(), 1);
        assert!(report.violations[0].field.starts_with("joins[0]"));
        // 非法计划绝不给出规范化结果。 / An invalid plan never gets a canonical form.
        assert_eq!(report.canonical, None);
        assert_eq!(report.canonical_hash, None);
        // 重复数据源被拒。 / A duplicated source is refused.
        let mut duplicated = plan_spec();
        duplicated.joins[0].source = QuerySourceSpec { name: "orders".into(), alias: None };
        assert!(!validate_query_plan(&duplicated).unwrap().valid);
        // 空 join 条件被拒。 / An empty join condition is refused.
        let mut hollow = plan_spec();
        hollow.joins[0].condition = QueryFilter::Trivalent { value: QueryTrivalent::True };
        assert!(!validate_query_plan(&hollow).unwrap().valid);
        // 页大小为 0 被拒。 / A zero page limit is refused.
        let mut zero_page = plan_spec();
        zero_page.page = Some(QueryPageSpec { limit: 0, offset: 0 });
        assert!(!validate_query_plan(&zero_page).unwrap().valid);
        // 声明的限制生效。 / The declared limits take effect.
        let mut over = plan_spec();
        over.limits = Some(QueryPlanLimits { max_joins: 0, max_depth: 16, max_projections: 128 });
        assert!(!validate_query_plan(&over).unwrap().valid);
    }

    #[test]
    fn unsupported_or_malformed_filters_are_refused_rather_than_approximated() {
        // 空谓词列表、非有限数字、空列引用、超深嵌套都必须被拒。
        // Empty item lists, non-finite numbers, blank column references and excessive nesting must all be refused.
        let broken: Vec<(&str, QueryFilter)> = vec![
            ("empty and", QueryFilter::And { items: Vec::new() }),
            ("empty or", QueryFilter::Or { items: Vec::new() }),
            ("non-finite", QueryFilter::Comparison { operator: QueryComparison::Eq,
                left: QueryScalar::Number { value: f64::NAN }, right: QueryScalar::Null }),
            ("blank column", QueryFilter::Comparison { operator: QueryComparison::Eq,
                left: QueryScalar::Column { source: " ".into(), path: "id".into() }, right: QueryScalar::Null }),
            ("blank null check", QueryFilter::NullCheck { source: "o".into(), path: "".into(), negated: false }),
        ];
        for (label, filter_value) in broken {
            let mut spec = plan_spec();
            spec.joins[0].condition = filter_value;
            assert!(validate_query_plan(&spec).is_err(), "{label} must be refused");
        }
        // 超深嵌套。 / Excessive nesting.
        let mut deep = QueryFilter::Trivalent { value: QueryTrivalent::True };
        for _ in 0..MAX_FILTER_DEPTH + 2 {
            deep = QueryFilter::Not { item: Box::new(deep) };
        }
        let mut spec = plan_spec();
        spec.predicate = Some(deep);
        assert!(validate_query_plan(&spec).is_err());
    }
}
