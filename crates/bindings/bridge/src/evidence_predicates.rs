//! 续挖第 45 单：活动性证据的两个谓词——`satisfied()` 的 `None` 把**三件事**混成一个
//! （`analysis/activity.rs` 的 `ActivityEvidence`）
//! Continuation, delivery 45: the two predicates on activity evidence — `satisfied()`'s `None` conflates **three** different
//! things (`ActivityEvidence`).
//!
//! 第 44 单量的是判定的四段带子；这一单量**证据**本身的两个纯谓词。四条实测：
//!
//! 1. **`has_slack_metric()` 对同一个变体给两个答案**（`same_variant_two_answers()`）：
//!    `Semantic { margin: Some(_) }` → `true`，`Semantic { margin: None }` → `false`；
//!    另外 `LinearConstraint` / `VariableBound` / `IntegerComparison` 都是 `true`，`Unknown` 是 `false`。
//!    所以"这个证据有没有 slack 指标"**只能问谓词**，不能看变体名。
//! 2. **`satisfied()` 只对 `Semantic` 给 `Some`**（`satisfied_is_none_for_three_meanings()`）：
//!    `LinearConstraint` / `VariableBound` / `IntegerComparison` 也是 `None`——与 `Unknown` 的 `None` **无法区分**。
//!    于是 `None` 同时表示"求值失败"与"这类证据根本没有语义满足性"，调用方必须自己按变体再分一次。
//! 3. **`IntegerComparison` 有 slack 指标但**给不出 `satisfied`（`integer_comparison_cannot_answer()`）：
//!    整数比较在语义上明明可判定，接口却只回 `None`——两条谓词对同一个变体的**覆盖面不一致**。
//! 4. **`Unknown { reason }` 的 `reason` 不做任何校验**（`unknown_reason_is_unchecked()`）：空串、只有空格都能构造。
//!
//! Delivery 44 measured the four classification bands; this one measures the **evidence**'s two pure predicates. **The same
//! variant answers `has_slack_metric()` two ways** (`same_variant_two_answers()`): `Semantic { margin: Some(_) }` is `true`
//! while `Semantic { margin: None }` is `false`; `LinearConstraint`, `VariableBound` and `IntegerComparison` are all `true`
//! and `Unknown` is `false` — so "does this evidence carry a slack metric" **must** be asked of the predicate, never inferred
//! from the variant name. **`satisfied()` returns `Some` only for `Semantic`**
//! (`satisfied_is_none_for_three_meanings()`): `LinearConstraint`, `VariableBound` and `IntegerComparison` also answer `None`,
//! which is **indistinguishable** from `Unknown`'s `None`, so `None` simultaneously means "evaluation failed" and "this kind of
//! evidence has no semantic satisfaction" and callers must re-dispatch on the variant. **`IntegerComparison` carries a slack
//! metric yet cannot answer `satisfied()`** (`integer_comparison_cannot_answer()`): an integer comparison is semantically
//! decidable, but the interface only offers `None`, so the two predicates **disagree in coverage**. And **`Unknown { reason }`
//! validates nothing** (`unknown_reason_is_unchecked()`): an empty or whitespace-only reason constructs fine.

use ospf_rust_core::analysis::activity::ActivityEvidence;
use ospf_rust_core::analysis::BoundSide;
use ospf_rust_core::model::constraint_programming::expression::IntegerRelation;
use ospf_rust_core::solver::report::StableVariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条证据的字面量。 / One evidence literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceSpec {
    /// 证据种类：`linear` / `variableBound` / `integerComparison` / `semantic` / `unknown`。
    /// The kind: `linear`, `variableBound`, `integerComparison`, `semantic` or `unknown`.
    pub kind: String,
    /// `linear` 用：左侧值。 / For `linear`: the left-hand value.
    #[serde(default)]
    pub lhs: Option<f64>,
    /// `linear` 用：右侧值。 / For `linear`: the right-hand value.
    #[serde(default)]
    pub rhs: Option<f64>,
    /// `variableBound` 用：变量 ID。 / For `variableBound`: the variable ID.
    #[serde(default)]
    pub variable: Option<String>,
    /// `variableBound` 用：边界方向 `lower` / `upper`。 / For `variableBound`: `lower` or `upper`.
    #[serde(default)]
    pub side: Option<String>,
    /// `variableBound` 用：变量值与边界值。 / For `variableBound`: the value and the bound.
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub bound: Option<f64>,
    /// `integerComparison` 用：两个整数与关系。 / For `integerComparison`: two integers and the relation.
    #[serde(default)]
    pub integer_lhs: Option<i64>,
    #[serde(default)]
    pub integer_rhs: Option<i64>,
    /// `integerComparison` 用：`eq` / `ne` / `le` / `ge`。 / For `integerComparison`: `eq`, `ne`, `le` or `ge`.
    #[serde(default)]
    pub relation: Option<String>,
    /// `semantic` 用：是否满足。 / For `semantic`: whether it is satisfied.
    #[serde(default)]
    pub satisfied: Option<bool>,
    /// `semantic` 用：自然 margin（可为空）。 / For `semantic`: the natural margin (may be absent).
    #[serde(default)]
    pub margin: Option<f64>,
    /// `unknown` 用：失败原因。 / For `unknown`: the failure reason.
    #[serde(default)]
    pub reason: Option<String>,
}

/// 一条证据的结论。 / One evidence verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRow {
    /// 证据种类。 / The kind.
    pub kind: String,
    /// `hasSlackMetric()`。 / `hasSlackMetric()`.
    pub has_slack_metric: bool,
    /// `satisfied()` 的取值（`null` 表示 `None`）。 / The `satisfied()` value (`null` for `None`).
    pub satisfied: Option<bool>,
}

/// 证据谓词请求。 / The evidence-predicate request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceSpecRequest {
    /// 要检查的证据。 / The evidence to inspect.
    pub evidences: Vec<EvidenceSpec>,
}

/// 证据谓词结论。 / The evidence-predicate verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceReport {
    /// 逐条结论。 / One row per evidence.
    pub rows: Vec<EvidenceRow>,
    /// 有 slack 指标的证据条数。 / How many evidences carry a slack metric.
    pub with_slack_metric: u32,
    /// `satisfied()` 给出 `Some` 的条数。 / How many evidences answer `satisfied()`.
    pub with_satisfied: u32,
    /// 同一个变体给两个答案（**是**）。 / Whether one variant answers twice (**yes**).
    pub same_variant_two_answers: bool,
    /// `satisfied()` 的 `None` 混了三种含义（**是**）。 / Whether `satisfied()`'s `None` conflates three meanings (**yes**).
    pub satisfied_is_none_for_three_meanings: bool,
    /// 整数比较有 slack 却答不出 satisfied（**是**）。 / Whether an integer comparison cannot answer (**yes**).
    pub integer_comparison_cannot_answer: bool,
    /// `Unknown` 的原因不校验（**是**）。 / Whether `Unknown`'s reason is unchecked (**yes**).
    pub unknown_reason_is_unchecked: bool,
}

/// 证据条数上限（信任边界）。 / The evidence cap (a trust boundary).
const MAX_EVIDENCES: usize = 256;

/// 造一条证据。 / Build one evidence.
fn build(spec: &EvidenceSpec) -> Result<ActivityEvidence> {
    match spec.kind.as_str() {
        "linear" => Ok(ActivityEvidence::LinearConstraint {
            lhs: finite(spec.lhs, "linear lhs")?, rhs: finite(spec.rhs, "linear rhs")? }),
        "variableBound" => Ok(ActivityEvidence::VariableBound {
            variable_id: StableVariableId(spec.variable.clone().ok_or_else(|| {
                BridgeError::new("InvalidInput", "evidence failed validation: variableBound needs a variable")
            })?),
            side: match spec.side.as_deref() {
                Some("lower") => BoundSide::Lower,
                Some("upper") => BoundSide::Upper,
                _ => return Err(BridgeError::new("InvalidInput",
                    "evidence failed validation: side must be `lower` or `upper`")),
            },
            value: finite(spec.value, "variableBound value")?,
            bound: finite(spec.bound, "variableBound bound")? }),
        "integerComparison" => Ok(ActivityEvidence::IntegerComparison {
            lhs: spec.integer_lhs.ok_or_else(|| BridgeError::new("InvalidInput",
                "evidence failed validation: integerComparison needs two integers"))?,
            rhs: spec.integer_rhs.ok_or_else(|| BridgeError::new("InvalidInput",
                "evidence failed validation: integerComparison needs two integers"))?,
            relation: match spec.relation.as_deref() {
                Some("eq") => IntegerRelation::Equal,
                Some("ne") => IntegerRelation::NotEqual,
                Some("le") => IntegerRelation::LessOrEqual,
                Some("ge") => IntegerRelation::GreaterOrEqual,
                _ => return Err(BridgeError::new("InvalidInput",
                    "evidence failed validation: relation must be `eq`, `ne`, `le` or `ge`")),
            } }),
        "semantic" => Ok(ActivityEvidence::Semantic {
            satisfied: spec.satisfied.ok_or_else(|| BridgeError::new("InvalidInput",
                "evidence failed validation: semantic needs a satisfaction flag"))?,
            margin: spec.margin.map(|margin| margin).filter(|margin| margin.is_finite()) }),
        "unknown" => Ok(ActivityEvidence::Unknown {
            reason: spec.reason.clone().unwrap_or_default() }),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "evidence failed validation: unknown kind `{other}`"))),
    }
}

/// 取一个必须有限的字段。 / Fetch a field that must be finite.
fn finite(value: Option<f64>, name: &str) -> Result<f64> {
    value.filter(|value| value.is_finite()).ok_or_else(|| BridgeError::new("InvalidInput",
        &format!("evidence failed validation: {name} must be a finite number")))
}

/// 走一遍证据谓词。非法输入是**错误**。 / Walk the evidence predicates.
pub fn verify_evidence_predicates(request: &EvidenceSpecRequest) -> Result<EvidenceReport> {
    if request.evidences.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "evidence failed validation: at least one evidence is required"));
    }
    if request.evidences.len() > MAX_EVIDENCES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "evidence failed validation: at most {MAX_EVIDENCES} evidences are accepted")));
    }
    let evidences = request.evidences.iter().map(build).collect::<Result<Vec<_>>>()?;
    let rows: Vec<EvidenceRow> = request.evidences.iter().zip(evidences.iter())
        .map(|(spec, evidence)| EvidenceRow { kind: spec.kind.clone(),
            has_slack_metric: evidence.has_slack_metric(), satisfied: evidence.satisfied() })
        .collect();
    let findings = canonical_findings();
    Ok(EvidenceReport {
        with_slack_metric: u32::try_from(rows.iter().filter(|row| row.has_slack_metric).count())
            .unwrap_or(u32::MAX),
        with_satisfied: u32::try_from(rows.iter().filter(|row| row.satisfied.is_some()).count())
            .unwrap_or(u32::MAX),
        rows,
        same_variant_two_answers: findings.same_variant_two_answers,
        satisfied_is_none_for_three_meanings: findings.satisfied_is_none_for_three_meanings,
        integer_comparison_cannot_answer: findings.integer_comparison_cannot_answer,
        unknown_reason_is_unchecked: findings.unknown_reason_is_unchecked,
    })
}

/// 四条规则属性的内部固定探针。 / The four rule properties measured by fixed internal probes.
struct Findings {
    same_variant_two_answers: bool,
    satisfied_is_none_for_three_meanings: bool,
    integer_comparison_cannot_answer: bool,
    unknown_reason_is_unchecked: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let with_margin = ActivityEvidence::Semantic { satisfied: true, margin: Some(0.5) };
    let without_margin = ActivityEvidence::Semantic { satisfied: true, margin: None };
    // 1) 同一个变体给两个答案。 / One variant answers twice.
    let same_variant_two_answers = with_margin.has_slack_metric()
        && !without_margin.has_slack_metric();
    // 2) `satisfied()` 的 `None` 混了三种含义：未知 / 这类证据没有语义满足性 / 其它。
    //    2) `satisfied()`'s `None` conflates three meanings.
    let linear = ActivityEvidence::LinearConstraint { lhs: 1.0, rhs: 2.0 };
    let bound = ActivityEvidence::VariableBound { variable_id: StableVariableId("x".to_owned()),
        side: BoundSide::Lower, value: 1.0, bound: 0.0 };
    let unknown = ActivityEvidence::Unknown { reason: String::new() };
    let satisfied_is_none_for_three_meanings = linear.satisfied().is_none()
        && bound.satisfied().is_none() && unknown.satisfied().is_none()
        && with_margin.satisfied() == Some(true);
    // 3) 整数比较有 slack 却答不出 satisfied。 / An integer comparison cannot answer.
    let comparison = ActivityEvidence::IntegerComparison { lhs: 1, rhs: 2,
        relation: IntegerRelation::LessOrEqual };
    let integer_comparison_cannot_answer = comparison.has_slack_metric()
        && comparison.satisfied().is_none();
    // 4) `Unknown` 的原因不校验：空串与纯空格都能构造。 / `Unknown`'s reason is unchecked.
    let blank = ActivityEvidence::Unknown { reason: "   ".to_owned() };
    let unknown_reason_is_unchecked = !unknown.has_slack_metric()
        && !blank.has_slack_metric() && blank.satisfied().is_none();
    Findings { same_variant_two_answers, satisfied_is_none_for_three_meanings,
        integer_comparison_cannot_answer, unknown_reason_is_unchecked }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EvidenceSpecRequest {
        EvidenceSpecRequest { evidences: vec![
            EvidenceSpec { kind: "linear".to_owned(), lhs: Some(1.0), rhs: Some(2.0),
                variable: None, side: None, value: None, bound: None, integer_lhs: None,
                integer_rhs: None, relation: None, satisfied: None, margin: None, reason: None },
            EvidenceSpec { kind: "semantic".to_owned(), lhs: None, rhs: None, variable: None,
                side: None, value: None, bound: None, integer_lhs: None, integer_rhs: None,
                relation: None, satisfied: Some(true), margin: Some(0.5), reason: None },
            EvidenceSpec { kind: "unknown".to_owned(), lhs: None, rhs: None, variable: None,
                side: None, value: None, bound: None, integer_lhs: None, integer_rhs: None,
                relation: None, satisfied: None, margin: None, reason: Some("boom".to_owned()) },
        ] }
    }

    #[test]
    fn the_predicates_disagree_on_the_same_variant() {
        let report = verify_evidence_predicates(&request()).unwrap();
        // 线性证据：有 slack 指标，但 `satisfied()` 是 `None`。 
        // Linear evidence: has a slack metric, yet `satisfied()` is `None`.
        assert!(report.rows[0].has_slack_metric);
        assert_eq!(report.rows[0].satisfied, None);
        // 带 margin 的语义证据：两个谓词都答。 / A semantic evidence with a margin answers both.
        assert!(report.rows[1].has_slack_metric);
        assert_eq!(report.rows[1].satisfied, Some(true));
        // `Unknown`：两个都不答。 / `Unknown` answers neither.
        assert!(!report.rows[2].has_slack_metric);
        assert_eq!(report.rows[2].satisfied, None);
        assert_eq!(report.with_slack_metric, 2);
        assert_eq!(report.with_satisfied, 1);
        assert!(report.same_variant_two_answers);
        assert!(report.satisfied_is_none_for_three_meanings);
        assert!(report.integer_comparison_cannot_answer);
        assert!(report.unknown_reason_is_unchecked);
    }

    #[test]
    fn a_semantic_without_a_margin_loses_the_metric() {
        let mut document = request();
        document.evidences[1].margin = None;
        let report = verify_evidence_predicates(&document).unwrap();
        // 同一个变体，没有 margin 就没有 slack 指标。 / Same variant, no margin means no slack metric.
        assert!(!report.rows[1].has_slack_metric);
        assert_eq!(report.rows[1].satisfied, Some(true));
        assert_eq!(report.with_slack_metric, 1);
    }

    #[test]
    fn an_integer_comparison_has_a_metric_but_no_answer() {
        let document = EvidenceSpecRequest { evidences: vec![EvidenceSpec {
            kind: "integerComparison".to_owned(), lhs: None, rhs: None, variable: None,
            side: None, value: None, bound: None, integer_lhs: Some(1),
            integer_rhs: Some(2), relation: Some("le".to_owned()), satisfied: None,
            margin: None, reason: None }] };
        let report = verify_evidence_predicates(&document).unwrap();
        assert!(report.rows[0].has_slack_metric);
        assert_eq!(report.rows[0].satisfied, None);
    }

    #[test]
    fn a_variable_bound_keeps_its_side() {
        let document = EvidenceSpecRequest { evidences: vec![EvidenceSpec {
            kind: "variableBound".to_owned(), lhs: None, rhs: None,
            variable: Some("x".to_owned()), side: Some("upper".to_owned()), value: Some(3.0),
            bound: Some(5.0), integer_lhs: None, integer_rhs: None, relation: None,
            satisfied: None, margin: None, reason: None }] };
        let report = verify_evidence_predicates(&document).unwrap();
        assert!(report.rows[0].has_slack_metric);
        assert_eq!(report.rows[0].satisfied, None);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let document = EvidenceSpecRequest { evidences: vec![EvidenceSpec {
            kind: "unknown".to_owned(), lhs: None, rhs: None, variable: None, side: None,
            value: None, bound: None, integer_lhs: None, integer_rhs: None, relation: None,
            satisfied: None, margin: None, reason: None }] };
        let report = verify_evidence_predicates(&document).unwrap();
        assert!(report.same_variant_two_answers);
        assert!(report.satisfied_is_none_for_three_meanings);
        assert!(report.integer_comparison_cannot_answer);
        assert!(report.unknown_reason_is_unchecked);
        assert_eq!(report.with_slack_metric, 0);
        assert_eq!(report.with_satisfied, 0);
    }

    #[test]
    fn bad_input_is_refused() {
        let empty = EvidenceSpecRequest { evidences: Vec::new() };
        assert!(verify_evidence_predicates(&empty).unwrap_err().message
            .contains("at least one evidence"));
        let mut unknown_kind = request();
        unknown_kind.evidences[0].kind = "quadratic".to_owned();
        assert!(verify_evidence_predicates(&unknown_kind).unwrap_err().message
            .contains("unknown kind"));
        let mut missing_rhs = request();
        missing_rhs.evidences[0].rhs = None;
        assert!(verify_evidence_predicates(&missing_rhs).unwrap_err().message
            .contains("linear rhs must be a finite number"));
        let mut bad_side = request();
        bad_side.evidences[0] = EvidenceSpec { kind: "variableBound".to_owned(),
            lhs: None, rhs: None, variable: Some("x".to_owned()), side: Some("middle".to_owned()),
            value: Some(1.0), bound: Some(2.0), integer_lhs: None, integer_rhs: None,
            relation: None, satisfied: None, margin: None, reason: None };
        assert!(verify_evidence_predicates(&bad_side).unwrap_err().message
            .contains("side must be"));
        let too_many = EvidenceSpecRequest { evidences: (0..257).map(|_| EvidenceSpec {
            kind: "unknown".to_owned(), lhs: None, rhs: None, variable: None, side: None,
            value: None, bound: None, integer_lhs: None, integer_rhs: None, relation: None,
            satisfied: None, margin: None, reason: None }).collect() };
        assert!(verify_evidence_predicates(&too_many).unwrap_err().message
            .contains("at most 256 evidences"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_evidence_predicates(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"hasSlackMetric\""), "{json}");
        assert!(json.contains("\"withSlackMetric\""), "{json}");
        assert!(json.contains("\"satisfiedIsNoneForThreeMeanings\""), "{json}");
        assert!(!json.contains("has_slack"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<EvidenceReport>(&copy.to_string()).is_err());
    }
}
