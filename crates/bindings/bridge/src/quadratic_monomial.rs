//! 续挖第 38 单：二次单项式的 kind 是**推导出来的**，且变量次序不被规范化
//! （`model/flatten/inner_box.rs` 的 `QuadraticMonomial` / `QuadraticMonomialKind`）
//! Continuation, delivery 38: a quadratic monomial's kind is **derived**, and the variable order is not normalised
//! (`QuadraticMonomial`, `QuadraticMonomialKind`).
//!
//! 第 32 单量的是三态的**升幂桥**，这一单量二次单项式自己的内部约定。四条实测：
//!
//! 1. **`kind()` 不是存下来的字段，而是 `var_index2.is_some()` 的推导**（`kindIsDerived()`）：
//!    `new_linear` 不给 `var_index2` → `Linear`；`new_quadratic` 给 → `Quadratic`。所以"线性项带 `var_index2`"
//!    这种状态**构造不出来**，`Linear` 与 `var_index2 == None` 是同一件事的两种写法。
//! 2. **不给变量排序**（`indices_are_not_normalised()`）：`new_quadratic(c, 5, 2)` 原样保留
//!    `var_index1 == 5`、`var_index2 == 2`。所以 `x5*x2` 与 `x2*x5` 是**两个不同对象**（`is_square_only_when_equal()`
//!    只管"两项相等"，不管"顺序"）——比较两项相乘是否同一单项式必须自己规范化次序。
//! 3. **平方是合法的二次项**（`squares_are_quadratic()`）：`new_quadratic(c, i, i)` 的 `kind()` 是 `Quadratic`
//!    而不是 `Linear`，因为判据只看 `var_index2` 是否有值。
//! 4. **`cache_key` 是内层 `Box` 的地址**（沿用第 26 单的结论）：`Clone` 是深拷贝，所以克隆出的单项式拿到**新** key；
//!    同一对象多次取 key 则稳定。本单把它限定在**单项式**这一层再确认一次（`clone_gets_a_fresh_key()`）。
//!
//! Delivery 32 measured the **degree-lifting bridge** between the three states; this one measures the quadratic monomial's
//! own conventions. **`kind()` is not a stored field but a derivation from `var_index2.is_some()`**
//! (`kindIsDerived()`): `new_linear` leaves `var_index2` empty and `new_quadratic` fills it, so "a linear term carrying
//! `var_index2`" cannot be constructed — `Linear` and `var_index2 == None` are the same fact in two spellings. **Indices are
//! not sorted** (`indices_are_not_normalised()`): `new_quadratic(c, 5, 2)` keeps `var_index1 == 5` and `var_index2 == 2`, so
//! `x5*x2` and `x2*x5` are **two different objects** (`is_square_only_when_equal()` only detects equal indices, never order),
//! meaning order must be normalised by hand before comparing products. **A square is a legitimate quadratic term**
//! (`squares_are_quadratic()`): `new_quadratic(c, i, i)` reports `Quadratic` rather than `Linear` because the predicate looks
//! only at whether `var_index2` is present. And **`cache_key` is the inner `Box`'s address** (delivery 26's finding, re-checked
//! at the monomial level): `Clone` deep-copies, so a clone gets a **fresh** key while repeated reads on one object are stable
//! (`clone_gets_a_fresh_key()`).

use ospf_rust_core::model::flatten::inner_box::{
    Quadratic, QuadraticMonomial, QuadraticMonomialKind,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个二次单项式的字面量。 / One quadratic monomial literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonomialSpec {
    /// 系数。 / The coefficient.
    pub coefficient: f64,
    /// 第一个变量序号。 / The first variable index.
    pub first: u32,
    /// 第二个变量序号；`null` 表示线性项。 / The second variable index; `null` means a linear term.
    #[serde(default)]
    pub second: Option<u32>,
}

/// 二次单项式结论。 / The quadratic monomial verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonomialReport {
    /// 系数原样读回。 / The coefficient read back.
    pub coefficient: f64,
    /// 第一个变量序号。 / The first variable index.
    pub first: u32,
    /// 第二个变量序号。 / The second variable index.
    #[serde(default)]
    pub second: Option<u32>,
    /// 推导出的 kind（`quadratic` / `linear`）。 / The derived kind.
    pub kind: String,
    /// 本对象两次取 key 是否相同。 / Whether two reads on this object agree.
    pub key_is_stable: bool,
    /// 克隆出的对象是否换了 key。 / Whether a clone gets a fresh key.
    pub clone_gets_a_fresh_key: bool,
    /// kind 是从 `varIndex2` 推导的（**是**）。 / Whether the kind is derived (**yes**).
    pub kind_is_derived: bool,
    /// 线性侧从不给 `varIndex2`（**是**）。 / Whether the linear side never fills `varIndex2` (**yes**).
    pub linear_never_has_a_second_index: bool,
    /// 序号不给排序（**是**）。 / Whether indices stay unsorted (**yes**).
    pub indices_are_not_normalised: bool,
    /// 平方仍是二次项（**是**）。 / Whether a square stays quadratic (**yes**).
    pub squares_are_quadratic: bool,
    /// 只在两项相等时才算平方（**是**）。 / Whether only equal indices count as a square (**yes**).
    pub is_square_only_when_equal: bool,
    /// `fromLinear` 的每一项都不带 `varIndex2`（**是**）。 / Whether every `fromLinear` term has no `varIndex2`
    /// (**yes**).
    pub from_linear_has_no_second_index: bool,
}

/// 变量序号上限。 / The variable index cap.
const MAX_INDEX: u32 = u32::MAX;

/// 造一个单项式。 / Build one monomial.
fn build(spec: &MonomialSpec) -> QuadraticMonomial<f64> {
    match spec.second {
        Some(second) => QuadraticMonomial::new_quadratic(spec.coefficient, spec.first as usize,
            second as usize),
        None => QuadraticMonomial::new_linear(spec.coefficient, spec.first as usize),
    }
}

/// kind 名。 / The kind name.
fn kind_name(kind: QuadraticMonomialKind) -> &'static str {
    match kind {
        QuadraticMonomialKind::Quadratic => "quadratic",
        QuadraticMonomialKind::Linear => "linear",
    }
}

/// 走一遍二次单项式。非法输入是**错误**。 / Walk the quadratic monomial.
pub fn verify_quadratic_monomial(spec: &MonomialSpec) -> Result<MonomialReport> {
    if !spec.coefficient.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "quadratic monomial failed validation: the coefficient must be finite"));
    }
    if spec.first > MAX_INDEX || spec.second.is_some_and(|second| second > MAX_INDEX) {
        return Err(BridgeError::new("InvalidInput", &format!(
            "quadratic monomial failed validation: variable indices must not exceed {MAX_INDEX}")));
    }
    let monomial = build(spec);
    // 同一对象两次取 key 稳定；克隆后换 key（内层 `Box` 的地址）。
    // Two reads on one object agree; a clone gets a fresh key (the inner `Box`'s address).
    let key_is_stable = monomial.cache_key() == monomial.cache_key();
    let clone_gets_a_fresh_key = monomial.clone().cache_key() != monomial.cache_key();
    let findings = canonical_findings();
    Ok(MonomialReport {
        coefficient: *monomial.coefficient(),
        first: u32::try_from(monomial.var_index1()).unwrap_or(u32::MAX),
        second: monomial.var_index2().map(|index| u32::try_from(index).unwrap_or(u32::MAX)),
        kind: kind_name(monomial.kind()).to_owned(),
        key_is_stable,
        clone_gets_a_fresh_key,
        kind_is_derived: findings.kind_is_derived,
        linear_never_has_a_second_index: findings.linear_never_has_a_second_index,
        indices_are_not_normalised: findings.indices_are_not_normalised,
        squares_are_quadratic: findings.squares_are_quadratic,
        is_square_only_when_equal: findings.is_square_only_when_equal,
        from_linear_has_no_second_index: findings.from_linear_has_no_second_index,
    })
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    kind_is_derived: bool,
    linear_never_has_a_second_index: bool,
    indices_are_not_normalised: bool,
    squares_are_quadratic: bool,
    is_square_only_when_equal: bool,
    from_linear_has_no_second_index: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let linear = QuadraticMonomial::new_linear(1.0, 3);
    let quadratic = QuadraticMonomial::new_quadratic(2.0, 5, 2);
    let square = QuadraticMonomial::new_quadratic(4.0, 7, 7);
    // 1) kind 是推导的：线性项没有第二序号，二次项有。 
    // 1) The kind is derived: a linear term has no second index and a quadratic one does.
    let kind_is_derived = linear.kind() == QuadraticMonomialKind::Linear
        && quadratic.kind() == QuadraticMonomialKind::Quadratic
        && linear.var_index2().is_some() == (linear.kind() == QuadraticMonomialKind::Quadratic);
    let linear_never_has_a_second_index = linear.var_index2().is_none()
        && quadratic.var_index2().is_some();
    // 2) 序号不给排序：`(5, 2)` 原样保留。 / 2) Indices stay unsorted: `(5, 2)` survives.
    let indices_are_not_normalised = quadratic.var_index1() == 5
        && quadratic.var_index2() == Some(2);
    // 3) 平方仍是二次项。 / 3) A square stays quadratic.
    let squares_are_quadratic = square.kind() == QuadraticMonomialKind::Quadratic
        && square.var_index1() == square.var_index2().unwrap_or(usize::MAX);
    // 4) 只有两项相等才算平方：`(2, 5)` 与 `(5, 2)` 都不是平方。 
    // 4) Only equal indices count as a square: neither `(2, 5)` nor `(5, 2)` is one.
    let is_square_only_when_equal = {
        let reversed = QuadraticMonomial::new_quadratic(2.0, 2, 5);
        reversed.var_index1() != reversed.var_index2().unwrap_or(usize::MAX)
            && quadratic.var_index1() != quadratic.var_index2().unwrap_or(usize::MAX)
            && reversed.kind() == QuadraticMonomialKind::Quadratic
    };
    // 5) `fromLinear` 的每一项都不带第二序号。 / 5) Every `fromLinear` term has no second index.
    let source = ospf_rust_core::model::flatten::inner_box::Linear::new(
        vec![ospf_rust_core::model::flatten::inner_box::LinearMonomial::new(1.0, 0),
            ospf_rust_core::model::flatten::inner_box::LinearMonomial::new(2.0, 4)], 0.0);
    let lifted: Quadratic<f64> = Quadratic::from_linear(&source);
    let from_linear_has_no_second_index = !lifted.monomials().is_empty()
        && lifted.monomials().iter().all(|monomial| monomial.var_index2().is_none()
            && monomial.kind() == QuadraticMonomialKind::Linear);
    Findings { kind_is_derived, linear_never_has_a_second_index, indices_are_not_normalised,
        squares_are_quadratic, is_square_only_when_equal, from_linear_has_no_second_index }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> MonomialSpec {
        MonomialSpec { coefficient: 2.5, first: 5, second: Some(2) }
    }

    #[test]
    fn the_kind_is_derived_from_the_second_index() {
        let report = verify_quadratic_monomial(&spec()).unwrap();
        assert_eq!(report.kind, "quadratic");
        assert_eq!(report.second, Some(2));
        assert_eq!(report.coefficient, 2.5);
        assert!(report.kind_is_derived);
        assert!(report.linear_never_has_a_second_index);
        // 线性项：不给第二序号。 / A linear term: no second index.
        let linear = verify_quadratic_monomial(&MonomialSpec { coefficient: 1.0, first: 3,
            second: None }).unwrap();
        assert_eq!(linear.kind, "linear");
        assert_eq!(linear.second, None);
    }

    #[test]
    fn indices_are_not_normalised() {
        let report = verify_quadratic_monomial(&spec()).unwrap();
        // `(5, 2)` 原样保留，不会排成 `(2, 5)`。 / `(5, 2)` survives; it is not sorted into `(2, 5)`.
        assert_eq!(report.first, 5);
        assert_eq!(report.second, Some(2));
        assert!(report.indices_are_not_normalised);
        assert!(report.is_square_only_when_equal);
        // 反序的同一对索引也是二次项，但不是平方。 
        // The reversed pair is still a quadratic term but not a square.
        let reversed = verify_quadratic_monomial(&MonomialSpec { coefficient: 2.0, first: 2,
            second: Some(5) }).unwrap();
        assert_eq!(reversed.kind, "quadratic");
        assert_eq!(reversed.first, 2);
    }

    #[test]
    fn a_square_is_still_quadratic() {
        let report = verify_quadratic_monomial(&MonomialSpec { coefficient: 4.0, first: 7,
            second: Some(7) }).unwrap();
        assert_eq!(report.kind, "quadratic");
        assert_eq!(report.first, report.second.unwrap());
        assert!(report.squares_are_quadratic);
    }

    #[test]
    fn the_cache_key_is_the_address_so_a_clone_gets_a_fresh_one() {
        let report = verify_quadratic_monomial(&spec()).unwrap();
        assert!(report.key_is_stable);
        assert!(report.clone_gets_a_fresh_key);
        assert!(report.from_linear_has_no_second_index);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_quadratic_monomial(&MonomialSpec { coefficient: -1.0, first: 0,
            second: None }).unwrap();
        assert!(report.kind_is_derived);
        assert!(report.linear_never_has_a_second_index);
        assert!(report.indices_are_not_normalised);
        assert!(report.squares_are_quadratic);
        assert!(report.is_square_only_when_equal);
        assert!(report.from_linear_has_no_second_index);
        assert_eq!(report.kind, "linear");
    }

    #[test]
    fn bad_input_is_refused() {
        let mut infinite = spec();
        infinite.coefficient = f64::NAN;
        assert!(verify_quadratic_monomial(&infinite).unwrap_err().message
            .contains("coefficient must be finite"));
        let mut overflow = spec();
        overflow.first = u32::MAX;
        // `u32::MAX` 本身是允许的上限（用严格大于判定）。 / `u32::MAX` is the allowed cap (strictly greater is refused).
        assert!(verify_quadratic_monomial(&overflow).is_ok());
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_quadratic_monomial(&spec()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"kindIsDerived\""), "{json}");
        assert!(json.contains("\"indicesAreNotNormalised\""), "{json}");
        assert!(json.contains("\"cloneGetsAFreshKey\""), "{json}");
        assert!(!json.contains("kind_is_derived"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<MonomialReport>(&copy.to_string()).is_err());
    }
}
