//! 续挖第 10 单：布尔表达式作用域的收集与组合语义（`persistence/expression/boolean_expression_dsl.rs`）
//! Continuation, delivery 10: how the boolean-expression scope collects and combines.
//!
//! `BooleanExpressionScope` 是个很薄的门面：每个 `eq` / `ne` / `gt` / `ge` / `lt` / `le` / `in_values` /
//! `not_in_values` / `is_null` / `is_not_null` / `push` 都**先把表达式塞进作用域、再把它返回**，最后 `build_and` /
//! `build_or` 一次性组合。看着没什么，但三条实测语义决定了空集合与单集合的行为，而且很容易被想当然搞错：
//!
//! 1. **空作用域的组合有恒等元**：`build_and` 对零个表达式给出 `Constant(True)`，`build_or` 给出 `Constant(False)`。
//!    "没有条件"在两个方向上的含义**正好相反**，这不是细节——把两者混同会让"无条件过滤"变成"全拒绝"。
//! 2. **单个表达式不会被包一层**：`build_and` / `build_or` 在恰好一个操作数时**直接返回那个表达式**，不出现
//!    `And([x])` / `Or([x])`。所以"结果形状"能反推作用域里有几条。
//! 3. **`add` 推入的是克隆**：收集方法既返回表达式又把它复制一份进作用域，所以"返回的表达式"与"作用域里的表达式"
//!    是两份。这一层只观察结果，不去依赖它们的指针关系。
//!
//! 报告把每一步的**初步形状**也给出（`items[i].shape`），这样"`not_in_values` 真的是 `Not(In(...))` 而不是 `In`
//! 取反的另一个节点"这种问题可以直接断言。
//!
//! The `BooleanExpressionScope` is a thin facade: every collector **pushes the expression into the scope and returns it**,
//! then `build_and` / `build_or` combines once at the end. Three measured semantics decide the empty and single cases and are
//! easy to get backwards: **an empty scope has identity elements** — `build_and` over zero expressions yields
//! `Constant(True)` and `build_or` yields `Constant(False)`, so "no conditions" means **opposite** things in the two
//! directions, and conflating them turns "unfiltered" into "reject everything"; **a single expression is never wrapped**, since
//! both builders return that expression directly rather than `And([x])` / `Or([x])`, so the result's shape reveals how many
//! items the scope held; and **`add` pushes a clone**, so the returned expression and the stored one are two copies — this
//! layer only observes results and never relies on their pointer relation. Each step's **step shape** is reported too, so
//! "`not_in_values` is really `Not(In(...))` and not some other negated node" is directly assertable.

use ospf_rust_framework::persistence::expression::BooleanExpressionScope;
use ospf_rust_math::symbol::{BooleanExpression, ComparisonOperator, ExpressionValue, PropertyPath};
use ospf_rust_math::Trivalent;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个字面值。 / One literal value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeValue {
    /// `null` / `boolean` / `number` / `string`。 / One of `null`, `boolean`, `number`, `string`.
    pub kind: String,
    /// 字面文本（`null` 时忽略）。 / The literal text (ignored for `null`).
    #[serde(default)]
    pub text: String,
}

/// 一次收集。 / One collection step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeStep {
    /// `eq` / `ne` / `gt` / `ge` / `lt` / `le` / `in` / `notIn` / `isNull` / `isNotNull`。
    /// The collector.
    pub collector: String,
    /// 字段路径（`compare` 用不到，`isNull` / `isNotNull` 用它当目标）。 / The field path.
    #[serde(default)]
    pub path: String,
    /// 单值操作数的值。 / The value for a single-value collector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<ScopeValue>,
    /// `in` / `notIn` 的值列表。 / The values for `in` / `notIn`.
    #[serde(default)]
    pub values: Vec<ScopeValue>,
}

/// 作用域请求。 / The scope request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BooleanScopeRequest {
    /// 收集步骤（按顺序）。 / The collection steps, in order.
    #[serde(default)]
    pub steps: Vec<ScopeStep>,
    /// `and` 或 `or`。 / `and` or `or`.
    pub build: String,
}

/// 一步的结论。 / One step's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeStepRow {
    /// 收集器名。 / The collector name.
    pub collector: String,
    /// 该收集器**返回**的表达式的形状。 / The shape of the expression the collector **returned**.
    pub shape: String,
    /// 形状里的操作数个数（`single` 为 0）。 / The operand count inside that shape (0 for `single`).
    pub operand_count: u32,
}

/// 作用域结论。 / The scope verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BooleanScopeReport {
    /// 收集步骤数。 / The number of collection steps.
    pub step_count: u32,
    /// 组合方向（`and` / `or`）。 / The combine direction.
    pub build: String,
    /// 组合后的形状。 / The combined shape.
    pub shape: String,
    /// 组合后的操作数个数（`single` / 常量时为 0）。 / The combined operand count (0 for `single` or a constant).
    pub operand_count: u32,
    /// 组合结果是否是恒真常量。 / Whether the result is the constant true.
    pub is_constant_true: bool,
    /// 组合结果是否是恒假常量。 / Whether the result is the constant false.
    pub is_constant_false: bool,
    /// 逐步结论。 / Each step's verdict.
    pub steps: Vec<ScopeStepRow>,
    /// **空作用域**在 `and` 方向上的形状（恒真）。 / The shape of an **empty** scope under `and`.
    pub empty_and_shape: String,
    /// **空作用域**在 `or` 方向上的形状（恒假）。 / The shape of an **empty** scope under `or`.
    pub empty_or_shape: String,
    /// 单表达式是否被包了一层（**否**）。 / Whether a single expression gets wrapped (**no**).
    pub single_is_wrapped: bool,
    /// 收集器词表（桥接定义）。 / The collector vocabulary (bridge-defined).
    pub collector_vocabulary: Vec<String>,
}

/// 收集步数上限（信任边界）。 / The step cap (a trust boundary).
const MAX_STEPS: u32 = 1_024;

/// 收集驱动；`build` 决定最后合成成什么。 / Drive the collectors and combine them.
pub fn verify_boolean_scope(request: &BooleanScopeRequest) -> Result<BooleanScopeReport> {
    if u32::try_from(request.steps.len()).unwrap_or(u32::MAX) > MAX_STEPS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "boolean scope failed validation: at most {MAX_STEPS} steps are accepted")));
    }
    if !matches!(request.build.as_str(), "and" | "or") {
        return Err(BridgeError::new("InvalidInput", &format!(
            "boolean scope failed validation: unknown build direction `{}`", request.build)));
    }
    let mut scope = BooleanExpressionScope::<ExpressionValue>::new();
    let mut steps = Vec::with_capacity(request.steps.len());
    for step in &request.steps {
        let produced = apply(&mut scope, step)?;
        let (shape, operand_count) = classify(&produced);
        steps.push(ScopeStepRow { collector: step.collector.clone(), shape, operand_count });
    }
    // 组合方向决定恒等元：空 `and` 是恒真、空 `or` 是恒假。 / The direction picks the identity element.
    let combined = if request.build == "and" { scope.build_and() } else { scope.build_or() };
    let (shape, operand_count) = classify(&combined);
    let (is_constant_true, is_constant_false) = match &combined {
        BooleanExpression::Constant(value) => (
            *value == Trivalent::True, *value == Trivalent::False),
        _ => (false, false),
    };
    let empty_and = BooleanExpressionScope::<ExpressionValue>::new().build_and();
    let empty_or = BooleanExpressionScope::<ExpressionValue>::new().build_or();
    // 单表达式不包一层：用同一个比较分别过两种方向来观察。 / A single expression is never wrapped.
    let mut single = BooleanExpressionScope::<ExpressionValue>::new();
    // 用 `compare` 而不是 `eq`：后者与 `PartialEq::eq` 同名，在这个位置会被解析成前者。
    // Use `compare` rather than `eq`: the latter shares a name with `PartialEq::eq` and resolves to it here.
    single.compare(PropertyPath::parse("probe"), ComparisonOperator::Eq,
        ExpressionValue::Number(1.0));
    let (single_and_shape, single_and_operands) = classify(&single.build_and());
    let mut single_or = BooleanExpressionScope::<ExpressionValue>::new();
    single_or.compare(PropertyPath::parse("probe"), ComparisonOperator::Eq,
        ExpressionValue::Number(1.0));
    let (single_or_shape, single_or_operands) = classify(&single_or.build_or());
    Ok(BooleanScopeReport {
        step_count: u32::try_from(steps.len()).unwrap_or(u32::MAX),
        build: request.build.clone(),
        shape,
        operand_count,
        is_constant_true,
        is_constant_false,
        steps,
        empty_and_shape: classify(&empty_and).0,
        empty_or_shape: classify(&empty_or).0,
        // 单条表达式**不该**被包成 `and(1)` / `or(1)`；包了才说明领域行为变了。
        // A single expression must **not** become `and(1)` / `or(1)`; wrapping would mean the domain changed.
        single_is_wrapped: (single_and_shape == "and" || single_and_shape == "or")
            && single_and_operands == 1
            || (single_or_shape == "and" || single_or_shape == "or") && single_or_operands == 1,
        collector_vocabulary: vec!["eq".to_owned(), "ne".to_owned(), "gt".to_owned(),
            "ge".to_owned(), "lt".to_owned(), "le".to_owned(), "in".to_owned(),
            "notIn".to_owned(), "isNull".to_owned(), "isNotNull".to_owned()],
    })
}

/// 执行一步收集并返回该收集器**返回**的表达式。 / Run one collector and return the expression it returns.
fn apply<'a>(scope: &'a mut BooleanExpressionScope<ExpressionValue>, step: &ScopeStep)
    -> Result<BooleanExpression<ExpressionValue>>
{
    let path = step.path.trim();
    let single = || -> Result<ExpressionValue> {
        step.value.as_ref().map(literal).transpose()?.ok_or_else(|| BridgeError::new(
            "InvalidInput", &format!(
                "boolean scope failed validation: collector `{}` needs a value", step.collector)))
    };
    let list = || -> Result<Vec<ExpressionValue>> {
        if step.values.is_empty() {
            return Err(BridgeError::new("InvalidInput", &format!(
                "boolean scope failed validation: collector `{}` needs at least one value",
                step.collector)));
        }
        step.values.iter().map(literal).collect()
    };
    // 每个收集器都要一个非空字段路径。 / Every collector needs a non-blank field path.
    if path.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "boolean scope failed validation: collectors need a field path"));
    }
    Ok(match step.collector.as_str() {
        "eq" => scope.compare(path, ComparisonOperator::Eq, single()?),
        "ne" => scope.compare(path, ComparisonOperator::Ne, single()?),
        "gt" => scope.compare(path, ComparisonOperator::Gt, single()?),
        "ge" => scope.compare(path, ComparisonOperator::Ge, single()?),
        "lt" => scope.compare(path, ComparisonOperator::Lt, single()?),
        "le" => scope.compare(path, ComparisonOperator::Le, single()?),
        "in" => scope.in_values(path, list()?),
        "notIn" => scope.not_in_values(path, list()?),
        "isNull" => scope.is_null(path),
        "isNotNull" => scope.is_not_null(path),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "boolean scope failed validation: unknown collector `{other}`"))),
    })
}

/// 把表达式归成可断言的形状。 / Classify an expression into an assertable shape.
fn classify(expression: &BooleanExpression<ExpressionValue>) -> (String, u32) {
    match expression {
        BooleanExpression::Constant(Trivalent::True) => ("true".to_owned(), 0),
        BooleanExpression::Constant(Trivalent::False) => ("false".to_owned(), 0),
        BooleanExpression::Constant(Trivalent::Unknown) => ("unknown".to_owned(), 0),
        BooleanExpression::And(operands) => ("and".to_owned(), count(operands.len())),
        BooleanExpression::Or(operands) => ("or".to_owned(), count(operands.len())),
        BooleanExpression::Not(_) => ("not".to_owned(), 1),
        BooleanExpression::Comparison { .. } => ("comparison".to_owned(), 2),
        // **`not_in_values` 是 `In { negated: true }`，不是 `Not(In(...))`**——取反是节点自带的标志。
        // `not_in_values` yields `In { negated: true }`, not `Not(In(...))`: negation is a flag on the node.
        BooleanExpression::In { negated, .. } => (
            if *negated { "inNegated" } else { "in" }.to_owned(), 1),
        BooleanExpression::PatternMatch { .. } => ("patternMatch".to_owned(), 2),
        BooleanExpression::NullCheck { .. } => ("nullCheck".to_owned(), 1),
        BooleanExpression::Custom { .. } => ("custom".to_owned(), 0),
    }
}

/// 计数转 `u32`。 / A count as `u32`.
fn count(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

/// 解析一个字面值。 / Parse one literal value.
fn literal(value: &ScopeValue) -> Result<ExpressionValue> {
    match value.kind.as_str() {
        "null" => Ok(ExpressionValue::Null),
        "boolean" => value.text.trim().parse::<bool>().map(ExpressionValue::Boolean)
            .map_err(|_| BridgeError::new("InvalidInput",
                "boolean scope failed validation: a boolean must be `true` or `false`")),
        "number" => value.text.trim().parse::<f64>().map(ExpressionValue::Number)
            .map_err(|_| BridgeError::new("InvalidInput",
                "boolean scope failed validation: a number must parse as a float")),
        "string" => Ok(ExpressionValue::String(value.text.clone())),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "boolean scope failed validation: unknown value kind `{other}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> ScopeValue {
        ScopeValue { kind: "number".to_owned(), text: text.to_owned() }
    }

    fn step(collector: &str, path: &str) -> ScopeStep {
        ScopeStep { collector: collector.to_owned(), path: path.to_owned(), value: None,
            values: Vec::new() }
    }

    fn with_value(collector: &str, path: &str, text: &str) -> ScopeStep {
        ScopeStep { collector: collector.to_owned(), path: path.to_owned(),
            value: Some(number(text)), values: Vec::new() }
    }

    fn request(build: &str, steps: Vec<ScopeStep>) -> BooleanScopeRequest {
        BooleanScopeRequest { steps, build: build.to_owned() }
    }

    #[test]
    fn empty_scope_is_true_under_and_and_false_under_or() {
        // 空作用域的恒等元：`and` 恒真、`or` 恒假——两个方向**正好相反**。
        // The identity elements: empty `and` is true and empty `or` is false — **opposite** directions.
        let report = verify_boolean_scope(&request("and", Vec::new())).unwrap();
        assert_eq!(report.step_count, 0);
        assert_eq!(report.shape, "true");
        assert!(report.is_constant_true);
        assert!(!report.is_constant_false);
        assert_eq!(report.empty_and_shape, "true");
        assert_eq!(report.empty_or_shape, "false");
        let report = verify_boolean_scope(&request("or", Vec::new())).unwrap();
        assert_eq!(report.shape, "false");
        assert!(report.is_constant_false);
        assert!(!report.is_constant_true);
        // 单表达式不包一层，两个方向都一样。 / A single expression is not wrapped in either direction.
        assert!(!report.single_is_wrapped);
    }

    #[test]
    fn one_expression_is_returned_bare_and_many_are_combined() {
        // 单条：形状是 `comparison` 而不是 `and(1)`。 / One item: `comparison`, not `and(1)`.
        let report = verify_boolean_scope(&request("and", vec![with_value("eq", "status", "1")]))
            .unwrap();
        assert_eq!(report.step_count, 1);
        assert_eq!(report.shape, "comparison");
        assert_eq!(report.steps[0].shape, "comparison");
        assert!(!report.is_constant_true);
        assert!(!report.is_constant_false);
        // 两条：`and(2)` / `or(2)`。 / Two items: `and(2)` and `or(2)`.
        let report = verify_boolean_scope(&request("and", vec![with_value("eq", "a", "1"),
            with_value("gt", "b", "2")])).unwrap();
        assert_eq!(report.shape, "and");
        assert_eq!(report.operand_count, 2);
        assert_eq!(report.steps.len(), 2);
        assert_eq!(report.steps[1].collector, "gt");
        let report = verify_boolean_scope(&request("or", vec![with_value("eq", "a", "1"),
            with_value("gt", "b", "2")])).unwrap();
        assert_eq!(report.shape, "or");
        assert_eq!(report.operand_count, 2);
    }

    #[test]
    fn every_collector_reports_its_own_shape() {
        let mut in_list = step("in", "region");
        in_list.values = vec![number("1"), number("2")];
        let mut not_in_list = step("notIn", "region");
        not_in_list.values = vec![number("1")];
        let report = verify_boolean_scope(&request("and", vec![
            with_value("eq", "a", "1"),
            with_value("ne", "a", "2"),
            with_value("gt", "a", "3"),
            with_value("ge", "a", "4"),
            with_value("lt", "a", "5"),
            with_value("le", "a", "6"),
            in_list,
            not_in_list,
            step("isNull", "deleted_at"),
            step("isNotNull", "archived_at"),
        ])).unwrap();
        let shape_of = |name: &str| report.steps.iter()
            .find(|row| row.collector == name).map(|row| row.shape.clone())
            .unwrap_or_else(|| panic!("collector {name}"));
        // 六种比较都是 `comparison`（左路径、右常量）。 / All six comparisons are `comparison`.
        for name in ["eq", "ne", "gt", "ge", "lt", "le"] {
            assert_eq!(shape_of(name), "comparison", "{name}");
        }
        assert_eq!(shape_of("in"), "in");
        // **实测纠正**：`not_in_values` 产出的是 `In { negated: true }`，不是 `Not(In(...))`——取反是节点自带的
        // 标志，所以形状里要能和普通 `in` 分开。
        // **Measured correction**: `not_in_values` yields `In { negated: true }` rather than `Not(In(...))`, so the shape
        // must distinguish it from a plain `in`.
        assert_eq!(shape_of("notIn"), "inNegated");
        assert_ne!(shape_of("notIn"), shape_of("in"));
        assert_eq!(shape_of("isNull"), "nullCheck");
        assert_eq!(shape_of("isNotNull"), "nullCheck");
        assert_eq!(report.collector_vocabulary.len(), 10);
        // 十条一起组合成 `and(10)`。 / Ten items combine into `and(10)`.
        assert_eq!(report.shape, "and");
        assert_eq!(report.operand_count, 10);
    }

    #[test]
    fn bad_input_is_refused() {
        // 未知收集器 / 未知方向。 / An unknown collector and an unknown direction.
        assert!(verify_boolean_scope(&request("and", vec![step("between", "a")])).unwrap_err().message
            .contains("unknown collector"));
        assert!(verify_boolean_scope(&request("xor", Vec::new())).unwrap_err().message
            .contains("unknown build direction"));
        // 空白路径 / 缺值 / `in` 空列表 / 坏字面值。 / A blank path, a missing value, an empty `in`, a bad literal.
        assert!(verify_boolean_scope(&request("and", vec![step("eq", "  ")])).unwrap_err().message
            .contains("need a field path"));
        assert!(verify_boolean_scope(&request("and", vec![step("eq", "a")])).unwrap_err().message
            .contains("needs a value"));
        assert!(verify_boolean_scope(&request("and", vec![step("in", "a")])).unwrap_err().message
            .contains("at least one value"));
        let mut bad = with_value("eq", "a", "one");
        bad.value = Some(number("one"));
        assert!(verify_boolean_scope(&request("and", vec![bad])).unwrap_err().message
            .contains("parse as a float"));
        let mut boolean = with_value("eq", "a", "");
        boolean.value = Some(ScopeValue { kind: "boolean".to_owned(), text: "yes".to_owned() });
        assert!(verify_boolean_scope(&request("and", vec![boolean])).unwrap_err().message
            .contains("`true` or `false`"));
        let mut date = with_value("eq", "a", "");
        date.value = Some(ScopeValue { kind: "date".to_owned(), text: "today".to_owned() });
        assert!(verify_boolean_scope(&request("and", vec![date])).unwrap_err().message
            .contains("unknown value kind"));
        // 上限。 / The cap.
        let many = (0..=MAX_STEPS).map(|index| with_value("eq", &format!("f{index}"), "1"))
            .collect::<Vec<_>>();
        assert!(verify_boolean_scope(&request("and", many)).unwrap_err().message.contains("at most"));
    }
}
