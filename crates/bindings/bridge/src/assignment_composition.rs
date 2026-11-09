//! 续挖第 51 单：更新赋值集合的组合语义与属性路径的三种校验强度
//! （`persistence/expression/update_assignment.rs` + `math/symbol/expression/property_path.rs`）
//! Continuation, delivery 51: the combinator semantics of update assignments and the three validation strengths of a property
//! path (`persistence/expression/update_assignment.rs` + `math/symbol/expression/property_path.rs`).
//!
//! 前五十单量的是求解与报告面；这一单量 §11.14 里点名的 `persistence/expression`。七条实测：
//!
//! 1. **`then` 是 FIFO 追加，`Add` 与它等价**（`the_order_is_fifo_and_add_matches_then()`）。
//! 2. **同一个路径不会被合并**（`the_same_path_is_not_merged()`）：`set("x", 1).then_set("x", 2)` 是**两条**——
//!    "后写覆盖前写"是执行层的事，本层只按顺序留着。
//! 3. **`set_null("x")` 与 `set("x", Null)` 是两个不同变体**（`null_value_and_set_null_differ()`）：
//!    `SetNull { path }` 与 `SetValue { path, value: Null }` **不相等**，虽然语义上都是"置空"。
//! 4. **属性路径有三种校验强度**（`three_validation_strengths()`）：`new` **完全不校验**（原样留）、
//!    `parse` **只 trim**、`try_parse`/`parse_or_none` 才要求每段都是合法标识符。
//! 5. **`of` 不做任何校验**（`of_does_not_validate_segments()`）：`of(["a", "", "b"])` 直接得到 `"a..b"`。
//! 6. **空白路径是"没有分段"，不是"一段空串"**（`a_blank_path_has_no_segments()`）：
//!    `parse("   ")` 得到空路径，`segments()` 是**空**向量。
//! 7. **标识符判定是 Unicode 的**（`identifiers_are_unicode_aware()`）：`"名字"` 合法，
//!    而 `"1name"`、`"na-me"`、`"na me"` 都不合法。
//!
//! The previous fifty deliveries measured solving and reporting; this one measures the `persistence/expression` surface the
//! §11.14 audit names. **`then` appends FIFO and `Add` matches it** (`the_order_is_fifo_and_add_matches_then()`). **The same
//! path is never merged** (`the_same_path_is_not_merged()`): `set("x", 1).then_set("x", 2)` stays **two items** — "last write
//! wins" belongs to the execution layer, this layer just keeps the order. **`set_null("x")` and `set("x", Null)` are two
//! different variants** (`null_value_and_set_null_differ()`): `SetNull { path }` and `SetValue { path, value: Null }` are **not
//! equal** although both mean "clear the field". **A property path has three validation strengths** (`three_validation_strengths()`):
//! `new` **validates nothing** (kept verbatim), `parse` **only trims**, and only `try_parse`/`parse_or_none` demand that every
//! segment be a valid identifier. **`of` validates nothing either** (`of_does_not_validate_segments()`): `of(["a", "", "b"])`
//! yields `"a..b"`. **A blank path has no segments rather than one empty segment** (`a_blank_path_has_no_segments()`):
//! `parse("   ")` gives the empty path and `segments()` is an **empty** vector. And **identifier rules are Unicode-aware**
//! (`identifiers_are_unicode_aware()`): `"名字"` is valid while `"1name"`, `"na-me"` and `"na me"` are not.

use ospf_rust_framework::persistence::expression::update_assignment::{
    UpdateAssignment, UpdateAssignments,
};
use ospf_rust_math::symbol::{ExpressionValue, PropertyPath, ScalarExpression};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 赋值字面量（TS 可直接表达）。 / An assignment literal TS can express.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum AssignLiteral {
    /// 空值。 / The null value.
    Null,
    /// 布尔值。 / A boolean.
    Boolean(bool),
    /// 数字。 / A number.
    Number(f64),
    /// 字符串。 / A string.
    Text(String),
}

use crate::{BridgeError, Result};

/// 一步赋值。 / One assignment step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignStep {
    /// 字段路径。 / The field path.
    pub path: String,
    /// `value` / `null` / `expression`。 / The step kind.
    #[serde(default)]
    pub kind: Option<String>,
    /// `value` 的值。 / The `value` payload.
    #[serde(default)]
    pub value: Option<AssignLiteral>,
}

/// 一条赋值项的字面量。 / One assignment item literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignOutcome {
    /// 路径。 / The path.
    pub path: String,
    /// `set-value` / `set-null` / `set-from-expression`。 / The variant name.
    pub variant: String,
    /// 值种类：`null` / `boolean` / `number` / `string`，`set-null` 时为 `none`。
    /// The value kind; `none` for `set-null`.
    pub value_kind: String,
}

/// 更新赋值请求。 / The update-assignment request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentCompositionRequest {
    /// 按顺序追加的赋值步骤。 / The steps appended in order.
    #[serde(default)]
    pub steps: Vec<AssignStep>,
    /// 逐条试的路径（量三种校验强度）。 / The paths to try one by one (to measure the three strengths).
    #[serde(default)]
    pub paths: Vec<String>,
    /// `of` 的分段（量不做校验）。 / The segments for `of` (to measure the missing validation).
    #[serde(default)]
    pub segments: Vec<String>,
}

/// 更新赋值结论。 / The update-assignment verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentCompositionReport {
    /// 逐条赋值项。 / One entry per assignment item.
    pub items: Vec<AssignOutcome>,
    /// 集合是否为空。 / Whether the set is empty.
    pub empty: bool,
    /// 空集合追加空集合之后是否仍为空。 / Whether an empty set stays empty after appending an empty set.
    pub empty_after_then: bool,
    /// 每条路径的 `raw` / `parsed` / `validated` 结果。 / Per path: the `raw`, `parsed` and `validated` results.
    pub paths: Vec<PathOutcome>,
    /// `of` 拼出来的路径。 / The path `of` produced.
    pub of_path: String,
    /// `of` 的路径分段。 / The segments of that path.
    pub of_segments: Vec<String>,
    /// 顺序是 FIFO 且 `Add` 等价于 `then`（**是**）。 / Whether the order is FIFO and `Add` matches `then` (**yes**).
    pub the_order_is_fifo_and_add_matches_then: bool,
    /// 同一路径不被合并（**是**）。 / Whether the same path is left unmerged (**yes**).
    pub the_same_path_is_not_merged: bool,
    /// `setNull` 与 `set(_ , null)` 不同（**是**）。 / Whether `setNull` and `set(_, null)` differ (**yes**).
    pub null_value_and_set_null_differ: bool,
    /// 三种校验强度（**是**）。 / Whether the three validation strengths hold (**yes**).
    pub three_validation_strengths: bool,
    /// `of` 不校验分段（**是**）。 / Whether `of` skips segment validation (**yes**).
    pub of_does_not_validate_segments: bool,
    /// 空白路径没有分段（**是**）。 / Whether a blank path has no segments (**yes**).
    pub a_blank_path_has_no_segments: bool,
    /// 标识符判定是 Unicode 的（**是**）。 / Whether identifier rules are Unicode-aware (**yes**).
    pub identifiers_are_unicode_aware: bool,
}

/// 一条路径的三种结果。 / Three results for one path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathOutcome {
    /// 原文本。 / The original text.
    pub raw: String,
    /// `PropertyPath::new` 的结果。 / The `PropertyPath::new` result.
    pub from_new: String,
    /// `PropertyPath::parse` 的结果。 / The `PropertyPath::parse` result.
    pub from_parse: String,
    /// `PropertyPath::try_parse` 是否成功。 / Whether `PropertyPath::try_parse` succeeded.
    pub parsed: bool,
    /// `parse` 之后的分段。 / The segments after `parse`.
    pub segments: Vec<String>,
}

/// 赋值步骤上限（信任边界）。 / The step cap (a trust boundary).
const MAX_STEPS: usize = 64;

/// 把字面量转成值。 / Turn a literal into a value.
fn value_of(literal: Option<&AssignLiteral>) -> Result<ExpressionValue> {
    Ok(match literal {
        None | Some(AssignLiteral::Null) => ExpressionValue::Null,
        Some(AssignLiteral::Boolean(flag)) => ExpressionValue::Boolean(*flag),
        Some(AssignLiteral::Number(number)) => ExpressionValue::Number(*number),
        Some(AssignLiteral::Text(text)) => ExpressionValue::String(text.clone()),
    })
}

/// 造一条赋值项。 / Build one assignment item.
fn assignment(step: &AssignStep) -> Result<UpdateAssignment> {
    let path = PropertyPath::new(step.path.clone());
    match step.kind.as_deref().unwrap_or("value") {
        "value" => Ok(UpdateAssignment::SetValue(
            ospf_rust_framework::persistence::expression::update_assignment::SetValue::new(
                path, value_of(step.value.as_ref())?))),
        "null" => Ok(UpdateAssignment::SetNull(
            ospf_rust_framework::persistence::expression::update_assignment::SetNull::new(path))),
        "expression" => Ok(UpdateAssignment::SetFromExpression(
            ospf_rust_framework::persistence::expression::update_assignment::SetFromExpression::new(
                path, ScalarExpression::constant(value_of(step.value.as_ref())?)))),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "update assignment failed validation: unknown step kind `{other}`"))),
    }
}

/// 变体名。 / The variant name.
fn variant_name(assignment: &UpdateAssignment) -> &'static str {
    match assignment {
        UpdateAssignment::SetValue(_) => "set-value",
        UpdateAssignment::SetNull(_) => "set-null",
        UpdateAssignment::SetFromExpression(_) => "set-from-expression",
    }
}

/// 值种类名。 / The value-kind name.
fn value_kind_name(assignment: &UpdateAssignment) -> &'static str {
    match assignment {
        UpdateAssignment::SetNull(_) => "none",
        UpdateAssignment::SetValue(item) => kind_of(&item.value),
        UpdateAssignment::SetFromExpression(_) => "expression",
    }
}

/// 值的种类名。 / The name of a value's kind.
fn kind_of(value: &ExpressionValue) -> &'static str {
    match value {
        ExpressionValue::Null => "null",
        ExpressionValue::Boolean(_) => "boolean",
        ExpressionValue::Number(_) => "number",
        ExpressionValue::String(_) => "string",
    }
}

/// 走一遍更新赋值与路径校验。非法输入是**错误**。 / Walk the assignments and path checks.
pub fn verify_assignment_composition(request: &AssignmentCompositionRequest)
    -> Result<AssignmentCompositionReport>
{
    if request.steps.len() > MAX_STEPS || request.paths.len() > MAX_STEPS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "update assignment failed validation: at most {MAX_STEPS} entries are accepted")));
    }
    let mut assignments = UpdateAssignments::<ExpressionValue>::empty();
    for step in &request.steps {
        assignments = assignments.then(UpdateAssignments { items: vec![assignment(step)?] });
    }
    let items = assignments.items.iter().map(|assignment| AssignOutcome {
        path: assignment.path().value().to_owned(),
        variant: variant_name(assignment).to_owned(),
        value_kind: value_kind_name(assignment).to_owned() }).collect::<Vec<_>>();
    let empty = assignments.is_empty();
    let empty_after_then = UpdateAssignments::<ExpressionValue>::empty()
        .then(UpdateAssignments::empty()).is_empty();
    let paths = request.paths.iter().map(|raw| PathOutcome {
        raw: raw.clone(),
        from_new: PropertyPath::new(raw.clone()).value().to_owned(),
        from_parse: PropertyPath::parse(raw).value().to_owned(),
        parsed: PropertyPath::try_parse(raw).is_ok(),
        segments: PropertyPath::parse(raw).segments().into_iter()
            .map(|segment| segment.to_owned()).collect(),
    }).collect::<Vec<_>>();
    let of_path = PropertyPath::of(request.segments.iter().map(|segment| segment.as_str()));
    let of_segments = of_path.segments().into_iter().map(|segment| segment.to_owned())
        .collect::<Vec<_>>();
    let findings = canonical_findings();
    Ok(AssignmentCompositionReport { items, empty, empty_after_then, paths,
        of_path: of_path.value().to_owned(), of_segments,
        the_order_is_fifo_and_add_matches_then: findings.the_order_is_fifo_and_add_matches_then,
        the_same_path_is_not_merged: findings.the_same_path_is_not_merged,
        null_value_and_set_null_differ: findings.null_value_and_set_null_differ,
        three_validation_strengths: findings.three_validation_strengths,
        of_does_not_validate_segments: findings.of_does_not_validate_segments,
        a_blank_path_has_no_segments: findings.a_blank_path_has_no_segments,
        identifiers_are_unicode_aware: findings.identifiers_are_unicode_aware })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    the_order_is_fifo_and_add_matches_then: bool,
    the_same_path_is_not_merged: bool,
    null_value_and_set_null_differ: bool,
    three_validation_strengths: bool,
    of_does_not_validate_segments: bool,
    a_blank_path_has_no_segments: bool,
    identifiers_are_unicode_aware: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) FIFO 追加，`Add` 与 `then` 等价。 / FIFO appends, and `Add` matches `then`.
    let the_order_is_fifo_and_add_matches_then = {
        let first = UpdateAssignments::set("a", ExpressionValue::Number(1.0));
        let second = UpdateAssignments::set_null("b");
        let appended = first.clone().then(second.clone());
        let added = first.clone() + second.clone();
        let order = appended.items.iter().map(|assignment| assignment.path().value().to_owned())
            .collect::<Vec<_>>();
        order == vec!["a".to_owned(), "b".to_owned()] && added == appended
    };
    // 2) 同一路径不被合并。 / The same path is not merged.
    let the_same_path_is_not_merged = UpdateAssignments::set("x", ExpressionValue::Number(1.0))
        .then_set("x", ExpressionValue::Number(2.0)).items.len() == 2;
    // 3) `set_null` 与 `set(_, null)` 是两个不同变体。 / `set_null` and `set(_, null)` are different variants.
    let null_value_and_set_null_differ = {
        let cleared = UpdateAssignments::set_null("x").items;
        let nulled = UpdateAssignments::set("x", ExpressionValue::Null).items;
        variant_name(&cleared[0]) == "set-null" && variant_name(&nulled[0]) == "set-value"
            && kind_of(&ExpressionValue::Null) == "null" && cleared != nulled
    };
    // 4) 三种校验强度：`new` 不校验、`parse` 只 trim、`try_parse` 才验标识符。 
    // 4) Three strengths: `new` validates nothing, `parse` only trims, `try_parse` checks identifiers.
    let three_validation_strengths = {
        let raw = "  na me  ";
        PropertyPath::new(raw).value() == raw
            && PropertyPath::parse(raw).value() == "na me"
            && PropertyPath::try_parse(raw).is_err()
            && PropertyPath::try_parse("  ok.name  ").is_ok()
    };
    // 5) `of` 不校验分段。 / `of` skips segment validation.
    let of_does_not_validate_segments = {
        let path = PropertyPath::of(["a", "", "b"]);
        path.value() == "a..b" && PropertyPath::try_parse(path.value()).is_err()
    };
    // 6) 空白路径没有分段。 / A blank path has no segments.
    let a_blank_path_has_no_segments = {
        let path = PropertyPath::parse("   ");
        path.is_empty() && path.segments().is_empty() && path == PropertyPath::EMPTY
    };
    // 7) 标识符判定是 Unicode 的。 / Identifier rules are Unicode-aware.
    let identifiers_are_unicode_aware = PropertyPath::is_valid_identifier("名字")
        && PropertyPath::is_valid_identifier("_x1")
        && !PropertyPath::is_valid_identifier("1name")
        && !PropertyPath::is_valid_identifier("na-me")
        && !PropertyPath::is_valid_identifier("na me")
        && !PropertyPath::is_valid_identifier("");
    Findings { the_order_is_fifo_and_add_matches_then, the_same_path_is_not_merged,
        null_value_and_set_null_differ, three_validation_strengths,
        of_does_not_validate_segments, a_blank_path_has_no_segments, identifiers_are_unicode_aware }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(path: &str, kind: &str, value: Option<AssignLiteral>) -> AssignStep {
        AssignStep { path: path.to_owned(), kind: Some(kind.to_owned()), value }
    }

    fn request() -> AssignmentCompositionRequest {
        AssignmentCompositionRequest {
            steps: vec![step("status", "value", Some(AssignLiteral::Text("inactive".to_owned()))),
                step("deletedAt", "null", None),
                step("score", "expression", Some(AssignLiteral::Number(1.0)))],
            paths: vec!["  na me  ".to_owned(), "  ok.name  ".to_owned(), "   ".to_owned()],
            segments: vec!["a".to_owned(), String::new(), "b".to_owned()],
        }
    }

    #[test]
    fn the_set_keeps_order_and_variants() {
        let report = verify_assignment_composition(&request()).unwrap();
        assert_eq!(report.items.len(), 3);
        assert_eq!(report.items[0].path, "status");
        assert_eq!(report.items[0].variant, "set-value");
        assert_eq!(report.items[0].value_kind, "string");
        assert_eq!(report.items[1].variant, "set-null");
        assert_eq!(report.items[1].value_kind, "none");
        assert_eq!(report.items[2].variant, "set-from-expression");
        assert_eq!(report.items[2].value_kind, "expression");
        assert!(!report.empty);
        assert!(report.empty_after_then);
        assert!(report.the_order_is_fifo_and_add_matches_then);
        assert!(report.the_same_path_is_not_merged);
        assert!(report.null_value_and_set_null_differ);
        assert!(report.three_validation_strengths);
        assert!(report.of_does_not_validate_segments);
        assert!(report.a_blank_path_has_no_segments);
        assert!(report.identifiers_are_unicode_aware);
    }

    #[test]
    fn the_same_path_is_kept_twice() {
        let document = AssignmentCompositionRequest {
            steps: vec![step("x", "value", Some(AssignLiteral::Number(1.0))),
                step("x", "value", Some(AssignLiteral::Number(2.0)))],
            paths: Vec::new(), segments: Vec::new() };
        let report = verify_assignment_composition(&document).unwrap();
        assert_eq!(report.items.len(), 2);
        assert_eq!(report.items[0].path, "x");
        assert_eq!(report.items[1].path, "x");
    }

    #[test]
    fn null_value_and_set_null_are_different_variants() {
        let document = AssignmentCompositionRequest {
            steps: vec![step("x", "null", None),
                step("x", "value", Some(AssignLiteral::Null))],
            paths: Vec::new(), segments: Vec::new() };
        let report = verify_assignment_composition(&document).unwrap();
        assert_eq!(report.items[0].variant, "set-null");
        assert_eq!(report.items[0].value_kind, "none");
        assert_eq!(report.items[1].variant, "set-value");
        assert_eq!(report.items[1].value_kind, "null");
    }

    #[test]
    fn the_three_path_strengths() {
        let report = verify_assignment_composition(&request()).unwrap();
        // `new` 原样留、`parse` 只 trim、`tryParse` 拒绝。 / `new` keeps raw, `parse` trims, `tryParse` refuses.
        assert_eq!(report.paths[0].from_new, "  na me  ");
        assert_eq!(report.paths[0].from_parse, "na me");
        assert!(!report.paths[0].parsed);
        assert_eq!(report.paths[1].from_parse, "ok.name");
        assert!(report.paths[1].parsed);
        assert_eq!(report.paths[1].segments, vec!["ok".to_owned(), "name".to_owned()]);
        // 空白路径没有分段。 / A blank path has no segments.
        assert_eq!(report.paths[2].from_parse, "");
        assert!(report.paths[2].segments.is_empty());
        assert!(!report.paths[2].parsed);
    }

    #[test]
    fn of_joins_without_validating() {
        let report = verify_assignment_composition(&request()).unwrap();
        assert_eq!(report.of_path, "a..b");
        assert_eq!(report.of_segments, vec!["a".to_owned(), String::new(), "b".to_owned()]);
    }

    #[test]
    fn bad_input_is_refused() {
        let unknown = AssignmentCompositionRequest {
            steps: vec![step("x", "merge", None)], paths: Vec::new(), segments: Vec::new() };
        assert!(verify_assignment_composition(&unknown).unwrap_err().message
            .contains("unknown step kind"));
        let too_many = AssignmentCompositionRequest {
            steps: (0..65).map(|index| step(&format!("p{index}"), "null", None)).collect(),
            paths: Vec::new(), segments: Vec::new() };
        assert!(verify_assignment_composition(&too_many).unwrap_err().message
            .contains("at most 64 entries"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_assignment_composition(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"valueKind\""), "{json}");
        assert!(json.contains("\"ofPath\""), "{json}");
        assert!(json.contains("\"theOrderIsFifoAndAddMatchesThen\""), "{json}");
        assert!(!json.contains("value_kind"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<AssignmentCompositionReport>(&copy.to_string()).is_err());
        // 字面量枚举的文本形。 / The literal enum's wire form.
        let literal = serde_json::to_string(&AssignLiteral::Text("x".to_owned())).unwrap();
        assert_eq!(literal, "{\"kind\":\"text\",\"value\":\"x\"}");
        let null = serde_json::to_string(&AssignLiteral::Null).unwrap();
        assert_eq!(null, "{\"kind\":\"null\"}");
    }
}
