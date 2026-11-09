//! 续挖第 32 单：稀疏多项式三态的升幂契约（`model/flatten/inner_box.rs` 的 `Linear` / `Quadratic` /
//! `Canonical`）
//! Continuation, delivery 32: the degree-lifting contract of the three sparse polynomial states.
//!
//! 展平层有**三种**稀疏表示，但"能不能升幂"并不对称——这是本单的主结论：
//!
//! * **`Linear → Quadratic` 是完备的**：`Quadratic::from_linear(&linear)` 保留常数项，并把每个线性项变成
//!   `kind() == Linear` 的二次单项式（`var_index2` 为 `None`）；`from_constant` 只造常数。
//! * **`Quadratic → Canonical` 没有对应构造器**：`Canonical` 只有 `new(monomials, constant)` 与
//!   `from_constant`，没有 `from_linear` / `from_quadratic`。所以"线性 → 二次"有现成的桥，
//!   "二次 → 任意次"必须自己摊平幂次表。
//! * **`Canonical` 的多项式语义靠幂次表**：`CanonicalMonomial::new(coeff, powers)` 的 `total_power()` 是幂之和，
//!   `get_power(未出现的变量)` 返回 **`0`**（不是"无此项"），所以"某个变量没有幂次"与"幂次为 0"在这里**不可区分**。
//! * **`add_monomial` 不合并**：同一变量加两次就是两个单项式，`len()` 变 2。
//!
//! The flatten layer has **three** sparse representations, but lifting a degree is **asymmetric**, which is this delivery's
//! main point. **`Linear → Quadratic` is complete**: `Quadratic::from_linear(&linear)` keeps the constant term and turns every
//! linear term into a `kind() == Linear` quadratic monomial (`var_index2` is `None`), while `from_constant` builds a constant
//! only. **`Quadratic → Canonical` has no constructor at all**: `Canonical` offers only `new(monomials, constant)` and
//! `from_constant`, never `from_linear` / `from_quadratic`, so "linear to quadratic" has a ready bridge while "quadratic to
//! arbitrary degree" must flatten the power table by hand. **`Canonical`'s semantics live in the power table**:
//! `CanonicalMonomial::new(coeff, powers)` reports `total_power()` as the sum of powers and `get_power(missing variable)`
//! returns **`0`** (not "no such term"), so "this variable has no power" and "its power is zero" are **indistinguishable**
//! there. And **`add_monomial` never merges**: adding the same variable twice yields two monomials and `len()` becomes 2.

use ospf_rust_core::model::flatten::inner_box::{
    Canonical, CanonicalMonomial, Linear, LinearMonomial, Quadratic, QuadraticMonomialKind,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个线性项字面量。 / One linear term literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearTermSpec {
    /// 变量序号。 / The variable index.
    pub variable: u64,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// 一个幂次字面量。 / One power literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PowerSpec {
    /// 变量序号。 / The variable index.
    pub variable: u64,
    /// 幂次。 / The power.
    pub power: i32,
}

/// 多项式三态请求。 / The three-state polynomial request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolynomialLiftRequest {
    /// 常数项。 / The constant term.
    pub constant: f64,
    /// 线性项（**允许同一变量出现多次**）。 / The linear terms (**duplicates allowed**).
    #[serde(default)]
    pub terms: Vec<LinearTermSpec>,
    /// 规范化单项式的幂次表。 / The power table of the canonical monomial.
    #[serde(default)]
    pub powers: Vec<PowerSpec>,
    /// 用来查"不存在的幂次"的变量序号。 / The variable index used to look up an absent power.
    #[serde(default)]
    pub missing_variable: u64,
}

/// 三态的形状。 / The three states' shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolynomialLiftReport {
    /// 线性态的单项式数。 / The linear state's monomial count.
    pub linear_count: u64,
    /// 线性态的常数项。 / The linear state's constant.
    pub linear_constant: f64,
    /// 线性态是否"只有常数"。 / Whether the linear state is constant-only.
    pub linear_is_constant_only: bool,
    /// 二次态的单项式数。 / The quadratic state's monomial count.
    pub quadratic_count: u64,
    /// 二次态的常数项。 / The quadratic state's constant.
    pub quadratic_constant: f64,
    /// 二次态里全是线性项（**是**）。 / Whether every quadratic monomial is a linear one (**yes**).
    pub quadratic_all_linear_kind: bool,
    /// 规范化态的单项式数。 / The canonical state's monomial count.
    pub canonical_count: u64,
    /// 规范化态的常数项。 / The canonical state's constant.
    pub canonical_constant: f64,
    /// 规范化单项式的总幂次。 / The canonical monomial's total power.
    pub canonical_total_power: i32,
    /// 查不存在的幂次得到 0。 / Whether an absent power reads back as 0.
    pub missing_power_is_zero: i32,
    /// 零与常数是同一个形状（**是**）。 / Whether "zero" and "constant" are the same shape (**yes**).
    pub zero_matches_constant: bool,
    /// `from_linear` 保留常数项（**是**）。 / Whether `from_linear` keeps the constant (**yes**).
    pub from_linear_keeps_constant: bool,
    /// `from_linear` 把每一项变成线性 kind（**是**）。 / Whether `from_linear` makes every term a linear kind (**yes**).
    pub from_linear_keeps_arity: bool,
    /// 规范化态只有 `new` / `from_constant` 两个入口（**是**）。 / Whether the canonical state only has `new` /
    /// `from_constant` (**yes**).
    pub canonical_has_no_lift: bool,
    /// `add_monomial` 不合并重复项（**是**）。 / Whether `add_monomial` does not merge duplicates (**yes**).
    pub add_monomial_does_not_merge: bool,
    /// 只有常数的线性态同时是"空"（**是**）。 / Whether a constant-only linear state is also "empty" (**yes**).
    pub constant_only_is_empty: bool,
}

/// 项数上限（信任边界）。 / The term cap (a trust boundary).
const MAX_TERMS: usize = 1_024;
/// 变量序号上限。 / The variable index cap.
const MAX_INDEX: u64 = u32::MAX as u64;

/// 走一遍三态升幂。非法输入是**错误**。 / Walk the three-state lifting.
pub fn verify_polynomial_lift(request: &PolynomialLiftRequest) -> Result<PolynomialLiftReport> {
    if request.terms.len() > MAX_TERMS || request.powers.len() > MAX_TERMS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "polynomial lift failed validation: at most {MAX_TERMS} entries are accepted")));
    }
    if !request.constant.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "polynomial lift failed validation: the constant must be finite"));
    }
    for term in &request.terms {
        if !term.coefficient.is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "polynomial lift failed validation: every coefficient must be finite"));
        }
        if term.variable > MAX_INDEX {
            return Err(BridgeError::new("InvalidInput", &format!(
                "polynomial lift failed validation: variable indices must be below {MAX_INDEX}")));
        }
    }
    for power in &request.powers {
        if power.variable > MAX_INDEX {
            return Err(BridgeError::new("InvalidInput", &format!(
                "polynomial lift failed validation: variable indices must be below {MAX_INDEX}")));
        }
        if power.power < 0 {
            return Err(BridgeError::new("InvalidInput",
                "polynomial lift failed validation: powers must be non-negative"));
        }
    }
    if request.missing_variable > MAX_INDEX {
        return Err(BridgeError::new("InvalidInput", &format!(
            "polynomial lift failed validation: variable indices must be below {MAX_INDEX}")));
    }
    let linear = build_linear(request);
    let quadratic = Quadratic::from_linear(&linear);
    let powers: std::collections::HashMap<usize, i32> = request.powers.iter()
        .map(|power| (power.variable as usize, power.power)).collect();
    let canonical_monomial = CanonicalMonomial::new(request.constant, powers);
    let canonical = Canonical::new(vec![canonical_monomial.clone()], request.constant);
    let findings = canonical_findings();
    Ok(PolynomialLiftReport {
        linear_count: linear.len() as u64,
        linear_constant: *linear.constant_term(),
        linear_is_constant_only: linear.is_constant_only(),
        quadratic_count: quadratic.len() as u64,
        quadratic_constant: *quadratic.constant(),
        quadratic_all_linear_kind: quadratic.monomials().iter()
            .all(|monomial| monomial.kind() == QuadraticMonomialKind::Linear),
        canonical_count: canonical.len() as u64,
        canonical_constant: *canonical.constant(),
        canonical_total_power: canonical_monomial.total_power(),
        missing_power_is_zero: canonical_monomial.get_power(request.missing_variable as usize),
        zero_matches_constant: findings.zero_matches_constant,
        from_linear_keeps_constant: findings.from_linear_keeps_constant,
        from_linear_keeps_arity: findings.from_linear_keeps_arity,
        canonical_has_no_lift: findings.canonical_has_no_lift,
        add_monomial_does_not_merge: findings.add_monomial_does_not_merge,
        constant_only_is_empty: findings.constant_only_is_empty,
    })
}

/// 按请求造线性态。 / Build the linear state from the request.
fn build_linear(request: &PolynomialLiftRequest) -> Linear<f64> {
    let monomials: Vec<LinearMonomial<f64>> = request.terms.iter()
        .map(|term| LinearMonomial::new(term.coefficient, term.variable as usize))
        .collect();
    Linear::new(monomials, request.constant)
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    zero_matches_constant: bool,
    from_linear_keeps_constant: bool,
    from_linear_keeps_arity: bool,
    canonical_has_no_lift: bool,
    add_monomial_does_not_merge: bool,
    constant_only_is_empty: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // `zero(v)` 与 `constant(v)` 形状相同：无单项式、常数项为 v。
    // `zero(v)` and `constant(v)` share a shape: no monomials, constant v.
    let zero = Linear::<f64>::zero(7.0);
    let constant = Linear::<f64>::constant(7.0);
    let zero_matches_constant = zero.len() == constant.len()
        && *zero.constant_term() == *constant.constant_term()
        && zero.is_constant_only() == constant.is_constant_only();
    // `from_linear` 保留常数项与项数，且每一项都是线性 kind。
    // `from_linear` keeps the constant and the arity, and every term is a linear kind.
    let source = Linear::new(vec![LinearMonomial::new(1.5, 0), LinearMonomial::new(-2.0, 3)], 4.25);
    let lifted = Quadratic::from_linear(&source);
    let from_linear_keeps_constant = *lifted.constant() == *source.constant_term();
    let from_linear_keeps_arity = lifted.len() == source.len()
        && lifted.monomials().iter().all(|monomial| monomial.kind() == QuadraticMonomialKind::Linear);
    // 规范化态没有升幂构造器：只能 `new` / `from_constant`（源码层面的事实，这里用"两个入口都能用"来锚定）。
    // The canonical state has no lifting constructor: only `new` / `from_constant`, anchored here by both working.
    let from_constant = Canonical::<f64>::from_constant(3.5);
    let canonical_has_no_lift = from_constant.len() == 0 && *from_constant.constant() == 3.5
        && Canonical::new(Vec::new(), 3.5).len() == 0;
    // `add_monomial` 不合并：同一变量两次 → 两个单项式。
    // `add_monomial` does not merge: the same variable twice yields two monomials.
    let mut growing = Linear::<f64>::zero(0.0);
    growing.add_monomial(LinearMonomial::new(1.0, 2));
    growing.add_monomial(LinearMonomial::new(1.0, 2));
    let add_monomial_does_not_merge = growing.len() == 2;
    // 只有常数时同时是"空"。 / A constant-only state is also "empty".
    let constant_only_is_empty = constant.is_constant_only() && constant.is_empty()
        && !source.is_empty() && !source.is_constant_only();
    Findings { zero_matches_constant, from_linear_keeps_constant, from_linear_keeps_arity,
        canonical_has_no_lift, add_monomial_does_not_merge, constant_only_is_empty }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PolynomialLiftRequest {
        PolynomialLiftRequest { constant: 4.25,
            terms: vec![LinearTermSpec { variable: 0, coefficient: 1.5 },
                LinearTermSpec { variable: 3, coefficient: -2.0 }],
            powers: vec![PowerSpec { variable: 1, power: 2 },
                PowerSpec { variable: 4, power: 1 }],
            missing_variable: 7 }
    }

    #[test]
    fn the_linear_state_lifts_to_quadratic_and_keeps_everything() {
        let report = verify_polynomial_lift(&request()).unwrap();
        assert_eq!(report.linear_count, 2);
        assert_eq!(report.linear_constant, 4.25);
        assert!(!report.linear_is_constant_only);
        assert_eq!(report.quadratic_count, 2);
        assert_eq!(report.quadratic_constant, 4.25);
        assert!(report.quadratic_all_linear_kind);
        assert!(report.from_linear_keeps_constant);
        assert!(report.from_linear_keeps_arity);
    }

    #[test]
    fn the_canonical_state_needs_a_power_table() {
        let report = verify_polynomial_lift(&request()).unwrap();
        assert_eq!(report.canonical_count, 1);
        assert_eq!(report.canonical_constant, 4.25);
        // 幂之和：2 + 1 = 3。 / The sum of powers.
        assert_eq!(report.canonical_total_power, 3);
        // 未出现的变量读回 0，而不是"没有这个变量"。 / An absent variable reads back as 0, not as "no such variable".
        assert_eq!(report.missing_power_is_zero, 0);
        assert!(report.canonical_has_no_lift);
    }

    #[test]
    fn zero_and_constant_are_the_same_shape() {
        let report = verify_polynomial_lift(&request()).unwrap();
        assert!(report.zero_matches_constant);
        assert!(report.constant_only_is_empty);
        // 只有常数时 `is_constant_only` 与 `is_empty` 同时为真。
        // A constant-only state has both `is_constant_only` and `is_empty` true.
        let mut bare = request();
        bare.terms = Vec::new();
        let bare = verify_polynomial_lift(&bare).unwrap();
        assert!(bare.linear_is_constant_only);
        assert_eq!(bare.linear_count, 0);
        assert_eq!(bare.quadratic_count, 0);
    }

    #[test]
    fn add_monomial_does_not_merge_duplicates() {
        let mut document = request();
        document.terms = vec![LinearTermSpec { variable: 2, coefficient: 1.0 },
            LinearTermSpec { variable: 2, coefficient: 1.0 }];
        let report = verify_polynomial_lift(&document).unwrap();
        // 同一变量两次 → 两个单项式（不合并）。 / The same variable twice yields two monomials.
        assert_eq!(report.linear_count, 2);
        assert!(report.add_monomial_does_not_merge);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.constant = 0.0;
        other.terms = Vec::new();
        other.powers = Vec::new();
        other.missing_variable = 9;
        let report = verify_polynomial_lift(&other).unwrap();
        assert!(report.zero_matches_constant);
        assert!(report.from_linear_keeps_constant);
        assert!(report.from_linear_keeps_arity);
        assert!(report.canonical_has_no_lift);
        assert!(report.add_monomial_does_not_merge);
        assert!(report.constant_only_is_empty);
        assert_eq!(report.canonical_total_power, 0);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut infinite = request();
        infinite.constant = f64::NAN;
        assert!(verify_polynomial_lift(&infinite).unwrap_err().message
            .contains("constant must be finite"));
        let mut bad_coefficient = request();
        bad_coefficient.terms = vec![LinearTermSpec { variable: 0, coefficient: f64::INFINITY }];
        assert!(verify_polynomial_lift(&bad_coefficient).unwrap_err().message
            .contains("coefficient must be finite"));
        let mut negative = request();
        negative.powers = vec![PowerSpec { variable: 1, power: -1 }];
        assert!(verify_polynomial_lift(&negative).unwrap_err().message
            .contains("powers must be non-negative"));
        let mut index = request();
        index.missing_variable = MAX_INDEX + 1;
        assert!(verify_polynomial_lift(&index).unwrap_err().message.contains("must be below"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_polynomial_lift(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"canonicalTotalPower\""), "{json}");
        assert!(json.contains("\"canonicalHasNoLift\""), "{json}");
        assert!(json.contains("\"quadraticAllLinearKind\""), "{json}");
        assert!(!json.contains("canonical_total"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PolynomialLiftReport>(&copy.to_string()).is_err());
    }
}
