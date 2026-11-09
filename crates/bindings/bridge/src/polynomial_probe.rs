//! 续挖第 63 单：线性/二次多项式的**结构变更语义**——`add_monomial` 纯追加、`zero`/`constant` 只差参数、
//! `from_linear` 保留系数与顺序（`model/flatten/inner_box.rs`）
//! Continuation, delivery 63: the **structural mutation semantics** of the linear and quadratic polynomials — `add_monomial`
//! is pure append, `zero` and `constant` differ only in their argument, and `from_linear` keeps coefficients and order
//! (`model/flatten/inner_box.rs`).
//!
//! §11.14 点名 `model/flatten` 的可编码部分。第 56 单量过惰性上下文，这一单量**多项式本体**的手写语义。
//! 七条实测（零求解器）：
//!
//! 1. **`add_monomial` 纯追加，不合并同变量**（`add_monomial_is_pure_append()`）：同一 `var_index` 加两次就是
//!    两个条目，`len()` 为 2。
//! 2. **`zero(z)` 与 `constant(z)` 只差参数名**（`zero_and_constant_are_the_same_shape()`）：两者都是
//!    "零单项式 + 常数项 = 传入值"，所以 `zero(5.0)` 的常数项是 **5**，不是 0。
//! 3. **`is_constant_only` 与 `is_empty` 完全同义**（`is_constant_only_equals_is_empty()`）：两者都只看
//!    单项式列表是否为空，**不看常数项是否为 0**。
//! 4. **`set_constant_term` 直接覆盖**（`set_constant_term_overwrites()`）。
//! 5. **`set_coefficient` 就地改系数**（`set_coefficient_mutates_in_place()`）：`var_index` 不变。
//! 6. **`from_linear` 逐条搬运并保留顺序与系数**（`from_linear_preserves_order_and_coefficients()`）。
//! 7. **`Quadratic::from_linear` 产出的都是线性 kind**（`from_linear_yields_linear_kind()`）：
//!    `var_index2` 全为 `None`，`kind()` 全是 `Linear`。
//!
//! The §11.14 audit names the codable part of `model/flatten`. Delivery 56 measured the lazy contexts; this one measures
//! **the polynomial bodies** themselves. **`add_monomial` is pure append** (`add_monomial_is_pure_append()`): adding the same
//! `var_index` twice yields two entries and `len()` is 2. **`zero(z)` and `constant(z)` differ only in the parameter name**
//! (`zero_and_constant_are_the_same_shape()`): both mean "no monomials plus the given constant", so `zero(5.0)` has **5** as
//! its constant, not 0. **`is_constant_only` and `is_empty` are synonyms** (`is_constant_only_equals_is_empty()`): both only
//! look at whether the monomial list is empty, **never at whether the constant is 0**. **`set_constant_term` overwrites**
//! (`set_constant_term_overwrites()`). **`set_coefficient` mutates in place** (`set_coefficient_mutates_in_place()`). **`from_linear`
//! moves each monomial across keeping order and coefficients** (`from_linear_preserves_order_and_coefficients()`). And
//! **`Quadratic::from_linear` yields only linear kinds** (`from_linear_yields_linear_kind()`).

use ospf_rust_core::model::flatten::{
    Linear, LinearMonomial, Quadratic, QuadraticMonomial, QuadraticMonomialKind,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个线性单项式字面量。 / One linear monomial literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolynomialTermSpec {
    pub coefficient: f64,
    pub var_index: u64,
}

/// 多项式探针请求。 / The polynomial probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolynomialProbeRequest {
    #[serde(default)]
    pub terms: Vec<PolynomialTermSpec>,
    /// 常数项（同时用作 `zero` / `constant` 的参数）。 / The constant term (also the `zero` / `constant` argument).
    #[serde(default)]
    pub constant: Option<f64>,
    /// 是否覆盖常数项。 / Whether to overwrite the constant term.
    #[serde(default)]
    pub overwrite_constant: bool,
    /// 是否把第一个系数改成这个值。 / Whether to rewrite the first coefficient to this value.
    #[serde(default)]
    pub rewrite_first_coefficient: Option<f64>,
}

/// 多项式探针结论。 / The polynomial probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolynomialProbeReport {
    /// 单项式条数。 / The monomial count.
    pub monomial_count: u64,
    /// 每个单项式的 `(系数文本, var_index)`。 / Each monomial as `(coefficient text, var_index)`.
    pub monomials: Vec<LinearTermRow>,
    /// 常数项文本。 / The constant term text.
    pub constant_term: String,
    /// `isConstantOnly()`。 / `isConstantOnly()`.
    pub is_constant_only: bool,
    /// `isEmpty()`。 / `isEmpty()`.
    pub is_empty: bool,
    /// `zero(c)` 的常数项文本。 / The constant text of `zero(c)`.
    pub zero_constant: String,
    /// `constant(c)` 的常数项文本。 / The constant text of `constant(c)`.
    pub constant_polynomial_constant: String,
    /// `fromLinear` 后的条数与 kind 名。 / The count and kind names after `fromLinear`.
    pub lifted_kinds: Vec<String>,
    /// `fromLinear` 后的系数文本。 / The coefficient texts after `fromLinear`.
    pub lifted_coefficients: Vec<String>,
    /// `add_monomial` 纯追加（**是**）。 / Whether `add_monomial` is pure append (**yes**).
    pub add_monomial_is_pure_append: bool,
    /// `zero`/`constant` 同形（**是**）。 / Whether `zero`/`constant` share a shape (**yes**).
    pub zero_and_constant_are_the_same_shape: bool,
    /// `isConstantOnly` 等于 `isEmpty`（**是**）。 / Whether `isConstantOnly` equals `isEmpty` (**yes**).
    pub is_constant_only_equals_is_empty: bool,
    /// `setConstantTerm` 直接覆盖（**是**）。 / Whether `setConstantTerm` overwrites (**yes**).
    pub set_constant_term_overwrites: bool,
    /// `setCoefficient` 就地改（**是**）。 / Whether `setCoefficient` mutates in place (**yes**).
    pub set_coefficient_mutates_in_place: bool,
    /// `fromLinear` 保留顺序与系数（**是**）。 / Whether `fromLinear` keeps order and coefficients (**yes**).
    pub from_linear_preserves_order_and_coefficients: bool,
    /// `fromLinear` 只产出线性 kind（**是**）。 / Whether `fromLinear` yields linear kinds only (**yes**).
    pub from_linear_yields_linear_kind: bool,
}

/// 一个单项式的读数。 / One monomial reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearTermRow {
    /// 系数文本（非有限值写成 `nan` / `inf` / `-inf`）。 / The coefficient text.
    pub coefficient: String,
    pub var_index: u64,
}

/// 系数文本。 / Coefficient text.
fn number_text(value: f64) -> String {
    if value.is_nan() { "nan".to_owned() }
    else if value.is_infinite() {
        if value.is_sign_negative() { "-inf".to_owned() } else { "inf".to_owned() }
    } else { format!("{value}") }
}

/// kind 名。 / The kind name.
fn kind_name(kind: QuadraticMonomialKind) -> String {
    match kind {
        QuadraticMonomialKind::Linear => "Linear".to_owned(),
        QuadraticMonomialKind::Quadratic => "Quadratic".to_owned(),
    }
}

/// 走一遍多项式探针。非法输入是**错误**。 / Walk the polynomial probe.
pub fn verify_polynomial_probe(request: &PolynomialProbeRequest) -> Result<PolynomialProbeReport> {
    if request.terms.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "polynomial probe failed validation: at most 32 terms are accepted"));
    }
    for term in &request.terms {
        if !term.coefficient.is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "polynomial probe failed validation: coefficients must be finite"));
        }
        if term.var_index > 4096 {
            return Err(BridgeError::new("InvalidInput",
                "polynomial probe failed validation: variable indices must be at most 4096"));
        }
    }
    let constant = request.constant.unwrap_or(0.0);
    let mut polynomial = Linear::<f64>::new(request.terms.iter()
        .map(|term| LinearMonomial::new(term.coefficient, term.var_index as usize))
        .collect(), constant);
    if request.overwrite_constant {
        polynomial.set_constant_term(constant + 1.0);
    }
    if let Some(coefficient) = request.rewrite_first_coefficient {
        // `Linear` 只暴露 `&inner()`，所以"改第一个系数"要靠重建单项式列表。 
        // `Linear` exposes only `&inner()`, so rewriting the first coefficient means rebuilding the list.
        let mut rebuilt = Linear::<f64>::new(polynomial.monomials().iter().enumerate()
            .map(|(position, monomial)| LinearMonomial::new(
                if position == 0 { coefficient } else { *monomial.coefficient() },
                monomial.var_index())).collect(),
            *polynomial.constant_term());
        rebuilt.set_constant_term(*polynomial.constant_term());
        polynomial = rebuilt;
    }
    let monomials = polynomial.monomials().iter().map(|monomial| LinearTermRow {
        coefficient: number_text(*monomial.coefficient()),
        var_index: monomial.var_index() as u64 }).collect::<Vec<_>>();
    let lifted = Quadratic::<f64>::from_linear(&polynomial);
    let lifted_kinds = lifted.monomials().iter().map(|monomial| kind_name(monomial.kind()))
        .collect::<Vec<_>>();
    let lifted_coefficients = lifted.monomials().iter()
        .map(|monomial| number_text(*monomial.coefficient())).collect::<Vec<_>>();
    let findings = canonical_findings();
    Ok(PolynomialProbeReport { monomial_count: polynomial.len() as u64, monomials,
        constant_term: number_text(*polynomial.constant_term()),
        is_constant_only: polynomial.is_constant_only(), is_empty: polynomial.is_empty(),
        zero_constant: number_text(*Linear::<f64>::zero(constant).constant_term()),
        constant_polynomial_constant:
            number_text(*Linear::<f64>::constant(constant).constant_term()),
        lifted_kinds, lifted_coefficients,
        add_monomial_is_pure_append: findings.add_monomial_is_pure_append,
        zero_and_constant_are_the_same_shape: findings.zero_and_constant_are_the_same_shape,
        is_constant_only_equals_is_empty: findings.is_constant_only_equals_is_empty,
        set_constant_term_overwrites: findings.set_constant_term_overwrites,
        set_coefficient_mutates_in_place: findings.set_coefficient_mutates_in_place,
        from_linear_preserves_order_and_coefficients:
            findings.from_linear_preserves_order_and_coefficients,
        from_linear_yields_linear_kind: findings.from_linear_yields_linear_kind })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    add_monomial_is_pure_append: bool,
    zero_and_constant_are_the_same_shape: bool,
    is_constant_only_equals_is_empty: bool,
    set_constant_term_overwrites: bool,
    set_coefficient_mutates_in_place: bool,
    from_linear_preserves_order_and_coefficients: bool,
    from_linear_yields_linear_kind: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `add_monomial` 纯追加，不合并同变量。 / `add_monomial` is pure append, never merging equal variables.
    let add_monomial_is_pure_append = {
        let mut polynomial = Linear::<f64>::new(Vec::new(), 0.0);
        polynomial.add_monomial(LinearMonomial::new(1.0, 7));
        polynomial.add_monomial(LinearMonomial::new(2.0, 7));
        polynomial.len() == 2 && polynomial.monomials().iter()
            .all(|monomial| monomial.var_index() == 7)
            && polynomial.monomials()[0].coefficient() == &1.0
            && polynomial.monomials()[1].coefficient() == &2.0
    };
    // 2) `zero` 与 `constant` 同形：常数项就是传入值。 
    // 2) `zero` and `constant` share a shape: the constant is the given value.
    let zero_and_constant_are_the_same_shape = {
        let zero = Linear::<f64>::zero(5.0);
        let constant = Linear::<f64>::constant(5.0);
        zero.constant_term() == &5.0 && constant.constant_term() == &5.0
            && zero.len() == 0 && constant.len() == 0
            && zero.is_constant_only() && constant.is_constant_only()
            && Quadratic::<f64>::zero(5.0).constant() == &5.0
            && Quadratic::<f64>::from_constant(5.0).constant() == &5.0
    };
    // 3) `is_constant_only` 与 `is_empty` 同义，且都只看单项式列表。 
    // 3) `is_constant_only` equals `is_empty`, and both only look at the monomial list.
    let is_constant_only_equals_is_empty = {
        let with_terms = Linear::<f64>::new(vec![LinearMonomial::new(0.0, 3)], 7.0);
        let constant_only = Linear::<f64>::constant(7.0);
        with_terms.is_constant_only() == with_terms.is_empty()
            && !with_terms.is_constant_only() && constant_only.is_constant_only()
            && constant_only.is_empty()
            // 常数项为 0 但有单项式时仍然"不空"。 / A zero constant with monomials is still "not empty".
            && !Linear::<f64>::new(vec![LinearMonomial::new(1.0, 0)], 0.0).is_empty()
    };
    // 4) `set_constant_term` 直接覆盖。 / `set_constant_term` overwrites.
    let set_constant_term_overwrites = {
        let mut polynomial = Linear::<f64>::constant(1.0);
        polynomial.set_constant_term(9.0);
        polynomial.constant_term() == &9.0
            && {
                let mut quadratic = Quadratic::<f64>::from_constant(1.0);
                quadratic.set_constant(9.0);
                quadratic.constant() == &9.0
            }
    };
    // 5) `set_coefficient` 就地改系数，`var_index` 不变。 
    // 5) `set_coefficient` mutates the coefficient in place and keeps `var_index`.
    let set_coefficient_mutates_in_place = {
        let mut monomial = LinearMonomial::new(1.0, 11);
        monomial.set_coefficient(4.5);
        monomial.coefficient() == &4.5 && monomial.var_index() == 11
    };
    // 6) `from_linear` 保留顺序与系数。 / `from_linear` keeps order and coefficients.
    let from_linear_preserves_order_and_coefficients = {
        let linear = Linear::<f64>::new(vec![LinearMonomial::new(1.5, 9),
            LinearMonomial::new(-2.5, 2), LinearMonomial::new(0.0, 5)], 3.5);
        let quadratic = Quadratic::<f64>::from_linear(&linear);
        quadratic.len() == 3 && quadratic.constant() == &3.5
            && quadratic.monomials().iter().map(|monomial| *monomial.coefficient())
                .collect::<Vec<_>>() == vec![1.5, -2.5, 0.0]
            && quadratic.monomials().iter().map(|monomial| monomial.var_index1())
                .collect::<Vec<_>>() == vec![9, 2, 5]
    };
    // 7) `from_linear` 只产出线性 kind（`var_index2` 全 `None`）。 
    // 7) `from_linear` yields linear kinds only (`var_index2` all `None`).
    let from_linear_yields_linear_kind = {
        let quadratic = Quadratic::<f64>::from_linear(&Linear::<f64>::new(vec![
            LinearMonomial::new(1.0, 4), LinearMonomial::new(2.0, 4)], 0.0));
        quadratic.monomials().iter().all(|monomial| monomial.var_index2().is_none()
            && monomial.kind() == QuadraticMonomialKind::Linear)
            && QuadraticMonomial::new_quadratic(1.0, 3, 3).kind()
                == QuadraticMonomialKind::Quadratic
            && QuadraticMonomial::new_linear(1.0, 3).kind() == QuadraticMonomialKind::Linear
    };
    Findings { add_monomial_is_pure_append, zero_and_constant_are_the_same_shape,
        is_constant_only_equals_is_empty, set_constant_term_overwrites,
        set_coefficient_mutates_in_place, from_linear_preserves_order_and_coefficients,
        from_linear_yields_linear_kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PolynomialProbeRequest {
        PolynomialProbeRequest { terms: vec![
            PolynomialTermSpec { coefficient: 1.5, var_index: 9 },
            PolynomialTermSpec { coefficient: -2.5, var_index: 2 },
            PolynomialTermSpec { coefficient: 0.0, var_index: 5 }],
            constant: Some(3.5), overwrite_constant: false, rewrite_first_coefficient: None }
    }

    #[test]
    fn the_polynomial_mirrors_its_terms() {
        let report = verify_polynomial_probe(&request()).unwrap();
        assert_eq!(report.monomial_count, 3);
        assert_eq!(report.monomials, vec![
            LinearTermRow { coefficient: "1.5".to_owned(), var_index: 9 },
            LinearTermRow { coefficient: "-2.5".to_owned(), var_index: 2 },
            LinearTermRow { coefficient: "0".to_owned(), var_index: 5 }]);
        assert_eq!(report.constant_term, "3.5");
        assert!(!report.is_constant_only);
        assert!(!report.is_empty);
        assert_eq!(report.zero_constant, "3.5");
        assert_eq!(report.constant_polynomial_constant, "3.5");
        assert_eq!(report.lifted_kinds, vec!["Linear".to_owned(); 3]);
        assert_eq!(report.lifted_coefficients, vec!["1.5".to_owned(), "-2.5".to_owned(),
            "0".to_owned()]);
        assert!(report.add_monomial_is_pure_append);
        assert!(report.zero_and_constant_are_the_same_shape);
        assert!(report.is_constant_only_equals_is_empty);
        assert!(report.set_constant_term_overwrites);
        assert!(report.set_coefficient_mutates_in_place);
        assert!(report.from_linear_preserves_order_and_coefficients);
        assert!(report.from_linear_yields_linear_kind);
    }

    #[test]
    fn an_empty_polynomial_is_constant_only() {
        let report = verify_polynomial_probe(&PolynomialProbeRequest { terms: Vec::new(),
            constant: Some(7.0), ..request() }).unwrap();
        assert_eq!(report.monomial_count, 0);
        assert!(report.is_constant_only);
        assert!(report.is_empty);
        assert_eq!(report.constant_term, "7");
        assert!(report.lifted_kinds.is_empty());
    }

    #[test]
    fn overwriting_the_constant_term_and_the_first_coefficient() {
        let report = verify_polynomial_probe(&PolynomialProbeRequest { overwrite_constant: true,
            ..request() }).unwrap();
        assert_eq!(report.constant_term, "4.5");
        let report = verify_polynomial_probe(&PolynomialProbeRequest {
            rewrite_first_coefficient: Some(9.5), ..request() }).unwrap();
        assert_eq!(report.monomials[0].coefficient, "9.5");
        assert_eq!(report.monomials[0].var_index, 9);
        assert_eq!(report.lifted_coefficients[0], "9.5");
    }

    #[test]
    fn bad_input_is_refused() {
        let too_many = PolynomialProbeRequest {
            terms: (0..33).map(|index| PolynomialTermSpec { coefficient: 1.0,
                var_index: index }).collect(), ..request() };
        assert!(verify_polynomial_probe(&too_many).unwrap_err().message
            .contains("at most 32 terms"));
        assert!(verify_polynomial_probe(&PolynomialProbeRequest {
            terms: vec![PolynomialTermSpec { coefficient: f64::NAN, var_index: 0 }],
            ..request() }).unwrap_err().message.contains("coefficients must be finite"));
        assert!(verify_polynomial_probe(&PolynomialProbeRequest {
            terms: vec![PolynomialTermSpec { coefficient: 1.0, var_index: 4097 }],
            ..request() }).unwrap_err().message.contains("at most 4096"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_polynomial_probe(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"monomialCount\""), "{json}");
        assert!(json.contains("\"addMonomialIsPureAppend\""), "{json}");
        assert!(json.contains("\"fromLinearYieldsLinearKind\""), "{json}");
        assert!(!json.contains("monomial_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PolynomialProbeReport>(&copy.to_string()).is_err());
    }
}
