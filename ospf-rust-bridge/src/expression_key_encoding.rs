//! 续挖第 42 单：`expression_key()` 的**位模式编码**——它是位身份，不是数值身份
//! （`solver/logic_based_benders.rs` 的 `MasterCut::expression_key`）
//! Continuation, delivery 42: the **bit-pattern encoding** of `expression_key()` — it is a bit identity, not a numeric one
//! (`MasterCut::expression_key`).
//!
//! 第 28 单量的是"key 忽略出处与身份"；这一单钻到它**怎么拼**。五条实测：
//!
//! 1. **浮点用 `to_bits()` 的定宽十六进制**（`{:016x}`）：`1.0` → `3ff0000000000000`、
//!    `-1.0` → `bff0000000000000`，永远 16 位，所以不同数值永远不会因为"位数不同"而拼出同一个 key。
//! 2. **于是 `0.0` 与 `-0.0` 数值相等但 key 不同**（`zeros_differ_by_sign()`）：
//!    `0000000000000000` vs `8000000000000000`。key 是**位**身份，`-0.0` 与 `0.0` 在这里是两个表达式。
//! 3. **系数按变量 ID 的字典序拼接**（`order_does_not_matter()`）：`coefficients` 是 `BTreeMap`，
//!    所以构造时给的项顺序**不**影响 key；两个只有顺序不同的 cut 得到完全相同的 key。
//! 4. **每个变量写成 `{长度}:{id}={位模式};`**（`id_is_length_prefixed()`）：长度前缀是十进制、紧接 `:`，
//!    所以 `1:a` 与 `2:ab` 不会互相混淆。
//! 5. **NaN 走的是同一条位模式路**（`nan_payloads_differ()`）：不同 payload 的 NaN 给出**不同**的 key。
//!    这条只在**未经 `normalize()`** 的 cut 上可达——`normalize()` 会拒掉非有限右端项与系数。
//!
//! Delivery 28 measured that the key ignores provenance and identity; this one goes inside how it is assembled. **Floats use
//! `to_bits()` in fixed-width hex** (`{:016x}`): `1.0` becomes `3ff0000000000000` and `-1.0` becomes `bff0000000000000`, always
//! sixteen digits, so two different values can never collide by digit count. **Hence `0.0` and `-0.0` are numerically equal
//! but key differently** (`zeros_differ_by_sign()`): the key is a **bit** identity, so `-0.0` and `0.0` are two expressions
//! here. **Coefficients are concatenated in the variable ID's lexicographic order** (`order_does_not_matter()`): they live in a
//! `BTreeMap`, so the construction order of the terms does **not** affect the key. **Each variable is written
//! `{len}:{id}={bits};`** (`id_is_length_prefixed()`), and the decimal length immediately followed by `:` keeps `1:a` and `2:ab`
//! apart. And **NaN takes the same bit path** (`nan_payloads_differ()`): NaNs with different payloads give **different** keys —
//! reachable only on a cut that has **not** been `normalize()`d, since that rejects non-finite right-hand sides and
//! coefficients.

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::CutSense;
use ospf_rust_framework::solver::logic_based_benders::{
    MasterCut, MasterCutProofStatus, MasterCutValidity,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个系数字面量。 / One coefficient literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyTerm {
    /// 稳定变量 ID。 / The stable variable ID.
    pub variable: String,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// 位模式编码请求。 / The bit-pattern encoding request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionKeyRequest {
    /// 方向：`le` / `ge` / `eq`。 / The sense: `le`, `ge` or `eq`.
    pub sense: String,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
    /// 系数项（**顺序不影响 key**）。 / The terms (**their order does not affect the key**).
    #[serde(default)]
    pub terms: Vec<KeyTerm>,
    /// 一个额外的右端项，用于量"两个数值相等但位不同的 rhs"（`-0.0`）。
    /// An extra right-hand side used to measure "two numerically equal but bit-different rhs values" (`-0.0`).
    #[serde(default)]
    pub compare_rhs: Option<f64>,
}

/// 位模式编码结论。 / The bit-pattern encoding verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpressionKeyReport {
    /// 这个 cut 的 key。 / This cut's key.
    pub key: String,
    /// rhs 的位模式（16 位十六进制）。 / The rhs bit pattern (sixteen hex digits).
    pub rhs_bits: String,
    /// 按**逆序**给系数时的 key（应与 `key` 相同）。 / The key with the terms reversed (identical to `key`).
    pub reordered_key: String,
    /// 每个系数的位模式（按 ID 字典序）。 / Each coefficient's bit pattern (in ID order).
    pub coefficient_bits: Vec<String>,
    /// `compareRhs` 的 key（未给则与 `key` 相同）。 / The `compareRhs` key (equal to `key` when absent).
    pub compare_key: String,
    /// `-0.0` 的 rhs 的 key（固定探针）。 / The key of a `-0.0` rhs (a fixed probe).
    pub negative_zero_key: String,
    /// `0.0` 的 rhs 的 key（固定探针）。 / The key of a `0.0` rhs (a fixed probe).
    pub positive_zero_key: String,
    /// 位模式是定宽十六进制（**是**）。 / Whether the bit pattern is fixed-width hex (**yes**).
    pub bit_pattern_is_fixed_width: bool,
    /// `0.0` 与 `-0.0` 的 key 不同（**是**）。 / Whether `0.0` and `-0.0` key differently (**yes**).
    pub zeros_differ_by_sign: bool,
    /// 系数顺序不影响 key（**是**）。 / Whether the term order does not matter (**yes**).
    pub order_does_not_matter: bool,
    /// 变量写成"长度:id"（**是**）。 / Whether a variable is written as `len:id` (**yes**).
    pub id_is_length_prefixed: bool,
    /// 不同 NaN payload 的 key 不同（**是**）。 / Whether NaN payloads differ in the key (**yes**).
    pub nan_payloads_differ: bool,
}

/// 项数上限（信任边界）。 / The term cap (a trust boundary).
const MAX_TERMS: usize = 256;

/// 把字面量转成方向。 / Turn a literal into a sense.
fn sense(name: &str) -> Result<CutSense> {
    match name {
        "le" => Ok(CutSense::LessOrEqual),
        "ge" => Ok(CutSense::GreaterOrEqual),
        "eq" => Ok(CutSense::Equal),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "expression key failed validation: unknown sense `{other}`"))),
    }
}

/// 造一个 cut（不归一化，好观察纯粹的编码）。 / Build a cut (unnormalised, to observe the raw encoding).
fn cut(rhs: f64, terms: &[KeyTerm], sense: CutSense, name: &str) -> MasterCut {
    MasterCut::new(name, terms.iter().map(|term| {
        (StableVariableId(term.variable.clone()), term.coefficient)
    }).collect::<Vec<_>>(), sense, rhs, MasterCutValidity::Global,
        MasterCutProofStatus::None, None, "probe", "probe")
}

/// 走一遍位模式编码。非法输入是**错误**。 / Walk the bit-pattern encoding.
pub fn verify_expression_key(request: &ExpressionKeyRequest) -> Result<ExpressionKeyReport> {
    if request.terms.len() > MAX_TERMS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "expression key failed validation: at most {MAX_TERMS} terms are accepted")));
    }
    for term in &request.terms {
        if term.variable.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "expression key failed validation: variable IDs must not be blank"));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for term in &request.terms {
        if !seen.insert(term.variable.as_str()) {
            // 同一变量两次会被 `BTreeMap` 覆盖成一项，所以直接拒绝。
            // The same variable twice is **overwritten** into one entry by the `BTreeMap`, so it is refused.
            return Err(BridgeError::new("InvalidInput", &format!(
                "expression key failed validation: the request repeats variable {:?}",
                term.variable)));
        }
    }
    let direction = sense(&request.sense)?;
    let forward = cut(request.rhs, &request.terms, direction, "forward").expression_key();
    let reversed_terms: Vec<KeyTerm> = request.terms.iter().rev().cloned().collect();
    let reordered_key = cut(request.rhs, &reversed_terms, direction, "reordered").expression_key();
    let compare_key = match request.compare_rhs {
        Some(rhs) => cut(rhs, &request.terms, direction, "compare").expression_key(),
        None => forward.clone(),
    };
    // 按 ID 字典序读回的位模式。 / The bit patterns read back in ID order.
    let mut ordered: Vec<(&str, f64)> = request.terms.iter()
        .map(|term| (term.variable.as_str(), term.coefficient)).collect();
    ordered.sort_by_key(|(variable, _)| *variable);
    let coefficient_bits: Vec<String> = ordered.iter()
        .map(|(_, coefficient)| format!("{:016x}", coefficient.to_bits())).collect();
    let findings = canonical_findings();
    Ok(ExpressionKeyReport {
        key: forward,
        rhs_bits: format!("{:016x}", request.rhs.to_bits()),
        reordered_key,
        coefficient_bits,
        compare_key,
        negative_zero_key: findings.negative_zero_key,
        positive_zero_key: findings.positive_zero_key,
        bit_pattern_is_fixed_width: findings.bit_pattern_is_fixed_width,
        zeros_differ_by_sign: findings.zeros_differ_by_sign,
        order_does_not_matter: findings.order_does_not_matter,
        id_is_length_prefixed: findings.id_is_length_prefixed,
        nan_payloads_differ: findings.nan_payloads_differ,
    })
}

/// 五条规则属性的内部固定探针。 / The five rule properties measured by fixed internal probes.
struct Findings {
    negative_zero_key: String,
    positive_zero_key: String,
    bit_pattern_is_fixed_width: bool,
    zeros_differ_by_sign: bool,
    order_does_not_matter: bool,
    id_is_length_prefixed: bool,
    nan_payloads_differ: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let term = |variable: &str, coefficient: f64| KeyTerm { variable: variable.to_owned(),
        coefficient };
    let probe = [term("x", 1.0), term("y", -2.0)];
    let positive_zero_key = cut(0.0, &probe, CutSense::LessOrEqual, "pz").expression_key();
    let negative_zero_key = cut(-0.0, &probe, CutSense::LessOrEqual, "nz").expression_key();
    // 1) 定宽十六进制：`1.0` 与 `-1.0` 都恰好 16 位。 / Fixed width: both `1.0` and `-1.0` take sixteen digits.
    let bit_pattern_is_fixed_width = format!("{:016x}", 1.0f64.to_bits()) == "3ff0000000000000"
        && format!("{:016x}", (-1.0f64).to_bits()) == "bff0000000000000"
        && positive_zero_key.contains("rhs=0000000000000000;")
        && negative_zero_key.contains("rhs=8000000000000000;");
    // 2) `0.0` 与 `-0.0` 的 key 不同。 / `0.0` and `-0.0` key differently.
    let zeros_differ_by_sign = positive_zero_key != negative_zero_key
        && 0.0f64 == -0.0f64;
    // 3) 项顺序不影响 key。 / The term order does not matter.
    let reversed = [term("y", -2.0), term("x", 1.0)];
    let order_does_not_matter = cut(3.0, &probe, CutSense::GreaterOrEqual, "a").expression_key()
        == cut(3.0, &reversed, CutSense::GreaterOrEqual, "b").expression_key();
    // 4) 长度前缀：`1:x` 与 `2:xy` 不会混淆。 / Length prefixes keep `1:x` and `2:xy` apart.
    let single = cut(0.0, &[term("x", 1.0)], CutSense::Equal, "s").expression_key();
    let double = cut(0.0, &[term("xy", 1.0)], CutSense::Equal, "d").expression_key();
    let id_is_length_prefixed = single.contains("1:x=") && double.contains("2:xy=")
        && single != double;
    // 5) 不同 NaN payload 的 key 不同。 / Different NaN payloads key differently.
    let one_nan = f64::from_bits(0x7ff8_0000_0000_0001);
    let another_nan = f64::from_bits(0x7ff8_0000_0000_0002);
    let nan_payloads_differ = one_nan.is_nan() && another_nan.is_nan()
        && cut(0.0, &[term("x", one_nan)], CutSense::Equal, "n1").expression_key()
            != cut(0.0, &[term("x", another_nan)], CutSense::Equal, "n2").expression_key();
    Findings { negative_zero_key, positive_zero_key, bit_pattern_is_fixed_width,
        zeros_differ_by_sign, order_does_not_matter, id_is_length_prefixed, nan_payloads_differ }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ExpressionKeyRequest {
        ExpressionKeyRequest {
            sense: "le".to_owned(),
            rhs: 7.0,
            terms: vec![KeyTerm { variable: "x".to_owned(), coefficient: 1.5 },
                KeyTerm { variable: "y".to_owned(), coefficient: -2.0 }],
            compare_rhs: None,
        }
    }

    #[test]
    fn the_bit_pattern_is_fixed_width_hex() {
        let report = verify_expression_key(&request()).unwrap();
        // `7.0` 的位模式恰好 16 位。 / `7.0`'s bit pattern is exactly sixteen digits.
        assert_eq!(report.rhs_bits, format!("{:016x}", 7.0f64.to_bits()));
        assert_eq!(report.rhs_bits.len(), 16);
        assert!(report.key.contains(&format!("rhs={};", report.rhs_bits)));
        // 系数位模式按 ID 字典序回。 / The coefficient bits come back in ID order.
        assert_eq!(report.coefficient_bits.len(), 2);
        assert_eq!(report.coefficient_bits[0], format!("{:016x}", 1.5f64.to_bits()));
        assert!(report.bit_pattern_is_fixed_width);
    }

    #[test]
    fn zero_and_negative_zero_key_differently() {
        let report = verify_expression_key(&request()).unwrap();
        assert_ne!(report.positive_zero_key, report.negative_zero_key);
        assert!(report.positive_zero_key.contains("rhs=0000000000000000;"));
        assert!(report.negative_zero_key.contains("rhs=8000000000000000;"));
        assert!(report.zeros_differ_by_sign);
        // 数值上它们相等。 / Numerically they are equal.
        assert_eq!(0.0f64, -0.0f64);
    }

    #[test]
    fn the_term_order_does_not_matter() {
        let report = verify_expression_key(&request()).unwrap();
        assert_eq!(report.key, report.reordered_key);
        assert!(report.order_does_not_matter);
        assert!(report.id_is_length_prefixed);
    }

    #[test]
    fn the_two_zero_keys_come_back_even_without_comparison() {
        let report = verify_expression_key(&request()).unwrap();
        // 未给 `compareRhs` 时 `compareKey` 等于 `key`。 / Without `compareRhs` the compare key equals the key.
        assert_eq!(report.compare_key, report.key);
        let mut compared = request();
        compared.compare_rhs = Some(-0.0);
        let compared = verify_expression_key(&compared).unwrap();
        assert_ne!(compared.compare_key, compared.key);
        assert!(compared.nan_payloads_differ);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_expression_key(&ExpressionKeyRequest { sense: "eq".to_owned(),
            rhs: 0.5, terms: vec![KeyTerm { variable: "z".to_owned(), coefficient: 0.0 }],
            compare_rhs: None }).unwrap();
        assert!(report.bit_pattern_is_fixed_width);
        assert!(report.zeros_differ_by_sign);
        assert!(report.order_does_not_matter);
        assert!(report.id_is_length_prefixed);
        assert!(report.nan_payloads_differ);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut unknown = request();
        unknown.sense = "lt".to_owned();
        assert!(verify_expression_key(&unknown).unwrap_err().message
            .contains("unknown sense"));
        let mut blank = request();
        blank.terms[0].variable = "  ".to_owned();
        assert!(verify_expression_key(&blank).unwrap_err().message
            .contains("variable IDs must not be blank"));
        let mut repeated = request();
        repeated.terms[1].variable = "x".to_owned();
        assert!(verify_expression_key(&repeated).unwrap_err().message
            .contains("repeats variable"));
        let too_many = ExpressionKeyRequest { sense: "le".to_owned(), rhs: 0.0,
            terms: (0..257).map(|index| KeyTerm { variable: format!("v{index}"),
                coefficient: 1.0 }).collect(), compare_rhs: None };
        assert!(verify_expression_key(&too_many).unwrap_err().message
            .contains("at most 256 terms"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_expression_key(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"rhsBits\""), "{json}");
        assert!(json.contains("\"zerosDifferBySign\""), "{json}");
        assert!(json.contains("\"reorderedKey\""), "{json}");
        assert!(!json.contains("rhs_bits"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ExpressionKeyReport>(&copy.to_string()).is_err());
    }
}
