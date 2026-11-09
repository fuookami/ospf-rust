//! 续挖第 74 单：`IntegerExpression::bounds` 的**保守界**——它只读域的包络，不看稀疏空洞
//! （`model/constraint_programming/expression.rs` 的 `bounds` 与 `linear`）
//! Continuation, delivery 74: the **conservative bounds** of `IntegerExpression::bounds` — it reads only the domain hull
//! and never the sparse gaps (`bounds` and `linear` in `model/constraint_programming/expression.rs`).
//!
//! 第 73 单量过区间怎么被求值；这一单量**同一个表达式**的界从哪来。八条实测：
//!
//! 1. **`linear` 会合并重复稳定变量、丢掉零和项、按稳定 ID 排序**（`terms_are_merged_sorted_and_zero_free()`）：
//!    所以 `validate_bindings` 的"零系数"闸门经公开构造器**不可达**。
//! 2. **负系数翻转区间两端**（`a_negative_coefficient_flips_the_interval_ends()`）：下界取 `upper()`。
//! 3. **`bounds` 只读域的包络**（`bounds_read_only_the_domain_hull()`）：稀疏域 `{0,10}` 给出 `[0,10]`，
//!    中间值落在界内却**不可达**。
//! 4. **纯常量表达式从不碰域**（`a_constant_only_expression_never_touches_a_domain()`）：空域表也能算。
//! 5. **缺域是错误而不是界**（`a_missing_domain_is_an_error_not_a_bound()`）。
//! 6. **`i128` 乘法闸门不可达，而 `i64` 导出闸门与累加闸门都可达**
//!    （`the_multiplication_gate_is_unreachable_while_the_other_two_are_not()`）：
//!    `i64 × i64 < 2^126 < i128::MAX`，但单项 `i64::MAX²` 已超出 `i64`，三项之和已超出 `i128`。
//! 7. **`evaluate` 是精确的，而且不看域**（`evaluate_is_exact_and_never_consults_a_domain()`）：
//!    给一个域外的赋值也能算出算术结果。
//! 8. **界恒包含每一个可达值**（`the_bounds_always_contain_every_reachable_value()`）。
//!
//! Delivery 73 measured how intervals are evaluated; this one measures where **one expression**'s bounds come from.
//! **`linear` merges duplicate stable variables, drops zero sums and sorts by stable ID**, which makes
//! `validate_bindings`' zero-coefficient gate **unreachable** through the public constructor. **A negative coefficient
//! flips the interval ends.** **`bounds` reads only the domain hull**: the sparse `{0,10}` yields `[0,10]` and the gap is
//! inside the bounds yet **unreachable**. **A constant-only expression never touches a domain.** **A missing domain is an
//! error, not a bound.** **The `i128` multiplication gate is unreachable while the `i64` export and accumulation gates are
//! not.** **`evaluate` is exact and never consults a domain.** And **the bounds always contain every reachable value**.

use std::collections::BTreeMap;

use ospf_rust_core::model::constraint_programming::{
    IntegerDomain, IntegerExpression, IntegerTerm, IntegerVariable,
};
use ospf_rust_core::solver::report::StableVariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AssignmentValueSpec, BridgeError, Result};

/// 一个整数线性项的声明。 / One integer-linear term declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionTermSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 整数系数（`0` 会被丢掉）。 / The integer coefficient (`0` is dropped).
    pub coefficient: i64,
}

/// 一个域的声明：`values` 非空即稀疏域，否则用 `lower`/`upper` 造区间。
/// One domain declaration: a non-empty `values` means a sparse domain, otherwise `lower`/`upper` build a range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionDomainSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 区间下界。 / The range lower bound.
    #[serde(default)]
    pub lower: Option<i64>,
    /// 区间上界。 / The range upper bound.
    #[serde(default)]
    pub upper: Option<i64>,
    /// 稀疏取值（非空时优先）。 / The sparse values (preferred when non-empty).
    #[serde(default)]
    pub values: Vec<i64>,
}

/// 表达式界探针请求。 / The expression-bounds probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionBoundsRequest {
    /// 常数项。 / The constant term.
    pub constant: i64,
    /// 线性项。 / The linear terms.
    #[serde(default)]
    pub terms: Vec<ExpressionTermSpec>,
    /// 各变量的域。 / Each variable's domain.
    #[serde(default)]
    pub domains: Vec<ExpressionDomainSpec>,
    /// 精确求值用的赋值（可选）。 / The assignment used for exact evaluation (optional).
    #[serde(default)]
    pub assignment: Vec<AssignmentValueSpec>,
}

/// 一个规范化后的项。 / One normalized term.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionTermView {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 系数。 / The coefficient.
    pub coefficient: i64,
}

/// 表达式界探针结论。 / The expression-bounds probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionBoundsReport {
    /// 是否算出界。 / Whether the bounds were computed.
    pub accepted: bool,
    /// 被拒文案。 / The refusal message.
    #[serde(default)]
    pub refusal: Option<String>,
    /// 常数项。 / The constant term.
    pub constant: i64,
    /// 规范化后的项。 / The normalized terms.
    pub terms: Vec<ExpressionTermView>,
    /// 保守下界。 / The conservative lower bound.
    #[serde(default)]
    pub lower: Option<i64>,
    /// 保守上界。 / The conservative upper bound.
    #[serde(default)]
    pub upper: Option<i64>,
    /// 精确求值结果（给了赋值才有）。 / The exact value (present when an assignment was given).
    #[serde(default)]
    pub evaluated: Option<i64>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub terms_are_merged_sorted_and_zero_free: bool,
    pub a_negative_coefficient_flips_the_interval_ends: bool,
    pub bounds_read_only_the_domain_hull: bool,
    pub a_constant_only_expression_never_touches_a_domain: bool,
    pub a_missing_domain_is_an_error_not_a_bound: bool,
    pub the_multiplication_gate_is_unreachable_while_the_other_two_are_not: bool,
    pub evaluate_is_exact_and_never_consults_a_domain: bool,
    pub the_bounds_always_contain_every_reachable_value: bool,
}

/// 造一个变量。 / Build a variable.
fn variable_of(id: &str) -> IntegerVariable {
    IntegerVariable::new(id)
}

/// 变量账本：稳定 ID → **同一个实例**。`IntegerVariable::new` 每次都发新本地 ID，而 `linear` 只合并
/// "同一本地 ID"的重复项，两个独立构造的同名变量会被拒（"bound to multiple local identities"）。
/// **按实测**：这也意味着"合并"只在复用实例时才可达。
/// The variable ledger: stable ID → **the same instance**. `IntegerVariable::new` issues a fresh local ID each call
/// while `linear` only merges terms sharing one local ID, so two independently built variables with the same stable ID
/// are refused ("bound to multiple local identities"). **Measured**: merging is therefore only reachable when the
/// instance is reused.
type VariableLedger = BTreeMap<String, IntegerVariable>;

/// 取（或造）账本里的那个实例。 / Fetch or create the ledger's instance.
fn ledger_variable(ledger: &mut VariableLedger, id: &str) -> IntegerVariable {
    ledger.entry(id.to_owned()).or_insert_with(|| variable_of(id)).clone()
}

/// 造一份域表。 / Build the domain table.
fn domains_of(specs: &[ExpressionDomainSpec]) -> Result<BTreeMap<StableVariableId, IntegerDomain>> {
    let mut domains = BTreeMap::new();
    for spec in specs {
        if spec.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "expression bounds failed validation: domain IDs must not be blank"));
        }
        let domain = if spec.values.is_empty() {
            let lower = spec.lower.ok_or_else(|| BridgeError::new("InvalidInput", &format!(
                "expression bounds failed validation: domain `{}` needs a lower bound", spec.id)))?;
            let upper = spec.upper.ok_or_else(|| BridgeError::new("InvalidInput", &format!(
                "expression bounds failed validation: domain `{}` needs an upper bound", spec.id)))?;
            IntegerDomain::range(lower, upper)
        } else {
            IntegerDomain::values(spec.values.iter().copied())
        }
        .map_err(|error| BridgeError::new("InvalidInput", &format!(
            "expression bounds failed validation: {error}")))?;
        domains.insert(StableVariableId(spec.id.clone()), domain);
    }
    Ok(domains)
}

/// 造一份表达式。 / Build the expression.
fn expression_of(request: &ExpressionBoundsRequest) -> Result<IntegerExpression> {
    let mut ledger = VariableLedger::new();
    let mut terms = Vec::with_capacity(request.terms.len());
    for spec in &request.terms {
        if spec.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "expression bounds failed validation: term IDs must not be blank"));
        }
        terms.push(IntegerTerm { variable: ledger_variable(&mut ledger, &spec.id),
            coefficient: spec.coefficient });
    }
    IntegerExpression::linear(request.constant, terms).map_err(|error| BridgeError::new(
        "InvalidInput", &format!("expression bounds failed validation: {error}")))
}

/// 把表达式读成项视图。 / Read the expression into a term view.
fn terms_of(expression: &IntegerExpression) -> Vec<ExpressionTermView> {
    expression.terms.iter().map(|term| ExpressionTermView {
        id: term.variable.stable_id.0.clone(), coefficient: term.coefficient }).collect()
}

/// 走一遍表达式界探针。非法输入是**错误**。 / Walk the expression-bounds probe.
pub fn verify_expression_bounds(request: &ExpressionBoundsRequest) -> Result<ExpressionBoundsReport> {
    if request.terms.len() > 64 {
        return Err(BridgeError::new("InvalidInput",
            "expression bounds failed validation: at most 64 terms are accepted"));
    }
    let findings = canonical_findings();
    let expression = expression_of(request)?;
    let domains = domains_of(&request.domains)?;
    let terms = terms_of(&expression);
    let bounds = expression.bounds(|id| domains.get(id));
    let evaluated = if request.assignment.is_empty() {
        None
    } else {
        let mut values = BTreeMap::new();
        for entry in &request.assignment {
            if entry.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput",
                    "expression bounds failed validation: assignment IDs must not be blank"));
            }
            values.insert(StableVariableId(entry.id.clone()), entry.value);
        }
        Some(expression.evaluate(&values).map_err(|error| BridgeError::new("InvalidInput",
            &format!("expression bounds failed validation: {error}")))?)
    };
    let (accepted, refusal, lower, upper) = match bounds {
        Ok((lower, upper)) => (true, None, Some(lower), Some(upper)),
        Err(error) => (false, Some(error.to_string()), None, None),
    };
    Ok(ExpressionBoundsReport {
        accepted,
        refusal,
        constant: expression.constant,
        terms,
        lower,
        upper,
        evaluated,
        terms_are_merged_sorted_and_zero_free: findings.terms_are_merged_sorted_and_zero_free,
        a_negative_coefficient_flips_the_interval_ends:
            findings.a_negative_coefficient_flips_the_interval_ends,
        bounds_read_only_the_domain_hull: findings.bounds_read_only_the_domain_hull,
        a_constant_only_expression_never_touches_a_domain:
            findings.a_constant_only_expression_never_touches_a_domain,
        a_missing_domain_is_an_error_not_a_bound: findings.a_missing_domain_is_an_error_not_a_bound,
        the_multiplication_gate_is_unreachable_while_the_other_two_are_not:
            findings.the_multiplication_gate_is_unreachable_while_the_other_two_are_not,
        evaluate_is_exact_and_never_consults_a_domain:
            findings.evaluate_is_exact_and_never_consults_a_domain,
        the_bounds_always_contain_every_reachable_value:
            findings.the_bounds_always_contain_every_reachable_value,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    terms_are_merged_sorted_and_zero_free: bool,
    a_negative_coefficient_flips_the_interval_ends: bool,
    bounds_read_only_the_domain_hull: bool,
    a_constant_only_expression_never_touches_a_domain: bool,
    a_missing_domain_is_an_error_not_a_bound: bool,
    the_multiplication_gate_is_unreachable_while_the_other_two_are_not: bool,
    evaluate_is_exact_and_never_consults_a_domain: bool,
    the_bounds_always_contain_every_reachable_value: bool,
}

/// 一份项声明。 / A term declaration.
fn term(id: &str, coefficient: i64) -> ExpressionTermSpec {
    ExpressionTermSpec { id: id.to_owned(), coefficient }
}

/// 一份区间域声明。 / A range domain declaration.
fn range(id: &str, lower: i64, upper: i64) -> ExpressionDomainSpec {
    ExpressionDomainSpec { id: id.to_owned(), lower: Some(lower), upper: Some(upper),
        values: Vec::new() }
}

/// 一份稀疏域声明。 / A sparse domain declaration.
fn sparse(id: &str, values: &[i64]) -> ExpressionDomainSpec {
    ExpressionDomainSpec { id: id.to_owned(), lower: None, upper: None,
        values: values.to_vec() }
}

/// 一份只带常数与项与域的请求。 / A request with a constant, terms and domains only.
fn request(constant: i64, terms: Vec<ExpressionTermSpec>,
    domains: Vec<ExpressionDomainSpec>) -> ExpressionBoundsRequest {
    ExpressionBoundsRequest { constant, terms, domains, assignment: Vec::new() }
}

/// 只取界，失败时返回文案。**内部探针专用**：直接走 `bounds`，不经公开入口。
/// Take the bounds, or the refusal message. **Internal probes only**: it calls `bounds` directly instead of going
/// through the public entry, which would recurse forever via `canonical_findings`.
fn bounds_of(constant: i64, terms: Vec<ExpressionTermSpec>,
    domains: Vec<ExpressionDomainSpec>) -> std::result::Result<(i64, i64), String> {
    let request = request(constant, terms, domains);
    let expression = expression_of(&request).map_err(|error| error.message)?;
    let domains = domains_of(&request.domains).map_err(|error| error.message)?;
    expression.bounds(|id| domains.get(id)).map_err(|error| error.to_string())
}

/// 只求值。**内部探针专用**（同上）。 / Evaluate only. **Internal probes only** (same reason).
fn evaluate_of(constant: i64, terms: Vec<ExpressionTermSpec>, domains: Vec<ExpressionDomainSpec>,
    id: &str, value: i64) -> std::result::Result<i64, String> {
    let request = ExpressionBoundsRequest { constant, terms, domains,
        assignment: vec![AssignmentValueSpec { id: id.to_owned(), value }] };
    let expression = expression_of(&request).map_err(|error| error.message)?;
    let values = BTreeMap::from([(StableVariableId(id.to_owned()), value)]);
    expression.evaluate(&values).map_err(|error| error.to_string())
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 合并、排序、丢零和——**只在复用实例时**。 / Merge, sort, drop zero sums — **only when the instance is reused**.
    let terms_are_merged_sorted_and_zero_free = {
        // `b:2` + `a:3` + `b:-2` → 只剩 `a:3`（`b` 的和是零，整项消失）。
        // `b:2` + `a:3` + `b:-2` leaves only `a:3`: `b` sums to zero and the whole term disappears.
        let expression = expression_of(&request(1,
            vec![term("b", 2), term("a", 3), term("b", -2)],
            vec![range("a", 0, 1), range("b", 0, 1)])).expect("probe");
        let terms = terms_of(&expression);
        let ids = terms.iter().map(|entry| entry.id.clone()).collect::<Vec<_>>();
        // 显式的零系数也在构造时被跳过。 / An explicit zero coefficient is skipped at construction too.
        let zero = expression_of(&request(0, vec![term("a", 0)], Vec::new())).expect("probe");
        // 两个独立构造的同名变量**不合并**，而是被拒。 
        // Two independently built variables with one stable ID are **not** merged but refused.
        let refused = IntegerExpression::linear(0, vec![
            IntegerTerm { variable: variable_of("b"), coefficient: 2 },
            IntegerTerm { variable: variable_of("b"), coefficient: -2 }])
            .err().map(|error| error.to_string()).unwrap_or_default();
        ids == vec!["a".to_owned()] && terms[0].coefficient == 3
            && zero.terms.is_empty()
            && bounds_of(0, vec![term("a", 0)], vec![range("a", 0, 1)]) == Ok((0, 0))
            && refused.contains("bound to multiple local identities")
    };
    // 2) 负系数翻转两端。 / A negative coefficient flips the ends.
    let a_negative_coefficient_flips_the_interval_ends = {
        // `-2·x`，`x ∈ [1,5]` → `[-10,-2]`。 / `-2·x` with `x ∈ [1,5]` gives `[-10,-2]`.
        let negative = bounds_of(0, vec![term("x", -2)], vec![range("x", 1, 5)]);
        let positive = bounds_of(0, vec![term("x", 2)], vec![range("x", 1, 5)]);
        negative == Ok((-10, -2)) && positive == Ok((2, 10))
    };
    // 3) 只读包络。 / Hull only.
    let bounds_read_only_the_domain_hull = {
        // 稀疏 `{0,10}`：界是 `[0,10]`，但 5 落在界内却不可达。 
        // The sparse `{0,10}` gives `[0,10]`, yet 5 sits inside the bounds and is unreachable.
        let hull = bounds_of(0, vec![term("x", 1)], vec![sparse("x", &[0, 10])]);
        let domain = IntegerDomain::values([0, 10]).expect("domain");
        hull == Ok((0, 10)) && !domain.contains(5) && domain.contains(0) && domain.contains(10)
    };
    // 4) 纯常量从不碰域。 / A constant-only expression never touches a domain.
    let a_constant_only_expression_never_touches_a_domain = {
        // 域表为空：若 `bounds` 查域就会报"missing domain"。 / An empty domain table: any lookup would say "missing domain".
        bounds_of(7, Vec::new(), Vec::new()) == Ok((7, 7))
    };
    // 5) 缺域是错误。 / A missing domain is an error.
    let a_missing_domain_is_an_error_not_a_bound = {
        let message = bounds_of(0, vec![term("ghost", 1)], Vec::new()).err().unwrap_or_default();
        message.contains("missing domain for variable ghost")
    };
    // 6) 乘法闸门不可达，导出与累加闸门可达。 / The multiplication gate is unreachable, the other two are not.
    let the_multiplication_gate_is_unreachable_while_the_other_two_are_not = {
        // 单项 `i64::MAX · i64::MAX` 已超出 `i64`，但乘本身没溢出 `i128`。 
        // One `i64::MAX · i64::MAX` already exceeds `i64` while the multiply itself never overflows `i128`.
        let export = bounds_of(0, vec![term("x", i64::MAX)],
            vec![range("x", 0, i64::MAX)]).err().unwrap_or_default();
        // 三项之和才把 `i128` 撑破。 / Three such products break `i128`.
        let accumulate = bounds_of(0, vec![term("a", i64::MAX), term("b", i64::MAX),
            term("c", i64::MAX)], vec![range("a", 0, i64::MAX), range("b", 0, i64::MAX),
            range("c", 0, i64::MAX)]).err().unwrap_or_default();
        export.contains("upper bound exceeds i64")
            && accumulate.contains("upper bound overflow")
            // 两个闸门都没说"乘法"。 / Neither gate blames the multiplication.
            && !export.contains("multiplication") && !accumulate.contains("multiplication")
    };
    // 7) `evaluate` 精确且不看域。 / `evaluate` is exact and never consults a domain.
    let evaluate_is_exact_and_never_consults_a_domain = {
        // 域 `{0,10}` 上给 5：界仍是 `[0,10]`，求值给 5——域外值也算。 
        // Feeding 5 on the domain `{0,10}`: the bounds stay `[0,10]` and evaluation returns 5 — out-of-domain values
        // are evaluated too.
        evaluate_of(1, vec![term("x", 2)], vec![sparse("x", &[0, 10])], "x", 5) == Ok(11)
            && bounds_of(1, vec![term("x", 2)], vec![sparse("x", &[0, 10])]) == Ok((1, 21))
    };
    // 8) 界恒包含每个可达值。 / The bounds always contain every reachable value.
    let the_bounds_always_contain_every_reachable_value = {
        // 枚举稀疏域上的每个可达值，逐个确认落在界内。 
        // Enumerate every reachable value on a sparse domain and confirm each sits inside the bounds.
        let (lower, upper) = bounds_of(3, vec![term("x", -2), term("y", 1)],
            vec![sparse("x", &[0, 4, 9]), range("y", -3, 2)]).unwrap_or((0, 0));
        let mut contained = true;
        for x in [0, 4, 9] {
            for y in -3..=2 {
                let value = 3 - 2 * x + y;
                contained &= (lower..=upper).contains(&value);
            }
        }
        // 界本身也在可达值处取到（保守但紧）：下界 `3 - 2·9 - 3`，上界 `3 - 2·0 + 2`。 
        // The bounds are attained at reachable values (conservative but tight): the lower is `3 - 2·9 - 3` and the upper
        // is `3 - 2·0 + 2`.
        contained && lower == -18 && upper == 5
    };
    Findings {
        terms_are_merged_sorted_and_zero_free,
        a_negative_coefficient_flips_the_interval_ends,
        bounds_read_only_the_domain_hull,
        a_constant_only_expression_never_touches_a_domain,
        a_missing_domain_is_an_error_not_a_bound,
        the_multiplication_gate_is_unreachable_while_the_other_two_are_not,
        evaluate_is_exact_and_never_consults_a_domain,
        the_bounds_always_contain_every_reachable_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_merged_sorted_and_zero_free() {
        let report = verify_expression_bounds(&request(1,
            vec![term("b", 2), term("a", 3), term("b", -2)],
            vec![range("a", 0, 1), range("b", 0, 1)])).unwrap();
        assert!(report.accepted, "{:?}", report.refusal);
        assert_eq!(report.terms.len(), 1);
        assert_eq!(report.terms[0].id, "a");
        assert_eq!(report.terms[0].coefficient, 3);
        assert_eq!((report.lower, report.upper), (Some(1), Some(4)));
        // 显式零系数被跳过。 / An explicit zero coefficient is skipped.
        let zero = verify_expression_bounds(&request(0, vec![term("a", 0)],
            vec![range("a", 0, 1)])).unwrap();
        assert!(zero.terms.is_empty());
        assert_eq!((zero.lower, zero.upper), (Some(0), Some(0)));
    }

    #[test]
    fn a_negative_coefficient_flips_the_ends() {
        let negative = verify_expression_bounds(&request(0, vec![term("x", -2)],
            vec![range("x", 1, 5)])).unwrap();
        assert_eq!((negative.lower, negative.upper), (Some(-10), Some(-2)));
        let positive = verify_expression_bounds(&request(0, vec![term("x", 2)],
            vec![range("x", 1, 5)])).unwrap();
        assert_eq!((positive.lower, positive.upper), (Some(2), Some(10)));
    }

    #[test]
    fn bounds_read_only_the_domain_hull() {
        let report = verify_expression_bounds(&request(0, vec![term("x", 1)],
            vec![sparse("x", &[0, 10])])).unwrap();
        assert_eq!((report.lower, report.upper), (Some(0), Some(10)));
        // 5 落在界内却不可达——界只是包络。 / 5 sits inside the bounds yet is unreachable: the bounds are only a hull.
        let evaluated = verify_expression_bounds(&ExpressionBoundsRequest {
            constant: 0, terms: vec![term("x", 1)], domains: vec![sparse("x", &[0, 10])],
            assignment: vec![AssignmentValueSpec { id: "x".to_owned(), value: 5 }] }).unwrap();
        assert_eq!(evaluated.evaluated, Some(5));
        assert!((evaluated.lower.unwrap()..=evaluated.upper.unwrap()).contains(&5));
    }

    #[test]
    fn a_constant_only_expression_never_touches_a_domain() {
        let report = verify_expression_bounds(&request(7, Vec::new(), Vec::new())).unwrap();
        assert!(report.accepted, "{:?}", report.refusal);
        assert_eq!((report.lower, report.upper), (Some(7), Some(7)));
    }

    #[test]
    fn a_missing_domain_is_an_error_not_a_bound() {
        let report = verify_expression_bounds(&request(0, vec![term("ghost", 1)],
            Vec::new())).unwrap();
        assert!(!report.accepted);
        assert!(report.refusal.as_deref().unwrap()
            .contains("missing domain for variable ghost"));
        assert_eq!((report.lower, report.upper), (None, None));
    }

    #[test]
    fn the_multiplication_gate_is_unreachable_while_the_other_two_are_not() {
        // 单项 `i64::MAX²`：导出闸门。 / One `i64::MAX²`: the export gate.
        let export = verify_expression_bounds(&request(0, vec![term("x", i64::MAX)],
            vec![range("x", 0, i64::MAX)])).unwrap();
        assert!(!export.accepted);
        assert!(export.refusal.as_deref().unwrap().contains("upper bound exceeds i64"));
        // 三项之和：累加闸门。 / Three products: the accumulation gate.
        let accumulate = verify_expression_bounds(&request(0,
            vec![term("a", i64::MAX), term("b", i64::MAX), term("c", i64::MAX)],
            vec![range("a", 0, i64::MAX), range("b", 0, i64::MAX),
                range("c", 0, i64::MAX)])).unwrap();
        assert!(!accumulate.accepted);
        assert!(accumulate.refusal.as_deref().unwrap().contains("upper bound overflow"));
        // 两个闸门都没说"乘法"——`i128` 乘法闸门不可达。 
        // Neither gate blames the multiplication: the `i128` multiplication gate is unreachable.
        for message in [export.refusal.unwrap(), accumulate.refusal.unwrap()] {
            assert!(!message.contains("multiplication"), "{message}");
        }
    }

    #[test]
    fn evaluate_is_exact_and_never_consults_a_domain() {
        let report = verify_expression_bounds(&ExpressionBoundsRequest {
            constant: 1, terms: vec![term("x", 2)], domains: vec![sparse("x", &[0, 10])],
            assignment: vec![AssignmentValueSpec { id: "x".to_owned(), value: 5 }] }).unwrap();
        // 5 不在域 `{0,10}` 里，但求值照样给 11。 / 5 is not in `{0,10}` yet evaluation still returns 11.
        assert_eq!(report.evaluated, Some(11));
        assert_eq!((report.lower, report.upper), (Some(1), Some(21)));
    }

    #[test]
    fn the_bounds_always_contain_every_reachable_value() {
        let report = verify_expression_bounds(&request(3, vec![term("x", -2), term("y", 1)],
            vec![sparse("x", &[0, 4, 9]), range("y", -3, 2)])).unwrap();
        let (lower, upper) = (report.lower.unwrap(), report.upper.unwrap());
        assert_eq!((lower, upper), (-18, 5));
        for x in [0, 4, 9] {
            for y in -3..=2 {
                let value = 3 - 2 * x + y;
                assert!((lower..=upper).contains(&value), "{value} escapes [{lower},{upper}]");
            }
        }
    }

    #[test]
    fn an_assignment_can_be_partial_only_if_the_expression_needs_no_more() {
        // 表达式只引用 `x`：只给 `x` 就够。 / The expression only references `x`, so giving `x` suffices.
        let report = verify_expression_bounds(&ExpressionBoundsRequest {
            constant: 0, terms: vec![term("x", 1)], domains: vec![range("x", 0, 1)],
            assignment: vec![AssignmentValueSpec { id: "x".to_owned(), value: 1 }] }).unwrap();
        assert_eq!(report.evaluated, Some(1));
        // 缺一个被引用的变量 → 输入错误。 / Missing a referenced variable is an input error.
        assert!(verify_expression_bounds(&ExpressionBoundsRequest {
            constant: 0, terms: vec![term("x", 1), term("y", 1)], domains: vec![range("x", 0, 1)],
            assignment: vec![AssignmentValueSpec { id: "x".to_owned(), value: 1 }] }).unwrap_err()
            .message.contains("missing assignment for variable y"));
    }

    #[test]
    fn every_rule_reading_holds_on_a_real_report() {
        let report = verify_expression_bounds(&request(0, vec![term("x", 1)],
            vec![range("x", 0, 1)])).unwrap();
        assert!(report.terms_are_merged_sorted_and_zero_free);
        assert!(report.a_negative_coefficient_flips_the_interval_ends);
        assert!(report.bounds_read_only_the_domain_hull);
        assert!(report.a_constant_only_expression_never_touches_a_domain);
        assert!(report.a_missing_domain_is_an_error_not_a_bound);
        assert!(report.the_multiplication_gate_is_unreachable_while_the_other_two_are_not);
        assert!(report.evaluate_is_exact_and_never_consults_a_domain);
        assert!(report.the_bounds_always_contain_every_reachable_value);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_expression_bounds(&request(0, vec![term(" ", 1)], Vec::new()))
            .unwrap_err().message.contains("term IDs must not be blank"));
        assert!(verify_expression_bounds(&request(0, Vec::new(),
            vec![ExpressionDomainSpec { id: " ".to_owned(), lower: Some(0), upper: Some(1),
                values: Vec::new() }])).unwrap_err().message.contains("domain IDs must not be blank"));
        assert!(verify_expression_bounds(&request(0, Vec::new(),
            vec![ExpressionDomainSpec { id: "x".to_owned(), lower: None, upper: Some(1),
                values: Vec::new() }])).unwrap_err().message.contains("needs a lower bound"));
        assert!(verify_expression_bounds(&request(0, Vec::new(),
            vec![range("x", 5, 1)])).unwrap_err().message
            .contains("lower bound exceeds upper bound"));
        assert!(verify_expression_bounds(&request(0,
            (0..65).map(|index| term(&format!("v{index}"), 1)).collect(), Vec::new()))
            .unwrap_err().message.contains("at most 64 terms"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_expression_bounds(&request(0, vec![term("x", 1)],
            vec![range("x", 0, 1)])).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"termsAreMergedSortedAndZeroFree\""), "{json}");
        assert!(json.contains("\"boundsReadOnlyTheDomainHull\""), "{json}");
        assert!(!json.contains("terms_are"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ExpressionBoundsReport>(&copy.to_string()).is_err());
    }
}
