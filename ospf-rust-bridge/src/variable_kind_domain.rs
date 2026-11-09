//! 续挖第 24 单：变量类型的定义域注册表（`variable/variable_type.rs` 的八个标记类型）
//! Continuation, delivery 24: the variable-type domain registry (eight marker types).
//!
//! 八种变量类型各自实现 `VariableTypeTrait`，回答四件事：`var_type()`（枚举）、`min_value()` / `max_value()`
//! （可能为无界）、`is_integer()`、`is_signed()`，外加 `is_valid_value()` 这个取值判定。这张表是所有写入/校验收口的
//! 共同依据，而且它是**离散与连续的混合**：
//!
//! * `Binary` / `Ternary` / `BalancedTernary` 的界是**离散的整数**，但 `Value` 类型是 `f64`，所以
//!   `is_valid_value(0.5)` 靠的是**界**而不是整数性——把 `is_integer()` 当成"值必须整数"会读错。
//! * `Continuous` / `UContinuous` / `Integer` / `UInteger` 至少有一侧无界，`default_range()` 因此会带 `None`；
//!   `is_valid_value` 把 `None` 当作"该侧不限制"。
//! * `is_signed()` 与 `min_value()` 是**两套独立信息**：无符号类型的 `min_value()` 是 `0`，但有些类型下界为
//!   `None`（无界）却仍声明"有符号"。所以要判"能否取负"必须同时看两处。
//!
//! The eight variable types each implement `VariableTypeTrait`, answering four things — `var_type()`, `min_value()` /
//! `max_value()` (possibly unbounded), `is_integer()`, `is_signed()` — plus the `is_valid_value()` decision, and the table
//! all write/validation doors share is a **mix of discrete and continuous**. The bounds of `Binary` / `Ternary` /
//! `BalancedTernary` are **discrete integers** while `Value` is `f64`, so `is_valid_value(0.5)` is decided by the **bounds**,
//! not by integrality: reading `is_integer()` as "the value must be integral" is wrong. `Continuous` / `UContinuous` /
//! `Integer` / `UInteger` are unbounded on at least one side, so `default_range()` carries `None` and `is_valid_value` treats
//! `None` as "no restriction on that side". And `is_signed()` and `min_value()` are **two independent pieces of
//! information**, so "can it go negative" must be read from both.

use ospf_rust_core::variable::variable_type::{
    BalancedTernary, Binary, Continuous, Integer, Percentage, Ternary, UContinuous, UInteger,
    VariableTypeTrait,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 定义域请求。 / The domain request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableKindDomainRequest {
    /// 要判定的取值。 / The values to test.
    #[serde(default)]
    pub probes: Vec<f64>,
}

/// 一个变量类型的定义域。 / One variable type's domain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableKindRow {
    /// 枚举名。 / The enum name.
    pub kind: String,
    /// `type_name()`。 / `type_name()`.
    pub type_name: String,
    /// 下界（无界则为空）。 / The lower bound (absent when unbounded).
    #[serde(default)]
    pub min: Option<f64>,
    /// 上界（无界则为空）。 / The upper bound (absent when unbounded).
    #[serde(default)]
    pub max: Option<f64>,
    /// `is_integer()`。 / `is_integer()`.
    pub is_integer: bool,
    /// `is_signed()`。 / `is_signed()`.
    pub is_signed: bool,
    /// `0.5` 是否合法。 / Whether `0.5` is valid.
    pub half_valid: bool,
    /// `1e9` 是否合法。 / Whether `1e9` is valid.
    pub large_valid: bool,
    /// 每个探测取值的判定。 / The verdict for each probed value.
    pub validity: Vec<bool>,
}

/// 定义域结论。 / The domain verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableKindDomainReport {
    /// 探测的取值。 / The probed values.
    pub probes: Vec<f64>,
    /// 八种类型的定义域。 / The eight domains.
    pub kinds: Vec<VariableKindRow>,
    /// 枚举与 `type_name()` 一致（**是**）。 / Whether the enum matches `type_name()` (**yes**).
    pub kind_matches_type_name: bool,
    /// 有界时 `min <= max`（**是**）。 / Whether bounded kinds satisfy `min <= max` (**yes**).
    pub bounds_are_ordered: bool,
    /// 整数性根本没有被 `is_valid_value` 校验：离散类型照样放行 `0.5`（**是**）。
    /// Whether integrality is not enforced at all: discrete kinds accept `0.5` (**yes**).
    pub integrality_is_not_enforced: bool,
    /// `Percentage` 的上界是 `1` 而不是 `100`（**是**）。 / Whether `Percentage` is bounded by `1` rather than `100`
    /// (**yes**).
    pub percentage_upper_is_one: bool,
    /// 无界类型接受大值（**是**）。 / Whether unbounded kinds accept large values (**yes**).
    pub unbounded_kinds_accept_large: bool,
    /// `is_signed()` 与下界是两套信息（**是**）。 / Whether signedness and the lower bound are independent (**yes**).
    pub signedness_is_independent: bool,
}

/// 探测上限（信任边界）。 / The probe cap (a trust boundary).
const MAX_PROBES: usize = 1_024;

/// 半值探针。 / The half probe.
const HALF: f64 = 0.5;
/// 大值探针。 / The large probe.
const LARGE: f64 = 1.0e9;

/// 走一遍变量类型定义域。非法输入是**错误**。 / Walk the variable-type domains.
pub fn verify_variable_kind_domain(
    request: &VariableKindDomainRequest,
) -> Result<VariableKindDomainReport> {
    if request.probes.len() > MAX_PROBES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "variable kind domain failed validation: at most {MAX_PROBES} probes are accepted")));
    }
    for probe in &request.probes {
        if !probe.is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "variable kind domain failed validation: every probe must be finite"));
        }
    }
    let kinds = vec![
        row::<Binary>(&request.probes),
        row::<Ternary>(&request.probes),
        row::<BalancedTernary>(&request.probes),
        row::<Percentage>(&request.probes),
        row::<Integer>(&request.probes),
        row::<UInteger>(&request.probes),
        row::<Continuous>(&request.probes),
        row::<UContinuous>(&request.probes),
    ];
    // 枚举名与 `type_name()` 必须一致，否则"类型"和"名字"两套源头会分叉。
    // The enum name must match `type_name()`, or the type and the name diverge.
    let kind_matches_type_name = kinds.iter().all(|row| row.kind == row.type_name);
    let bounds_are_ordered = kinds.iter()
        .all(|row| match (row.min, row.max) { (Some(min), Some(max)) => min <= max, _ => true });
    // 按实测纠正：整数性**根本没被校验**——`Binary` / `Ternary` / `BalancedTernary` / `Integer` 都放行 `0.5`。
    // Corrected by measurement: integrality is **not enforced at all**; the discrete kinds all accept `0.5`.
    let integrality_is_not_enforced = kinds.iter()
        .filter(|row| row.is_integer)
        .all(|row| row.half_valid);
    // `Percentage` 的上界是 1（比例），不是 100。 / `Percentage` is bounded by 1, not 100.
    let percentage_upper_is_one = kinds.iter()
        .find(|row| row.kind == "Percentage")
        .is_some_and(|row| row.max == Some(1.0));
    // 上界无界的类型接受 1e9。 / Kinds unbounded above accept 1e9.
    let unbounded_kinds_accept_large = kinds.iter()
        .filter(|row| row.max.is_none())
        .all(|row| row.large_valid);
    // `is_signed()` 与下界是两套信息：存在"下界 0 却声明有符号"或"无下界却声明无符号"的类型。
    // Signedness and the lower bound are independent.
    let signedness_is_independent = kinds.iter()
        .any(|row| row.min == Some(0.0) && row.is_signed)
        || kinds.iter().any(|row| row.min.is_none() && !row.is_signed);
    Ok(VariableKindDomainReport {
        probes: request.probes.clone(),
        kinds,
        kind_matches_type_name,
        bounds_are_ordered,
        integrality_is_not_enforced,
        percentage_upper_is_one,
        unbounded_kinds_accept_large,
        signedness_is_independent,
    })
}

/// 一个类型的定义域行。 / One kind's domain row.
fn row<T>(probes: &[f64]) -> VariableKindRow
where
    T: VariableTypeTrait<Value = f64>,
{
    VariableKindRow {
        kind: format!("{:?}", T::var_type()),
        type_name: T::type_name().to_owned(),
        min: T::min_value(),
        max: T::max_value(),
        is_integer: T::is_integer(),
        is_signed: T::is_signed(),
        half_valid: T::is_valid_value(&HALF),
        large_valid: T::is_valid_value(&LARGE),
        validity: probes.iter().map(|value| T::is_valid_value(value)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> VariableKindDomainReport {
        verify_variable_kind_domain(&VariableKindDomainRequest {
            probes: vec![-1.0, 0.0, 0.5, 1.0, 2.0, 100.0, 101.0] }).unwrap()
    }

    fn row<'a>(report: &'a VariableKindDomainReport, kind: &str) -> &'a VariableKindRow {
        report.kinds.iter().find(|row| row.kind == kind)
            .unwrap_or_else(|| panic!("kind {kind}"))
    }

    #[test]
    fn the_table_is_eight_kinds_with_matching_names() {
        let report = report();
        assert_eq!(report.kinds.len(), 8);
        assert!(report.kind_matches_type_name);
        assert!(report.bounds_are_ordered);
        assert_eq!(report.probes.len(), 7);
        // 三种离散类型的界。 / The bounds of the three discrete kinds.
        assert_eq!(row(&report, "Binary").min, Some(0.0));
        assert_eq!(row(&report, "Binary").max, Some(1.0));
        assert_eq!(row(&report, "Ternary").max, Some(2.0));
        assert_eq!(row(&report, "BalancedTernary").min, Some(-1.0));
        assert_eq!(row(&report, "BalancedTernary").max, Some(1.0));
    }

    #[test]
    fn integrality_is_not_enforced_by_the_value_check() {
        // 判据只看界：`Binary` 的界是 [0,1]，所以 `0.5` 合法——离散性没被校验。
        // Only the bounds are checked: `Binary` is bounded by [0,1], so `0.5` passes — integrality is not enforced.
        let report = report();
        assert!(report.integrality_is_not_enforced);
        for kind in ["Binary", "Ternary", "BalancedTernary", "Integer", "UInteger"] {
            assert!(row(&report, kind).is_integer, "{kind}");
            assert!(row(&report, kind).half_valid, "{kind}");
        }
        // `Binary` 拒 -1 与 2（界外），但放行 0.5。 / `Binary` rejects -1 and 2 (out of bounds) yet accepts 0.5.
        assert_eq!(row(&report, "Binary").validity,
            vec![false, true, true, true, false, false, false]);
        assert_eq!(row(&report, "Ternary").validity,
            vec![false, true, true, true, true, false, false]);
        assert_eq!(row(&report, "BalancedTernary").validity,
            vec![true, true, true, true, false, false, false]);
    }

    #[test]
    fn percentage_is_bounded_by_one_not_a_hundred() {
        let report = report();
        assert!(report.percentage_upper_is_one);
        let percentage = row(&report, "Percentage");
        assert_eq!(percentage.max, Some(1.0));
        assert!(!percentage.is_integer);
        // `100` 与 `101` 都非法，`1` 合法。 / `100` and `101` are both invalid; `1` is valid.
        assert_eq!(percentage.validity, vec![false, true, true, true, false, false, false]);
    }

    #[test]
    fn signedness_is_independent_of_the_lower_bound() {
        let report = report();
        assert!(report.signedness_is_independent);
        // 下界 0 却声明"有符号"的三个类型。 / Three kinds have a zero lower bound yet declare themselves signed.
        for kind in ["Binary", "Ternary", "Percentage"] {
            assert_eq!(row(&report, kind).min, Some(0.0), "{kind}");
            assert!(row(&report, kind).is_signed, "{kind}");
        }
        // 只有无符号的两个类型声明 false。 / Only the two unsigned kinds declare false.
        assert!(!row(&report, "UInteger").is_signed);
        assert!(!row(&report, "UContinuous").is_signed);
        // 无下界的类型接受 1e9，`UInteger` / `UContinuous` 也接受（只卡下界）。
        // Unbounded kinds accept 1e9; the unsigned ones do too (only the lower bound is enforced).
        assert!(report.unbounded_kinds_accept_large);
        assert_eq!(row(&report, "Integer").min, None);
        assert_eq!(row(&report, "Continuous").max, None);
        assert!(row(&report, "UInteger").large_valid);
        assert!(!row(&report, "Binary").large_valid);
    }

    #[test]
    fn probe_limits_and_bad_input() {
        let report = verify_variable_kind_domain(&VariableKindDomainRequest {
            probes: Vec::new() }).unwrap();
        assert!(report.kinds.iter().all(|row| row.validity.is_empty()));
        assert!(verify_variable_kind_domain(&VariableKindDomainRequest {
            probes: vec![f64::NAN] }).unwrap_err().message.contains("must be finite"));
        let many = VariableKindDomainRequest { probes: vec![0.0; MAX_PROBES + 1] };
        assert!(verify_variable_kind_domain(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = report();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"kindMatchesTypeName\""), "{json}");
        assert!(json.contains("\"integralityIsNotEnforced\""), "{json}");
        assert!(json.contains("\"percentageUpperIsOne\""), "{json}");
        assert!(json.contains("\"halfValid\""), "{json}");
        assert!(!json.contains("kind_matches"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<VariableKindDomainReport>(&copy.to_string()).is_err());
    }
}
