//! 续挖第 114 单：**谓词收集器的空/单项语义，以及"不支持谓词"策略里那个两者皆假的选项**——空作用域
//! `build_and()` 给出**恒真**、`build_or()` 给出**恒假**；**单项作用域直接返回该项本身**而**不包** `And`/`Or`，
//! 只有两项以上才真的合取/析取；`push` 与每个 builder 都**返回该表达式本身**、同时把**副本**收进作用域
//! （`build_*` 消费 `self`，于是同一作用域只能 build 一次）；`Default` 就是 `new()`；而 `UnsupportedPredicatePolicy`
//! 里只有 `FailFast` 会 fail-fast、只有 `AlwaysFalse` 会恒假，**`ClientFilter` 两者皆假**，默认是 `AlwaysFalse`，
//! `IGNORE`/`ERROR` 只是别名
//! （`persistence/expression/boolean_expression_dsl.rs` 的 `BooleanExpressionScope` 与
//! `persistence/expression/unsupported_predicate_policy.rs` 的 `UnsupportedPredicatePolicy`）
//! Continuation, delivery 114: **the predicate collector's empty and single-item semantics, and the policy option that
//! answers false to both** — an empty scope's `build_and()` yields **a constant true** and `build_or()` **a constant false**;
//! **a single-item scope returns that item itself** rather than wrapping it in `And`/`Or`, and only two or more items actually
//! conjoin or disjoin; `push` and every builder **return the expression itself** while collecting **a copy** into the scope
//! (`build_*` consumes `self`, so one scope can only be built once); `Default` is `new()`; and in
//! `UnsupportedPredicatePolicy` only `FailFast` fails fast, only `AlwaysFalse` becomes always-false, **`ClientFilter` answers
//! false to both**, the default is `AlwaysFalse`, and `IGNORE`/`ERROR` are mere aliases
//! (`BooleanExpressionScope` in `persistence/expression/boolean_expression_dsl.rs` and `UnsupportedPredicatePolicy` in
//! `persistence/expression/unsupported_predicate_policy.rs`).
//!
//! 第 113 单量的是 schema 的两个查表面，这一单量**谓词收集与不支持策略**。八条实测：
//!
//! 1. **空作用域的 `build_and()` 是恒真**（`the_empty_conjunction_is_true()`）。
//! 2. **空作用域的 `build_or()` 是恒假**（`the_empty_disjunction_is_false()`）。
//! 3. **单项作用域直接返回该项**（`a_single_item_scope_returns_the_item_itself()`）。
//! 4. **两项以上才真的合并**（`two_or_more_items_are_combined()`）。
//! 5. **builder 返回表达式本身且把副本收进作用域**（`the_builders_return_and_collect_the_same_expression()`）；
//!    另外**实测**：在显式标注 `T` 的作用域上 `scope.eq(path, value)` 无法调用（解析到 `PartialEq::eq`，`E0061`），
//!    得走 `typed_eq::<T>(..)` 加成 `push(..)`——这是本单踩出来的真实 API 形状。
//! 6. **`Default` 就是 `new()`**（`the_default_is_the_empty_scope()`）。
//! 7. **`FailFast` 是唯一 fail-fast、`AlwaysFalse` 是唯一恒假**（`only_one_policy_answers_each_way()`）。
//! 8. **`ClientFilter` 两者皆假，别名只是别名**（`the_client_filter_answers_false_to_both()`）。
//!
//! Delivery 113 measured the schema's two lookup faces; this one measures **predicate collection and the unsupported
//! policy**. **The empty conjunction is true** and **the empty disjunction is false**. **A single-item scope returns the item
//! itself**, and **two or more items are combined**. **The builders return and collect the same expression.** **The default is
//! the empty scope.** **Only one policy answers each way**, and **the client filter answers false to both** while the aliases
//! stay aliases.

use ospf_rust_framework::persistence::expression::boolean_expression_dsl::{
    typed_eq, BooleanExpressionScope,
};
use ospf_rust_framework::persistence::expression::unsupported_predicate_policy::UnsupportedPredicatePolicy;
use ospf_rust_math::Trivalent;
use ospf_rust_math::symbol::{BooleanExpression, ExpressionValue};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条等值谓词的描述。 / The description of one equality predicate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateScopeSpec {
    /// 字段路径。 / The field path.
    pub path: String,
    /// 期望值。 / The expected value.
    pub value: String,
}

/// 谓词作用域探针请求。 / The predicate scope probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateScopeRequest {
    /// 依次收集的等值谓词。 / The equality predicates collected in order.
    #[serde(default)]
    pub predicates: Vec<PredicateScopeSpec>,
    /// 不支持谓词策略名（默认 `AlwaysFalse`）。 / The unsupported predicate policy name.
    #[serde(default)]
    pub policy: Option<String>,
}

/// 谓词作用域探针结论。 / The predicate scope probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateScopeReport {
    /// 收集条数。 / The collected count.
    pub collected: u32,
    /// `build_and()` 的形状名（`true` / `false` / `single` / `and`）。 / The shape of `build_and()`.
    pub and_shape: String,
    /// `build_or()` 的形状名。 / The shape of `build_or()`.
    pub or_shape: String,
    /// `build_and()` 里被合并的条数。 / How many items `build_and()` combined.
    pub and_operands: u32,
    /// `build_or()` 里被合并的条数。 / How many items `build_or()` combined.
    pub or_operands: u32,
    /// 策略名回读。 / The policy name, read back.
    pub policy: String,
    /// 该策略是否 fail-fast。 / Whether the policy fails fast.
    pub fails_fast: bool,
    /// 该策略是否翻译为恒假。 / Whether the policy becomes always-false.
    pub always_false: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_empty_conjunction_is_true: bool,
    pub the_empty_disjunction_is_false: bool,
    pub a_single_item_scope_returns_the_item_itself: bool,
    pub two_or_more_items_are_combined: bool,
    pub the_builders_return_and_collect_the_same_expression: bool,
    pub the_default_is_the_empty_scope: bool,
    pub only_one_policy_answers_each_way: bool,
    pub the_client_filter_answers_false_to_both: bool,
}

/// 按名字取策略。 / Resolve a policy by name.
fn policy_named(name: &str) -> Option<UnsupportedPredicatePolicy> {
    match name {
        "AlwaysFalse" => Some(UnsupportedPredicatePolicy::AlwaysFalse),
        "FailFast" => Some(UnsupportedPredicatePolicy::FailFast),
        "ClientFilter" => Some(UnsupportedPredicatePolicy::ClientFilter),
        _ => None,
    }
}

/// 策略名。 / The policy name.
fn policy_name(policy: UnsupportedPredicatePolicy) -> String {
    match policy {
        UnsupportedPredicatePolicy::AlwaysFalse => "AlwaysFalse",
        UnsupportedPredicatePolicy::FailFast => "FailFast",
        UnsupportedPredicatePolicy::ClientFilter => "ClientFilter",
    }.to_owned()
}

/// 把返回值写成形状名。 / Render a built expression as a shape name.
fn shape_of(expression: &BooleanExpression<ExpressionValue>) -> String {
    match expression {
        BooleanExpression::Constant(Trivalent::True) => "true",
        BooleanExpression::Constant(Trivalent::False) => "false",
        BooleanExpression::Constant(Trivalent::Unknown) => "unknown",
        BooleanExpression::And(_) => "and",
        BooleanExpression::Or(_) => "or",
        _ => "single",
    }.to_owned()
}

/// 合并条数。 / How many items a built expression combined.
fn operand_count(expression: &BooleanExpression<ExpressionValue>) -> u32 {
    match expression {
        BooleanExpression::And(operands) | BooleanExpression::Or(operands) => {
            u32::try_from(operands.len()).unwrap_or(u32::MAX)
        }
        _ => 1,
    }
}

/// 造一个收集了给定谓词的作用域。 / Build a scope that collected the given predicates.
fn scope_of(request: &PredicateScopeRequest)
    -> Result<BooleanExpressionScope<ExpressionValue>>
{
    let mut scope = BooleanExpressionScope::new();
    for spec in &request.predicates {
        if spec.path.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "predicate scope probe failed \
                validation: a path must not be blank"));
        }
        scope.push(typed_eq::<ExpressionValue>(spec.path.as_str(), spec.value.as_str()));
    }
    Ok(scope)
}

/// 走一遍谓词作用域探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_predicate_scope_probe(
    request: &PredicateScopeRequest,
) -> Result<PredicateScopeReport> {
    if request.predicates.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "predicate scope probe failed validation: \
            at most eight predicates are accepted"));
    }
    let findings = canonical_findings();
    let policy = match request.policy.as_deref() {
        None => UnsupportedPredicatePolicy::default(),
        Some(name) => policy_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "predicate scope probe failed validation: unknown policy"))?,
    };
    // `build_*` 消费 self，所以两次构建要两个独立作用域。 
    // `build_*` consumes the scope, so the two builds need two independent scopes.
    let conjunction = scope_of(request)?.build_and();
    let disjunction = scope_of(request)?.build_or();
    Ok(PredicateScopeReport {
        collected: u32::try_from(request.predicates.len()).unwrap_or(u32::MAX),
        and_shape: shape_of(&conjunction),
        or_shape: shape_of(&disjunction),
        and_operands: operand_count(&conjunction),
        or_operands: operand_count(&disjunction),
        policy: policy_name(policy),
        fails_fast: policy.should_fail_fast(),
        always_false: policy.should_use_always_false(),
        the_empty_conjunction_is_true: findings.the_empty_conjunction_is_true,
        the_empty_disjunction_is_false: findings.the_empty_disjunction_is_false,
        a_single_item_scope_returns_the_item_itself:
            findings.a_single_item_scope_returns_the_item_itself,
        two_or_more_items_are_combined: findings.two_or_more_items_are_combined,
        the_builders_return_and_collect_the_same_expression:
            findings.the_builders_return_and_collect_the_same_expression,
        the_default_is_the_empty_scope: findings.the_default_is_the_empty_scope,
        only_one_policy_answers_each_way: findings.only_one_policy_answers_each_way,
        the_client_filter_answers_false_to_both: findings.the_client_filter_answers_false_to_both,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_empty_conjunction_is_true: bool,
    the_empty_disjunction_is_false: bool,
    a_single_item_scope_returns_the_item_itself: bool,
    two_or_more_items_are_combined: bool,
    the_builders_return_and_collect_the_same_expression: bool,
    the_default_is_the_empty_scope: bool,
    only_one_policy_answers_each_way: bool,
    the_client_filter_answers_false_to_both: bool,
}

/// 造一个收集了两条等值谓词的作用域。 / Build a scope that collected two equalities.
fn two_item_scope() -> BooleanExpressionScope<ExpressionValue> {
    let mut scope = BooleanExpressionScope::new();
    scope.push(typed_eq::<ExpressionValue>("status", "active"));
    scope.push(typed_eq::<ExpressionValue>("age", "18"));
    scope
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 空合取为真。 / The empty conjunction is true.
        let the_empty_conjunction_is_true = {
            let built = BooleanExpressionScope::<ExpressionValue>::new().build_and();
            built == BooleanExpression::Constant(Trivalent::True)
                && shape_of(&built) == "true"
                // 空析取不是真。 / The empty disjunction is not true.
                && BooleanExpressionScope::<ExpressionValue>::new().build_or()
                    != BooleanExpression::Constant(Trivalent::True)
        };
        // 2) 空析取为假。 / The empty disjunction is false.
        let the_empty_disjunction_is_false = {
            let built = BooleanExpressionScope::<ExpressionValue>::new().build_or();
            built == BooleanExpression::Constant(Trivalent::False)
                && shape_of(&built) == "false"
        };
        // 3) 单项直接返回该项。 / A single item comes back as itself.
        let a_single_item_scope_returns_the_item_itself = {
            let mut scope = BooleanExpressionScope::<ExpressionValue>::new();
            let returned = scope.push(typed_eq::<ExpressionValue>("status", "active"));
            let built = scope.build_and();
            // 单项作用域不包 And，也不退化成常量。 
            // A single-item scope neither wraps in `And` nor collapses to a constant.
            shape_of(&built) == "single"
                && built == returned
                && !matches!(built, BooleanExpression::Constant(_))
        };
        // 4) 两项以上才合并。 / Two or more items are combined.
        let two_or_more_items_are_combined = {
            let conjunction = two_item_scope().build_and();
            let disjunction = two_item_scope().build_or();
            matches!(conjunction, BooleanExpression::And(_))
                && matches!(disjunction, BooleanExpression::Or(_))
                && operand_count(&conjunction) == 2
                && operand_count(&disjunction) == 2
                // 析取与合取形状不同。 / The two shapes differ.
                && shape_of(&conjunction) != shape_of(&disjunction)
        };
        // 5) builder 返回表达式本身并收集副本。 
        // The builders return their expression and collect a copy.
        let the_builders_return_and_collect_the_same_expression = {
            let mut scope = BooleanExpressionScope::<ExpressionValue>::new();
            let first = scope.push(typed_eq::<ExpressionValue>("a", "1"));
            let second = scope.push(first.clone());
            // `push` 返回原表达式，且作用域用的是副本：两者相等但互不影响。 
            // `push` returns the expression unchanged while the scope holds a copy: equal yet independent.
            second == first
                && scope.build_or() == BooleanExpression::or(vec![first, second])
        };
        // 6) Default 就是空作用域。 / `Default` is the empty scope.
        let the_default_is_the_empty_scope = {
            BooleanExpressionScope::<ExpressionValue>::default().build_and()
                == BooleanExpressionScope::<ExpressionValue>::new().build_and()
                && BooleanExpressionScope::<ExpressionValue>::default().build_or()
                    == BooleanExpression::Constant(Trivalent::False)
        };
        // 7) 每种答案只有一个策略。 / Exactly one policy answers each way.
        let only_one_policy_answers_each_way = {
            let policies = [UnsupportedPredicatePolicy::AlwaysFalse,
                UnsupportedPredicatePolicy::FailFast,
                UnsupportedPredicatePolicy::ClientFilter];
            let fail_fast: Vec<_> = policies.iter()
                .filter(|policy| policy.should_fail_fast()).collect();
            let always_false: Vec<_> = policies.iter()
                .filter(|policy| policy.should_use_always_false()).collect();
            fail_fast.len() == 1 && always_false.len() == 1
                && *fail_fast[0] == UnsupportedPredicatePolicy::FailFast
                && *always_false[0] == UnsupportedPredicatePolicy::AlwaysFalse
                // 默认是 AlwaysFalse。 / The default is always-false.
                && UnsupportedPredicatePolicy::default() == UnsupportedPredicatePolicy::AlwaysFalse
        };
        // 8) ClientFilter 两者皆假，别名只是别名。 
        // The client filter answers false to both, and the aliases stay aliases.
        let the_client_filter_answers_false_to_both = {
            let client = UnsupportedPredicatePolicy::ClientFilter;
            !client.should_fail_fast() && !client.should_use_always_false()
                && UnsupportedPredicatePolicy::IGNORE == UnsupportedPredicatePolicy::AlwaysFalse
                && UnsupportedPredicatePolicy::ERROR == UnsupportedPredicatePolicy::FailFast
                && UnsupportedPredicatePolicy::IGNORE != UnsupportedPredicatePolicy::ERROR
        };
        Findings {
            the_empty_conjunction_is_true,
            the_empty_disjunction_is_false,
            a_single_item_scope_returns_the_item_itself,
            two_or_more_items_are_combined,
            the_builders_return_and_collect_the_same_expression,
            the_default_is_the_empty_scope,
            only_one_policy_answers_each_way,
            the_client_filter_answers_false_to_both,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: PredicateScopeRequest) -> PredicateScopeReport {
        verify_predicate_scope_probe(&request).unwrap()
    }

    fn spec(path: &str, value: &str) -> PredicateScopeSpec {
        PredicateScopeSpec { path: path.to_owned(), value: value.to_owned() }
    }

    fn request() -> PredicateScopeRequest {
        PredicateScopeRequest { predicates: vec![spec("status", "active"), spec("age", "18")],
            policy: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert_eq!(report.collected, 2);
        assert_eq!(report.and_shape, "and");
        assert_eq!(report.or_shape, "or");
        assert_eq!(report.and_operands, 2);
        assert_eq!(report.or_operands, 2);
        assert_eq!(report.policy, "AlwaysFalse");
        assert!(!report.fails_fast);
        assert!(report.always_false);
        assert!(report.the_empty_conjunction_is_true);
        assert!(report.the_empty_disjunction_is_false);
        assert!(report.a_single_item_scope_returns_the_item_itself);
        assert!(report.two_or_more_items_are_combined);
        assert!(report.the_builders_return_and_collect_the_same_expression);
        assert!(report.the_default_is_the_empty_scope);
        assert!(report.only_one_policy_answers_each_way);
        assert!(report.the_client_filter_answers_false_to_both);
    }

    #[test]
    fn an_empty_scope_gives_the_identity_elements() {
        let report = ask(PredicateScopeRequest { predicates: Vec::new(), ..request() });
        // 空合取真、空析取假。 / The empty conjunction is true and the empty disjunction is false.
        assert_eq!(report.and_shape, "true");
        assert_eq!(report.or_shape, "false");
        assert_eq!(report.and_operands, 1);
        assert_eq!(report.or_operands, 1);
        assert_eq!(report.collected, 0);
    }

    #[test]
    fn a_single_item_is_returned_unwrapped() {
        let report = ask(PredicateScopeRequest { predicates: vec![spec("a", "1")],
            ..request() });
        // 单项既不包 And/Or 也不退化成常量。 / A single item is neither wrapped nor collapsed.
        assert_eq!(report.and_shape, "single");
        assert_eq!(report.or_shape, "single");
        assert_eq!(report.and_operands, 1);
    }

    #[test]
    fn the_policies_answer_predictably() {
        let always_false = ask(request());
        assert!(!always_false.fails_fast && always_false.always_false);
        let fail_fast = ask(PredicateScopeRequest { policy: Some("FailFast".to_owned()),
            ..request() });
        assert!(fail_fast.fails_fast && !fail_fast.always_false);
        // 客户端过滤两者皆假。 / The client filter answers false to both.
        let client = ask(PredicateScopeRequest { policy: Some("ClientFilter".to_owned()),
            ..request() });
        assert!(!client.fails_fast && !client.always_false);
        assert_eq!(client.policy, "ClientFilter");
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_predicate_scope_probe(&PredicateScopeRequest {
            predicates: (0..9).map(|index| spec(&format!("f{index}"), "v")).collect(),
            ..request() }).unwrap_err().message.contains("at most eight predicates"));
        assert!(verify_predicate_scope_probe(&PredicateScopeRequest {
            predicates: vec![spec("   ", "v")], ..request() }).unwrap_err()
            .message.contains("must not be blank"));
        assert!(verify_predicate_scope_probe(&PredicateScopeRequest {
            policy: Some("Ignore".to_owned()), ..request() }).unwrap_err()
            .message.contains("unknown policy"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"andShape\""), "{json}");
        assert!(json.contains("\"alwaysFalse\""), "{json}");
        assert!(json.contains("\"theClientFilterAnswersFalseToBoth\""), "{json}");
        assert!(!json.contains("and_shape"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PredicateScopeReport>(&copy.to_string()).is_err());
    }
}
