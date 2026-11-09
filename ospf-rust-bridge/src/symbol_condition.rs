//! 批次 F 第 1 单：`symbol/functions` 的条件词汇与参数校验 / Batch F, delivery 1: the condition vocabulary and parameter
//! validation from `symbol/functions`.
//!
//! 这一单只做**词汇 + 参数校验**，不承诺求值（不求值 `Linear`、不建 var/token、不生成约束）。挑的是
//! `symbol/functions/conditional.rs` 里三个**纯数值**入口，它们本来就不需要模型：
//!
//! * `ConditionBounds::validate` —— 范围有限且 `lower <= upper`；
//! * `validate_relation_bounds` —— 除上述外还要求严格边界**有限且为正**，并**拒绝整个范围都落在 `Undefined` 间隔内**
//!   的条件：那种条件永远无法判定，悄悄归类成 `false` 会让调用方以为证明过；
//! * `branch_coverage` —— 有限范围是否把条件**钉死**在单一分支（可判定为恒真/恒假）；
//! * `classify` —— 单个差值按关系与边界分成三值 `true` / `false` / `undefined`。
//!
//! TS 侧拿到的是一份**结论**：词汇（关系四种、真值三种）、范围是否合法、是否被钉死、以及每个样本差值的三值分类。
//! 这正是调用方在把条件交给线性化之前需要知道的事——而三值逻辑的边界规则（`>= boundary` 才是真、
//! `<= 0` 才是假，中间是"未定义"）在 TS 里重写一遍几乎必然写错。
//!
//! This delivery covers only the **vocabulary and parameter validation** and promises no evaluation: no `Linear` is
//! evaluated, no variable or token is created and no constraint is generated. It picks the three **purely numeric** entry
//! points in `symbol/functions/conditional.rs`, which never needed a model: `ConditionBounds::validate` (finite and ordered),
//! `validate_relation_bounds` (also requiring a finite positive strict boundary and **rejecting a range that lies entirely
//! inside the `Undefined` interval**, since such a condition can never be decided and silently calling it false would let the
//! caller believe it was proven), `branch_coverage` (whether the finite range **pins** the condition to a single branch), and
//! `classify` (three-valued `true` / `false` / `undefined` per difference). TypeScript receives a **verdict**: the vocabulary
//! (four relations, three truth values), whether the range is legal, whether it is pinned, and the three-valued
//! classification of each sampled difference — exactly what a caller needs before handing a condition to linearization,
//! since the three-valued boundary rules (`>= boundary` is true, `<= 0` is false, in between is undefined) are almost
//! certainly wrong when rewritten by hand.

use ospf_rust_core::symbol::function::conditional::{
    branch_coverage, classify, validate_relation_bounds, ConditionBounds, ConditionRelation, TruthValue,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 条件词汇与参数校验请求。 / The condition vocabulary and parameter-validation request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolConditionRequest {
    /// 关系：`greater` / `greaterEqual` / `less` / `lessEqual`。 / The relation.
    pub relation: String,
    /// 严格边界；必须有限且为正。 / The strict boundary; must be finite and positive.
    pub strict_boundary: f64,
    /// 条件差值的有限范围。 / Finite bounds for the condition difference.
    pub bounds: SymbolConditionBounds,
    /// 待分类的样本差值（可选）。 / Sampled differences to classify (optional).
    #[serde(default)]
    pub differences: Vec<f64>,
}

/// 条件差值的有限范围。 / Finite bounds for a condition difference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolConditionBounds {
    pub lower: f64,
    pub upper: f64,
}

/// 一个样本差值的三值分类。 / The three-valued classification of one sampled difference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolConditionSample {
    /// 样本差值。 / The sampled difference.
    pub difference: f64,
    /// 分类：`true` / `false` / `undefined`。 / The classification.
    pub truth: String,
}

/// 条件词汇与参数校验结论。 / The condition vocabulary and parameter-validation verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolConditionReport {
    /// 关系（线格式代码）。 / The relation (wire code).
    pub relation: String,
    /// 严格边界。 / The strict boundary.
    pub strict_boundary: f64,
    /// 范围下界。 / The range's lower bound.
    pub lower: f64,
    /// 范围上界。 / The range's upper bound.
    pub upper: f64,
    /// 范围是否把条件**钉死**在单一分支：`true` / `false` / `undefined`，未钉死时为 `undefined`。
    /// Whether the range **pins** the condition to a single branch; `undefined` when it does not.
    pub coverage: String,
    /// 范围是否已完全可判定（`coverage != "undefined"`）。 / Whether the range is fully decidable.
    pub decidable: bool,
    /// 样本分类。 / The sample classifications.
    pub samples: Vec<SymbolConditionSample>,
    /// 分类为 `true` 的样本数。 / How many samples classify as `true`.
    pub true_count: u32,
    /// 分类为 `false` 的样本数。 / How many samples classify as `false`.
    pub false_count: u32,
    /// 分类为 `undefined` 的样本数。 / How many samples classify as `undefined`.
    pub undefined_count: u32,
    /// 关系词表（桥接定义）。 / The relation vocabulary (bridge-defined).
    pub relation_vocabulary: Vec<String>,
    /// 真值词表。 / The truth-value vocabulary.
    pub truth_vocabulary: Vec<String>,
}

/// 校验条件参数并投影成结论。非法参数是**错误**，原因原样带出。
/// Validate the condition parameters and project a verdict. An illegal parameter is an **error** carrying the reason.
pub fn verify_symbol_condition(request: &SymbolConditionRequest) -> Result<SymbolConditionReport> {
    let relation = parse_relation(&request.relation)?;
    // 非有限数值在 JSON 里会变成 `null`，只能在 DTO 边界拦。
    // Non-finite numbers become `null` in JSON, so they can only be refused at the DTO boundary.
    for (label, value) in [("strict boundary", request.strict_boundary), ("lower bound", request.bounds.lower),
        ("upper bound", request.bounds.upper)]
    {
        if !value.is_finite() {
            return Err(BridgeError::new("InvalidInput", &format!(
                "symbol condition failed validation: the {label} must be finite")));
        }
    }
    let bounds = ConditionBounds::<f64> { lower: request.bounds.lower, upper: request.bounds.upper };
    // 领域校验：范围有限有序、严格边界为正、且范围不得整个落在 `Undefined` 间隔内。
    // Domain validation: finite ordered bounds, a positive strict boundary, and no range wholly inside the `Undefined`
    // interval.
    validate_relation_bounds(&bounds, relation, &request.strict_boundary)
        .map_err(|error| BridgeError::new("InvalidInput", &format!(
            "symbol condition failed validation: {error}")))?;
    // 领域判定：范围是否把条件钉死在单一分支。 / Domain decision: whether the range pins the condition to one branch.
    let coverage = branch_coverage(&bounds, relation, &request.strict_boundary)
        .map_err(|error| BridgeError::new("InvalidInput", &format!(
            "symbol condition failed validation: {error}")))?;
    let mut samples = Vec::with_capacity(request.differences.len());
    let mut true_count = 0u32;
    let mut false_count = 0u32;
    let mut undefined_count = 0u32;
    for difference in &request.differences {
        let truth = classify(difference, relation, &request.strict_boundary).map_err(|error|
            BridgeError::new("InvalidInput", &format!("symbol condition failed validation: {error}")))?;
        match truth {
            TruthValue::True => true_count = true_count.saturating_add(1),
            TruthValue::False => false_count = false_count.saturating_add(1),
            TruthValue::Undefined => undefined_count = undefined_count.saturating_add(1),
        }
        samples.push(SymbolConditionSample { difference: *difference, truth: truth_name(truth) });
    }
    Ok(SymbolConditionReport {
        relation: relation_name(relation),
        strict_boundary: request.strict_boundary,
        lower: request.bounds.lower,
        upper: request.bounds.upper,
        coverage: coverage.map_or_else(|| "undefined".to_owned(), truth_name),
        decidable: coverage.is_some(),
        samples,
        true_count,
        false_count,
        undefined_count,
        relation_vocabulary: vec!["greater".to_owned(), "greaterEqual".to_owned(), "less".to_owned(),
            "lessEqual".to_owned()],
        truth_vocabulary: vec!["true".to_owned(), "false".to_owned(), "undefined".to_owned()],
    })
}

/// 解析关系代码。 / Parse a relation code.
fn parse_relation(code: &str) -> Result<ConditionRelation> {
    match code {
        "greater" => Ok(ConditionRelation::Greater),
        "greaterEqual" => Ok(ConditionRelation::GreaterEqual),
        "less" => Ok(ConditionRelation::Less),
        "lessEqual" => Ok(ConditionRelation::LessEqual),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "symbol condition failed validation: unknown relation `{other}`"))),
    }
}

/// 关系的线格式代码。 / The wire code of a relation.
fn relation_name(relation: ConditionRelation) -> String {
    match relation {
        ConditionRelation::Greater => "greater".to_owned(),
        ConditionRelation::GreaterEqual => "greaterEqual".to_owned(),
        ConditionRelation::Less => "less".to_owned(),
        ConditionRelation::LessEqual => "lessEqual".to_owned(),
    }
}

/// 真值的线格式代码。 / The wire code of a truth value.
fn truth_name(truth: TruthValue) -> String {
    match truth {
        TruthValue::True => "true".to_owned(),
        TruthValue::False => "false".to_owned(),
        TruthValue::Undefined => "undefined".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(relation: &str, boundary: f64, lower: f64, upper: f64) -> SymbolConditionRequest {
        SymbolConditionRequest { relation: relation.to_owned(), strict_boundary: boundary,
            bounds: SymbolConditionBounds { lower, upper }, differences: Vec::new() }
    }

    #[test]
    fn bounds_are_validated_and_coverage_is_projected() {
        // 范围钉死在真分支。 / A range pinned to the true branch.
        let report = verify_symbol_condition(&request("greater", 2.0, 2.0, 10.0)).unwrap();
        assert_eq!(report.relation, "greater");
        assert_eq!(report.coverage, "true");
        assert!(report.decidable);
        assert_eq!(report.relation_vocabulary.len(), 4);
        assert_eq!(report.truth_vocabulary, vec!["true".to_owned(), "false".to_owned(),
            "undefined".to_owned()]);
        // 范围钉死在假分支。 / A range pinned to the false branch.
        let report = verify_symbol_condition(&request("greater", 2.0, -5.0, 0.0)).unwrap();
        assert_eq!(report.coverage, "false");
        assert!(report.decidable);
        // 未钉死：范围跨越三个区段。 / Not pinned: the range spans all three regions.
        let report = verify_symbol_condition(&request("greater", 2.0, -1.0, 5.0)).unwrap();
        assert_eq!(report.coverage, "undefined");
        assert!(!report.decidable);
    }

    #[test]
    fn three_valued_classification_is_measured() {
        // `greater`：`>= boundary` 为真、`<= 0` 为假、中间未定义。 / `greater`: `>= boundary` true, `<= 0` false.
        let mut document = request("greater", 2.0, -1.0, 5.0);
        document.differences = vec![2.0, 3.0, 0.0, -1.0, 1.0];
        let report = verify_symbol_condition(&document).unwrap();
        let truths: Vec<&str> = report.samples.iter().map(|sample| sample.truth.as_str()).collect();
        assert_eq!(truths, vec!["true", "true", "false", "false", "undefined"]);
        assert_eq!(report.true_count, 2);
        assert_eq!(report.false_count, 2);
        assert_eq!(report.undefined_count, 1);
        // `greaterEqual`：`>= 0` 为真、`<= -boundary` 为假。 / `greaterEqual`: `>= 0` true, `<= -boundary` false.
        let mut document = request("greaterEqual", 2.0, -5.0, 5.0);
        document.differences = vec![0.0, -3.0, -1.0];
        let report = verify_symbol_condition(&document).unwrap();
        let truths: Vec<&str> = report.samples.iter().map(|sample| sample.truth.as_str()).collect();
        assert_eq!(truths, vec!["true", "false", "undefined"]);
        // `less`：`<= -boundary` 为真、`>= 0` 为假。 / `less`: `<= -boundary` true, `>= 0` false.
        let mut document = request("less", 2.0, -5.0, 5.0);
        document.differences = vec![-2.0, 0.0, -1.0];
        let report = verify_symbol_condition(&document).unwrap();
        let truths: Vec<&str> = report.samples.iter().map(|sample| sample.truth.as_str()).collect();
        assert_eq!(truths, vec!["true", "false", "undefined"]);
        // `lessEqual`：`<= 0` 为真、`>= boundary` 为假。 / `lessEqual`: `<= 0` true, `>= boundary` false.
        let mut document = request("lessEqual", 2.0, -5.0, 5.0);
        document.differences = vec![0.0, 2.0, 1.0];
        let report = verify_symbol_condition(&document).unwrap();
        let truths: Vec<&str> = report.samples.iter().map(|sample| sample.truth.as_str()).collect();
        assert_eq!(truths, vec!["true", "false", "undefined"]);
        // 无样本。 / No samples.
        let report = verify_symbol_condition(&request("lessEqual", 1.0, 0.0, 0.0)).unwrap();
        assert!(report.samples.is_empty());
        assert_eq!((report.true_count, report.false_count, report.undefined_count), (0, 0, 0));
        assert_eq!(report.coverage, "true");
    }

    #[test]
    fn illegal_parameters_are_refused() {
        // 严格边界必须为正。 / The strict boundary must be positive.
        for boundary in [0.0, -1.0] {
            let error = verify_symbol_condition(&request("greater", boundary, 0.0, 5.0)).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains("positive"), "{}", error.message);
        }
        // 下界不得超过上界。 / The lower bound must not exceed the upper bound.
        let error = verify_symbol_condition(&request("greater", 1.0, 5.0, 0.0)).unwrap_err();
        assert!(error.message.contains("must not exceed"), "{}", error.message);
        // 非有限数值在 DTO 边界被拒。 / Non-finite numbers are refused at the DTO boundary.
        let mut infinite = request("greater", 1.0, 0.0, 5.0);
        infinite.bounds.upper = f64::INFINITY;
        assert!(verify_symbol_condition(&infinite).unwrap_err().message.contains("must be finite"));
        // 整个范围落在 `Undefined` 间隔内：拒绝（否则永远无法判定，却又不像错的）。
        // A range wholly inside the `Undefined` interval is refused: it can never be decided, yet looks harmless.
        let error = verify_symbol_condition(&request("greater", 3.0, 0.5, 2.5)).unwrap_err();
        assert!(error.message.contains("Undefined interval"), "{}", error.message);
        // 未知关系。 / An unknown relation.
        let error = verify_symbol_condition(&request("equal", 1.0, 0.0, 5.0)).unwrap_err();
        assert!(error.message.contains("unknown relation"), "{}", error.message);
        // 样本差值非有限。 / A non-finite sampled difference.
        let mut document = request("greater", 1.0, -5.0, 5.0);
        document.differences = vec![f64::NAN];
        assert!(verify_symbol_condition(&document).unwrap_err().message.contains("must be finite"));
    }
}
