//! 续挖第 9 单：更新赋值的三型与组合语义（`persistence/expression/update_assignment.rs`）
//! Continuation, delivery 9: the three update-assignment kinds and how they compose.
//!
//! `update_assignment.rs` 的四个未引用类型（`UpdateAssignment` / `SetValue` / `SetNull` / `SetFromExpression`）
//! 是同一件事的三个变体加一个容器。它们都没有 serde，所以线格式由桥接定义，但**语义一条不重写**：
//! 全部走领域构造器（`UpdateAssignments::empty()` 起手，`then` / `then_set` / `then_set_null` / `then_set_expr` 叠加）。
//!
//! 五条可断言的事实：
//!
//! 1. **三型各有各的载荷**：`setValue` 带值、`setNull` 不带值、`setExpression` 带表达式；`path()` 对三型都返回目标路径。
//! 2. **`then` 按顺序拼接，不排序、不去重**。组合顺序就是**执行顺序**，这对"先读后写"的赋值是语义而不是风格。
//! 3. **同一路径重复赋值，领域不拒**：`items` 里会有两条。谁生效由后端决定，所以报告把重复路径**显式列出**
//!    （`duplicatePaths`）而不是替它拒。
//! 4. **`set_null` 与 `set_value(null)` 不是同一个东西**：前者是 `SetNull` 变体、后者是 `SetValue(ExpressionValue::Null)`，
//!    两者在后端可能映射成不同的 SQL。这一层不合并它们。
//! 5. **空集合合法**：`is_empty()` 为真，`then` 一个空集合不改变任何东西。
//!
//! The four unreferenced types here (`UpdateAssignment` / `SetValue` / `SetNull` / `SetFromExpression`) are three variants of
//! one thing plus a container. None has serde, so the wire format is bridge-defined, but **no semantics are rewritten**:
//! everything goes through the domain constructors (`UpdateAssignments::empty()` followed by `then` / `then_set` /
//! `then_set_null` / `then_set_expr`). Five assertable facts: each kind has its own payload, and `path()` names the target for
//! all three; **`then` concatenates in order without sorting or deduplicating**, so composition order is execution order — a
//! semantic property, not a style choice, for read-then-write assignments; **assigning one path twice is not refused**, the
//! container simply holds two items and the backend decides, so the report lists those paths in `duplicatePaths` instead of
//! refusing; **`set_null` and `set_value(null)` are not the same thing** — `SetNull` versus `SetValue(ExpressionValue::Null)` may
//! map to different backend statements and this layer never merges them; and an empty set is legal, with `then` of an empty set
//! changing nothing.

use ospf_rust_framework::persistence::expression::{UpdateAssignments, UpdateAssignment};
use ospf_rust_math::symbol::{ExpressionValue, ScalarExpression};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个字面值。 / One literal value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAssignmentValue {
    /// `null` / `boolean` / `number` / `string`。 / One of `null`, `boolean`, `number`, `string`.
    pub kind: String,
    /// 字面文本（`null` 时忽略）。 / The literal text (ignored for `null`).
    #[serde(default)]
    pub text: String,
}

/// 表达式项。 / One expression term.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentExpressionSpec {
    /// `reference`（读另一字段）/ `constant`（字面值）/ `add`（两项相加）。
    /// `reference` / `constant` / `add`.
    pub kind: String,
    /// `reference` 的路径。 / The `reference` path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    /// `constant` 的值。 / The `constant` value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<UpdateAssignmentValue>,
    /// `add` 的两个操作数。 / The two operands of `add`.
    #[serde(default)]
    pub operands: Vec<AssignmentExpressionSpec>,
}

/// 一条赋值。 / One assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAssignmentSpec {
    /// 目标字段路径（不能空白）。 / The target field path (must not be blank).
    pub path: String,
    /// `setValue` / `setNull` / `setExpression`。 / The assignment kind.
    pub kind: String,
    /// `setValue` 的值。 / The value for `setValue`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<UpdateAssignmentValue>,
    /// `setExpression` 的表达式。 / The expression for `setExpression`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expression: Option<AssignmentExpressionSpec>,
}

/// 一组赋值。 / One group of assignments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentGroup {
    /// 组名（只用于报告）。 / The group name (reporting only).
    pub name: String,
    /// 组内赋值（按顺序）。 / The assignments, in order.
    #[serde(default)]
    pub assignments: Vec<UpdateAssignmentSpec>,
}

/// 赋值组合请求。 / The assignment-composition request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAssignmentRequest {
    /// 要按顺序 `then` 起来的若干组。 / The groups combined through `then`, in order.
    #[serde(default)]
    pub groups: Vec<AssignmentGroup>,
}

/// 一条赋值的结论。 / The verdict for one assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentRow {
    /// 在合并结果里的位置。 / The position in the combined result.
    pub position: u32,
    /// 所属组名。 / The owning group.
    pub group: String,
    /// 目标字段路径（取自领域的 `path()`）。 / The target path, taken from the domain's `path()`.
    pub path: String,
    /// 赋值类型（线格式代码）。 / The assignment kind (wire code).
    pub kind: String,
    /// 值或表达式的规范化文本（走领域 `Display`）。 / The normalized value or expression text (via the domain's `Display`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub payload: Option<String>,
}

/// 赋值组合结论。 / The assignment-composition verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAssignmentReport {
    /// 组数。 / The number of groups.
    pub group_count: u32,
    /// 合并后的条数。 / The number of items in the combined set.
    pub item_count: u32,
    /// 合并是否为空。 / Whether the combined set is empty.
    pub is_empty: bool,
    /// 逐条结论，按**组合顺序**。 / Each item, in **composition order**.
    pub items: Vec<AssignmentRow>,
    /// 合并后的路径序列（顺序即执行顺序）。 / The combined path sequence (order is execution order).
    pub paths: Vec<String>,
    /// 出现多于一次的路径（领域不拒，如实列出）。 / Paths appearing more than once (not refused, listed).
    pub duplicate_paths: Vec<String>,
    /// 每一组的条数。 / The item count of each group.
    pub group_sizes: Vec<u32>,
    /// 赋值类型词表（桥接定义）。 / The assignment kind vocabulary (bridge-defined).
    pub kind_vocabulary: Vec<String>,
}

/// 赋值条数上限（信任边界）。 / The assignment cap (a trust boundary).
const MAX_ASSIGNMENTS: u32 = 4_096;

/// 用领域构造器叠加赋值并报告结果。非法输入是**错误**。
/// Layer the assignments with the domain constructors and report the result. Illegal input is an **error**.
pub fn verify_update_assignments(request: &UpdateAssignmentRequest)
    -> Result<UpdateAssignmentReport>
{
    let total = request.groups.iter()
        .try_fold(0u32, |sum, group| sum.checked_add(u32::try_from(group.assignments.len())
            .unwrap_or(u32::MAX)))
        .unwrap_or(u32::MAX);
    if total > MAX_ASSIGNMENTS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "update assignment failed validation: at most {MAX_ASSIGNMENTS} assignments are accepted")));
    }
    // 从空集合起手，逐组构造再 `then` 起来——构造器与 `then` 都是领域的。
    // Start from an empty set and build each group before `then`-ing them; both are the domain's.
    let mut combined = UpdateAssignments::empty();
    let mut group_sizes = Vec::with_capacity(request.groups.len());
    let mut items = Vec::with_capacity(total as usize);
    let mut paths = Vec::with_capacity(total as usize);
    for group in &request.groups {
        let mut set = UpdateAssignments::empty();
        for assignment in &group.assignments {
            set = apply(set, assignment)?;
        }
        group_sizes.push(u32::try_from(set.items.len()).unwrap_or(u32::MAX));
        // 逐条投影：路径取自 **领域的 `path()`**，不是从请求抄回来。 / Paths come from the domain's `path()`.
        for item in &set.items {
            let path = item.path().value().to_owned();
            paths.push(path.clone());
            items.push(AssignmentRow {
                position: u32::try_from(items.len()).unwrap_or(u32::MAX),
                group: group.name.clone(),
                path,
                kind: kind_code(item).to_owned(),
                payload: payload_text(item),
            });
        }
        // `then` 的顺序就是执行顺序。 / `then` order is execution order.
        combined = combined.then(set);
    }
    let mut counts = std::collections::BTreeMap::<&str, u32>::new();
    for path in &paths { *counts.entry(path.as_str()).or_default() += 1; }
    let duplicate_paths = counts.into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(path, _)| path.to_owned())
        .collect::<Vec<_>>();
    Ok(UpdateAssignmentReport {
        group_count: u32::try_from(request.groups.len()).unwrap_or(u32::MAX),
        item_count: u32::try_from(combined.items.len()).unwrap_or(u32::MAX),
        is_empty: combined.items.is_empty(),
        items,
        paths,
        duplicate_paths,
        group_sizes,
        kind_vocabulary: vec!["setValue".to_owned(), "setNull".to_owned(),
            "setExpression".to_owned()],
    })
}

/// 用领域的 `then_set*` 追加一条赋值。 / Append one assignment through the domain's `then_set*`.
fn apply(set: UpdateAssignments, spec: &UpdateAssignmentSpec) -> Result<UpdateAssignments> {
    let path = spec.path.trim();
    if path.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "update assignment failed validation: target paths must not be blank"));
    }
    Ok(match spec.kind.as_str() {
        "setValue" => {
            let value = spec.value.as_ref().map(literal).transpose()?.ok_or_else(|| {
                BridgeError::new("InvalidInput",
                    "update assignment failed validation: `setValue` needs a value") })?;
            set.then_set(path, value)
        }
        "setNull" => set.then_set_null(path),
        "setExpression" => {
            let expression = spec.expression.as_ref().ok_or_else(|| BridgeError::new(
                "InvalidInput", "update assignment failed validation: `setExpression` needs an expression"))?;
            set.then_set_expr(path, compile(expression)?)
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "update assignment failed validation: unknown assignment kind `{other}`"))),
    })
}

/// 编一个表达式。 / Compile one expression.
fn compile(spec: &AssignmentExpressionSpec) -> Result<ScalarExpression<ExpressionValue>> {
    match spec.kind.as_str() {
        "reference" => {
            let path = spec.path.as_deref().map(str::trim).filter(|path| !path.is_empty())
                .ok_or_else(|| BridgeError::new("InvalidInput",
                    "update assignment failed validation: a reference needs a path"))?;
            Ok(ScalarExpression::reference(path))
        }
        "constant" => {
            let value = spec.value.as_ref().map(literal).transpose()?.ok_or_else(|| {
                BridgeError::new("InvalidInput",
                    "update assignment failed validation: a constant needs a value") })?;
            Ok(ScalarExpression::constant(value))
        }
        "add" => {
            if spec.operands.len() != 2 {
                return Err(BridgeError::new("InvalidInput",
                    "update assignment failed validation: `add` needs exactly two operands"));
            }
            let left = compile(&spec.operands[0])?;
            let right = compile(&spec.operands[1])?;
            Ok(ScalarExpression::add_expr(left, right))
        }
        other => Err(BridgeError::new("InvalidInput", &format!(
            "update assignment failed validation: unknown expression kind `{other}`"))),
    }
}

/// 解析一个字面值。 / Parse one literal value.
fn literal(value: &UpdateAssignmentValue) -> Result<ExpressionValue> {
    match value.kind.as_str() {
        "null" => Ok(ExpressionValue::Null),
        "boolean" => value.text.trim().parse::<bool>().map(ExpressionValue::Boolean)
            .map_err(|_| BridgeError::new("InvalidInput",
                "update assignment failed validation: a boolean must be `true` or `false`")),
        "number" => value.text.trim().parse::<f64>().map(ExpressionValue::Number)
            .map_err(|_| BridgeError::new("InvalidInput",
                "update assignment failed validation: a number must parse as a float")),
        "string" => Ok(ExpressionValue::String(value.text.clone())),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "update assignment failed validation: unknown value kind `{other}`"))),
    }
}

/// 赋值类型的线格式代码。 / The wire code of an assignment kind.
fn kind_code(item: &UpdateAssignment<ExpressionValue>) -> &'static str {
    match item {
        UpdateAssignment::SetValue(_) => "setValue",
        UpdateAssignment::SetNull(_) => "setNull",
        UpdateAssignment::SetFromExpression(_) => "setExpression",
    }
}

/// 值或表达式的规范化文本。 / The normalized text of the value or expression.
fn payload_text(item: &UpdateAssignment<ExpressionValue>) -> Option<String> {
    match item {
        UpdateAssignment::SetValue(value) => Some(value.value.to_string()),
        UpdateAssignment::SetNull(_) => None,
        UpdateAssignment::SetFromExpression(value) => Some(format!("{:?}", value.expression)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(kind: &str, text: &str) -> UpdateAssignmentValue {
        UpdateAssignmentValue { kind: kind.to_owned(), text: text.to_owned() }
    }

    fn assignment(path: &str, kind: &str) -> UpdateAssignmentSpec {
        UpdateAssignmentSpec { path: path.to_owned(), kind: kind.to_owned(), value: None, expression: None }
    }

    fn set_value(path: &str, kind: &str, text: &str) -> UpdateAssignmentSpec {
        UpdateAssignmentSpec { path: path.to_owned(), kind: "setValue".to_owned(),
            value: Some(value(kind, text)), expression: None }
    }

    fn reference(path: &str) -> AssignmentExpressionSpec {
        AssignmentExpressionSpec { kind: "reference".to_owned(), path: Some(path.to_owned()),
            value: None, operands: Vec::new() }
    }

    fn constant(kind: &str, text: &str) -> AssignmentExpressionSpec {
        AssignmentExpressionSpec { kind: "constant".to_owned(), path: None,
            value: Some(value(kind, text)), operands: Vec::new() }
    }

    fn group(name: &str, assignments: Vec<UpdateAssignmentSpec>) -> AssignmentGroup {
        AssignmentGroup { name: name.to_owned(), assignments }
    }

    fn request(groups: Vec<AssignmentGroup>) -> UpdateAssignmentRequest {
        UpdateAssignmentRequest { groups }
    }

    #[test]
    fn kinds_and_payloads() {
        let report = verify_update_assignments(&request(vec![group("g", vec![
            set_value("status", "string", "inactive"),
            assignment("deleted_at", "setNull"),
            UpdateAssignmentSpec { path: "score".to_owned(), kind: "setExpression".to_owned(), value: None,
                expression: Some(AssignmentExpressionSpec { kind: "add".to_owned(), path: None,
                    value: None, operands: vec![reference("score"), constant("number", "1")] }) },
        ])])).unwrap();
        assert_eq!(report.item_count, 3);
        assert_eq!(report.paths, vec!["status".to_owned(), "deleted_at".to_owned(),
            "score".to_owned()]);
        // 三型各自的名字与载荷。 / Each kind carries its own name and payload.
        assert_eq!(report.items[0].kind, "setValue");
        assert_eq!(report.items[0].payload.as_deref(), Some("inactive"));
        assert_eq!(report.items[1].kind, "setNull");
        assert_eq!(report.items[1].payload, None);
        assert_eq!(report.items[2].kind, "setExpression");
        assert!(report.items[2].payload.as_deref().is_some_and(|text| text.contains("score")));
        assert_eq!(report.kind_vocabulary.len(), 3);
    }

    #[test]
    fn then_preserves_order_and_duplicate_paths_are_listed_not_refused() {
        let report = verify_update_assignments(&request(vec![
            group("first", vec![set_value("a", "number", "1"), set_value("b", "number", "2")]),
            group("second", vec![set_value("a", "number", "3")]),
        ])).unwrap();
        // 顺序即执行顺序，跨组也是。 / Order is execution order, across groups too.
        assert_eq!(report.paths, vec!["a".to_owned(), "b".to_owned(), "a".to_owned()]);
        assert_eq!(report.items[2].group, "second");
        assert_eq!(report.items[0].group, "first");
        assert_eq!(report.group_sizes, vec![2, 1]);
        // 同一路径两次：**不拒**，只列出来。 / The same path twice is **not** refused, only listed.
        assert_eq!(report.duplicate_paths, vec!["a".to_owned()]);
        assert_eq!(report.item_count, 3);
        assert!(!report.is_empty);
    }

    #[test]
    fn set_null_and_set_value_of_null_stay_distinct() {
        let report = verify_update_assignments(&request(vec![group("g", vec![
            assignment("deleted_at", "setNull"),
            set_value("archived_at", "null", ""),
        ])])).unwrap();
        // `setNull` 是 `SetNull` 变体。 / `setNull` is the `SetNull` variant.
        assert_eq!(report.items[0].kind, "setNull");
        assert_eq!(report.items[0].payload, None);
        // `setValue(null)` 是 `SetValue(ExpressionValue::Null)`，**不是**同一个东西。 
        // `setValue(null)` is `SetValue(ExpressionValue::Null)`, a **different** thing.
        assert_eq!(report.items[1].kind, "setValue");
        assert_eq!(report.items[1].payload.as_deref(), Some("null"));
    }

    #[test]
    fn empty_sets_are_legal_and_then_changes_nothing() {
        let report = verify_update_assignments(&request(vec![group("empty", Vec::new()),
            group("real", vec![set_value("a", "boolean", "true")]),
            group("also-empty", Vec::new())])).unwrap();
        assert_eq!(report.group_count, 3);
        assert_eq!(report.group_sizes, vec![0, 1, 0]);
        assert_eq!(report.item_count, 1);
        assert_eq!(report.paths, vec!["a".to_owned()]);
        assert!(!report.is_empty);
        // 全空时 `is_empty` 为真。 / With nothing at all `is_empty` holds.
        let report = verify_update_assignments(&request(vec![group("a", Vec::new())])).unwrap();
        assert!(report.is_empty);
        assert_eq!(report.item_count, 0);
        assert!(report.paths.is_empty());
        assert!(report.duplicate_paths.is_empty());
        let report = verify_update_assignments(&request(Vec::new())).unwrap();
        assert_eq!(report.group_count, 0);
        assert!(report.is_empty);
    }

    #[test]
    fn bad_input_is_refused() {
        // 空白路径。 / A blank path.
        assert!(verify_update_assignments(&request(vec![group("g",
            vec![set_value("  ", "number", "1")])])).unwrap_err().message
            .contains("paths must not be blank"));
        // `setValue` 缺值 / `setExpression` 缺表达式 / 未知类型。 / Missing payloads and an unknown kind.
        assert!(verify_update_assignments(&request(vec![group("g", vec![assignment("a", "setValue")])]))
            .unwrap_err().message.contains("needs a value"));
        assert!(verify_update_assignments(&request(vec![group("g", vec![assignment("a", "setExpression")])]))
            .unwrap_err().message.contains("needs an expression"));
        assert!(verify_update_assignments(&request(vec![group("g", vec![assignment("a", "addOne")])]))
            .unwrap_err().message.contains("unknown assignment kind"));
        // 坏字面值。 / A malformed literal.
        assert!(verify_update_assignments(&request(vec![group("g",
            vec![set_value("a", "number", "one")])])).unwrap_err().message
            .contains("parse as a float"));
        assert!(verify_update_assignments(&request(vec![group("g",
            vec![set_value("a", "boolean", "yes")])])).unwrap_err().message
            .contains("`true` or `false`"));
        assert!(verify_update_assignments(&request(vec![group("g",
            vec![set_value("a", "date", "today")])])).unwrap_err().message
            .contains("unknown value kind"));
        // 表达式错误：引用缺路径 / `add` 操作数不是 2 / 未知表达式类型。 
        // Expression errors: a reference without a path, an `add` without two operands, an unknown kind.
        let bad_reference = UpdateAssignmentSpec { path: "a".to_owned(), kind: "setExpression".to_owned(),
            value: None, expression: Some(AssignmentExpressionSpec { kind: "reference".to_owned(),
                path: Some("  ".to_owned()), value: None, operands: Vec::new() }) };
        assert!(verify_update_assignments(&request(vec![group("g", vec![bad_reference])]))
            .unwrap_err().message.contains("reference needs a path"));
        let short_add = UpdateAssignmentSpec { path: "a".to_owned(), kind: "setExpression".to_owned(),
            value: None, expression: Some(AssignmentExpressionSpec { kind: "add".to_owned(),
                path: None, value: None, operands: vec![reference("x")] }) };
        assert!(verify_update_assignments(&request(vec![group("g", vec![short_add])]))
            .unwrap_err().message.contains("exactly two operands"));
        let unknown_expression = UpdateAssignmentSpec { path: "a".to_owned(), kind: "setExpression".to_owned(),
            value: None, expression: Some(AssignmentExpressionSpec { kind: "multiply".to_owned(),
                path: None, value: None, operands: Vec::new() }) };
        assert!(verify_update_assignments(&request(vec![group("g", vec![unknown_expression])]))
            .unwrap_err().message.contains("unknown expression kind"));
    }
}
