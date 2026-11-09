//! 批次 F 第 2 单：big-M 标定旋钮与二值化方法的参数词汇 / Batch F, delivery 2: the big-M calibration knob and the
//! binaryzation-method parameter vocabulary.
//!
//! 全是**公开且纯标量**的入口，不需要模型、不建变量、不生成约束：
//!
//! * `default_big_m()`（1e6）与 `MIN_BIG_M`（1.0）——Big-M 的下限不是 0：太小的推断界会产生退化约束；
//! * `ensure_positive_big_m(m)`——非有限、零、负数**报错**；正数但小于 `MIN_BIG_M` 时**被抬到下限**。所以"我给了一个
//!   合法的 M"与"求解器实际用了哪个 M"是两件事，报告把两者都写出来并标出是否被抬过；
//! * `BinaryzationMethod`（4 种）与它的**机制层归一**：`indicator` / `sos1` 在这一层只表达"我要原生能力"，
//!   实际编码仍是 Big-M。这条最容易误解——调用方以为自己拿到了原生指示器或 SOS1，其实拿到的是 Big-M；
//! * `binaryzation_core_relations(method)`——每种的 `whenTrue` / `whenFalse` 关系与右端项、两条松弛行。
//!
//! 这是硬件无关的"标定旋钮"：`MIN_BIG_M` 与 `BINARYZATION_STRICT_EPSILON`（`f64::EPSILON * 16`）都是数值稳定性
//! 相关的经验取值，TS 侧不该自己拍一组数，而应当读这里的结论。
//!
//! Everything here is **public and purely scalar**, needing no model, no variable and no constraint: `default_big_m()`
//! (1e6) and `MIN_BIG_M` (1.0) — the big-M floor is not zero, since a too-small inferred bound produces degenerate
//! constraints; `ensure_positive_big_m(m)`, where non-finite, zero and negative values **fail** while a positive value below
//! the floor is **raised to it**, so "the M I supplied" and "the M the solver used" are two different things and the report
//! states both plus whether it was raised; `BinaryzationMethod` (four kinds) and its **mechanism-layer normalisation** —
//! `indicator` / `sos1` only request a native capability at this layer and are still encoded as big-M, the easiest thing to
//! misread; and `binaryzation_core_relations(method)` with each kind's `whenTrue` / `whenFalse` relation and right-hand side
//! plus both relaxed rows. This is the hardware-independent calibration knob: `MIN_BIG_M` and
//! `BINARYZATION_STRICT_EPSILON` (`f64::EPSILON * 16`) are empirical numerical-stability values, and TypeScript should read
//! this verdict rather than invent a second set.

use ospf_rust_core::model::ConstraintRelation;
use ospf_rust_core::symbol::function::binaryzation::{
    binaryzation_core_relations, BinaryzationCoreRelations, BinaryzationMethod,
};
use ospf_rust_core::symbol::function::big_m::{default_big_m, ensure_positive_big_m};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// big-M 与二值化方法的参数校验请求。 / The big-M and binaryzation-method parameter request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolLinearizationRequest {
    /// 调用方想用的 Big-M（可选；缺省时用 `defaultBigM`）。 / The big-M the caller wants (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub big_m: Option<f64>,
    /// 想描述的二值化方法（可选；缺省时给出全部四种）。 / The methods to describe (optional; all four by default).
    #[serde(default)]
    pub methods: Vec<String>,
}

/// 一种二值化方法的机制事实。 / The mechanism facts of one binaryzation method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolBinaryzationRow {
    /// 请求的方法（线格式代码）。 / The requested method (wire code).
    pub method: String,
    /// 机制层实际使用的等价方法。 / The equivalent method actually used at the mechanism layer.
    pub mechanism_equivalent: String,
    /// 这一层是否把请求的方法归一成别的（"我要原生能力"不等于"我拿到了原生能力"）。
    /// Whether this layer normalises the requested method into another one.
    pub normalised: bool,
    /// 结果取真时的关系。 / The relation when the result is true.
    pub when_true_relation: String,
    /// 结果取真时的右端项。 / The right-hand side when the result is true.
    pub when_true_rhs: f64,
    /// 结果取假时的关系。 / The relation when the result is false.
    pub when_false_relation: String,
    /// 结果取假时的右端项。 / The right-hand side when the result is false.
    pub when_false_rhs: f64,
    /// 下侧松弛行需要证明的右端项。 / The right-hand side the lower relaxation must prove.
    pub relaxed_lower_rhs: f64,
    /// 上侧松弛行需要证明的右端项。 / The right-hand side the upper relaxation must prove.
    pub relaxed_upper_rhs: f64,
}

/// big-M 与二值化方法的参数校验结论。 / The big-M and binaryzation-method verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolLinearizationReport {
    /// 默认 Big-M。 / The default big-M.
    pub default_big_m: f64,
    /// Big-M 的**下限**（不是 0）。 / The big-M **floor** (not zero).
    pub min_big_m: f64,
    /// 调用方给出的 Big-M（缺省时等于 `defaultBigM`）。 / The caller's big-M (equal to the default when omitted).
    pub requested_big_m: f64,
    /// 求解器实际会用的 Big-M（`ensure_positive_big_m` 之后）。 / The big-M actually used after the floor is applied.
    pub effective_big_m: f64,
    /// 是否被抬到下限（`effective != requested`）。 / Whether it was raised to the floor.
    pub clamped_to_floor: bool,
    /// 方法词表（桥接定义）。 / The method vocabulary (bridge-defined).
    pub method_vocabulary: Vec<String>,
    /// 逐方法的机制事实。 / The mechanism facts per method.
    pub methods: Vec<SymbolBinaryzationRow>,
}

/// 校验 big-M 并给出每种二值化方法的机制事实。非法参数是**错误**，原因原样带出。
/// Validate the big-M and report each binaryzation method's mechanism facts. Illegal parameters are **errors**.
pub fn verify_symbol_linearization(request: &SymbolLinearizationRequest) -> Result<SymbolLinearizationReport> {
    let requested = request.big_m.unwrap_or_else(default_big_m);
    // 领域规则：非有限、零、负数报错；正数但小于下限时**被抬到下限**（不是报错，也不是原样放行）。
    // Domain rule: non-finite, zero and negative values fail; a positive value below the floor is **raised to it** —
    // neither an error nor passed through untouched.
    let effective = ensure_positive_big_m(requested).map_err(|error| BridgeError::new("InvalidInput",
        &format!("symbol linearization failed validation: {error}")))?;
    let requested_methods = if request.methods.is_empty() {
        method_vocabulary_codes().iter().map(|code| (*code).to_owned()).collect::<Vec<_>>()
    } else {
        request.methods.clone()
    };
    let mut methods = Vec::with_capacity(requested_methods.len());
    for code in &requested_methods {
        let method = parse_method(code)?;
        let equivalent = method.mechanism_equivalent();
        let relations: BinaryzationCoreRelations = binaryzation_core_relations(method);
        methods.push(SymbolBinaryzationRow {
            method: method_code(method),
            mechanism_equivalent: method_code(equivalent),
            normalised: equivalent != method,
            when_true_relation: relation_code(relations.when_true.0),
            when_true_rhs: relations.when_true.1,
            when_false_relation: relation_code(relations.when_false.0),
            when_false_rhs: relations.when_false.1,
            relaxed_lower_rhs: relations.relaxed_lower_rhs,
            relaxed_upper_rhs: relations.relaxed_upper_rhs,
        });
    }
    Ok(SymbolLinearizationReport {
        default_big_m: default_big_m(),
        min_big_m: ospf_rust_core::symbol::function::big_m::MIN_BIG_M,
        requested_big_m: requested,
        effective_big_m: effective,
        clamped_to_floor: effective != requested,
        method_vocabulary: method_vocabulary_codes().iter().map(|code| (*code).to_owned()).collect(),
        methods,
    })
}

/// 方法词表。 / The method vocabulary.
fn method_vocabulary_codes() -> [&'static str; 4] {
    ["bigM", "threshold", "indicator", "sos1"]
}

/// 解析方法代码。 / Parse a method code.
fn parse_method(code: &str) -> Result<BinaryzationMethod> {
    match code {
        "bigM" => Ok(BinaryzationMethod::BigM),
        "threshold" => Ok(BinaryzationMethod::Threshold),
        "indicator" => Ok(BinaryzationMethod::Indicator),
        "sos1" => Ok(BinaryzationMethod::SOS1),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "symbol linearization failed validation: unknown binaryzation method `{other}`"))),
    }
}

/// 方法的线格式代码。 / The wire code of a method.
fn method_code(method: BinaryzationMethod) -> String {
    match method {
        BinaryzationMethod::BigM => "bigM".to_owned(),
        BinaryzationMethod::Threshold => "threshold".to_owned(),
        BinaryzationMethod::Indicator => "indicator".to_owned(),
        BinaryzationMethod::SOS1 => "sos1".to_owned(),
    }
}

/// 约束关系的线格式代码。 / The wire code of a constraint relation.
fn relation_code(relation: ConstraintRelation) -> String {
    match relation {
        ConstraintRelation::LessEqual => "lessEqual".to_owned(),
        ConstraintRelation::Equal => "equal".to_owned(),
        ConstraintRelation::GreaterEqual => "greaterEqual".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(big_m: Option<f64>, methods: &[&str]) -> SymbolLinearizationRequest {
        SymbolLinearizationRequest { big_m,
            methods: methods.iter().map(|method| (*method).to_owned()).collect() }
    }

    #[test]
    fn big_m_floor_and_defaults_are_reported() {
        // 缺省：默认值，未触发下限。 / Omitted: the default, floor not triggered.
        let report = verify_symbol_linearization(&request(None, &[])).unwrap();
        assert_eq!(report.default_big_m, 1_000_000.0);
        assert_eq!(report.min_big_m, 1.0);
        assert_eq!(report.requested_big_m, 1_000_000.0);
        assert_eq!(report.effective_big_m, 1_000_000.0);
        assert!(!report.clamped_to_floor);
        // 合法的较小值：原样放行（>= 下限）。 / A legal smaller value passes through untouched.
        let report = verify_symbol_linearization(&request(Some(250.0), &[])).unwrap();
        assert_eq!(report.effective_big_m, 250.0);
        assert!(!report.clamped_to_floor);
        // 低于下限但为正：**被抬到下限**，并标出来。 / Positive but below the floor: **raised**, and flagged.
        let report = verify_symbol_linearization(&request(Some(0.25), &[])).unwrap();
        assert_eq!(report.effective_big_m, 1.0);
        assert!(report.clamped_to_floor);
        // 恰好在边界上不算被抬。 / Exactly at the floor is not a clamp.
        let report = verify_symbol_linearization(&request(Some(1.0), &[])).unwrap();
        assert!(!report.clamped_to_floor);
    }

    #[test]
    fn every_method_reports_its_mechanism_facts() {
        let report = verify_symbol_linearization(&request(Some(100.0), &[])).unwrap();
        assert_eq!(report.method_vocabulary,
            vec!["bigM".to_owned(), "threshold".to_owned(), "indicator".to_owned(), "sos1".to_owned()]);
        assert_eq!(report.methods.len(), 4);
        let row = |code: &str| report.methods.iter()
            .find(|row| row.method == code).unwrap_or_else(|| panic!("missing {code}"));
        // `bigM`：`y=1 ⇒ s >= eps`、`y=0 ⇒ s <= 0`；松弛行 `s <= M`、`s >= eps - M`。
        // `bigM`: `y=1 ⇒ s >= eps`, `y=0 ⇒ s <= 0`; relaxed rows `s <= M` and `s >= eps - M`.
        let big_m = row("bigM");
        assert_eq!(big_m.mechanism_equivalent, "bigM");
        assert!(!big_m.normalised);
        assert_eq!(big_m.when_true_relation, "greaterEqual");
        assert!(big_m.when_true_rhs > 0.0 && big_m.when_true_rhs < 1e-12, "{}", big_m.when_true_rhs);
        assert_eq!(big_m.when_false_relation, "lessEqual");
        assert_eq!(big_m.when_false_rhs, 0.0);
        assert_eq!(big_m.relaxed_lower_rhs, big_m.when_true_rhs);
        assert_eq!(big_m.relaxed_upper_rhs, 0.0);
        // `threshold`：`y=1 ⇒ s >= 0`、`y=0 ⇒ s <= -eps`。 / `threshold`: `y=1 ⇒ s >= 0`, `y=0 ⇒ s <= -eps`.
        let threshold = row("threshold");
        assert_eq!(threshold.mechanism_equivalent, "threshold");
        assert!(!threshold.normalised);
        assert_eq!(threshold.when_true_rhs, 0.0);
        assert!(threshold.when_false_rhs < 0.0 && threshold.when_false_rhs > -1e-12);
        // **`indicator` 与 `sos1` 在机制层都被归一成 `bigM`**——这是这一单最值钱的一条事实。
        // **`indicator` and `sos1` are both normalised to `bigM` at the mechanism layer** — the most valuable fact here.
        for code in ["indicator", "sos1"] {
            let native = row(code);
            assert_eq!(native.mechanism_equivalent, "bigM", "{code}");
            assert!(native.normalised, "{code}");
            assert_eq!(native.when_true_relation, big_m.when_true_relation, "{code}");
            assert_eq!(native.when_true_rhs, big_m.when_true_rhs, "{code}");
            assert_eq!(native.when_false_rhs, big_m.when_false_rhs, "{code}");
        }
        // 指定子集时只报告该子集，词表仍然是全量。 / A subset reports only that subset; the vocabulary stays full.
        let report = verify_symbol_linearization(&request(Some(10.0), &["threshold"])).unwrap();
        assert_eq!(report.methods.len(), 1);
        assert_eq!(report.methods[0].method, "threshold");
        assert_eq!(report.method_vocabulary.len(), 4);
    }

    #[test]
    fn illegal_parameters_are_refused() {
        // 非有限、零、负数：报错（不是被抬到下限）。 / Non-finite, zero and negative values fail (not clamped).
        for value in [0.0, -1.0, f64::NAN] {
            let error = verify_symbol_linearization(&request(Some(value), &[])).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains("finite and positive"), "{}: {}", value, error.message);
        }
        // 未知方法。 / An unknown method.
        let error = verify_symbol_linearization(&request(None, &["simplex"])).unwrap_err();
        assert!(error.message.contains("unknown binaryzation method"), "{}", error.message);
        // 重复方法被如实重复报告（不静默去重）。 / A repeated method is reported twice, not silently deduplicated.
        let report = verify_symbol_linearization(&request(None, &["bigM", "bigM"])).unwrap();
        assert_eq!(report.methods.len(), 2);
    }
}
