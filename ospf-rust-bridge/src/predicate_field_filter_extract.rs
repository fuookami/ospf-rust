//! 续挖第 4 单：谓词的三级字段过滤提取（`persistence/expression/predicate_field_filter.rs`）
//! Continuation, delivery 4: the three-level predicate field-filter extraction.
//!
//! `predicate_field_filter.rs` 里有**三个不同严格度的提取器**，它们吃掉同一棵 `BooleanExpression`，返回 `Option`：
//!
//! | 提取器 | 接受的形状 | 拒绝时 |
//! | --- | --- | --- |
//! | `eq_filters` | `And` 分支下的 `Eq` 比较 | `ne` / 区间 / `in` / `or` / `not` 全部 `None` |
//! | `eq_or_in_filters` | `And` 分支下的 `Eq` 比较与 `In` | 区间比较也给 `None` |
//! | `field_filters` | `Eq` / `Ne` / 四向区间 / `In` / `IsNull` / `IsNotNull` 的 `And` | `Or` / `Not` / 自定义节点 `None` |
//!
//! 三条只有读实现才知道的行为，这一单把它们变成可断言的事实：
//!
//! 1. **`Or` 一律拒绝**：三个提取器都只递归 `And`，`Or` 落到 `_ => false`。所以 `a = 1 OR b = 2` 不是"两个过滤"，
//!    而是**根本没有**字段过滤——这一条最容易想当然地搞错。
//! 2. **重复路径静默覆盖**：内部是 `HashMap::insert`，所以 `a = 1 AND a = 2` 得到**一个** `a`（值为后一个），
//!    不报错、不合并。报告用 `duplicatePaths` 把这件事显式写出来，让调用方自己决定要不要拒绝。
//! 3. **`BooleanExpression::and` / `or` 在空操作数上 `assert!` 直接 panic**。所以条件列表为空必须在桥接入口
//!    **先拒绝**，绝不能把空向量交下去——宿主里 panic 等于整个进程崩。
//!
//! 4. **反向比较会被翻转而不是拒绝**：`field_comparison` 同时接受 `path op constant` 与 `constant op path`，
//!    后者走 `flip_comparison_side`（`1 < a` 等价于 `a > 1`）。这一条我一开始照着文档想当然写成"反向提取不到"，
//!    读实现才发现错了。
//! 5. **`ne` 被 `field_filters` 拒绝**（`ComparisonOperator::Ne => false`），而 `FieldFilter` 根本没有"不等"槽位；
//!    同时 **`eq` 配 `null` 值不算等值**，它会落进 `is_null = true` 槽。
//!
//! That file holds **three extractors of increasing leniency** over the same `BooleanExpression`, each returning `Option`:
//! `eq_filters` accepts `Eq` comparisons under `And`; `eq_or_in_filters` additionally accepts `In`; `field_filters` accepts
//! `Eq` / `Ne` / the four interval operators / `In` / `IsNull` / `IsNotNull`. Three behaviours are only visible in the
//! implementation and are turned into assertable facts here: **`Or` is refused by all three** (they only recurse into `And`,
//! so `a = 1 OR b = 2` yields *no* field filter at all rather than two); **repeated paths are silently overwritten** because the
//! collector uses `HashMap::insert`, so `a = 1 AND a = 2` yields a single `a` holding the latter value with no error — and the
//! report surfaces those paths in `duplicatePaths` so the caller decides; and **`BooleanExpression::and` / `or` `assert!` on
//! empty operand lists**, so an empty condition list must be refused at the entry rather than handed down, since a panic there
//! takes the whole host process with it. Comparisons must also keep the field on the left and the constant on the right
//! Reversed comparisons are **flipped rather than refused** (`field_comparison` accepts both `path op constant` and
//! `constant op path`, the latter through `flip_comparison_side`), `ne` is refused by `field_filters` (and `FieldFilter` has
//! no inequality slot at all), and an `eq` against `null` is not an equality but an `is_null = true` slot.

use ospf_rust_framework::persistence::expression::predicate_field_filter::{
    EqOrInFilter, PredicateFieldFilterExt,
};
use ospf_rust_framework::persistence::expression::FieldPath;
use ospf_rust_math::symbol::{BooleanExpression, ComparisonOperator, ExpressionValue};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个过滤值：`kind` 加字面文本。 / One filter value: a `kind` plus its literal text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterValueSpec {
    /// `null` / `boolean` / `number` / `string`。 / One of `null`, `boolean`, `number`, `string`.
    pub kind: String,
    /// 字面文本（`null` 时忽略）。 / The literal text (ignored for `null`).
    #[serde(default)]
    pub text: String,
}

/// 一个条件。 / One condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterConditionSpec {
    /// 字段路径。 / The field path.
    pub path: String,
    /// `eq` / `ne` / `gt` / `ge` / `lt` / `le` / `in` / `isNull` / `isNotNull`。 / The operator.
    pub operator: String,
    /// 单值操作数的值。 / The value for a single-value operator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<FilterValueSpec>,
    /// `in` 的值列表。 / The values for `in`.
    #[serde(default)]
    pub values: Vec<FilterValueSpec>,
}

/// 提取请求。 / The extraction request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateFieldFilterRequest {
    /// `and` / `or`（`or` 用来观察三个提取器的拒绝）。 / `and` or `or`, the latter to observe the refusals.
    pub combinator: String,
    /// 条件列表（**不能为空**）。 / The conditions, which must not be empty.
    pub conditions: Vec<FilterConditionSpec>,
}

/// 等值提取的一行。 / One row of the equality extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EqFilterRow {
    /// 字段名。 / The field name.
    pub field: String,
    /// 等值。 / The equality value.
    pub value: FilterValueSpec,
}

/// 等值或 IN 提取的一行。 / One row of the equality-or-IN extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EqOrInRow {
    /// 字段名。 / The field name.
    pub field: String,
    /// `eq` 或 `in`。 / `eq` or `in`.
    pub kind: String,
    /// `eq` 的值。 / The value for `eq`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<FilterValueSpec>,
    /// `in` 的值列表。 / The values for `in`.
    #[serde(default)]
    pub values: Vec<FilterValueSpec>,
}

/// 字段级过滤提取的一行（七个槽互不覆盖）。 / One row of the field-level extraction (seven independent slots).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldFilterRow {
    /// 字段名。 / The field name.
    pub field: String,
    /// 等值槽。 / The equality slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub eq: Option<FilterValueSpec>,
    /// IN 槽。 / The IN slot.
    #[serde(default)]
    pub in_values: Vec<FilterValueSpec>,
    /// 大于槽。 / The greater-than slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gt: Option<FilterValueSpec>,
    /// 大于等于槽。 / The greater-or-equal slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ge: Option<FilterValueSpec>,
    /// 小于槽。 / The less-than slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lt: Option<FilterValueSpec>,
    /// 小于等于槽。 / The less-or-equal slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub le: Option<FilterValueSpec>,
    /// 空值检查；`true` 表示 `IS NULL`。 / The null check; `true` means `IS NULL`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub is_null: Option<bool>,
}

/// 提取结论。 / The extraction verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateFieldFilterReport {
    /// 条件数。 / The number of conditions.
    pub condition_count: u32,
    /// `eq_filters` 的结论；`undefined` 表示拒绝。 / The `eq_filters` verdict; `undefined` means refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub eq_filters: Option<Vec<EqFilterRow>>,
    /// `eq_or_in_filters` 的结论；`undefined` 表示拒绝。 / The `eq_or_in_filters` verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub eq_or_in_filters: Option<Vec<EqOrInRow>>,
    /// `field_filters` 的结论；`undefined` 表示拒绝。 / The `field_filters` verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub field_filters: Option<Vec<FieldFilterRow>>,
    /// 三个提取器里拒绝的那些（按严格度升序）。 / Which of the three refused, in increasing leniency.
    pub rejected: Vec<String>,
    /// 在条件里出现多于一次的字段路径（领域会**静默覆盖**）。 / Paths appearing more than once (silently overwritten).
    pub duplicate_paths: Vec<String>,
    /// 操作符词表（桥接定义）。 / The operator vocabulary (bridge-defined).
    pub operator_vocabulary: Vec<String>,
}

/// 条件数上限（信任边界）。 / The condition cap (a trust boundary).
const MAX_CONDITIONS: usize = 1_024;

/// 把请求条件编成领域表达式，再跑三个提取器。非法输入是**错误**。
/// Compile the request into a domain expression, then run all three extractors. Illegal input is an **error**.
pub fn verify_predicate_field_filters(request: &PredicateFieldFilterRequest)
    -> Result<PredicateFieldFilterReport>
{
    if request.conditions.is_empty() {
        // `BooleanExpression::and` / `or` 在空操作数上 panic，所以这里必须**先**拒绝。
        // `BooleanExpression::and` / `or` panic on empty operands, so this must be refused **first**.
        return Err(BridgeError::new("InvalidInput",
            "predicate field filter extraction failed validation: at least one condition is required"));
    }
    if request.conditions.len() > MAX_CONDITIONS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "predicate field filter extraction failed validation: at most {MAX_CONDITIONS} conditions are accepted")));
    }
    let mut compiled = Vec::with_capacity(request.conditions.len());
    for condition in &request.conditions {
        compiled.push(compile(condition)?);
    }
    let expression = match request.combinator.as_str() {
        "and" => BooleanExpression::and(compiled),
        "or" => BooleanExpression::or(compiled),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "predicate field filter extraction failed validation: unknown combinator `{other}`"))),
    };
    // 三个提取器，同一棵表达式。 / Three extractors over the same expression.
    let eq = expression.eq_filters();
    let eq_or_in = expression.eq_or_in_filters();
    let field = expression.field_filters();
    let mut rejected = Vec::new();
    if eq.is_none() { rejected.push("eqFilters".to_owned()); }
    if eq_or_in.is_none() { rejected.push("eqOrInFilters".to_owned()); }
    if field.is_none() { rejected.push("fieldFilters".to_owned()); }
    let seen = {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for condition in &request.conditions {
            *counts.entry(condition.path.trim().to_owned()).or_default() += 1;
        }
        // 领域用 `HashMap::insert` 静默覆盖，所以这里把重复路径显式列出来供调用方判断。
        // The domain silently overwrites through `HashMap::insert`, so the repeats are listed for the caller to judge.
        counts.into_iter().filter(|(_, count)| *count > 1).map(|(path, _)| path).collect::<Vec<_>>()
    };
    Ok(PredicateFieldFilterReport {
        condition_count: u32::try_from(request.conditions.len()).unwrap_or(u32::MAX),
        eq_filters: eq.map(|filters| {
            let mut rows = filters.into_iter().map(|(field, value)| EqFilterRow {
                field, value: describe_value(&value) }).collect::<Vec<_>>();
            rows.sort_by(|left, right| left.field.cmp(&right.field));
            rows
        }),
        eq_or_in_filters: eq_or_in.map(|filters| {
            let mut rows = filters.into_iter().map(|(field, filter)| EqOrInRow {
                field,
                kind: match filter { EqOrInFilter::Eq(_) => "eq", EqOrInFilter::In(_) => "in" }.to_owned(),
                value: match &filter { EqOrInFilter::Eq(value) => Some(describe_value(value)), _ => None },
                values: match &filter {
                    EqOrInFilter::In(values) => values.iter().map(describe_value).collect(),
                    _ => Vec::new(),
                },
            }).collect::<Vec<_>>();
            rows.sort_by(|left, right| left.field.cmp(&right.field));
            rows
        }),
        field_filters: field.map(|filters| {
            let mut rows = filters.into_iter().map(|(field, filter)| FieldFilterRow {
                field,
                eq: filter.eq.as_ref().map(describe_value),
                in_values: filter.in_values.iter().flatten().map(describe_value).collect(),
                gt: filter.gt.as_ref().map(describe_value),
                ge: filter.ge.as_ref().map(describe_value),
                lt: filter.lt.as_ref().map(describe_value),
                le: filter.le.as_ref().map(describe_value),
                is_null: filter.is_null,
            }).collect::<Vec<_>>();
            rows.sort_by(|left, right| left.field.cmp(&right.field));
            rows
        }),
        rejected,
        duplicate_paths: seen,
        operator_vocabulary: vec!["eq".to_owned(), "ne".to_owned(), "gt".to_owned(), "ge".to_owned(),
            "lt".to_owned(), "le".to_owned(), "in".to_owned(), "isNull".to_owned(),
            "isNotNull".to_owned()],
    })
}

/// 编一个条件。 / Compile one condition.
fn compile(condition: &FilterConditionSpec) -> Result<BooleanExpression<ExpressionValue>> {
    let path = condition.path.trim();
    if path.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "predicate field filter extraction failed validation: field paths must not be blank"));
    }
    let field = FieldPath::<ExpressionValue>::parse(path);
    let single = |operator: &str| -> Result<ExpressionValue> {
        condition.value.as_ref().map(parse_value).transpose()?.ok_or_else(|| BridgeError::new(
            "InvalidInput", &format!(
                "predicate field filter extraction failed validation: operator `{operator}` needs a value")))
    };
    let expression = match condition.operator.as_str() {
        "eq" => field.compare(ComparisonOperator::Eq, single("eq")?),
        "ne" => field.compare(ComparisonOperator::Ne, single("ne")?),
        "gt" => field.compare(ComparisonOperator::Gt, single("gt")?),
        "ge" => field.compare(ComparisonOperator::Ge, single("ge")?),
        "lt" => field.compare(ComparisonOperator::Lt, single("lt")?),
        "le" => field.compare(ComparisonOperator::Le, single("le")?),
        "in" => {
            if condition.values.is_empty() {
                return Err(BridgeError::new("InvalidInput",
                    "predicate field filter extraction failed validation: `in` needs at least one value"));
            }
            let values = condition.values.iter().map(parse_value)
                .collect::<Result<Vec<_>>>()?;
            field.in_values(values)
        }
        "isNull" => BooleanExpression::is_null(path),
        "isNotNull" => BooleanExpression::is_not_null(path),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "predicate field filter extraction failed validation: unknown operator `{other}`"))),
    };
    Ok(expression)
}

/// 解析一个值。 / Parse one value.
fn parse_value(value: &FilterValueSpec) -> Result<ExpressionValue> {
    match value.kind.as_str() {
        "null" => Ok(ExpressionValue::Null),
        "boolean" => value.text.trim().parse::<bool>().map(ExpressionValue::Boolean)
            .map_err(|_| BridgeError::new("InvalidInput",
                "predicate field filter extraction failed validation: a boolean value must be `true` or `false`")),
        "number" => value.text.trim().parse::<f64>().map(ExpressionValue::Number)
            .map_err(|_| BridgeError::new("InvalidInput",
                "predicate field filter extraction failed validation: a number value must parse as a float")),
        "string" => Ok(ExpressionValue::String(value.text.clone())),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "predicate field filter extraction failed validation: unknown value kind `{other}`"))),
    }
}

/// 把领域值摊成 `kind` + 字面文本（文本走领域 `Display`，因此 `1.0` 写作 `1`）。
/// Flatten a domain value into `kind` plus literal text (the text goes through the domain `Display`, so `1.0` reads `1`).
fn describe_value(value: &ExpressionValue) -> FilterValueSpec {
    let kind = match value {
        ExpressionValue::Null => "null",
        ExpressionValue::Boolean(_) => "boolean",
        ExpressionValue::Number(_) => "number",
        ExpressionValue::String(_) => "string",
    };
    FilterValueSpec { kind: kind.to_owned(), text: value.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_math::symbol::{PropertyPath, ScalarExpression};

    fn text(kind: &str, value: &str) -> FilterValueSpec {
        FilterValueSpec { kind: kind.to_owned(), text: value.to_owned() }
    }

    fn condition(path: &str, operator: &str, value: Option<FilterValueSpec>) -> FilterConditionSpec {
        FilterConditionSpec { path: path.to_owned(), operator: operator.to_owned(), value,
            values: Vec::new() }
    }

    fn request(combinator: &str, conditions: Vec<FilterConditionSpec>) -> PredicateFieldFilterRequest {
        PredicateFieldFilterRequest { combinator: combinator.to_owned(), conditions }
    }

    #[test]
    fn the_three_extractors_have_different_leniency() {
        // 两个 `eq`：三个提取器都接受。 / Two `eq` conditions: all three extractors accept.
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("status", "eq", Some(text("string", "active"))),
            condition("tier", "eq", Some(text("number", "2"))),
        ])).unwrap();
        assert!(report.rejected.is_empty());
        let eq = report.eq_filters.as_ref().unwrap();
        assert_eq!(eq.len(), 2);
        assert_eq!(eq[0].field, "status");
        assert_eq!(eq[0].value.kind, "string");
        assert_eq!(eq[0].value.text, "active");
        assert_eq!(eq[1].value.text, "2");
        let eq_or_in = report.eq_or_in_filters.as_ref().unwrap();
        assert!(eq_or_in.iter().all(|row| row.kind == "eq"));
        assert_eq!(eq_or_in[0].value.as_ref().unwrap().text, "active");
        let field = report.field_filters.as_ref().unwrap();
        assert_eq!(field[0].eq.as_ref().unwrap().text, "active");

        // 加一个 `gt`：`eq_filters` 与 `eq_or_in_filters` 拒绝，`field_filters` 接受。
        // Add a `gt`: the first two refuse while `field_filters` accepts.
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("status", "eq", Some(text("string", "active"))),
            condition("score", "gt", Some(text("number", "1.5"))),
        ])).unwrap();
        assert_eq!(report.rejected, vec!["eqFilters".to_owned(), "eqOrInFilters".to_owned()]);
        assert_eq!(report.eq_filters, None);
        assert_eq!(report.eq_or_in_filters, None);
        // 行按字段名排序，所以按名字找而不是按下标。 / Rows are sorted by field name, so look them up by name.
        let field = report.field_filters.as_ref().unwrap();
        let score = field.iter().find(|row| row.field == "score").expect("score row");
        assert_eq!(score.gt.as_ref().unwrap().text, "1.5");

        // 加一个 `in`：只有 `eq_filters` 拒绝。 / Add an `in`: only `eq_filters` refuses.
        let mut in_condition = condition("region", "in", None);
        in_condition.values = vec![text("string", "eu"), text("string", "us")];
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("status", "eq", Some(text("string", "active"))), in_condition,
        ])).unwrap();
        assert_eq!(report.rejected, vec!["eqFilters".to_owned()]);
        let eq_or_in = report.eq_or_in_filters.as_ref().unwrap();
        let region = eq_or_in.iter().find(|row| row.field == "region").expect("region row");
        assert_eq!(region.kind, "in");
        assert_eq!(region.values.len(), 2);
        assert_eq!(region.value, None);
        let field = report.field_filters.as_ref().unwrap();
        assert_eq!(field.iter().find(|row| row.field == "region").unwrap().in_values.len(), 2);

        // 空值检查：只有 `field_filters` 接受，且 `isNull` 为真。 / Null checks only reach `field_filters`.
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("deleted_at", "isNull", None),
        ])).unwrap();
        assert_eq!(report.rejected.len(), 2);
        assert_eq!(report.field_filters.as_ref().unwrap()[0].is_null, Some(true));
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("deleted_at", "isNotNull", None),
        ])).unwrap();
        assert_eq!(report.field_filters.as_ref().unwrap()[0].is_null, Some(false));
    }

    #[test]
    fn or_is_refused_by_all_three_and_duplicates_are_silently_overwritten() {
        // `or` 在三个提取器里都落进 `_ => false`。 / `or` falls into `_ => false` in all three.
        let report = verify_predicate_field_filters(&request("or", vec![
            condition("status", "eq", Some(text("string", "active"))),
            condition("tier", "eq", Some(text("number", "2"))),
        ])).unwrap();
        assert_eq!(report.rejected, vec!["eqFilters".to_owned(), "eqOrInFilters".to_owned(),
            "fieldFilters".to_owned()]);
        assert_eq!(report.eq_filters, None);
        assert_eq!(report.eq_or_in_filters, None);
        assert_eq!(report.field_filters, None);

        // 同一字段两次 `eq`：**静默覆盖**，只剩后一个值，但 `duplicatePaths` 把它标出来。
        // Two `eq`s on one field are **silently overwritten** down to the latter value, and `duplicatePaths` surfaces it.
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("status", "eq", Some(text("string", "active"))),
            condition("status", "eq", Some(text("string", "archived"))),
        ])).unwrap();
        let eq = report.eq_filters.as_ref().unwrap();
        assert_eq!(eq.len(), 1, "the domain keeps one entry per path");
        assert_eq!(eq[0].value.text, "archived");
        assert_eq!(report.duplicate_paths, vec!["status".to_owned()]);
        // 不同字段的多个槽位互不覆盖。 / Different slots of one field do not overwrite each other.
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("score", "gt", Some(text("number", "1"))),
            condition("score", "lt", Some(text("number", "9"))),
        ])).unwrap();
        let field = report.field_filters.as_ref().unwrap();
        assert_eq!(field.len(), 1);
        assert_eq!(field[0].gt.as_ref().unwrap().text, "1");
        assert_eq!(field[0].lt.as_ref().unwrap().text, "9");
    }

    #[test]
    fn ne_is_refused_and_eq_null_becomes_a_null_check() {
        // `ne`：`field_filters` 明确拒绝（`Ne => false`）。 / `ne`: `field_filters` refuses it outright.
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("status", "ne", Some(text("string", "active"))),
        ])).unwrap();
        assert_eq!(report.rejected, vec!["eqFilters".to_owned(), "eqOrInFilters".to_owned(),
            "fieldFilters".to_owned()]);
        // `eq` 配 `null`：`field_filters` 落进 `is_null = true`，而不是 `eq = null`。
        // `eq` against `null`: `field_filters` puts it in `is_null = true` rather than `eq = null`.
        let report = verify_predicate_field_filters(&request("and", vec![
            condition("deleted_at", "eq", Some(text("null", ""))),
        ])).unwrap();
        let field = report.field_filters.as_ref().unwrap();
        assert_eq!(field[0].is_null, Some(true));
        assert_eq!(field[0].eq, None);
        // 但前两个提取器仍然把它当等值。 / The first two extractors still treat it as an equality.
        assert_eq!(report.eq_filters.as_ref().unwrap()[0].value.kind, "null");
    }

    #[test]
    fn reversed_comparisons_are_flipped_not_refused() {
        // 直接手搭 `constant op path`：`field_comparison` 会翻转（`1 < a` → `a > 1`）。
        // Hand-build `constant op path`: `field_comparison` flips it (`1 < a` becomes `a > 1`).
        let expression: BooleanExpression<ExpressionValue> = BooleanExpression::Comparison {
            operator: ComparisonOperator::Lt,
            left: ScalarExpression::constant(ExpressionValue::Number(1.0)),
            right: ScalarExpression::reference(PropertyPath::parse("a")),
        };
        let filters = expression.field_filters().unwrap();
        let row = filters.get("a").expect("the flipped form still names the field");
        assert_eq!(row.gt.as_ref().map(|value| value.to_string()), Some("1".to_owned()));
        assert_eq!(row.lt, None);
        // 反向的 `eq` 仍然是 `eq`（翻转 `Eq` 还是 `Eq`）。 / A reversed `eq` stays an `eq`.
        let expression: BooleanExpression<ExpressionValue> = BooleanExpression::Comparison {
            operator: ComparisonOperator::Eq,
            left: ScalarExpression::constant(ExpressionValue::String("hot".to_owned())),
            right: ScalarExpression::reference(PropertyPath::parse("temp")),
        };
        assert_eq!(expression.field_filters().unwrap().get("temp").unwrap().eq
            .as_ref().map(|value| value.to_string()), Some("hot".to_owned()));
    }

    #[test]
    fn empty_and_malformed_requests_are_refused_before_the_domain_panics() {
        // 空条件：`BooleanExpression::and` 会 panic，所以入口先拒。 / Empty conditions would panic downstream.
        assert!(verify_predicate_field_filters(&request("and", Vec::new())).unwrap_err().message
            .contains("at least one condition"));
        assert!(verify_predicate_field_filters(&request("or", Vec::new())).is_err());
        // 空白路径 / 未知操作符 / 未知组合子。 / A blank path, an unknown operator, an unknown combinator.
        assert!(verify_predicate_field_filters(&request("and", vec![
            condition(" ", "eq", Some(text("string", "x"))),
        ])).unwrap_err().message.contains("paths must not be blank"));
        assert!(verify_predicate_field_filters(&request("and", vec![
            condition("a", "like", Some(text("string", "x"))),
        ])).unwrap_err().message.contains("unknown operator"));
        assert!(verify_predicate_field_filters(&request("xor", vec![
            condition("a", "eq", Some(text("string", "x"))),
        ])).unwrap_err().message.contains("unknown combinator"));
        // 缺值 / `in` 空列表 / 坏值。 / A missing value, an empty `in`, a malformed value.
        assert!(verify_predicate_field_filters(&request("and", vec![
            condition("a", "eq", None),
        ])).unwrap_err().message.contains("needs a value"));
        assert!(verify_predicate_field_filters(&request("and", vec![
            condition("a", "in", None),
        ])).unwrap_err().message.contains("at least one value"));
        assert!(verify_predicate_field_filters(&request("and", vec![
            condition("a", "eq", Some(text("boolean", "yes"))),
        ])).unwrap_err().message.contains("`true` or `false`"));
        assert!(verify_predicate_field_filters(&request("and", vec![
            condition("a", "eq", Some(text("number", "one"))),
        ])).unwrap_err().message.contains("parse as a float"));
        assert!(verify_predicate_field_filters(&request("and", vec![
            condition("a", "eq", Some(text("date", "2026-11-06"))),
        ])).unwrap_err().message.contains("unknown value kind"));
        // 条件数上限。 / The condition cap.
        let too_many = (0..=MAX_CONDITIONS)
            .map(|index| condition(&format!("f{index}"), "eq", Some(text("number", "1"))))
            .collect::<Vec<_>>();
        assert!(verify_predicate_field_filters(&request("and", too_many)).unwrap_err().message
            .contains("at most"));
    }
}
