//! 续挖第 75 单：`IntegerDomain` 的**取值面**——基数、枚举闸门、成员判定与规范字节
//! （`model/constraint_programming/domain.rs`）
//! Continuation, delivery 75: the **value surface** of `IntegerDomain` — cardinality, the enumeration gate, membership
//! and the canonical bytes (`model/constraint_programming/domain.rs`).
//!
//! 第 74 单量过表达式的界怎么从域里读；这一单量**域本身**。八条实测：
//!
//! 1. **`values` 在构造时排序并去重，而 `validate` 会拒掉反序列化重新引入的非规范形态**
//!    （`values_are_sorted_and_deduped_at_construction()`）：乱序、重复、空表三种都被 `validate` 点名。
//! 2. **区间从不被物化**（`a_range_is_never_materialized()`）：基数纯算术，`[0, 1000000]` 不展开。
//! 3. **整个 `i64` 区间的基数恰好是 `2^64`**（`the_full_i64_range_has_a_u128_cardinality()`）：`i128` 中间量
//!    装得下，所以不溢出；也因此 `usize` 的枚举上限永远够不到它。
//! 4. **`is_boolean` 只认稀疏 `{0,1}`**（`is_boolean_accepts_only_the_sparse_pair()`）：`Range(0,1)` 不算布尔，
//!    而 `boolean()` 就是那个稀疏对。
//! 5. **枚举闸门是"大于才拒"**（`the_enumeration_gate_is_exclusive_at_the_boundary()`）：基数等于上限通过，
//!    多 1 就拒，文案同时报出两个数。
//! 6. **枚举按闭区间从下界走到上界**（`enumeration_walks_the_closed_interval()`）：含两端，且 `upper = i64::MAX`
//!    靠 `== upper` 收尾而不是溢出。
//! 7. **`contains` 是成员判定而不是包络判定**（`contains_is_membership_not_hull()`）：稀疏域的空洞不被包含。
//! 8. **同一组取值有两种规范字节**（`the_same_value_set_has_two_canonical_encodings()`）：`Range(0,1)` 与
//!    `Values([0,1])` 取值相同、指纹不同——域的形态进了指纹。
//!
//! Delivery 74 measured how an expression's bounds read a domain; this one measures **the domain itself**. **`values`
//! sorts and dedups at construction while `validate` refuses the non-canonical shapes deserialization can reintroduce.**
//! **A range is never materialized** — cardinality is pure arithmetic. **The full `i64` range has cardinality exactly
//! `2^64`**, which fits the `i128` intermediate and therefore never overflows, and which `usize` can never reach. **`is_boolean`
//! accepts only the sparse `{0,1}`** — `Range(0,1)` is not Boolean while `boolean()` is exactly that pair. **The enumeration
//! gate refuses only when strictly greater.** **Enumeration walks the closed interval** and terminates at `i64::MAX` by
//! equality rather than overflow. **`contains` is membership, not hull.** And **one value set has two canonical
//! encodings**: `Range(0,1)` and `Values([0,1])` hold the same values yet fingerprint differently.

use std::collections::BTreeMap;

use ospf_rust_core::model::constraint_programming::{
    ConstraintProgrammingModel, IntegerDomain, IntegerVariable,
};
use ospf_rust_core::solver::report::StableVariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个域的声明。 / One domain declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainProbeSpec {
    /// 读面用的 ID。 / The ID used by the readable face.
    pub id: String,
    /// 区间下界。 / The range lower bound.
    #[serde(default)]
    pub lower: Option<i64>,
    /// 区间上界。 / The range upper bound.
    #[serde(default)]
    pub upper: Option<i64>,
    /// 稀疏取值（非空即稀疏域）。 / The sparse values (non-empty means a sparse domain).
    #[serde(default)]
    pub values: Vec<i64>,
    /// 是否**绕过构造器**直接搭结构（用来量 `validate`）。 / Whether to build the struct **bypassing the constructor**
    /// (to measure `validate`).
    #[serde(default)]
    pub raw: bool,
}

/// 域取值面探针请求。 / The domain value-surface probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainProbeRequest {
    /// 待测的域。 / The domains under test.
    #[serde(default)]
    pub domains: Vec<DomainProbeSpec>,
    /// 枚举上限（默认 8）。 / The enumeration limit (default 8).
    #[serde(default)]
    pub maximum_amount: Option<u32>,
    /// 成员判定的探针值。 / The membership probe values.
    #[serde(default)]
    pub probes: Vec<i64>,
}

/// 一个域的读面。 / One domain's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainView {
    /// 读面 ID。 / The readable-face ID.
    pub id: String,
    /// 是否通过 `validate`。 / Whether `validate` passed.
    pub valid: bool,
    /// `validate` 的拒因。 / The `validate` refusal.
    #[serde(default)]
    pub invalid_reason: Option<String>,
    /// 形态（`range` / `values`）。 / The shape (`range` / `values`).
    #[serde(default)]
    pub kind: Option<String>,
    /// 下界。 / The lower bound.
    #[serde(default)]
    pub lower: Option<i64>,
    /// 上界。 / The upper bound.
    #[serde(default)]
    pub upper: Option<i64>,
    /// 基数（十进制字符串；`u128` 不能跨 JSON）。 / The cardinality (a decimal string; `u128` cannot cross JSON).
    #[serde(default)]
    pub cardinality: Option<String>,
    /// 是否恰好是布尔域。 / Whether this is exactly the Boolean domain.
    #[serde(default)]
    pub is_boolean: Option<bool>,
    /// 枚举结果（被拒时为空）。 / The enumeration result (empty when refused).
    pub enumerated: Vec<i64>,
    /// 枚举拒因。 / The enumeration refusal.
    #[serde(default)]
    pub enumeration_refusal: Option<String>,
    /// 每个探针值是否被包含（与请求的 `probes` 等长）。 / Whether each probe value is contained.
    pub contains: Vec<bool>,
}

/// 域取值面探针结论。 / The domain value-surface probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainProbeReport {
    /// 每个域的读面。 / Each domain's readable face.
    pub domains: Vec<DomainView>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub values_are_sorted_and_deduped_at_construction: bool,
    pub a_range_is_never_materialized: bool,
    pub the_full_i64_range_has_a_u128_cardinality: bool,
    pub is_boolean_accepts_only_the_sparse_pair: bool,
    pub the_enumeration_gate_is_exclusive_at_the_boundary: bool,
    pub enumeration_walks_the_closed_interval: bool,
    pub contains_is_membership_not_hull: bool,
    pub the_same_value_set_has_two_canonical_encodings: bool,
}

/// 造一个域（可选绕过构造器）。 / Build a domain, optionally bypassing the constructor.
fn domain_of(spec: &DomainProbeSpec) -> Result<IntegerDomain> {
    let built = if spec.values.is_empty() {
        let lower = spec.lower.ok_or_else(|| BridgeError::new("InvalidInput", &format!(
            "domain probe failed validation: domain `{}` needs a lower bound", spec.id)))?;
        let upper = spec.upper.ok_or_else(|| BridgeError::new("InvalidInput", &format!(
            "domain probe failed validation: domain `{}` needs an upper bound", spec.id)))?;
        if spec.raw {
            Ok(IntegerDomain::Range { lower, upper })
        } else {
            IntegerDomain::range(lower, upper)
        }
    } else if spec.raw {
        Ok(IntegerDomain::Values(spec.values.clone()))
    } else {
        IntegerDomain::values(spec.values.iter().copied())
    };
    built.map_err(|error| BridgeError::new("InvalidInput", &format!(
        "domain probe failed validation: domain `{}`: {error}", spec.id)))
}

/// 走一遍域取值面探针。非法输入是**错误**。 / Walk the domain value-surface probe.
pub fn verify_domain_surface(request: &DomainProbeRequest) -> Result<DomainProbeReport> {
    if request.domains.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "domain probe failed validation: at most 16 domains are accepted"));
    }
    let maximum = request.maximum_amount.unwrap_or(8);
    let findings = canonical_findings();
    let mut domains = Vec::with_capacity(request.domains.len());
    for spec in &request.domains {
        if spec.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "domain probe failed validation: domain IDs must not be blank"));
        }
        let domain = domain_of(spec)?;
        // `validate` 是读侧闸门：构造器挡住了非法形态，反序列化挡不住。
        // `validate` is the read-side gate: the constructor blocks malformed shapes but deserialization does not.
        let (valid, invalid_reason) = match domain.validate() {
            Ok(()) => (true, None),
            Err(error) => (false, Some(error.to_string())),
        };
        let (kind, lower, upper, cardinality, is_boolean) = if valid {
            let kind = match &domain {
                IntegerDomain::Range { .. } => "range",
                IntegerDomain::Values(_) => "values",
            };
            (Some(kind.to_owned()), Some(domain.lower()), Some(domain.upper()),
                Some(domain.cardinality().to_string()), Some(domain.is_boolean()))
        } else {
            (None, None, None, None, None)
        };
        let (enumerated, enumeration_refusal) = if valid {
            match domain.enumerate(maximum as usize) {
                Ok(values) => (values, None),
                Err(error) => (Vec::new(), Some(error.to_string())),
            }
        } else {
            (Vec::new(), None)
        };
        let contains = if valid {
            request.probes.iter().map(|value| domain.contains(*value)).collect()
        } else {
            Vec::new()
        };
        domains.push(DomainView { id: spec.id.clone(), valid, invalid_reason, kind, lower, upper,
            cardinality, is_boolean, enumerated, enumeration_refusal, contains });
    }
    Ok(DomainProbeReport {
        domains,
        values_are_sorted_and_deduped_at_construction:
            findings.values_are_sorted_and_deduped_at_construction,
        a_range_is_never_materialized: findings.a_range_is_never_materialized,
        the_full_i64_range_has_a_u128_cardinality: findings.the_full_i64_range_has_a_u128_cardinality,
        is_boolean_accepts_only_the_sparse_pair: findings.is_boolean_accepts_only_the_sparse_pair,
        the_enumeration_gate_is_exclusive_at_the_boundary:
            findings.the_enumeration_gate_is_exclusive_at_the_boundary,
        enumeration_walks_the_closed_interval: findings.enumeration_walks_the_closed_interval,
        contains_is_membership_not_hull: findings.contains_is_membership_not_hull,
        the_same_value_set_has_two_canonical_encodings:
            findings.the_same_value_set_has_two_canonical_encodings,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    values_are_sorted_and_deduped_at_construction: bool,
    a_range_is_never_materialized: bool,
    the_full_i64_range_has_a_u128_cardinality: bool,
    is_boolean_accepts_only_the_sparse_pair: bool,
    the_enumeration_gate_is_exclusive_at_the_boundary: bool,
    enumeration_walks_the_closed_interval: bool,
    contains_is_membership_not_hull: bool,
    the_same_value_set_has_two_canonical_encodings: bool,
}

/// 冻结一个只带一个变量的模型并取指纹。 / Freeze a one-variable model and take its fingerprint.
fn fingerprint_of(domain: IntegerDomain) -> String {
    let mut model = ConstraintProgrammingModel::new("domain-probe");
    model.register_variable(IntegerVariable::new("x"), domain).expect("probe register");
    model.freeze().expect("probe freeze").fingerprint.value.clone()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 构造排序去重，`validate` 拒非规范形态。 / Construction sorts and dedups; `validate` refuses non-canonical shapes.
    let values_are_sorted_and_deduped_at_construction = {
        let built = IntegerDomain::values([3, 1, 3, -2, 1]).expect("probe");
        // 三种非规范形态：乱序、重复、空表。 / Three non-canonical shapes: unsorted, repeated, empty.
        let unsorted = IntegerDomain::Values(vec![3, 1]).validate().is_err();
        let repeated = IntegerDomain::Values(vec![1, 1]).validate().is_err();
        let empty = IntegerDomain::Values(Vec::new()).validate().is_err();
        // 区间倒置同样被拒。 / An inverted range is refused too.
        let inverted = IntegerDomain::Range { lower: 2, upper: 1 }.validate().is_err();
        built == IntegerDomain::Values(vec![-2, 1, 3])
            && built.validate().is_ok() && unsorted && repeated && empty && inverted
    };
    // 2) 区间从不被物化。 / A range is never materialized.
    let a_range_is_never_materialized = {
        let wide = IntegerDomain::range(0, 1_000_000).expect("probe");
        // 基数纯算术，且枚举立刻被闸门挡住——没有 100 万个元素的分配。
        // Cardinality is pure arithmetic and enumeration is refused at once: no million-element allocation happens.
        wide.cardinality() == 1_000_001
            && wide.enumerate(8).is_err_and(|error| error.to_string().contains("exceeds enumeration limit"))
    };
    // 3) 整个 `i64` 区间的基数是 `2^64`。 / The full `i64` range has cardinality `2^64`.
    let the_full_i64_range_has_a_u128_cardinality = {
        let full = IntegerDomain::range(i64::MIN, i64::MAX).expect("probe");
        // `2^64` 装得进 `u128`，所以不溢出；`usize::MAX` 比它小 1，所以枚举永远够不到。
        // `2^64` fits `u128` so nothing overflows, and `usize::MAX` is one less, so enumeration can never reach it.
        full.cardinality() == 1u128 << 64
            && full.enumerate(usize::MAX).is_err_and(|error| {
                let message = error.to_string();
                message.contains("18446744073709551616")
                    && message.contains(&usize::MAX.to_string())
            })
    };
    // 4) `is_boolean` 只认稀疏 `{0,1}`。 / `is_boolean` accepts only the sparse `{0,1}`.
    let is_boolean_accepts_only_the_sparse_pair = {
        IntegerDomain::boolean().is_boolean()
            && IntegerDomain::values([0, 1]).expect("probe").is_boolean()
            && IntegerDomain::boolean() == IntegerDomain::values([0, 1]).expect("probe")
            // 区间 `[0,1]` 与稀疏 `{0,1}` 取值相同但不算布尔。 
            // The range `[0,1]` holds the same values as `{0,1}` yet is not Boolean.
            && !IntegerDomain::range(0, 1).expect("probe").is_boolean()
            && !IntegerDomain::values([0, 1, 2]).expect("probe").is_boolean()
    };
    // 5) 枚举闸门"大于才拒"。 / The enumeration gate refuses only when strictly greater.
    let the_enumeration_gate_is_exclusive_at_the_boundary = {
        let domain = IntegerDomain::range(0, 3).expect("probe");
        // 基数 4：上限 4 通过，上限 3 拒。 / Cardinality 4: limit 4 passes, limit 3 refuses.
        let at = domain.enumerate(4).is_ok_and(|values| values.len() == 4);
        let under = domain.enumerate(3).is_err_and(|error| {
            let message = error.to_string();
            message.contains("cardinality 4") && message.contains("limit 3")
        });
        at && under
    };
    // 6) 枚举走闭区间。 / Enumeration walks the closed interval.
    let enumeration_walks_the_closed_interval = {
        let walked = IntegerDomain::range(-1, 2).expect("probe").enumerate(8).ok()
            == Some(vec![-1, 0, 1, 2]);
        // `upper = i64::MAX` 靠 `== upper` 收尾，不是溢出。 
        // `upper = i64::MAX` terminates by `== upper` rather than overflowing.
        let edge = IntegerDomain::range(i64::MAX - 1, i64::MAX).expect("probe").enumerate(8).ok()
            == Some(vec![i64::MAX - 1, i64::MAX]);
        // 稀疏域原样返回（已排序去重）。 / A sparse domain comes back as-is (sorted and deduped).
        let sparse = IntegerDomain::values([2, 0, 2]).expect("probe").enumerate(8).ok() == Some(vec![0, 2]);
        walked && edge && sparse
    };
    // 7) `contains` 是成员判定而不是包络判定。 / `contains` is membership, not hull.
    let contains_is_membership_not_hull = {
        let sparse = IntegerDomain::values([0, 10]).expect("probe");
        let range = IntegerDomain::range(0, 10).expect("probe");
        // 同一个 5：稀疏域拒、区间收。 / The same 5: the sparse domain refuses it, the range accepts it.
        !sparse.contains(5) && range.contains(5)
            && sparse.contains(0) && sparse.contains(10)
            && !range.contains(-1) && !range.contains(11)
    };
    // 8) 同一组取值有两种规范字节。 / One value set has two canonical encodings.
    let the_same_value_set_has_two_canonical_encodings = {
        let range = fingerprint_of(IntegerDomain::range(0, 1).expect("probe"));
        let sparse = fingerprint_of(IntegerDomain::values([0, 1]).expect("probe"));
        // 取值相同（都能枚举出 `[0,1]`），但指纹不同：域的形态进了指纹。
        // The values are identical (both enumerate `[0,1]`) yet the fingerprints differ: the shape enters the fingerprint.
        range != sparse
            && IntegerDomain::range(0, 1).expect("probe").enumerate(8).ok()
                == IntegerDomain::values([0, 1]).expect("probe").enumerate(8).ok()
            && range.len() == 64 && sparse.len() == 64
    };
    Findings {
        values_are_sorted_and_deduped_at_construction,
        a_range_is_never_materialized,
        the_full_i64_range_has_a_u128_cardinality,
        is_boolean_accepts_only_the_sparse_pair,
        the_enumeration_gate_is_exclusive_at_the_boundary,
        enumeration_walks_the_closed_interval,
        contains_is_membership_not_hull,
        the_same_value_set_has_two_canonical_encodings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, lower: i64, upper: i64) -> DomainProbeSpec {
        DomainProbeSpec { id: id.to_owned(), lower: Some(lower), upper: Some(upper),
            values: Vec::new(), raw: false }
    }

    fn sparse(id: &str, values: &[i64]) -> DomainProbeSpec {
        DomainProbeSpec { id: id.to_owned(), lower: None, upper: None, values: values.to_vec(),
            raw: false }
    }

    fn request(domains: Vec<DomainProbeSpec>, maximum: u32, probes: Vec<i64>) -> DomainProbeRequest {
        DomainProbeRequest { domains, maximum_amount: Some(maximum), probes }
    }

    #[test]
    fn a_sparse_domain_comes_back_sorted_and_deduped() {
        let report = verify_domain_surface(&request(vec![sparse("s", &[3, 1, 3, -2, 1])], 8,
            vec![-2, 1, 2, 3])).unwrap();
        let view = &report.domains[0];
        assert!(view.valid);
        assert_eq!(view.kind.as_deref(), Some("values"));
        assert_eq!(view.enumerated, vec![-2, 1, 3]);
        assert_eq!(view.lower, Some(-2));
        assert_eq!(view.upper, Some(3));
        assert_eq!(view.cardinality.as_deref(), Some("3"));
        assert_eq!(view.contains, vec![true, true, false, true]);
        assert_eq!(view.is_boolean, Some(false));
    }

    #[test]
    fn a_raw_malformed_domain_is_caught_by_validate() {
        // 绕过构造器：乱序、重复、空表、倒置区间四种都被 `validate` 点名。 
        // Bypassing the constructor: unsorted, repeated, empty and inverted are all named by `validate`.
        for (spec, expected) in [
            (DomainProbeSpec { values: vec![3, 1], raw: true, ..sparse("x", &[0]) },
                "strictly increasing and deduplicated"),
            (DomainProbeSpec { values: vec![1, 1], raw: true, ..sparse("x", &[0]) },
                "strictly increasing and deduplicated"),
            (DomainProbeSpec { raw: true, ..spec("x", 2, 1) }, "lower bound exceeds upper bound"),
        ] {
            let report = verify_domain_surface(&request(vec![spec], 8, vec![0])).unwrap();
            let view = &report.domains[0];
            assert!(!view.valid, "{expected}");
            assert!(view.invalid_reason.as_deref().unwrap().contains(expected),
                "{:?}", view.invalid_reason);
            // 非法域不给任何读面。 / An invalid domain exposes nothing.
            assert_eq!(view.kind, None);
            assert_eq!(view.cardinality, None);
            assert!(view.enumerated.is_empty());
            assert!(view.contains.is_empty());
        }
        // **按实测**：`values` 为空在请求里表示"区间"，所以空稀疏域**经这个探针不可达**——构造器本身也拒它。
        // **Measured**: an empty `values` means "range" in this request, so an empty sparse domain is **unreachable
        // through this probe**; the constructor refuses it as well.
        assert!(verify_domain_surface(&request(vec![DomainProbeSpec { id: "x".to_owned(),
            lower: None, upper: None, values: Vec::new(), raw: true }], 8, Vec::new()))
            .unwrap_err().message.contains("needs a lower bound"));
        assert!(IntegerDomain::values(Vec::<i64>::new()).is_err());
    }

    #[test]
    fn a_wide_range_keeps_its_cardinality_without_materializing() {
        let report = verify_domain_surface(&request(vec![spec("w", 0, 1_000_000)], 8,
            vec![0, 1_000_000, 1_000_001])).unwrap();
        let view = &report.domains[0];
        assert_eq!(view.cardinality.as_deref(), Some("1000001"));
        assert!(view.enumerated.is_empty());
        assert!(view.enumeration_refusal.as_deref().unwrap()
            .contains("exceeds enumeration limit"));
        assert_eq!(view.contains, vec![true, true, false]);
    }

    #[test]
    fn the_full_i64_range_reports_two_to_the_sixty_four() {
        let report = verify_domain_surface(&request(vec![spec("full", i64::MIN, i64::MAX)],
            8, vec![i64::MIN, 0, i64::MAX])).unwrap();
        let view = &report.domains[0];
        assert_eq!(view.cardinality.as_deref(), Some("18446744073709551616"));
        assert_eq!(view.contains, vec![true, true, true]);
        // 枚举上限即便给 `u32::MAX` 也够不到。 / Even `u32::MAX` as the limit cannot reach it.
        assert!(view.enumeration_refusal.is_some());
    }

    #[test]
    fn only_the_sparse_pair_counts_as_boolean() {
        let report = verify_domain_surface(&request(vec![spec("r", 0, 1), sparse("s", &[0, 1]),
            sparse("t", &[0, 1, 2])], 8, vec![0, 1, 2])).unwrap();
        assert_eq!(report.domains[0].is_boolean, Some(false));
        assert_eq!(report.domains[1].is_boolean, Some(true));
        assert_eq!(report.domains[2].is_boolean, Some(false));
        // 取值相同但形态不同。 / Same values, different shape.
        assert_eq!(report.domains[0].enumerated, report.domains[1].enumerated);
        assert_ne!(report.domains[0].kind, report.domains[1].kind);
    }

    #[test]
    fn the_enumeration_gate_is_exclusive_at_the_boundary() {
        let at = verify_domain_surface(&request(vec![spec("x", 0, 3)], 4, Vec::new())).unwrap();
        assert_eq!(at.domains[0].enumerated, vec![0, 1, 2, 3]);
        assert_eq!(at.domains[0].enumeration_refusal, None);
        let under = verify_domain_surface(&request(vec![spec("x", 0, 3)], 3, Vec::new())).unwrap();
        assert!(under.domains[0].enumerated.is_empty());
        let message = under.domains[0].enumeration_refusal.clone().unwrap();
        assert!(message.contains("cardinality 4") && message.contains("limit 3"), "{message}");
    }

    #[test]
    fn enumeration_walks_the_closed_interval() {
        let report = verify_domain_surface(&request(vec![spec("x", -1, 2)], 8, Vec::new())).unwrap();
        assert_eq!(report.domains[0].enumerated, vec![-1, 0, 1, 2]);
        // `i64::MAX` 处靠相等收尾。 / Termination at `i64::MAX` is by equality.
        let edge = verify_domain_surface(&request(vec![spec("e", i64::MAX - 1, i64::MAX)], 8,
            Vec::new())).unwrap();
        assert_eq!(edge.domains[0].enumerated, vec![i64::MAX - 1, i64::MAX]);
    }

    #[test]
    fn contains_is_membership_not_hull() {
        let report = verify_domain_surface(&request(vec![sparse("s", &[0, 10]), spec("r", 0, 10)],
            8, vec![5])).unwrap();
        assert_eq!(report.domains[0].contains, vec![false]);
        assert_eq!(report.domains[1].contains, vec![true]);
    }

    #[test]
    fn the_same_values_have_two_fingerprints() {
        let range = fingerprint_of(IntegerDomain::range(0, 1).expect("probe"));
        let sparse = fingerprint_of(IntegerDomain::values([0, 1]).expect("probe"));
        assert_ne!(range, sparse);
        assert_eq!(range.len(), 64);
        assert_eq!(sparse.len(), 64);
    }

    #[test]
    fn every_rule_reading_holds_on_a_real_report() {
        let report = verify_domain_surface(&request(vec![spec("x", 0, 1)], 8, vec![0])).unwrap();
        assert!(report.values_are_sorted_and_deduped_at_construction);
        assert!(report.a_range_is_never_materialized);
        assert!(report.the_full_i64_range_has_a_u128_cardinality);
        assert!(report.is_boolean_accepts_only_the_sparse_pair);
        assert!(report.the_enumeration_gate_is_exclusive_at_the_boundary);
        assert!(report.enumeration_walks_the_closed_interval);
        assert!(report.contains_is_membership_not_hull);
        assert!(report.the_same_value_set_has_two_canonical_encodings);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_domain_surface(&request(vec![DomainProbeSpec { id: " ".to_owned(),
            lower: Some(0), upper: Some(1), values: Vec::new(), raw: false }], 8, Vec::new()))
            .unwrap_err().message.contains("domain IDs must not be blank"));
        assert!(verify_domain_surface(&request(vec![DomainProbeSpec { id: "x".to_owned(),
            lower: None, upper: Some(1), values: Vec::new(), raw: false }], 8, Vec::new()))
            .unwrap_err().message.contains("needs a lower bound"));
        // 经构造器的非法域是**输入错误**；只有 `raw` 形态才变成"被 `validate` 拒"。
        // A malformed domain through the constructor is an **input error**; only the `raw` shape becomes "refused by
        // `validate`".
        assert!(verify_domain_surface(&request(vec![spec("x", 2, 1)], 8, Vec::new()))
            .unwrap_err().message.contains("lower bound exceeds upper bound"));
        assert!(verify_domain_surface(&DomainProbeRequest {
            domains: (0..17).map(|index| spec(&format!("d{index}"), 0, 1)).collect(),
            maximum_amount: Some(8), probes: Vec::new() }).unwrap_err().message
            .contains("at most 16 domains"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_domain_surface(&request(vec![spec("x", 0, 1)], 8, vec![0])).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"isBoolean\""), "{json}");
        assert!(json.contains("\"enumerationRefusal\""), "{json}");
        assert!(json.contains("\"theFullI64RangeHasAU128Cardinality\""), "{json}");
        assert!(!json.contains("is_boolean"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<DomainProbeReport>(&copy.to_string()).is_err());
    }
}
