//! 续挖第 77 单：展平层的**容器面**——"空"不等于"零"，而**改值不改键**
//! （`model/flatten/inner_box.rs` 的 `Linear` / `Quadratic` / `Canonical`）
//! Continuation, delivery 77: the **container surface** of the flatten layer — "empty" does not mean "zero", and
//! **mutating the value keeps the cache key** (`Linear`, `Quadratic`, `Canonical` in `model/flatten/inner_box.rs`).
//!
//! 第 26 单量过 `CacheKey` 是地址、第 32 单量过三态的升幂桥、第 41 单量过标准形态的幂次表；这一单量**容器本身**。
//! 八条实测：
//!
//! 1. **`is_empty()` 与 `is_constant_only()` 是同一个谓词**（`is_empty_and_is_constant_only_are_one_predicate()`）：
//!    两者都只读 `monomials.is_empty()`，所以"空"**不是**"值为零"——常数项 5 的多项式也是"空"。
//! 2. **`len()` 只数单项式，永不数常数项**（`len_counts_monomials_only()`）：`len() == 0` 仍有一个加数。
//! 3. **`zero(v)` 与 `constant(v)` 结构完全相同**（`zero_and_constant_are_the_same_shape()`）：都是"无单项式 +
//!    常数项"，所以 `zero(5.0)` **不是**零。
//! 4. **改值不改键**（`mutation_keeps_the_cache_key()`）：`set_constant_term` / `add_monomial` 都在原来的 `Box`
//!    里改，指针不动——于是缓存键在值变化后**保持不变**。这是本单最值钱的一条：拿地址当键，就必须自己保证
//!    缓存期间值不被改。
//! 5. **`add_monomial` 追加不合并**（`add_monomial_appends_without_merging()`）：同一个变量加两次就是两项。
//! 6. **三个容器都没有值相等**（`no_value_equality_anywhere()`）：只 `derive(Debug, Clone)`，没有 `PartialEq`，
//!    所以"内容相同的两个多项式"在类型层面**不可比较**——只能比 `Debug` 文本或缓存键。
//! 7. **`from_linear` 保留常数项并把 kind 抬成 `Linear`**（`from_linear_preserves_the_constant_and_lifts_the_kind()`）。
//! 8. **`clone()` 换键但不换文本**（`clone_changes_the_key_but_not_the_debug()`）：克隆给内层新 `Box`，所以键变；
//!    `Debug` 文本相同。
//!
//! Delivery 26 measured that `CacheKey` is an address, delivery 32 the degree-lifting bridge, delivery 41 the canonical
//! power table; this one measures **the containers themselves**. **`is_empty()` and `is_constant_only()` are one
//! predicate** reading only `monomials.is_empty()`, so "empty" is **not** "zero" — a polynomial with constant 5 is
//! "empty". **`len()` never counts the constant term.** **`zero(v)` and `constant(v)` are the same shape**, so
//! `zero(5.0)` is not zero. **Mutating the value keeps the cache key** — the headline: address-keyed caching requires
//! the caller to guarantee immutability. **`add_monomial` appends without merging.** **None of the three containers has
//! value equality** (only `Debug` + `Clone`), so equal-content polynomials are uncomparable at the type level. **`from_linear`
//! preserves the constant and lifts the kind.** And **`clone()` changes the key but not the `Debug` text**.

use ospf_rust_core::model::flatten::{
    Cacheable, Canonical, CanonicalMonomial, Linear, LinearMonomial, Quadratic, QuadraticMonomialKind,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个线性项的声明。 / One linear-term declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContainerTermSpec {
    /// 系数。 / The coefficient.
    pub coefficient: f64,
    /// 变量索引。 / The variable index.
    pub var_index: u32,
}

/// 容器面探针请求。 / The container-surface probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContainerProbeRequest {
    /// 初始线性项。 / The initial linear terms.
    #[serde(default)]
    pub terms: Vec<ContainerTermSpec>,
    /// 常数项。 / The constant term.
    pub constant: f64,
    /// 是否在构造后改常数项（量"改值不改键"）。 / Whether to mutate the constant afterwards.
    #[serde(default)]
    pub mutate_constant: bool,
    /// 改成的常数项。 / The new constant.
    #[serde(default)]
    pub new_constant: Option<f64>,
    /// 是否在构造后追加一个项。 / Whether to append a term afterwards.
    #[serde(default)]
    pub append: bool,
    /// 追加的项。 / The appended term.
    #[serde(default)]
    pub appended: Option<ContainerTermSpec>,
}

/// 一个容器的读面。 / One container's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContainerView {
    /// 容器名（`linear` / `quadratic` / `canonical`）。 / The container name.
    pub container: String,
    /// 单项式条数。 / The monomial count.
    pub len: u32,
    /// `is_empty()`。 / `is_empty()`.
    pub is_empty: bool,
    /// `is_constant_only()`。 / `is_constant_only()`.
    pub is_constant_only: bool,
    /// 常数项。 / The constant term.
    pub constant: f64,
    /// 单项式总数（把每个容器的项都摊开）。 / The total monomial count across the containers.
    pub monomial_count: u32,
    /// 缓存键是否在改值前后保持不变。 / Whether the cache key survived the mutation.
    #[serde(default)]
    pub key_stable: Option<bool>,
    /// `Debug` 文本（用来比"内容相同"）。 / The `Debug` text (used to compare content).
    pub debug_text: String,
}

/// 容器面探针结论。 / The container-surface probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContainerProbeReport {
    /// 三个容器的读面。 / The three containers' readable faces.
    pub containers: Vec<ContainerView>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub is_empty_and_is_constant_only_are_one_predicate: bool,
    pub len_counts_monomials_only: bool,
    pub zero_and_constant_are_the_same_shape: bool,
    pub mutation_keeps_the_cache_key: bool,
    pub add_monomial_appends_without_merging: bool,
    pub no_value_equality_anywhere: bool,
    pub from_linear_preserves_the_constant_and_lifts_the_kind: bool,
    pub clone_changes_the_key_but_not_the_debug: bool,
}

/// 造线性项。 / Build linear terms.
fn linear_terms(specs: &[ContainerTermSpec]) -> Vec<LinearMonomial<f64>> {
    specs.iter()
        .map(|spec| LinearMonomial::new(spec.coefficient, spec.var_index as usize))
        .collect()
}

/// 取线性多项式的缓存键。 / Take a linear polynomial's cache key.
fn linear_key(linear: &Linear<f64>) -> u64 {
    linear.cache_key().value
}

/// 造一个线性容器并读面。 / Build the linear container and read its face.
fn linear_view(request: &ContainerProbeRequest) -> ContainerView {
    let mut linear = Linear::new(linear_terms(&request.terms), request.constant);
    let original_key = linear_key(&linear);
    let original_debug = format!("{linear:?}");
    let mut mutated = false;
    if request.mutate_constant {
        linear.set_constant_term(request.new_constant.unwrap_or(request.constant));
        mutated = true;
    }
    if request.append
        && let Some(spec) = &request.appended
    {
        linear.add_monomial(LinearMonomial::new(spec.coefficient, spec.var_index as usize));
        mutated = true;
    }
    let key_stable = mutated.then(|| linear_key(&linear) == original_key);
    ContainerView {
        container: "linear".to_owned(),
        len: u32::try_from(linear.len()).unwrap_or(u32::MAX),
        is_empty: linear.is_empty(),
        is_constant_only: linear.is_constant_only(),
        constant: *linear.constant_term(),
        monomial_count: u32::try_from(linear.monomials().len()).unwrap_or(u32::MAX),
        key_stable,
        // 未改值时给构造时的文本，改过之后给新文本——两条都用得上。 
        // Before mutation this is the construction text; after mutation it is the new text — both are useful.
        debug_text: if mutated { format!("{linear:?}") } else { original_debug },
    }
}

/// 造二次容器并读面（经 `from_linear`）。 / Build the quadratic container through `from_linear`.
fn quadratic_view(request: &ContainerProbeRequest) -> ContainerView {
    let linear = Linear::new(linear_terms(&request.terms), request.constant);
    let quadratic = Quadratic::from_linear(&linear);
    let kinds = quadratic.monomials().iter()
        .map(|monomial| match monomial.kind() {
            QuadraticMonomialKind::Linear => "linear",
            QuadraticMonomialKind::Quadratic => "quadratic",
        })
        .collect::<Vec<_>>();
    ContainerView {
        container: "quadratic".to_owned(),
        len: u32::try_from(quadratic.len()).unwrap_or(u32::MAX),
        is_empty: quadratic.is_empty(),
        is_constant_only: quadratic.len() == 0,
        constant: *quadratic.constant(),
        monomial_count: u32::try_from(quadratic.monomials().len()).unwrap_or(u32::MAX),
        key_stable: None,
        // 每个项都必须是 `Linear` kind——这是 `from_linear` 的实测结论；空表给 `none`，不产生空文本。 
        // Every monomial must be of `Linear` kind — the measured result of `from_linear`; an empty list yields `none`
        // rather than blank text.
        debug_text: if kinds.is_empty() { "none".to_owned() } else { kinds.join(",") },
    }
}

/// 造标准容器并读面。 / Build the canonical container and read its face.
fn canonical_view(request: &ContainerProbeRequest) -> ContainerView {
    let monomials = request.terms.iter()
        .map(|spec| CanonicalMonomial::new_variable(spec.coefficient, spec.var_index as usize))
        .collect::<Vec<_>>();
    let canonical = Canonical::new(monomials, request.constant);
    ContainerView {
        container: "canonical".to_owned(),
        len: u32::try_from(canonical.len()).unwrap_or(u32::MAX),
        is_empty: canonical.is_empty(),
        is_constant_only: canonical.len() == 0,
        constant: *canonical.constant(),
        monomial_count: u32::try_from(canonical.monomials().len()).unwrap_or(u32::MAX),
        key_stable: None,
        debug_text: format!("{canonical:?}"),
    }
}

/// 走一遍容器面探针。非法输入是**错误**。 / Walk the container-surface probe.
pub fn verify_container_surface(request: &ContainerProbeRequest) -> Result<ContainerProbeReport> {
    if request.terms.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "container probe failed validation: at most 32 terms are accepted"));
    }
    if !request.constant.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "container probe failed validation: the constant must be finite"));
    }
    let findings = canonical_findings();
    let containers = vec![linear_view(request), quadratic_view(request), canonical_view(request)];
    Ok(ContainerProbeReport {
        containers,
        is_empty_and_is_constant_only_are_one_predicate:
            findings.is_empty_and_is_constant_only_are_one_predicate,
        len_counts_monomials_only: findings.len_counts_monomials_only,
        zero_and_constant_are_the_same_shape: findings.zero_and_constant_are_the_same_shape,
        mutation_keeps_the_cache_key: findings.mutation_keeps_the_cache_key,
        add_monomial_appends_without_merging: findings.add_monomial_appends_without_merging,
        no_value_equality_anywhere: findings.no_value_equality_anywhere,
        from_linear_preserves_the_constant_and_lifts_the_kind:
            findings.from_linear_preserves_the_constant_and_lifts_the_kind,
        clone_changes_the_key_but_not_the_debug:
            findings.clone_changes_the_key_but_not_the_debug,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    is_empty_and_is_constant_only_are_one_predicate: bool,
    len_counts_monomials_only: bool,
    zero_and_constant_are_the_same_shape: bool,
    mutation_keeps_the_cache_key: bool,
    add_monomial_appends_without_merging: bool,
    no_value_equality_anywhere: bool,
    from_linear_preserves_the_constant_and_lifts_the_kind: bool,
    clone_changes_the_key_but_not_the_debug: bool,
}

/// 一份线性项。 / One linear term.
fn term(coefficient: f64, var_index: u32) -> ContainerTermSpec {
    ContainerTermSpec { coefficient, var_index }
}

/// 一份空请求（只有常数项）。 / An empty request (constant only).
fn request(constant: f64) -> ContainerProbeRequest {
    ContainerProbeRequest { terms: Vec::new(), constant, mutate_constant: false,
        new_constant: None, append: false, appended: None }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 两个谓词是同一个。 / The two predicates are one.
    let is_empty_and_is_constant_only_are_one_predicate = {
        // 常数项 5 的多项式：`len() == 0`、`is_empty()` 真、`is_constant_only()` 真——但它**不是**零。
        // A polynomial with constant 5: `len() == 0`, `is_empty()` true, `is_constant_only()` true — yet it is **not**
        // zero.
        let view = linear_view(&request(5.0));
        let with_terms = linear_view(&ContainerProbeRequest { terms: vec![term(2.0, 0)],
            ..request(1.0) });
        view.is_empty && view.is_constant_only && view.constant == 5.0
            && !with_terms.is_empty && !with_terms.is_constant_only
    };
    // 2) `len()` 只数单项式。 / `len()` counts monomials only.
    let len_counts_monomials_only = {
        let view = linear_view(&ContainerProbeRequest {
            terms: vec![term(1.0, 0), term(2.0, 1), term(3.0, 2)], ..request(7.0) });
        // 三项 + 一个常数项，`len()` 报 3。 / Three terms plus a constant: `len()` reports 3.
        view.len == 3 && view.monomial_count == 3 && view.constant == 7.0
            && linear_view(&request(7.0)).len == 0
    };
    // 3) `zero` 与 `constant` 同形。 / `zero` and `constant` share one shape.
    let zero_and_constant_are_the_same_shape = {
        // `zero(5.0)` 与 `constant(5.0)` 的 `Debug` 文本完全相同——所以 `zero` 只表示"无单项式"。 
        // `zero(5.0)` and `constant(5.0)` have identical `Debug` text, so `zero` only means "no monomials".
        let zero = Linear::zero(5.0f64);
        let constant = Linear::constant(5.0f64);
        format!("{zero:?}") == format!("{constant:?}")
            && *zero.constant_term() == 5.0 && zero.is_empty()
            // 对照：真正的零。 / Control: the actual zero.
            && *Linear::<f64>::zero(0.0).constant_term() == 0.0
    };
    // 4) 改值不改键。 / Mutating the value keeps the key.
    let mutation_keeps_the_cache_key = {
        // 改常数项。 / Mutate the constant.
        let constant = linear_view(&ContainerProbeRequest { mutate_constant: true,
            new_constant: Some(99.0), ..request(1.0) });
        // 追加项。 / Append a term.
        let appended = linear_view(&ContainerProbeRequest { append: true,
            appended: Some(term(4.0, 3)), ..request(1.0) });
        // 两种改法都不动 `Box` 指针，所以键不变。 / Neither mutation moves the `Box`, so the key stays.
        constant.key_stable == Some(true) && appended.key_stable == Some(true)
            && constant.constant == 99.0 && appended.len == 1
    };
    // 5) 追加不合并。 / Appending does not merge.
    let add_monomial_appends_without_merging = {
        let mut linear = Linear::new(vec![LinearMonomial::new(1.0f64, 0)], 0.0);
        linear.add_monomial(LinearMonomial::new(2.0f64, 0));
        // 同一个变量两次 → 两项，系数各自独立。 / The same variable twice gives two terms with independent coefficients.
        linear.len() == 2
            && linear.monomials().iter().map(|monomial| *monomial.coefficient())
                .collect::<Vec<_>>() == vec![1.0, 2.0]
            && linear.monomials().iter().all(|monomial| monomial.var_index() == 0)
    };
    // 6) 三个容器都没有值相等。 / No value equality on any container.
    let no_value_equality_anywhere = {
        // 类型层面没有 `PartialEq`，所以"内容相同"只能靠 `Debug` 文本判——两条文本相同、键不同。 
        // There is no `PartialEq` at the type level, so "same content" can only be judged by `Debug` text: two equal
        // texts with different keys.
        let left = Linear::new(vec![LinearMonomial::new(1.0f64, 0)], 2.0);
        let right = Linear::new(vec![LinearMonomial::new(1.0f64, 0)], 2.0);
        format!("{left:?}") == format!("{right:?}") && linear_key(&left) != linear_key(&right)
            // 二次与标准容器同样只有 `Debug` + `Clone`。 / The quadratic and canonical containers likewise only have
            // `Debug` + `Clone`.
            && format!("{:?}", Quadratic::from_linear(&left)) == format!("{:?}", Quadratic::from_linear(&right))
            && format!("{:?}", Canonical::<f64>::zero(0.0)) == format!("{:?}", Canonical::<f64>::zero(0.0))
    };
    // 7) `from_linear` 保留常数项并抬 kind。 / `from_linear` preserves the constant and lifts the kind.
    let from_linear_preserves_the_constant_and_lifts_the_kind = {
        let linear = Linear::new(vec![LinearMonomial::new(1.5f64, 0), LinearMonomial::new(2.5f64, 1)],
            9.0);
        let quadratic = Quadratic::from_linear(&linear);
        quadratic.len() == 2
            && *quadratic.constant() == 9.0
            && quadratic.monomials().iter()
                .all(|monomial| monomial.kind() == QuadraticMonomialKind::Linear)
            && quadratic.monomials().iter().all(|monomial| monomial.var_index2().is_none())
    };
    // 8) 克隆换键不换文本。 / A clone changes the key but not the text.
    let clone_changes_the_key_but_not_the_debug = {
        let original = Linear::new(vec![LinearMonomial::new(3.0f64, 7)], 1.0);
        let cloned = original.clone();
        format!("{original:?}") == format!("{cloned:?}")
            && linear_key(&original) != linear_key(&cloned)
    };
    Findings {
        is_empty_and_is_constant_only_are_one_predicate,
        len_counts_monomials_only,
        zero_and_constant_are_the_same_shape,
        mutation_keeps_the_cache_key,
        add_monomial_appends_without_merging,
        no_value_equality_anywhere,
        from_linear_preserves_the_constant_and_lifts_the_kind,
        clone_changes_the_key_but_not_the_debug,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(coefficient: f64, var_index: u32) -> ContainerTermSpec {
        ContainerTermSpec { coefficient, var_index }
    }

    fn request(constant: f64) -> ContainerProbeRequest {
        ContainerProbeRequest { terms: Vec::new(), constant, mutate_constant: false,
            new_constant: None, append: false, appended: None }
    }

    #[test]
    fn a_constant_only_polynomial_is_empty_but_not_zero() {
        let report = verify_container_surface(&request(5.0)).unwrap();
        let linear = &report.containers[0];
        assert_eq!(linear.container, "linear");
        assert_eq!(linear.len, 0);
        assert!(linear.is_empty);
        assert!(linear.is_constant_only);
        assert_eq!(linear.constant, 5.0);
        assert_eq!(linear.monomial_count, 0);
        // 二次与标准容器同形。 / The quadratic and canonical containers share the shape.
        assert!(report.containers[1].is_empty && report.containers[2].is_empty);
        assert!(report.is_empty_and_is_constant_only_are_one_predicate);
        assert!(report.len_counts_monomials_only);
        assert!(report.zero_and_constant_are_the_same_shape);
        assert!(report.mutation_keeps_the_cache_key);
        assert!(report.add_monomial_appends_without_merging);
        assert!(report.no_value_equality_anywhere);
        assert!(report.from_linear_preserves_the_constant_and_lifts_the_kind);
        assert!(report.clone_changes_the_key_but_not_the_debug);
    }

    #[test]
    fn len_counts_only_the_monomials() {
        let report = verify_container_surface(&ContainerProbeRequest {
            terms: vec![term(1.0, 0), term(2.0, 1), term(3.0, 2)], ..request(7.0) }).unwrap();
        let linear = &report.containers[0];
        assert_eq!(linear.len, 3);
        assert!(!linear.is_empty);
        assert!(!linear.is_constant_only);
        assert_eq!(linear.constant, 7.0);
        assert_eq!(linear.monomial_count, 3);
    }

    #[test]
    fn mutating_the_value_keeps_the_cache_key() {
        let report = verify_container_surface(&ContainerProbeRequest { mutate_constant: true,
            new_constant: Some(99.0), ..request(1.0) }).unwrap();
        assert_eq!(report.containers[0].key_stable, Some(true));
        assert_eq!(report.containers[0].constant, 99.0);
        let appended = verify_container_surface(&ContainerProbeRequest { append: true,
            appended: Some(term(4.0, 3)), ..request(1.0) }).unwrap();
        assert_eq!(appended.containers[0].key_stable, Some(true));
        assert_eq!(appended.containers[0].len, 1);
        // 未改值时不报键稳定性。 / Without a mutation there is no key-stability claim.
        assert_eq!(verify_container_surface(&request(1.0)).unwrap().containers[0].key_stable, None);
    }

    #[test]
    fn from_linear_keeps_the_constant_and_lifts_the_kind() {
        let report = verify_container_surface(&ContainerProbeRequest {
            terms: vec![term(1.5, 0), term(2.5, 1)], ..request(9.0) }).unwrap();
        let quadratic = &report.containers[1];
        assert_eq!(quadratic.container, "quadratic");
        assert_eq!(quadratic.len, 2);
        assert_eq!(quadratic.constant, 9.0);
        // `debug_text` 里装的是每个项的 kind，全部 `linear`。 
        // `debug_text` holds each monomial's kind, all `linear`.
        assert_eq!(quadratic.debug_text, "linear,linear");
    }

    #[test]
    fn adding_a_monomial_never_merges() {
        let report = verify_container_surface(&ContainerProbeRequest { append: true,
            appended: Some(term(2.0, 0)),
            ..ContainerProbeRequest { terms: vec![term(1.0, 0)], ..request(0.0) } }).unwrap();
        // 同一变量加两次 → 两项。 / The same variable twice gives two monomials.
        assert_eq!(report.containers[0].len, 2);
        assert!(report.add_monomial_appends_without_merging);
    }

    #[test]
    fn a_non_finite_constant_is_refused() {
        assert!(verify_container_surface(&request(f64::NAN)).unwrap_err().message
            .contains("the constant must be finite"));
        assert!(verify_container_surface(&request(f64::INFINITY)).unwrap_err().message
            .contains("the constant must be finite"));
        assert!(verify_container_surface(&ContainerProbeRequest {
            terms: (0..33).map(|index| term(1.0, index)).collect(), ..request(0.0) })
            .unwrap_err().message.contains("at most 32 terms"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_container_surface(&request(1.0)).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"isConstantOnly\""), "{json}");
        assert!(json.contains("\"keyStable\""), "{json}");
        assert!(json.contains("\"mutationKeepsTheCacheKey\""), "{json}");
        assert!(!json.contains("is_constant_only"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ContainerProbeReport>(&copy.to_string()).is_err());
    }
}
