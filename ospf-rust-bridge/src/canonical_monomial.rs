//! 续挖第 41 单：标准单项式的幂次表——**幂次 0 与"变量不出现"是两种可表示状态**，
//! 而 `total_power()` 把它们算成一样（`model/flatten/inner_box.rs` 的 `CanonicalMonomial` / `Canonical`）
//! Continuation, delivery 41: the canonical monomial's power table — **"power zero" and "variable absent" are two
//! representable states**, while `total_power()` computes both as zero (`CanonicalMonomial`, `Canonical`).
//!
//! 第 32 单量的是三态的**升幂桥**、第 38 单量的是二次单项式的 kind 推导；这一单量标准形态自己的幂次表。
//! 五条实测：
//!
//! 1. **`powers` 是 `HashMap<usize, i32>`，`get_power` 对缺席的变量读回 `0`**——于是"幂次 0"与"变量不出现"
//!    在读取上**不可区分**（`absent_reads_as_zero()`），但它们在**表示上**是可区分的：
//!    `CanonicalMonomial::new(c, {x: 0})` 的 `powers().len() == 1`、"变量不出现"的则是 `0`。
//! 2. **`total_power()` 只是幂次求和**（`total_power_is_a_sum()`）：它对空表给 `0`、对 `{x: 0}` 也给 `0`，
//!    所以"总幂次 0"**不能**用来判断"这是常数"——常数项根本不在单项式列表里，它在 `constant` 字段。
//! 3. **幂次可以是负数、可以是 0**（`powers_are_signed_and_unvalidated()`）：`new` 对幂次表不做任何校验，
//!    所以 `{x: -2}` 与 `{x: 0}` 都被原样接受，`total_power()` 分别给出 `-2` 与 `0`。
//! 4. **`add_monomial` 不合并、不约简**（`add_monomial_does_not_merge()`）：追加两个幂次表完全相同的单项式后
//!    `monomials().len() == 2`，系数也没有相加；`Canonical::new` 同样不做规范化。
//! 5. **`new_variable` 是"单变量 1 次"的便捷构造**（`new_variable_is_a_single_unit_power()`）：
//!    它的幂次表长度恒为 1、`get_power(i) == 1`、`total_power() == 1`。
//!
//! Delivery 32 measured the **degree-lifting bridge** between the three states and delivery 38 the quadratic monomial's
//! derived kind; this one measures the standard form's own power table. **`powers` is a `HashMap<usize, i32>` and
//! `get_power` reads an absent variable as `0`**, so "power zero" and "variable absent" are **indistinguishable on read**
//! (`absent_reads_as_zero()`) yet **distinguishable in representation**: `CanonicalMonomial::new(c, {x: 0})` has
//! `powers().len() == 1` while an absent variable gives `0`. **`total_power()` is just a sum** (`total_power_is_a_sum()`):
//! it answers `0` for both an empty table and `{x: 0}`, so "total power zero" **cannot** mean "this is a constant" — the
//! constant lives in the `constant` field, not in the monomial list. **Powers may be negative or zero**
//! (`powers_are_signed_and_unvalidated()`): `new` validates nothing, so `{x: -2}` and `{x: 0}` are both accepted and
//! `total_power()` reports `-2` and `0`. **`add_monomial` neither merges nor simplifies**
//! (`add_monomial_does_not_merge()`): appending two monomials with identical power tables leaves `monomials().len() == 2` with
//! the coefficients unsummed, and `Canonical::new` normalises nothing either. And **`new_variable` is the "one variable, unit
//! power" convenience** (`new_variable_is_a_single_unit_power()`): its table has length one, `get_power(i) == 1` and
//! `total_power() == 1`.

use std::collections::HashMap;

use ospf_rust_core::model::flatten::inner_box::{Canonical, CanonicalMonomial};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个幂次项的字面量（与第 32 单的 `PowerSpec` 区分：那个是**线性/二次**升幂用的一元幂次）。
/// One power entry literal (distinct from delivery 32's `PowerSpec`, which is a **degree-lift** power).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PowerEntry {
    /// 变量序号。 / The variable index.
    pub variable: u32,
    /// 幂次（可负、可为 0）。 / The power (may be negative or zero).
    pub power: i32,
}

/// 标准单项式请求。 / The canonical monomial request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalMonomialRequest {
    /// 系数。 / The coefficient.
    pub coefficient: f64,
    /// 幂次表。 / The power table.
    #[serde(default)]
    pub powers: Vec<PowerEntry>,
    /// 是否再追加一个幂次表完全相同的项（用于量"不合并"）。
    /// Whether to append another monomial with an identical power table (to measure "no merging").
    #[serde(default)]
    pub append_again: bool,
}

/// 读回的一个幂次项。 / One power entry read back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PowerRow {
    /// 变量序号。 / The variable index.
    pub variable: u32,
    /// 读回的幂次。 / The power read back.
    pub power: i32,
}

/// 标准单项式结论。 / The canonical monomial verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalMonomialReport {
    /// 系数原样读回。 / The coefficient read back.
    pub coefficient: f64,
    /// 幂次表长度。 / The power table length.
    pub table_len: u64,
    /// 幂次表（按变量序号排序，便于比较）。 / The power table, sorted by variable index for comparison.
    pub powers: Vec<PowerRow>,
    /// 总幂次。 / The total power.
    pub total_power: i64,
    /// 一个**不出现**的变量读回的幂次。 / What an **absent** variable reads back as.
    pub absent_power: i32,
    /// 一个幂次为 0 的变量读回的幂次。 / What a **zero-power** variable reads back as.
    pub zero_power: i32,
    /// 追加后单项式列表的长度。 / The monomial list length after the append.
    pub appended_count: u64,
    /// 追加后系数之和（证明不合并）。 / The summed coefficient after the append (proving no merge).
    pub appended_coefficient_sum: f64,
    /// 追加后总幂次（证明不约简）。 / The total power after the append.
    pub appended_total_power: i64,
    /// 幂次 0 与"变量不出现"读取上不可区分（**是**）。 / Whether zero and absent are indistinguishable on read
    /// (**yes**).
    pub absent_reads_as_zero: bool,
    /// 幂次 0 与"变量不出现"表示上可区分（**是**）。 / Whether zero and absent differ in representation (**yes**).
    pub zero_power_is_representable: bool,
    /// `totalPower` 只是求和（**是**）。 / Whether `totalPower` is just a sum (**yes**).
    pub total_power_is_a_sum: bool,
    /// 幂次可负且不校验（**是**）。 / Whether powers may be negative and are unvalidated (**yes**).
    pub powers_are_signed_and_unvalidated: bool,
    /// `addMonomial` 不合并（**是**）。 / Whether `addMonomial` does not merge (**yes**).
    pub add_monomial_does_not_merge: bool,
    /// `newVariable` 是单变量 1 次（**是**）。 / Whether `newVariable` is a single unit power (**yes**).
    pub new_variable_is_a_single_unit_power: bool,
}

/// 幂次项条数上限（信任边界）。 / The power entry cap (a trust boundary).
const MAX_POWERS: usize = 64;

/// 造一个单项式。 / Build one monomial.
fn build(coefficient: f64, powers: &[PowerEntry]) -> CanonicalMonomial<f64> {
    let table: HashMap<usize, i32> = powers.iter()
        .map(|entry| (entry.variable as usize, entry.power)).collect();
    CanonicalMonomial::new(coefficient, table)
}

/// 走一遍标准单项式。非法输入是**错误**。 / Walk the canonical monomial.
pub fn verify_canonical_monomial(request: &CanonicalMonomialRequest)
    -> Result<CanonicalMonomialReport>
{
    if !request.coefficient.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "canonical monomial failed validation: the coefficient must be finite"));
    }
    if request.powers.len() > MAX_POWERS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "canonical monomial failed validation: at most {MAX_POWERS} power entries are accepted")));
    }
    // 同一变量给两次不是"合并"，是**互相覆盖**（`HashMap` 语义），所以直接拒绝。
    // The same variable twice is not a merge but an **overwrite** (HashMap semantics), so it is refused.
    let mut seen = std::collections::BTreeSet::new();
    for entry in &request.powers {
        if !seen.insert(entry.variable) {
            return Err(BridgeError::new("InvalidInput", &format!(
                "canonical monomial failed validation: the request repeats variable {}",
                entry.variable)));
        }
    }
    let monomial = build(request.coefficient, &request.powers);
    let mut powers: Vec<PowerRow> = monomial.powers().iter()
        .map(|(variable, power)| PowerRow { variable: u32::try_from(*variable).unwrap_or(u32::MAX),
            power: *power }).collect();
    powers.sort_by_key(|row| row.variable);
    let absent_power = monomial.get_power(u32::MAX as usize);
    // 幂次为 0 的读回值：优先用请求里真的给了的 0，否则用探针的结论（都由构造证明可得）。
    // The zero-power read-back: prefer a real zero from the request, otherwise the probe's finding.
    let zero_power = request.powers.iter().find(|entry| entry.power == 0)
        .map(|entry| monomial.get_power(entry.variable as usize))
        .unwrap_or_else(|| {
            let probe = build(1.0, &[PowerEntry { variable: 0, power: 0 }]);
            probe.get_power(0)
        });
    // 追加一个幂次表完全相同的项：不合并、不约简。 
    // Append a monomial with an identical power table: no merging, no simplification.
    let mut polynomial = Canonical::new(vec![monomial], request.coefficient);
    if request.append_again {
        polynomial.add_monomial(build(request.coefficient, &request.powers));
    }
    let appended_coefficient_sum: f64 = polynomial.monomials().iter()
        .map(|entry| *entry.coefficient()).sum();
    let appended_total_power: i64 = polynomial.monomials().iter()
        .map(|entry| i64::from(entry.total_power())).sum();
    let findings = canonical_findings();
    Ok(CanonicalMonomialReport {
        coefficient: *polynomial.monomials().first().map(|entry| entry.coefficient())
            .unwrap_or(&request.coefficient),
        table_len: polynomial.monomials().first().map(|entry| entry.powers().len() as u64)
            .unwrap_or(0),
        powers,
        total_power: i64::from(polynomial.monomials().first()
            .map(|entry| entry.total_power()).unwrap_or(0)),
        absent_power,
        zero_power,
        appended_count: polynomial.monomials().len() as u64,
        appended_coefficient_sum,
        appended_total_power,
        absent_reads_as_zero: findings.absent_reads_as_zero,
        zero_power_is_representable: findings.zero_power_is_representable,
        total_power_is_a_sum: findings.total_power_is_a_sum,
        powers_are_signed_and_unvalidated: findings.powers_are_signed_and_unvalidated,
        add_monomial_does_not_merge: findings.add_monomial_does_not_merge,
        new_variable_is_a_single_unit_power: findings.new_variable_is_a_single_unit_power,
    })
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    absent_reads_as_zero: bool,
    zero_power_is_representable: bool,
    total_power_is_a_sum: bool,
    powers_are_signed_and_unvalidated: bool,
    add_monomial_does_not_merge: bool,
    new_variable_is_a_single_unit_power: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let empty = CanonicalMonomial::new(1.0, HashMap::new());
    let mut zero_table = HashMap::new();
    zero_table.insert(4usize, 0i32);
    let zero = CanonicalMonomial::new(1.0, zero_table);
    let variable = CanonicalMonomial::new(2.0, {
        let mut table = HashMap::new();
        table.insert(7usize, 1i32);
        table
    });
    // 1) 缺席读回 0，而幂次 0 也是 0。 / An absent variable reads 0, and a zero power is also 0.
    let absent_reads_as_zero = empty.get_power(4) == 0 && zero.get_power(4) == 0;
    // 2) 但表示上可区分：长度 0 与长度 1。 / But they differ in representation: length 0 versus 1.
    let zero_power_is_representable = empty.powers().len() == 0 && zero.powers().len() == 1;
    // 3) 总幂次只是求和。 / The total power is just a sum.
    let total_power_is_a_sum = empty.total_power() == 0 && zero.total_power() == 0
        && variable.total_power() == 1;
    // 4) 幂次可负、不校验。 / Powers may be negative and are unvalidated.
    let mut negative_table = HashMap::new();
    negative_table.insert(9usize, -2i32);
    let negative = CanonicalMonomial::new(1.0, negative_table);
    let powers_are_signed_and_unvalidated = negative.get_power(9) == -2
        && negative.total_power() == -2;
    // 5) 追加不合并、不约简。 / Appending neither merges nor simplifies.
    let mut polynomial = Canonical::new(vec![variable.clone()], 0.0);
    polynomial.add_monomial(variable.clone());
    let add_monomial_does_not_merge = polynomial.monomials().len() == 2
        && polynomial.monomials().iter().all(|entry| *entry.coefficient() == 2.0)
        && polynomial.monomials().iter().map(|entry| entry.total_power()).sum::<i32>() == 2;
    // 6) `new_variable` 是单变量 1 次。 / `new_variable` is a single unit power.
    let convenience = CanonicalMonomial::new_variable(3.0, 5);
    let new_variable_is_a_single_unit_power = convenience.powers().len() == 1
        && convenience.get_power(5) == 1 && convenience.total_power() == 1;
    Findings { absent_reads_as_zero, zero_power_is_representable, total_power_is_a_sum,
        powers_are_signed_and_unvalidated, add_monomial_does_not_merge,
        new_variable_is_a_single_unit_power }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CanonicalMonomialRequest {
        CanonicalMonomialRequest {
            coefficient: 2.5,
            powers: vec![PowerEntry { variable: 3, power: 2 }, PowerEntry { variable: 1, power: 1 }],
            append_again: false,
        }
    }

    #[test]
    fn the_power_table_reads_back_sorted() {
        let report = verify_canonical_monomial(&request()).unwrap();
        assert_eq!(report.coefficient, 2.5);
        assert_eq!(report.table_len, 2);
        // 表按变量序号排序回，便于比较。 / The table comes back sorted by variable index.
        assert_eq!(report.powers, vec![PowerRow { variable: 1, power: 1 },
            PowerRow { variable: 3, power: 2 }]);
        assert_eq!(report.total_power, 3);
        assert_eq!(report.appended_count, 1);
    }

    #[test]
    fn zero_and_absent_are_the_same_on_read_but_not_in_representation() {
        // 幂次 0：表长 1，读回 0。 / Power zero: table length 1, reads back 0.
        let zero = verify_canonical_monomial(&CanonicalMonomialRequest { coefficient: 1.0,
            powers: vec![PowerEntry { variable: 4, power: 0 }], append_again: false }).unwrap();
        assert_eq!(zero.table_len, 1);
        assert_eq!(zero.zero_power, 0);
        assert_eq!(zero.total_power, 0);
        // 变量不出现：表长 0，也读回 0。 / Absent: table length 0, also reads back 0.
        let empty = verify_canonical_monomial(&CanonicalMonomialRequest { coefficient: 1.0,
            powers: Vec::new(), append_again: false }).unwrap();
        assert_eq!(empty.table_len, 0);
        assert_eq!(empty.absent_power, 0);
        assert_eq!(empty.total_power, 0);
        // 读回相同、表示不同。 / Same read-back, different representation.
        assert!(empty.absent_reads_as_zero);
        assert!(empty.zero_power_is_representable);
        assert!(empty.total_power_is_a_sum);
    }

    #[test]
    fn negative_powers_pass_through() {
        let report = verify_canonical_monomial(&CanonicalMonomialRequest { coefficient: 1.0,
            powers: vec![PowerEntry { variable: 9, power: -2 }], append_again: false }).unwrap();
        assert_eq!(report.powers[0].power, -2);
        assert_eq!(report.total_power, -2);
        assert!(report.powers_are_signed_and_unvalidated);
    }

    #[test]
    fn appending_does_not_merge_or_simplify() {
        let mut document = request();
        document.append_again = true;
        let report = verify_canonical_monomial(&document).unwrap();
        // 两个完全相同的幂次表仍占两项，系数不相加。 
        // Two identical power tables still occupy two entries and the coefficients do not merge.
        assert_eq!(report.appended_count, 2);
        assert_eq!(report.appended_coefficient_sum, 5.0);
        assert_eq!(report.appended_total_power, 6);
        assert!(report.add_monomial_does_not_merge);
        assert!(report.new_variable_is_a_single_unit_power);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_canonical_monomial(&CanonicalMonomialRequest { coefficient: -1.0,
            powers: vec![PowerEntry { variable: 0, power: 5 }], append_again: false }).unwrap();
        assert!(report.absent_reads_as_zero);
        assert!(report.zero_power_is_representable);
        assert!(report.total_power_is_a_sum);
        assert!(report.powers_are_signed_and_unvalidated);
        assert!(report.add_monomial_does_not_merge);
        assert!(report.new_variable_is_a_single_unit_power);
        assert_eq!(report.total_power, 5);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut infinite = request();
        infinite.coefficient = f64::INFINITY;
        assert!(verify_canonical_monomial(&infinite).unwrap_err().message
            .contains("coefficient must be finite"));
        // 同一变量两次是**覆盖**而不是合并，所以直接拒绝。 
        // The same variable twice is an **overwrite**, not a merge, so it is refused.
        let repeated = CanonicalMonomialRequest { coefficient: 1.0,
            powers: vec![PowerEntry { variable: 2, power: 1 }, PowerEntry { variable: 2, power: 3 }],
            append_again: false };
        assert!(verify_canonical_monomial(&repeated).unwrap_err().message
            .contains("repeats variable 2"));
        let too_many = CanonicalMonomialRequest { coefficient: 1.0,
            powers: (0..65).map(|index| PowerEntry { variable: index, power: 1 }).collect(),
            append_again: false };
        assert!(verify_canonical_monomial(&too_many).unwrap_err().message
            .contains("at most 64 power entries"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_canonical_monomial(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"tableLen\""), "{json}");
        assert!(json.contains("\"absentReadsAsZero\""), "{json}");
        assert!(json.contains("\"addMonomialDoesNotMerge\""), "{json}");
        assert!(!json.contains("table_len"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CanonicalMonomialReport>(&copy.to_string()).is_err());
    }
}
