//! 续挖第 48 单：`MasterBinding` 与 `MasterAssignment` 的成对契约——**恰好覆盖**、值域按 `0..=1` 与 `lower..=upper`
//! （`solver/logic_based_benders.rs`）
//! Continuation, delivery 48: the paired contract of `MasterBinding` and `MasterAssignment` — **exact coverage**, with domains
//! checked as `0..=1` and `lower..=upper` (`solver/logic_based_benders.rs`).
//!
//! 第 37 单只在探针里撞到过"赋值必须恰好覆盖"这条报错，没系统量过；第 47 单量的是 `normalize()` 那条路径上的域校验。
//! 这一单量**构造函数这一侧**的成对契约。六条实测：
//!
//! 1. **`MasterBinding::new` 拒绝空集合**（`empty_binding_is_refused()`）："requires at least one master binding"。
//!    所以"零个 master 变量"这个状态**构造不出来**。
//! 2. **重复 ID 在 `new` 里就被拒**（"duplicate master binding ID"），比 `normalize()` 那条报错更早、措辞也不同。
//! 3. **域校验在 `new` 里也会跑**（`new_validates_the_domains()`）：`lower > upper` 与超过 `2^53` 的界同样被拒，
//!    不必等到某条 cut 的 `normalize()`。
//! 4. **`MasterAssignment::new` 要求值集合与绑定集合\*\*恰好相等\*\***（`exact_coverage_is_required()`）：
//!    少一个变量、多一个变量都报 "does not match the stable binding set"——**"多给"与"少给"是同一个错误**。
//! 5. **越界与"多给"是两条不同的报错**（`out_of_domain_differs_from_extra()`）：覆盖对上了之后才查值域，
//!    越界报 "is outside its domain"；`Binary` 域按 `0..=1` 判定，所以 `2` 与 `-1` 都算越界。
//! 6. **`ids()` 给字典序**（`ids_are_lexicographic()`）：内部是 `BTreeMap`，所以与插入顺序无关。
//!
//! Delivery 37 only stumbled into the "must cover exactly" error inside a probe; delivery 47 measured the domain check on the
//! `normalize()` path. This one measures the **constructor side** of the paired contract. **`MasterBinding::new` refuses an
//! empty set** (`empty_binding_is_refused()`), so "zero master variables" **cannot be constructed**. **A duplicate ID is
//! refused in `new`** ("duplicate master binding ID"), earlier and worded differently than the `normalize()` error. **Domain
//! validation runs in `new` too** (`new_validates_the_domains()`): inverted bounds and bounds beyond `2^53` are refused there
//! without waiting for some cut's `normalize()`. **`MasterAssignment::new` requires the value set to **exactly equal** the
//! binding set** (`exact_coverage_is_required()`): a missing variable and an extra variable both report "does not match the
//! stable binding set" — **"too few" and "too many" are one error**. **An out-of-domain value is a different error**
//! (`out_of_domain_differs_from_extra()`): the domain check only runs once coverage matches, and a `Binary` domain judges by
//! `0..=1`, so both `2` and `-1` are out of domain. And **`ids()` is lexicographic** (`ids_are_lexicographic()`), being a
//! `BTreeMap` internally, independent of insertion order.

use std::collections::BTreeMap;

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::solver::logic_based_benders::{
    MasterAssignment, MasterBinding, MasterVariableBinding, MasterVariableDomain,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个绑定的字面量。 / One binding literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingLiteral {
    /// 稳定 ID。 / The stable ID.
    pub id: String,
    /// `binary` 或 `integer`。 / `binary` or `integer`.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub lower: Option<i64>,
    #[serde(default)]
    pub upper: Option<i64>,
    /// 若给出，就为该 ID 建一个赋值的值。 / When present, the value to assign that ID.
    #[serde(default)]
    pub value: Option<i64>,
}

/// 成对契约请求。 / The paired-contract request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingAssignmentRequest {
    /// 绑定声明（连同要赋的值）。 / The bindings (together with the values to assign).
    #[serde(default)]
    pub bindings: Vec<BindingLiteral>,
    /// 额外的赋值：某个 ID 与一个**不被任何绑定覆盖**的值（量"多给"）。
    /// An extra assignment: an ID and a value **covered by no binding** (to measure "too many").
    #[serde(default)]
    pub extra: Option<BindingLiteral>,
    /// 是否跳过某个绑定的赋值（量"少给"）。 / Whether to drop one binding's value (to measure "too few").
    #[serde(default)]
    pub drop_one: bool,
}

/// 成对契约结论。 / The paired-contract verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingAssignmentReport {
    /// 绑定集合是否建成。 / Whether the binding set was built.
    pub binding_accepted: bool,
    /// 绑定集合的失败消息。 / The binding failure message.
    #[serde(default)]
    pub binding_error: Option<String>,
    /// 绑定条数。 / The binding count.
    pub binding_count: u64,
    /// 绑定的 ID（字典序）。 / The binding IDs (lexicographic).
    pub binding_ids: Vec<String>,
    /// 每个 ID 的域字面量。 / Each ID's domain literal.
    pub domains: Vec<String>,
    /// 恰好覆盖的赋值是否建成。 / Whether the exactly-covering assignment was built.
    pub assignment_accepted: bool,
    /// 赋值的失败消息。 / The assignment failure message.
    #[serde(default)]
    pub assignment_error: Option<String>,
    /// "少给"的结果。 / The result of "too few".
    #[serde(default)]
    pub short_error: Option<String>,
    /// "多给"的结果。 / The result of "too many".
    #[serde(default)]
    pub extra_error: Option<String>,
    /// 空集合被拒（**是**）。 / Whether an empty set is refused (**yes**).
    pub empty_binding_is_refused: bool,
    /// 重复 ID 在 `new` 里被拒（**是**）。 / Whether a duplicate ID is refused in `new` (**yes**).
    pub duplicate_is_refused_in_new: bool,
    /// `new` 也校验域（**是**）。 / Whether `new` validates the domains (**yes**).
    pub new_validates_the_domains: bool,
    /// 要求恰好覆盖（**是**）。 / Whether exact coverage is required (**yes**).
    pub exact_coverage_is_required: bool,
    /// 越界与"多给"是两条报错（**是**）。 / Whether out-of-domain differs from extra (**yes**).
    pub out_of_domain_differs_from_extra: bool,
    /// `ids()` 是字典序（**是**）。 / Whether `ids()` is lexicographic (**yes**).
    pub ids_are_lexicographic: bool,
}

/// 声明条数上限（信任边界）。 / The declaration cap (a trust boundary).
const MAX_BINDINGS: usize = 64;

/// 造一个绑定。 / Build one binding.
fn binding(spec: &BindingLiteral) -> Result<MasterVariableBinding> {
    let id = StableVariableId(spec.id.clone());
    match spec.kind.as_deref().unwrap_or("binary") {
        "binary" => Ok(MasterVariableBinding { id, domain: MasterVariableDomain::Binary }),
        "integer" => Ok(MasterVariableBinding { id,
            domain: MasterVariableDomain::Integer {
                lower: spec.lower.ok_or_else(|| BridgeError::new("InvalidInput",
                    "binding failed validation: an integer domain needs a lower bound"))?,
                upper: spec.upper.ok_or_else(|| BridgeError::new("InvalidInput",
                    "binding failed validation: an integer domain needs an upper bound"))? } }),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "binding failed validation: unknown domain `{other}`"))),
    }
}

/// 域的字面量。 / The domain literal.
fn domain_name(domain: Option<MasterVariableDomain>) -> String {
    match domain {
        Some(MasterVariableDomain::Binary) => "binary".to_owned(),
        Some(MasterVariableDomain::Integer { lower, upper }) => format!("integer:{lower}..{upper}"),
        None => "absent".to_owned(),
    }
}

/// 走一遍成对契约。非法输入是**错误**。 / Walk the paired contract.
pub fn verify_binding_assignment(request: &BindingAssignmentRequest)
    -> Result<BindingAssignmentReport>
{
    if request.bindings.len() > MAX_BINDINGS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "binding failed validation: at most {MAX_BINDINGS} bindings are accepted")));
    }
    let declarations = request.bindings.iter().map(binding).collect::<Result<Vec<_>>>()?;
    let binding_outcome = MasterBinding::new(declarations.iter().cloned());
    let (binding_accepted, binding_error) = match &binding_outcome {
        Ok(_) => (true, None),
        Err(failure) => (false, Some(failure.to_string())),
    };
    let (binding_count, binding_ids, domains) = match &binding_outcome {
        Ok(binding) => (binding.len() as u64,
            binding.ids().map(|id| id.0.clone()).collect::<Vec<_>>(),
            binding.ids().map(|id| domain_name(binding.domain(id))).collect::<Vec<_>>()),
        Err(_) => (0, Vec::new(), Vec::new()),
    };
    let values = |skip_one: bool, extra: Option<&BindingLiteral>| {
        let mut values = BTreeMap::new();
        for (position, spec) in request.bindings.iter().enumerate() {
            if skip_one && position == 0 { continue; }
            if let Some(value) = spec.value {
                values.insert(StableVariableId(spec.id.clone()), value);
            }
        }
        if let Some(extra) = extra {
            if let Some(value) = extra.value {
                values.insert(StableVariableId(extra.id.clone()), value);
            }
        }
        values
    };
    let (assignment_accepted, assignment_error) = match &binding_outcome {
        Ok(binding) => match MasterAssignment::new(values(false, None), binding) {
            Ok(_) => (true, None),
            Err(failure) => (false, Some(failure.to_string())),
        },
        Err(_) => (false, None),
    };
    let short_error = match &binding_outcome {
        Ok(binding) => MasterAssignment::new(values(true, None), binding).err()
            .map(|failure| failure.to_string()),
        Err(_) => None,
    };
    let extra_error = match &binding_outcome {
        Ok(binding) => MasterAssignment::new(values(false, request.extra.as_ref()), binding)
            .err().map(|failure| failure.to_string()),
        Err(_) => None,
    };
    let findings = canonical_findings();
    Ok(BindingAssignmentReport { binding_accepted, binding_error, binding_count, binding_ids,
        domains, assignment_accepted, assignment_error, short_error, extra_error,
        empty_binding_is_refused: findings.empty_binding_is_refused,
        duplicate_is_refused_in_new: findings.duplicate_is_refused_in_new,
        new_validates_the_domains: findings.new_validates_the_domains,
        exact_coverage_is_required: findings.exact_coverage_is_required,
        out_of_domain_differs_from_extra: findings.out_of_domain_differs_from_extra,
        ids_are_lexicographic: findings.ids_are_lexicographic })
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    empty_binding_is_refused: bool,
    duplicate_is_refused_in_new: bool,
    new_validates_the_domains: bool,
    exact_coverage_is_required: bool,
    out_of_domain_differs_from_extra: bool,
    ids_are_lexicographic: bool,
}

/// 一个二进制绑定。 / One binary binding.
fn binary(id: &str) -> MasterVariableBinding {
    MasterVariableBinding { id: StableVariableId(id.to_owned()),
        domain: MasterVariableDomain::Binary }
}

/// 一个整数绑定。 / One integer binding.
fn integer(id: &str, lower: i64, upper: i64) -> MasterVariableBinding {
    MasterVariableBinding { id: StableVariableId(id.to_owned()),
        domain: MasterVariableDomain::Integer { lower, upper } }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 空集合被拒。 / An empty set is refused.
    let empty_binding_is_refused = match MasterBinding::new(Vec::<MasterVariableBinding>::new()) {
        Err(failure) => failure.to_string().contains("at least one master binding"),
        Ok(_) => false,
    };
    // 2) 重复 ID 在 `new` 里被拒，措辞与 `normalize()` 那条不同。 
    // 2) A duplicate ID is refused in `new`, worded differently from the `normalize()` error.
    let duplicate_is_refused_in_new = match MasterBinding::new(vec![binary("a"), binary("a")]) {
        Err(failure) => failure.to_string().contains("duplicate master binding ID"),
        Ok(_) => false,
    };
    // 3) `new` 也校验域。 / `new` validates the domains too.
    let new_validates_the_domains =
        MasterBinding::new(vec![integer("a", 5, 1)]).is_err()
            && MasterBinding::new(vec![integer("a", 0, (1_i64 << 53) + 1)]).is_err()
            && MasterBinding::new(vec![integer("a", 0, 1_i64 << 53)]).is_ok();
    // 4) 恰好覆盖：少给与多给是同一个错误。 / Exact coverage: too few and too many are one error.
    let exact_coverage_is_required = {
        let binding = MasterBinding::new(vec![binary("a"), binary("b")]).unwrap();
        let complete = MasterAssignment::new(BTreeMap::from([
            (StableVariableId("a".to_owned()), 1i64),
            (StableVariableId("b".to_owned()), 0i64)]), &binding);
        let short = MasterAssignment::new(BTreeMap::from([
            (StableVariableId("a".to_owned()), 1i64)]), &binding);
        let extra = MasterAssignment::new(BTreeMap::from([
            (StableVariableId("a".to_owned()), 1i64),
            (StableVariableId("b".to_owned()), 0i64),
            (StableVariableId("c".to_owned()), 0i64)]), &binding);
        complete.is_ok()
            && short.as_ref().err().is_some_and(|failure| failure.to_string()
                .contains("does not match the stable binding set"))
            && extra.as_ref().err().is_some_and(|failure| failure.to_string()
                .contains("does not match the stable binding set"))
    };
    // 5) 越界是另一条报错；`Binary` 按 `0..=1` 判定。 
    // 5) Out of domain is a different error; `Binary` judges by `0..=1`.
    let out_of_domain_differs_from_extra = {
        let binding = MasterBinding::new(vec![binary("a"), integer("b", 0, 3)]).unwrap();
        let out_of_range = MasterAssignment::new(BTreeMap::from([
            (StableVariableId("a".to_owned()), 1i64),
            (StableVariableId("b".to_owned()), 4i64)]), &binding);
        let twice = MasterAssignment::new(BTreeMap::from([
            (StableVariableId("a".to_owned()), 2i64),
            (StableVariableId("b".to_owned()), 0i64)]), &binding);
        let negative = MasterAssignment::new(BTreeMap::from([
            (StableVariableId("a".to_owned()), -1i64),
            (StableVariableId("b".to_owned()), 0i64)]), &binding);
        let ok = MasterAssignment::new(BTreeMap::from([
            (StableVariableId("a".to_owned()), 0i64),
            (StableVariableId("b".to_owned()), 3i64)]), &binding);
        ok.is_ok()
            && out_of_range.as_ref().err().is_some_and(|failure| failure.to_string()
                .contains("is outside its domain"))
            && twice.is_err() && negative.is_err()
    };
    // 6) `ids()` 是字典序：插入顺序反过来也给出同样的序列。 
    // 6) `ids()` is lexicographic: reversing the insertion order gives the same sequence.
    let ids_are_lexicographic = {
        let forward = MasterBinding::new(vec![binary("a"), binary("b"), binary("c")]).unwrap();
        let reversed = MasterBinding::new(vec![binary("c"), binary("b"), binary("a")]).unwrap();
        forward.ids().map(|id| id.0.clone()).collect::<Vec<_>>()
            == reversed.ids().map(|id| id.0.clone()).collect::<Vec<_>>()
    };
    Findings { empty_binding_is_refused, duplicate_is_refused_in_new,
        new_validates_the_domains, exact_coverage_is_required,
        out_of_domain_differs_from_extra, ids_are_lexicographic }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> BindingAssignmentRequest {
        BindingAssignmentRequest { bindings: vec![
            BindingLiteral { id: "a".to_owned(), kind: Some("binary".to_owned()), lower: None,
                upper: None, value: Some(1) },
            BindingLiteral { id: "b".to_owned(), kind: Some("integer".to_owned()), lower: Some(0),
                upper: Some(3), value: Some(2) },
        ], extra: None, drop_one: false }
    }

    #[test]
    fn the_pair_builds_and_the_ids_are_lexicographic() {
        let report = verify_binding_assignment(&request()).unwrap();
        assert!(report.binding_accepted);
        assert_eq!(report.binding_count, 2);
        assert_eq!(report.binding_ids, vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(report.domains, vec!["binary".to_owned(), "integer:0..3".to_owned()]);
        assert!(report.assignment_accepted, "{:?}", report.assignment_error);
        assert!(report.ids_are_lexicographic);
        assert!(report.exact_coverage_is_required);
        assert!(report.empty_binding_is_refused);
        assert!(report.duplicate_is_refused_in_new);
        assert!(report.new_validates_the_domains);
        assert!(report.out_of_domain_differs_from_extra);
    }

    /// **按实测纠正**：`dropOne` / `extra` 只影响各自的**旁路**尝试（`shortError` / `extraError`），
    /// 不改那份"按声明值建"的主赋值——所以 `assignmentAccepted` 仍然为真。
    /// Measured correction: `dropOne` / `extra` only affect their own **side** attempts (`shortError` / `extraError`) and
    /// leave the main assignment built from the declared values untouched, so `assignmentAccepted` stays true.
    #[test]
    fn too_few_and_too_many_are_the_same_error() {
        let mut short = request();
        short.drop_one = true;
        let report = verify_binding_assignment(&short).unwrap();
        assert!(report.assignment_accepted);
        assert!(report.short_error.as_deref().unwrap_or_default()
            .contains("does not match the stable binding set"));
        let mut extra = request();
        extra.extra = Some(BindingLiteral { id: "c".to_owned(),
            kind: Some("binary".to_owned()), lower: None, upper: None, value: Some(0) });
        let report = verify_binding_assignment(&extra).unwrap();
        assert!(report.assignment_accepted);
        // 多给与少给是**同一个**错误消息。 / Too many and too few are the **same** error message.
        assert!(report.extra_error.as_deref().unwrap_or_default()
            .contains("does not match the stable binding set"));
        assert_eq!(report.short_error, report.extra_error);
    }

    #[test]
    fn an_out_of_domain_value_is_a_different_error() {
        let mut document = request();
        document.bindings[0].value = Some(2);
        let report = verify_binding_assignment(&document).unwrap();
        assert!(!report.assignment_accepted);
        assert!(report.assignment_error.as_deref().unwrap_or_default()
            .contains("is outside its domain"));
        // 负数同样越界。 / A negative value is out of domain too.
        document.bindings[0].value = Some(-1);
        let report = verify_binding_assignment(&document).unwrap();
        assert!(report.assignment_error.as_deref().unwrap_or_default()
            .contains("is outside its domain"));
    }

    #[test]
    fn the_binding_set_is_refused_early() {
        let empty = BindingAssignmentRequest { bindings: Vec::new(), extra: None,
            drop_one: false };
        let report = verify_binding_assignment(&empty).unwrap();
        assert!(!report.binding_accepted);
        assert!(report.binding_error.as_deref().unwrap_or_default()
            .contains("at least one master binding"));
        assert_eq!(report.binding_count, 0);
        let mut duplicated = request();
        duplicated.bindings[1].id = "a".to_owned();
        let report = verify_binding_assignment(&duplicated).unwrap();
        assert!(!report.binding_accepted);
        assert!(report.binding_error.as_deref().unwrap_or_default()
            .contains("duplicate master binding ID"));
        let mut inverted = request();
        inverted.bindings[1].lower = Some(5);
        inverted.bindings[1].upper = Some(1);
        let report = verify_binding_assignment(&inverted).unwrap();
        assert!(!report.binding_accepted);
        assert!(report.binding_error.as_deref().unwrap_or_default()
            .contains("lower 5 above upper 1"));
    }

    #[test]
    fn bad_input_is_refused() {
        let mut missing = request();
        missing.bindings[1].upper = None;
        assert!(verify_binding_assignment(&missing).unwrap_err().message
            .contains("needs an upper bound"));
        let mut unknown = request();
        unknown.bindings[0].kind = Some("real".to_owned());
        assert!(verify_binding_assignment(&unknown).unwrap_err().message
            .contains("unknown domain"));
        let too_many = BindingAssignmentRequest { bindings: (0..65).map(|index| BindingLiteral {
            id: format!("v{index}"), kind: Some("binary".to_owned()), lower: None, upper: None,
            value: Some(0) }).collect(), extra: None, drop_one: false };
        assert!(verify_binding_assignment(&too_many).unwrap_err().message
            .contains("at most 64 bindings"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_binding_assignment(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"bindingIds\""), "{json}");
        assert!(json.contains("\"exactCoverageIsRequired\""), "{json}");
        assert!(json.contains("\"idsAreLexicographic\""), "{json}");
        assert!(!json.contains("binding_ids"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BindingAssignmentReport>(&copy.to_string()).is_err());
    }
}
