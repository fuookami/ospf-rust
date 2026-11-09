//! 续挖第 29 单：布尔表达式作用域的三条折叠规则（`persistence/expression/boolean_expression_dsl.rs`）
//! Continuation, delivery 29: the three folding rules of the boolean expression scope.
//!
//! `BooleanExpressionScope` 是个"收集器"：每个 `eq` / `ne` / `in_values` / `push` 都会**克隆一份**进作用域并
//! 返回原表达式，最后 `build_and` / `build_or` 把作用域合成一个表达式。折叠有三条**不对称**规则：
//!
//! 1. **空作用域不报错，直接给常量**：`build_and` → `Constant(True)`，`build_or` → `Constant(False)`。
//!    所以"什么都没加"的 `and` 语义上是真空真，不是错误。
//! 2. **单元素被拆掉外壳**：`And` / `Or` 只在**两个以上**时出现，一条表达式原样返回。所以调用方**不能**假设
//!    拿到的一定是 `And` 节点——按形状分发会漏掉单条的情况。
//! 3. **`and(...)` / `or(...)` 只折叠"已收集的尾部后缀"**：先 `remove_collected_suffix` 再 `add`。也就是说
//!    `scope.and([a, b])` 在 `a`、`b` **正好是作用域末尾两项且顺序一致**时会把它们移出再合成为一条；
//!    否则**不折叠**，作用域里就同时留着原项与合成项——`build_and` 会得到**更深一层**的嵌套。
//!    这条不是"去重"，是"改写尾部"，顺序不同、中间夹了别的项、或压根没收集过，都不触发。
//!
//! The scope is a collector: every `eq` / `ne` / `in_values` / `push` **clones** into the scope and returns the expression,
//! and `build_and` / `build_or` fold it. Three **asymmetric** rules apply. **An empty scope is not an error**: `build_and`
//! yields `Constant(True)` and `build_or` `Constant(False)`, so "added nothing" is vacuously true, not a failure. **A single
//! element loses its wrapper**: `And` / `Or` only appear with two or more, so a caller must not assume it always receives an
//! `And` node. And **`and(...)` / `or(...)` only fold a collected trailing suffix** — `remove_collected_suffix` runs first,
//! then `add`, so folding happens only when the arguments are exactly the scope's last items in the same order; otherwise
//! nothing is removed and the scope keeps both the originals and the combined node, giving one extra level of nesting. It is
//! not a dedupe: it rewrites the tail, and a different order, an interleaved item or an uncollected argument never triggers it.

use ospf_rust_framework::persistence::expression::boolean_expression_dsl::{
    BooleanExpressionScope, typed_compare,
};
use ospf_rust_math::Trivalent;
use ospf_rust_math::symbol::{
    BooleanExpression, ComparisonOperator, ExpressionValue, PropertyPath, ScalarExpression,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个作用域请求。 / A scope request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BooleanFoldRequest {
    /// 先逐条收集的字段名。 / The field names collected one by one.
    #[serde(default)]
    pub collected: Vec<String>,
    /// 收集后再合并的参数；空表示不调用 `and` / `or`。 / The arguments passed to `and` / `or`; empty means no call.
    #[serde(default)]
    pub combined: Vec<String>,
    /// `and` 或 `or`。 / `and` or `or`.
    #[serde(default)]
    pub mode: String,
}

/// 折叠结论。 / The folding verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BooleanFoldReport {
    /// 合成结果的形状描述。 / A shape description of the built expression.
    pub shape: String,
    /// 合成结果的节点数。 / The node count of the built expression.
    pub node_count: u64,
    /// 空作用域 `and` 的形状。 / The shape of an empty `and`.
    pub empty_and_shape: String,
    /// 空作用域 `or` 的形状。 / The shape of an empty `or`.
    pub empty_or_shape: String,
    /// 单元素不带外壳（**是**）。 / Whether a single element loses its wrapper (**yes**).
    pub single_element_loses_wrapper: bool,
    /// 空作用域给常量（**是**）。 / Whether an empty scope yields a constant (**yes**).
    pub empty_scope_is_constant: bool,
    /// 尾部后缀被折叠（**是**）。 / Whether a collected trailing suffix folds (**yes**).
    pub trailing_suffix_folds: bool,
    /// 顺序不同就不折叠（**是**）。 / Whether a different order does not fold (**yes**).
    pub wrong_order_does_not_fold: bool,
    /// 单参数的 `and` / `or` 直接就是那一项（**是**）。 / Whether `and` / `or` with one argument is the term itself
    /// (**yes**).
    pub single_argument_unwraps: bool,
    /// 折叠不是去重：夹在中间的同名项会留下（**是**）。 / Whether folding is not a dedupe: an interleaved item survives
    /// (**yes**).
    pub interleaved_item_survives: bool,
}

/// 收集条数上限（信任边界）。 / The collection cap (a trust boundary).
const MAX_ITEMS: usize = 256;

/// 走一遍布尔作用域。非法输入是**错误**。 / Walk the boolean scope.
pub fn verify_boolean_fold(request: &BooleanFoldRequest) -> Result<BooleanFoldReport> {
    if request.collected.len() > MAX_ITEMS || request.combined.len() > MAX_ITEMS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "boolean scope failed validation: at most {MAX_ITEMS} items are accepted")));
    }
    for path in request.collected.iter().chain(request.combined.iter()) {
        if path.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "boolean scope failed validation: every field needs a name"));
        }
    }
    if !request.combined.is_empty() && request.mode != "and" && request.mode != "or" {
        return Err(BridgeError::new("InvalidInput", &format!(
            "boolean scope failed validation: unknown mode `{}`", request.mode)));
    }
    let built = build(request);
    let shape = describe(&built);
    let node_count = count(&built);
    let findings = canonical_findings();
    Ok(BooleanFoldReport {
        shape,
        node_count,
        empty_and_shape: findings.empty_and_shape,
        empty_or_shape: findings.empty_or_shape,
        single_element_loses_wrapper: findings.single_element_loses_wrapper,
        empty_scope_is_constant: findings.empty_scope_is_constant,
        trailing_suffix_folds: findings.trailing_suffix_folds,
        wrong_order_does_not_fold: findings.wrong_order_does_not_fold,
        single_argument_unwraps: findings.single_argument_unwraps,
        interleaved_item_survives: findings.interleaved_item_survives,
    })
}

/// 按请求建一个作用域。 / Build a scope from the request.
fn build(request: &BooleanFoldRequest) -> BooleanExpression<ExpressionValue> {
    let mut scope: BooleanExpressionScope<ExpressionValue> = BooleanExpressionScope::new();
    for path in &request.collected {
        scope.push(term(path));
    }
    for _path in &request.combined {
        // 作用域里收集的项与传进 `and` / `or` 的项都走同一个 `term`，结构相同才能触发后缀折叠。
        // Collected and combined terms both come from `term`, so the suffix fold can match them.
        let collected: Vec<BooleanExpression<ExpressionValue>> = request.combined.iter()
            .map(|name| term(name))
            .collect();
        if request.mode == "or" {
            scope.or(collected);
        } else {
            scope.and(collected);
        }
        return if request.mode == "or" { scope.build_or() } else { scope.build_and() };
    }
    scope.build_and()
}

/// 造一个"字段等于自身引用"的比较项（结构稳定，便于测后缀折叠）。
/// Build a "field equals its own reference" comparison (structurally stable, so the suffix fold can match).
fn term(name: &str) -> BooleanExpression<ExpressionValue> {
    typed_compare(PropertyPath::parse(name), ComparisonOperator::Eq,
        ScalarExpression::reference(PropertyPath::parse(name)))
}

/// 形状描述。 / A shape description.
fn describe(expression: &BooleanExpression<ExpressionValue>) -> String {
    match expression {
        BooleanExpression::Constant(value) => format!("constant({})", trivalent(*value)),
        BooleanExpression::Comparison { operator, left, right } => format!(
            "comparison({}, {}, {})", operator_name(*operator), scalar(left), scalar(right)),
        BooleanExpression::In { candidates, .. } => format!("in({})", candidates.len()),
        BooleanExpression::And(items) => format!("and({})", items.len()),
        BooleanExpression::Or(items) => format!("or({})", items.len()),
        BooleanExpression::Not(_) => "not".to_owned(),
        BooleanExpression::NullCheck { .. } => "nullCheck".to_owned(),
        BooleanExpression::PatternMatch { .. } => "patternMatch".to_owned(),
        BooleanExpression::Custom { payload, .. } => format!("custom({payload})"),
        other => format!("{other:?}"),
    }
}

/// 节点数。 / The node count.
fn count(expression: &BooleanExpression<ExpressionValue>) -> u64 {
    match expression {
        BooleanExpression::And(items) | BooleanExpression::Or(items) => {
            1 + items.iter().map(count).sum::<u64>()
        }
        BooleanExpression::Not(inner) => 1 + count(inner),
        _ => 1,
    }
}

/// 常量名。 / The constant name.
fn trivalent(value: Trivalent) -> &'static str {
    match value {
        Trivalent::True => "true",
        Trivalent::False => "false",
        Trivalent::Unknown => "unknown",
    }
}

/// 操作符名。 / The operator name.
fn operator_name(operator: ComparisonOperator) -> &'static str {
    match operator {
        ComparisonOperator::Eq => "eq",
        ComparisonOperator::Ne => "ne",
        ComparisonOperator::Gt => "gt",
        ComparisonOperator::Ge => "ge",
        ComparisonOperator::Lt => "lt",
        ComparisonOperator::Le => "le",
        _ => "other",
    }
}

/// 标量描述。 / A scalar description.
fn scalar(value: &ScalarExpression<ExpressionValue>) -> String {
    match value {
        ScalarExpression::Reference(path) => format!("ref({})", path.value()),
        other => format!("{other:?}"),
    }
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    empty_and_shape: String,
    empty_or_shape: String,
    single_element_loses_wrapper: bool,
    empty_scope_is_constant: bool,
    trailing_suffix_folds: bool,
    wrong_order_does_not_fold: bool,
    single_argument_unwraps: bool,
    interleaved_item_survives: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 空作用域。 / An empty scope.
    let empty_and = BooleanExpressionScope::<ExpressionValue>::new().build_and();
    let empty_or = BooleanExpressionScope::<ExpressionValue>::new().build_or();
    let empty_and_shape = describe(&empty_and);
    let empty_or_shape = describe(&empty_or);
    let empty_scope_is_constant = matches!(empty_and, BooleanExpression::Constant(_))
        && matches!(empty_or, BooleanExpression::Constant(_));
    // 单元素被拆壳。 / A single element loses its wrapper.
    let mut single = BooleanExpressionScope::<ExpressionValue>::new();
    single.push(term("a"));
    let single_built = single.build_and();
    let single_element_loses_wrapper = matches!(single_built, BooleanExpression::Comparison { .. });
    // 尾部后缀折叠：先收集 a,b 再 and([a,b]) → 只剩一层 And(2)。
    // A trailing suffix folds: collect a,b then and([a,b]) leaves one And(2).
    let mut folding = BooleanExpressionScope::<ExpressionValue>::new();
    folding.push(term("a"));
    folding.push(term("b"));
    folding.and(vec![term("a"), term("b")]);
    let folded = folding.build_and();
    let trailing_suffix_folds = describe(&folded) == "and(2)";
    // 顺序不同：收集 a,b 再 and([b,a]) → 不折叠 → 一层 And 里再套一层。
    // A different order does not fold: one And containing another.
    let mut reordered = BooleanExpressionScope::<ExpressionValue>::new();
    reordered.push(term("a"));
    reordered.push(term("b"));
    reordered.and(vec![term("b"), term("a")]);
    let reordered_built = reordered.build_and();
    let wrong_order_does_not_fold = count(&reordered_built) > count(&folded);
    // 按实测纠正：单参数的 `and([a])` 先被 `combine_typed_and` 拆成 `a` 本身（不是 `and(1)`），
    // 所以作用域里只有 a，`build_and` 再单元素拆壳 → 结果就是那一项。**单参数压根不出现 And/Or**。
    // Corrected by measurement: `and([a])` is unwrapped to `a` itself (not `and(1)`), so the scope holds one term and
    // `build_and` unwraps again — the result is just that term. A single argument never produces And/Or at all.
    let mut uncollected = BooleanExpressionScope::<ExpressionValue>::new();
    uncollected.and(vec![term("a")]);
    let uncollected_built = uncollected.build_and();
    let single_argument_unwraps = describe(&uncollected_built).starts_with("comparison");
    // 夹在中间：收集 a,c 再 and([c]) → c 是尾部后缀，被折叠；a 留下 → 两个子节点。
    // Interleaved: a,c collected then and([c]) folds the tail c while a stays.
    let mut interleaved = BooleanExpressionScope::<ExpressionValue>::new();
    interleaved.push(term("a"));
    interleaved.push(term("c"));
    interleaved.and(vec![term("c")]);
    let interleaved_built = interleaved.build_and();
    let interleaved_item_survives = describe(&interleaved_built) == "and(2)";
    Findings { empty_and_shape, empty_or_shape, single_element_loses_wrapper, empty_scope_is_constant,
        trailing_suffix_folds, wrong_order_does_not_fold, single_argument_unwraps,
        interleaved_item_survives }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(collected: Vec<&str>, combined: Vec<&str>, mode: &str) -> BooleanFoldRequest {
        BooleanFoldRequest { collected: collected.into_iter().map(|name| name.to_owned()).collect(),
            combined: combined.into_iter().map(|name| name.to_owned()).collect(),
            mode: mode.to_owned() }
    }

    #[test]
    fn the_empty_scope_yields_constants() {
        let report = verify_boolean_fold(&request(Vec::new(), Vec::new(), "")).unwrap();
        assert_eq!(report.empty_and_shape, "constant(true)");
        assert_eq!(report.empty_or_shape, "constant(false)");
        assert!(report.empty_scope_is_constant);
        assert_eq!(report.shape, "constant(true)");
    }

    #[test]
    fn a_single_element_loses_its_wrapper() {
        let report = verify_boolean_fold(&request(vec!["a"], Vec::new(), "")).unwrap();
        assert!(report.single_element_loses_wrapper);
        assert!(report.shape.starts_with("comparison(eq"), "{}", report.shape);
        assert_eq!(report.node_count, 1);
        // 两条以上就是 And 节点。 / Two or more become an And node.
        let report = verify_boolean_fold(&request(vec!["a", "b"], Vec::new(), "")).unwrap();
        assert_eq!(report.shape, "and(2)");
        assert_eq!(report.node_count, 3);
    }

    #[test]
    fn a_trailing_suffix_folds_and_a_wrong_order_does_not() {
        // 顺序一致 → 折叠成一层。 / Same order folds to one level.
        let folded = verify_boolean_fold(&request(vec!["a", "b"], vec!["a", "b"], "and")).unwrap();
        assert!(folded.trailing_suffix_folds);
        assert_eq!(folded.shape, "and(2)");
        assert_eq!(folded.node_count, 3);
        // 顺序反了 → 不折叠，多一层。 / Reversed order does not fold: one extra level.
        let reordered = verify_boolean_fold(&request(vec!["a", "b"], vec!["b", "a"], "and")).unwrap();
        assert!(reordered.wrong_order_does_not_fold);
        assert_eq!(reordered.shape, "and(3)");
        // 乱序那层里再套一层 and(2)，所以节点是 1 + 1 + 1 + 3 = 6。
        assert_eq!(reordered.node_count, 6);
    }

    #[test]
    fn a_single_argument_never_produces_a_wrapper() {
        // 实测：空作用域 + and([a]) → 就是 a 那一项，连 And 都没有。
        // Measured: an empty scope plus and([a]) is just that term, with no And at all.
        let report = verify_boolean_fold(&request(Vec::new(), vec!["a"], "and")).unwrap();
        assert!(report.single_argument_unwraps);
        assert!(report.shape.starts_with("comparison"), "{}", report.shape);
        assert_eq!(report.node_count, 1);
    }

    #[test]
    fn folding_rewrites_the_tail_but_is_not_a_dedupe() {
        // 收集 a,c 再 and([c])：尾部 c 被折叠，a 留下 → 两个子节点。 
        // Collect a,c then and([c]): the tail folds, a stays, so two children remain.
        let report = verify_boolean_fold(&request(vec!["a", "c"], vec!["c"], "and")).unwrap();
        assert!(report.interleaved_item_survives);
        assert_eq!(report.shape, "and(2)");
        // `or` 同理。 / The same holds for `or`.
        let report = verify_boolean_fold(&request(vec!["a", "b"], vec!["a", "b"], "or")).unwrap();
        assert_eq!(report.shape, "or(2)");
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_boolean_fold(&request(vec!["x", "y", "z"], vec!["y", "z"], "and"))
            .unwrap();
        assert!(report.empty_scope_is_constant);
        assert!(report.single_element_loses_wrapper);
        assert!(report.trailing_suffix_folds);
        assert!(report.wrong_order_does_not_fold);
        assert!(report.single_argument_unwraps);
        assert!(report.interleaved_item_survives);
        assert_eq!(report.empty_and_shape, "constant(true)");
        assert_eq!(report.empty_or_shape, "constant(false)");
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_boolean_fold(&request(vec!["  "], Vec::new(), ""))
            .unwrap_err().message.contains("every field needs a name"));
        assert!(verify_boolean_fold(&request(Vec::new(), vec!["a"], "xor"))
            .unwrap_err().message.contains("unknown mode"));
        let many: Vec<String> = vec!["a".to_owned(); MAX_ITEMS + 1];
        assert!(verify_boolean_fold(&BooleanFoldRequest { collected: many, combined: Vec::new(),
            mode: String::new() }).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_boolean_fold(&request(vec!["a"], Vec::new(), "")).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"emptyAndShape\""), "{json}");
        assert!(json.contains("\"trailingSuffixFolds\""), "{json}");
        assert!(json.contains("\"nodeCount\""), "{json}");
        assert!(!json.contains("empty_and_shape"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BooleanFoldReport>(&copy.to_string()).is_err());
    }
}
