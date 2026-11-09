//! 续挖第 59 单：稳定 ID 取值器——先 `validate()`、缺解与缺 ID 共用一条消息、且**完全不看绑定**
//! （`solver/logic_based_benders_context.rs` 的 `LogicBasedBendersSolutionExtractor::stable_value`）
//! Continuation, delivery 59: the stable-ID value extractor — `validate()` first, one message for both a missing solution
//! and a missing ID, and it **never consults the binding** (`LogicBasedBendersSolutionExtractor::stable_value`).
//!
//! §11.14 点名 `solver/logic_based_benders.rs` 的剩余纯数据面。`stable_value` 是 LBB 里**不需要求解器**就能完整度量的一处
//! ——它只要一份报告和一个 ID。七条实测：
//!
//! 1. **先 `validate()` 再查表**（`validate_runs_before_the_lookup()`）：给一份**"不可行却带解"**的报告并请求一个
//!    **确实存在**的 ID，拿到的是校验错误（`cannot carry an incumbent`）而**不是**那个值——顺序由此证明。
//! 2. **"没有解"与"没有这个 ID"共用一条消息**（`a_missing_solution_and_a_missing_id_share_one_message()`）：
//!    两者都变成 `solution is missing stable variable <id>`，**调用方无法区分**。
//! 3. **消息带 ID 原文**（`the_message_carries_the_id()`）。
//! 4. **取出的值原样返回**（`the_value_is_returned_verbatim()`）：不做舍入、不做整性检查——整数绑定上取到 `2.5`
//!    也照样返回。
//! 5. **`0.0` 不是"缺失"**（`a_zero_value_is_still_returned()`）。
//! 6. **ID 精确匹配**（`ids_are_matched_exactly()`）：`" x"` 与 `"x"` 是两个不同的键，不 trim。
//! 7. **它完全不看绑定**（`it_ignores_the_binding()`）：`binding` 里没有的 ID，只要报告里有值就返回成功——
//!    所以"取值成功"**不证明**这个变量属于 master。
//!
//! The §11.14 audit names the remaining pure-data surface of `solver/logic_based_benders.rs`. `stable_value` is the part that
//! can be measured **without a solver** — it needs only a report and an ID. **`validate()` runs before the lookup**
//! (`validate_runs_before_the_lookup()`). **"No solution" and "no such ID" share one message**
//! (`a_missing_solution_and_a_missing_id_share_one_message()`): both become `solution is missing stable variable <id>`, so the
//! caller **cannot tell them apart**. **The message carries the ID verbatim** (`the_message_carries_the_id()`). **The value is
//! returned verbatim** (`the_value_is_returned_verbatim()`): no rounding and no integrality check, so a `2.5` on an integer
//! binding comes straight back. **`0.0` is not "missing"** (`a_zero_value_is_still_returned()`). **IDs match exactly**
//! (`ids_are_matched_exactly()`): `" x"` and `"x"` are different keys. And **the binding is never consulted**
//! (`it_ignores_the_binding()`): an ID outside the binding still returns a value, so a successful lookup **does not prove** the
//! variable belongs to the master.

use std::collections::BTreeMap;

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_core::solver::{
    ProblemStatus, SolveReport, SolveSolution, TerminationReason,
};
use ospf_rust_framework::solver::logic_based_benders::{
    MasterBinding, MasterVariableBinding, MasterVariableDomain,
};
use ospf_rust_framework::solver::logic_based_benders_context::LogicBasedBendersSolutionExtractor;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 取值请求。 / The value request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StableValueRequest {
    /// 报告里的稳定值（ID → 值）。 / The report's stable values (ID → value).
    #[serde(default)]
    pub stable_values: Vec<StableValueEntry>,
    /// 报告是否携带解。 / Whether the report carries a solution.
    #[serde(default = "default_true")]
    pub has_solution: bool,
    /// 请求的 ID（缺省取第一条稳定值）。 / The requested ID (defaults to the first stable value).
    #[serde(default)]
    pub requested: Option<String>,
    /// 绑定里的 ID。 / The IDs in the binding.
    #[serde(default)]
    pub binding: Vec<String>,
}

/// 缺省为真。 / Defaults to true.
fn default_true() -> bool {
    true
}

/// 一条稳定值。 / One stable value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StableValueEntry {
    pub id: String,
    pub value: f64,
}

/// 取值结论。 / The value verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StableValueReport {
    /// 取到的值。 / The value, when found.
    #[serde(default)]
    pub value: Option<f64>,
    /// 失败消息。 / The failure message.
    #[serde(default)]
    pub error: Option<String>,
    /// 请求的 ID。 / The requested ID.
    #[serde(default)]
    pub requested: Option<String>,
    /// 先 `validate()` 再查表（**是**）。 / Whether `validate()` runs before the lookup (**yes**).
    pub validate_runs_before_the_lookup: bool,
    /// 缺解与缺 ID 共用一条消息（**是**）。 / Whether both failures share one message (**yes**).
    pub a_missing_solution_and_a_missing_id_share_one_message: bool,
    /// 消息带 ID 原文（**是**）。 / Whether the message carries the ID (**yes**).
    pub the_message_carries_the_id: bool,
    /// 值原样返回（**是**）。 / Whether the value is returned verbatim (**yes**).
    pub the_value_is_returned_verbatim: bool,
    /// `0.0` 不是缺失（**是**）。 / Whether `0.0` counts as present (**yes**).
    pub a_zero_value_is_still_returned: bool,
    /// ID 精确匹配（**是**）。 / Whether IDs match exactly (**yes**).
    pub ids_are_matched_exactly: bool,
    /// 它完全不看绑定（**是**）。 / Whether the binding is ignored (**yes**).
    pub it_ignores_the_binding: bool,
}

/// 造一份报告。 / Build one report.
fn report_with(solution: Option<BTreeMap<StableVariableId, f64>>,
    status: ProblemStatus) -> SolveReport<f64>
{
    let mut builder = SolveReport::builder(status, TerminationReason::Completed);
    if let Some(stable_values) = solution {
        let mut built = SolveSolution::<f64>::vector(Vec::new());
        built.stable_values = stable_values;
        built.objective_value = Some(0.0);
        builder = builder.solution(built);
    }
    builder.build().unwrap_or_else(|_| SolveReport::builder(ProblemStatus::Unknown,
        TerminationReason::Completed).build().unwrap_or_else(|error| panic!("{error}")))
}

/// 造一份绑定。 / Build one binding.
fn binding_of(ids: &[&str]) -> MasterBinding {
    MasterBinding::new(ids.iter().map(|id| MasterVariableBinding {
        id: StableVariableId((*id).to_owned()),
        domain: MasterVariableDomain::Integer { lower: 0, upper: 10 } }))
        .expect("probe binding is valid")
}

/// 取一次值，返回 `(值, 消息)`。 / Take one value, returning `(value, message)`.
fn take(report: &SolveReport<f64>, id: &str, bound: &[&str]) -> (Option<f64>, Option<String>) {
    let extractor = LogicBasedBendersSolutionExtractor::new(binding_of(bound));
    match extractor.stable_value(report, &StableVariableId(id.to_owned())) {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(error.to_string())),
    }
}

/// 造一张稳定值表。 / Build a stable-value map.
fn values(entries: &[(&str, f64)]) -> BTreeMap<StableVariableId, f64> {
    entries.iter().map(|(id, value)| (StableVariableId((*id).to_owned()), *value)).collect()
}

/// 走一次取值。非法输入是**错误**。 / Take one value through the bridge.
pub fn verify_stable_value(request: &StableValueRequest) -> Result<StableValueReport> {
    if request.stable_values.len() > 64 || request.binding.len() > 64 {
        return Err(BridgeError::new("InvalidInput",
            "stable value failed validation: at most 64 stable values and bindings are accepted"));
    }
    let requested = request.requested.clone()
        .or_else(|| request.stable_values.first().map(|entry| entry.id.clone()));
    let map = request.stable_values.iter()
        .map(|entry| (StableVariableId(entry.id.clone()), entry.value))
        .collect::<BTreeMap<_, _>>();
    let report = report_with(request.has_solution.then_some(map), ProblemStatus::Feasible);
    let bound = request.binding.iter().map(|id| id.as_str()).collect::<Vec<_>>();
    let bound = if bound.is_empty() { vec!["x"] } else { bound };
    let (value, error) = match requested.as_deref() {
        Some(id) => take(&report, id, &bound),
        None => (None, Some("no requested ID".to_owned())),
    };
    let findings = canonical_findings();
    Ok(StableValueReport { value, error, requested,
        validate_runs_before_the_lookup: findings.validate_runs_before_the_lookup,
        a_missing_solution_and_a_missing_id_share_one_message:
            findings.a_missing_solution_and_a_missing_id_share_one_message,
        the_message_carries_the_id: findings.the_message_carries_the_id,
        the_value_is_returned_verbatim: findings.the_value_is_returned_verbatim,
        a_zero_value_is_still_returned: findings.a_zero_value_is_still_returned,
        ids_are_matched_exactly: findings.ids_are_matched_exactly,
        it_ignores_the_binding: findings.it_ignores_the_binding })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    validate_runs_before_the_lookup: bool,
    a_missing_solution_and_a_missing_id_share_one_message: bool,
    the_message_carries_the_id: bool,
    the_value_is_returned_verbatim: bool,
    a_zero_value_is_still_returned: bool,
    ids_are_matched_exactly: bool,
    it_ignores_the_binding: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 先 `validate()`：不可行却带解的报告会被校验挡下（值存在也取不到）。
    // 1) `validate()` first: an infeasible report carrying an incumbent is refused even though the value exists.
    let validate_runs_before_the_lookup = {
        // 先造一份自洽报告，再把结论改成不可行——`validate()` 会拒绝"不可行却带解"。 
        // Build a consistent report and then flip the status: `validate()` refuses an infeasible report with an incumbent.
        let mut broken = report_with(Some(values(&[("x", 1.0)])), ProblemStatus::Feasible);
        broken.problem_status = ProblemStatus::Infeasible;
        let (value, error) = take(&broken, "x", &["x"]);
        value.is_none() && error.is_some_and(|message| message.contains("incumbent")
            && !message.contains("missing stable variable"))
    };
    // 2) 缺解与缺 ID 共用一条消息。 / A missing solution and a missing ID share one message.
    let a_missing_solution_and_a_missing_id_share_one_message = {
        // `Infeasible` 通过校验但没有解。 / `Infeasible` passes validation yet carries no solution.
        let without_solution = report_with(None, ProblemStatus::Infeasible);
        let with_solution = report_with(Some(values(&[("x", 1.0)])), ProblemStatus::Feasible);
        // 两次请求同一个 ID，消息才能逐字比较。 / Request the same ID twice so the messages compare verbatim.
        let (no_solution_value, no_solution_error) = take(&without_solution, "ghost", &["x"]);
        let (missing_id_value, missing_id_error) = take(&with_solution, "ghost", &["x"]);
        no_solution_value.is_none() && missing_id_value.is_none()
            && no_solution_error.is_some() && no_solution_error == missing_id_error
    };
    // 3) 消息带 ID 原文。 / The message carries the ID verbatim.
    let the_message_carries_the_id = {
        let report = report_with(Some(values(&[("x", 1.0)])), ProblemStatus::Feasible);
        let (_, error) = take(&report, "ghost-name", &["x"]);
        error.is_some_and(|message| message.contains("missing stable variable ghost-name"))
    };
    // 4) 值原样返回：整数绑定上取到 2.5 也照给。 
    // 4) The value comes back verbatim: a 2.5 on an integer binding is returned.
    let the_value_is_returned_verbatim = {
        let report = report_with(Some(values(&[("x", 2.5)])), ProblemStatus::Feasible);
        take(&report, "x", &["x"]).0 == Some(2.5)
    };
    // 5) `0.0` 不是缺失。 / `0.0` is not missing.
    let a_zero_value_is_still_returned = {
        let report = report_with(Some(values(&[("x", 0.0)])), ProblemStatus::Feasible);
        take(&report, "x", &["x"]) == (Some(0.0), None)
    };
    // 6) ID 精确匹配，不 trim。 / IDs match exactly, no trimming.
    let ids_are_matched_exactly = {
        let report = report_with(Some(values(&[("x", 1.0)])), ProblemStatus::Feasible);
        let (trimmed_value, trimmed_error) = take(&report, " x", &["x"]);
        let (exact_value, _) = take(&report, "x", &["x"]);
        trimmed_value.is_none() && trimmed_error.is_some() && exact_value == Some(1.0)
            && take(&report, "X", &["x"]).0.is_none()
    };
    // 7) 完全不看绑定：绑定外的 ID 也能取到值。 
    // 7) The binding is ignored: an ID outside it still returns a value.
    let it_ignores_the_binding = {
        let report = report_with(Some(values(&[("ghost", 7.0)])), ProblemStatus::Feasible);
        take(&report, "ghost", &["x"]) == (Some(7.0), None)
    };
    Findings { validate_runs_before_the_lookup,
        a_missing_solution_and_a_missing_id_share_one_message, the_message_carries_the_id,
        the_value_is_returned_verbatim, a_zero_value_is_still_returned, ids_are_matched_exactly,
        it_ignores_the_binding }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> StableValueRequest {
        StableValueRequest {
            stable_values: vec![StableValueEntry { id: "x".to_owned(), value: 3.0 }],
            has_solution: true, requested: Some("x".to_owned()),
            binding: vec!["x".to_owned()] }
    }

    #[test]
    fn a_present_id_returns_its_value() {
        let report = verify_stable_value(&request()).unwrap();
        assert_eq!(report.value, Some(3.0));
        assert_eq!(report.error, None);
        assert_eq!(report.requested.as_deref(), Some("x"));
        assert!(report.validate_runs_before_the_lookup);
        assert!(report.a_missing_solution_and_a_missing_id_share_one_message);
        assert!(report.the_message_carries_the_id);
        assert!(report.the_value_is_returned_verbatim);
        assert!(report.a_zero_value_is_still_returned);
        assert!(report.ids_are_matched_exactly);
        assert!(report.it_ignores_the_binding);
    }

    #[test]
    fn a_missing_id_reports_the_id() {
        let report = verify_stable_value(&StableValueRequest { requested: Some("ghost".to_owned()),
            ..request() }).unwrap();
        assert_eq!(report.value, None);
        assert!(report.error.as_deref().unwrap_or_default()
            .contains("missing stable variable ghost"));
    }

    #[test]
    fn a_report_without_a_solution_falls_into_the_missing_id_message() {
        // `Feasible` 无解**通过** `validate()`，于是落到"缺 ID"那条消息上。 
        // A solution-less `Feasible` report **passes** `validate()`, so it lands on the missing-ID message.
        let report = verify_stable_value(&StableValueRequest { has_solution: false,
            ..request() }).unwrap();
        assert_eq!(report.value, None);
        assert!(report.error.as_deref().unwrap_or_default()
            .contains("missing stable variable x"));
    }

    #[test]
    fn exact_ids_and_zero_values() {
        let spaced = verify_stable_value(&StableValueRequest {
            stable_values: vec![StableValueEntry { id: "x".to_owned(), value: 1.0 }],
            requested: Some(" x".to_owned()), ..request() }).unwrap();
        assert_eq!(spaced.value, None);
        assert!(spaced.error.is_some());
        let zero = verify_stable_value(&StableValueRequest {
            stable_values: vec![StableValueEntry { id: "x".to_owned(), value: 0.0 }],
            ..request() }).unwrap();
        assert_eq!(zero.value, Some(0.0));
        assert_eq!(zero.error, None);
    }

    #[test]
    fn the_binding_does_not_gate_the_lookup() {
        let report = verify_stable_value(&StableValueRequest {
            stable_values: vec![StableValueEntry { id: "ghost".to_owned(), value: 7.0 }],
            requested: Some("ghost".to_owned()),
            binding: vec!["x".to_owned()], ..request() }).unwrap();
        assert_eq!(report.value, Some(7.0));
        assert_eq!(report.error, None);
    }

    #[test]
    fn the_request_defaults_to_the_first_entry() {
        let report = verify_stable_value(&StableValueRequest { requested: None,
            ..request() }).unwrap();
        assert_eq!(report.requested.as_deref(), Some("x"));
        assert_eq!(report.value, Some(3.0));
        let empty = verify_stable_value(&StableValueRequest {
            stable_values: Vec::new(), requested: None, ..request() }).unwrap();
        assert_eq!(empty.requested, None);
        assert!(empty.error.as_deref().unwrap_or_default().contains("no requested ID"));
    }

    #[test]
    fn bad_input_is_refused() {
        let too_many = StableValueRequest {
            stable_values: (0..65).map(|index| StableValueEntry { id: format!("v{index}"),
                value: 0.0 }).collect(), ..request() };
        assert!(verify_stable_value(&too_many).unwrap_err().message
            .contains("at most 64 stable values"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_stable_value(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"validateRunsBeforeTheLookup\""), "{json}");
        assert!(json.contains("\"itIgnoresTheBinding\""), "{json}");
        assert!(!json.contains("validate_runs"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<StableValueReport>(&copy.to_string()).is_err());
    }
}
