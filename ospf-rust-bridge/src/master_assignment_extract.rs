//! 续挖第 60 单：master 赋值提取的**容差与范围**语义——只收最优报告、`round()` 半上入、绝对差容差、两道范围闸
//! （`solver/logic_based_benders.rs` 的 `MasterAssignment::from_report`）
//! Continuation, delivery 60: the master-assignment extraction's **tolerance and range** semantics — optimal reports only,
//! `round()` half-up, an absolute-difference tolerance, and two range gates (`MasterAssignment::from_report`).
//!
//! 第 59 单量过 `stable_value`（只查表），这一单量它的兄弟入口 `from_report`：**查表 + 整性容差 + 域检查**。
//! 桥接层的 `extract_master_assignment` 把容差**硬编码成 `1e-7`**，所以容差本身只有在框架层直接绑才能量到。
//! 七条实测：
//!
//! 1. **只接受最优报告**（`only_optimal_reports_are_accepted()`）：`Feasible`（有解）也被拒，
//!    消息是 "requires a verified optimal report"。
//! 2. **容差必须有限且非负**（`the_tolerance_must_be_finite_and_non_negative()`）。
//! 3. **舍入是 `round()` 的"半数远离零"**（`rounding_is_half_away_from_zero()`）：`2.5 → 3`、`3.5 → 4`、
//!    而 **`-2.5 → -3`**——既不是银行家舍入，也不是"半上入"（负数方向相反）。
//! 4. **容差是绝对差**（`the_tolerance_is_an_absolute_difference()`）：`|raw - round(raw)| > tol` 拒绝，
//!    **恰好等于容差通过**。
//! 5. **两道范围闸**（`there_are_two_range_gates()`）：`1e19` 撞 `i64` 范围闸、`1e17`（`> 2^53`）撞
//!    精确整数闸——两者消息不同。
//! 6. **域检查在舍入之后**（`the_domain_check_sees_the_rounded_value()`）：`Integer{0..10}` 里 `10.4`
//!    （容差 `0.5`）舍入成 `10` 通过，`10.6` 舍入成 `11` 被拒。
//! 7. **缺 ID 与 `feasible` 的 `stable_value` 不同文案**（`a_missing_id_says_optimal_master_report()`）：
//!    这里是 "optimal master report is missing stable variable <id>"。
//!
//! Delivery 59 measured `stable_value` (a plain lookup); this one measures its sibling `from_report`: **lookup plus an
//! integrality tolerance plus domain checks**. The bridge wrapper `extract_master_assignment` hard-codes the tolerance to
//! **`1e-7`**, so the tolerance itself is only measurable by binding the framework layer directly. **Only optimal reports are
//! accepted** (`only_optimal_reports_are_accepted()`): even a solvable `Feasible` report is refused with "requires a verified
//! optimal report". **The tolerance must be finite and non-negative** (`the_tolerance_must_be_finite_and_non_negative()`).
//! **Rounding is `round()`, half-up** (`rounding_is_half_away_from_zero()`): `2.5` becomes **`3`** at tolerance `0.5`, not the banker's
//! `2`. **The tolerance is an absolute difference** (`the_tolerance_is_an_absolute_difference()`): it refuses when the gap is
//! **greater** than the tolerance, so an exact match passes. **There are two range gates**
//! (`there_are_two_range_gates()`): `1e19` hits the `i64` gate while `1e17` (above `2^53`) hits the exact-integer gate, with
//! different messages. **The domain check sees the rounded value** (`the_domain_check_sees_the_rounded_value()`): inside
//! `Integer{0..10}`, `10.4` rounds to `10` and passes while `10.6` rounds to `11` and is refused. And **a missing ID has its
//! own wording** (`a_missing_id_says_optimal_master_report()`).

use std::collections::BTreeMap;

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_core::solver::{
    ProblemStatus, SolveProof, SolveReport, SolveSolution, TerminationReason,
};
use ospf_rust_framework::solver::logic_based_benders::{
    MasterAssignment, MasterBinding, MasterVariableBinding, MasterVariableDomain,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一道 master 变量。 / One master variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterAssignedVariableSpec {
    pub id: String,
    /// 报告里的值。 / The value in the report.
    pub value: f64,
    /// `integer` 或 `binary`。 / `integer` or `binary`.
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub lower: Option<i64>,
    #[serde(default)]
    pub upper: Option<i64>,
}

/// 提取请求。 / The extraction request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterAssignmentRequest {
    #[serde(default)]
    pub variables: Vec<MasterAssignedVariableSpec>,
    /// `optimal` / `feasible`。 / The report status.
    #[serde(default)]
    pub status: Option<String>,
    /// 整性容差（缺省 1e-7，与桥接层一致）。 / The integrality tolerance (defaults to 1e-7 like the bridge).
    #[serde(default)]
    pub tolerance: Option<f64>,
}

/// 提取结论。 / The extraction verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterAssignmentReport {
    /// 提取到的赋值（按 ID 排序）。 / The extracted assignment (sorted by ID).
    pub assigned: Vec<MasterAssignedValue>,
    #[serde(default)]
    pub error: Option<String>,
    /// 只接受最优报告（**是**）。 / Whether only optimal reports are accepted (**yes**).
    pub only_optimal_reports_are_accepted: bool,
    /// 容差必须有限且非负（**是**）。 / Whether the tolerance must be finite and non-negative (**yes**).
    pub the_tolerance_must_be_finite_and_non_negative: bool,
    /// 舍入是"半数远离零"（**是**）。 / Whether rounding is half-away-from-zero (**yes**).
    pub rounding_is_half_away_from_zero: bool,
    /// 容差是绝对差（**是**）。 / Whether the tolerance is an absolute difference (**yes**).
    pub the_tolerance_is_an_absolute_difference: bool,
    /// 两道范围闸（**是**）。 / Whether there are two range gates (**yes**).
    pub there_are_two_range_gates: bool,
    /// 域检查看舍入后的值（**是**）。 / Whether the domain check sees the rounded value (**yes**).
    pub the_domain_check_sees_the_rounded_value: bool,
    /// 缺 ID 的文案点名最优报告（**是**）。 / Whether a missing ID names the optimal report (**yes**).
    pub a_missing_id_says_optimal_master_report: bool,
}

/// 一条提取结果。 / One extracted value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterAssignedValue {
    pub id: String,
    pub value: f64,
}

/// 精确 f64 整数范围的上界（`2^53 - 1`），也是探针域的界。
/// The exact-f64-integer upper bound (`2^53 - 1`), used as the probe domain bound.
const EXACT_LIMIT: i64 = 9_007_199_254_740_991;

/// 造一份报告（`optimal` 决定是否带最优证明）。 / Build one report (`optimal` attaches the optimality proof).
fn report_with(values: &[(&str, f64)], optimal: bool) -> SolveReport<f64> {
    let stable: BTreeMap<StableVariableId, f64> = values.iter()
        .map(|(id, value)| (StableVariableId((*id).to_owned()), *value)).collect();
    let mut solution = SolveSolution::<f64>::vector(Vec::new());
    solution.stable_values = stable;
    solution.objective_value = Some(0.0);
    let builder = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
        .solution(solution);
    let builder = if optimal { builder.proof(SolveProof::optimality()) } else { builder };
    builder.build().unwrap_or_else(|error| panic!("probe report cannot be built: {error}"))
}

/// 造一份绑定。 / Build one binding.
fn binding_of(entries: &[(&str, i64, i64)]) -> MasterBinding {
    MasterBinding::new(entries.iter().map(|(id, lower, upper)| MasterVariableBinding {
        id: StableVariableId((*id).to_owned()),
        domain: MasterVariableDomain::Integer { lower: *lower, upper: *upper } }))
        .expect("probe binding is valid")
}

/// 取一次赋值，返回 `(赋值, 消息)`。 / Extract once, returning `(values, message)`.
fn extract(report: &SolveReport<f64>, entries: &[(&str, i64, i64)], tolerance: f64)
    -> (Vec<(String, i64)>, Option<String>)
{
    match MasterAssignment::from_report(report, &binding_of(entries), tolerance) {
        Ok(assignment) => (entries.iter().filter_map(|(id, _, _)| assignment
            .get(&StableVariableId((*id).to_owned()))
            .map(|value| ((*id).to_owned(), value))).collect(), None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    }
}

/// 走一次提取。非法输入是**错误**。 / Extract one assignment through the bridge.
pub fn verify_master_assignment(request: &MasterAssignmentRequest) -> Result<MasterAssignmentReport> {
    if request.variables.is_empty() || request.variables.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "master assignment failed validation: between 1 and 32 variables are accepted"));
    }
    let tolerance = request.tolerance.unwrap_or(1e-7);
    let optimal = match request.status.as_deref().unwrap_or("optimal") {
        "optimal" => true,
        "feasible" => false,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "master assignment failed validation: unknown status `{other}`"))),
    };
    let mut values = Vec::new();
    let mut domains = Vec::new();
    for variable in &request.variables {
        if variable.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "master assignment failed validation: variable IDs must not be blank"));
        }
        if !variable.value.is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "master assignment failed validation: values must be finite"));
        }
        let (lower, upper) = match variable.domain.as_deref().unwrap_or("integer") {
            "integer" => (variable.lower.unwrap_or(0), variable.upper.unwrap_or(100)),
            "binary" => (0, 1),
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "master assignment failed validation: unknown domain `{other}`"))),
        };
        if lower > upper {
            return Err(BridgeError::new("InvalidInput",
                "master assignment failed validation: the domain lower bound must not exceed the \
                 upper bound"));
        }
        values.push((variable.id.clone(), variable.value));
        domains.push((variable.id.clone(), lower, upper));
    }
    let borrowed = values.iter().map(|(id, value)| (id.as_str(), *value)).collect::<Vec<_>>();
    let domain_refs = domains.iter()
        .map(|(id, lower, upper)| (id.as_str(), *lower, *upper)).collect::<Vec<_>>();
    let report = report_with(&borrowed, optimal);
    let (assigned, error) = extract(&report, &domain_refs, tolerance);
    let findings = canonical_findings();
    Ok(MasterAssignmentReport {
        assigned: assigned.into_iter().map(|(id, value)| MasterAssignedValue {
            id, value: value as f64 }).collect(),
        error,
        only_optimal_reports_are_accepted: findings.only_optimal_reports_are_accepted,
        the_tolerance_must_be_finite_and_non_negative:
            findings.the_tolerance_must_be_finite_and_non_negative,
        rounding_is_half_away_from_zero: findings.rounding_is_half_away_from_zero,
        the_tolerance_is_an_absolute_difference: findings.the_tolerance_is_an_absolute_difference,
        there_are_two_range_gates: findings.there_are_two_range_gates,
        the_domain_check_sees_the_rounded_value: findings.the_domain_check_sees_the_rounded_value,
        a_missing_id_says_optimal_master_report:
            findings.a_missing_id_says_optimal_master_report })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    only_optimal_reports_are_accepted: bool,
    the_tolerance_must_be_finite_and_non_negative: bool,
    rounding_is_half_away_from_zero: bool,
    the_tolerance_is_an_absolute_difference: bool,
    there_are_two_range_gates: bool,
    the_domain_check_sees_the_rounded_value: bool,
    a_missing_id_says_optimal_master_report: bool,
}

/// 一份可用的最优报告。 / A usable optimal report.
fn optimal(value: f64) -> SolveReport<f64> {
    report_with(&[("x", value)], true)
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 只接受最优报告。 / Only optimal reports are accepted.
    let only_optimal_reports_are_accepted = {
        let feasible = report_with(&[("x", 1.0)], false);
        let (_, error) = extract(&feasible, &[("x", 0, 10)], 1e-7);
        let optimal = extract(&optimal(1.0), &[("x", 0, 10)], 1e-7);
        error.is_some_and(|message| message.contains("verified optimal"))
            && optimal == (vec![("x".to_owned(), 1)], None)
    };
    // 2) 容差必须有限且非负。 / The tolerance must be finite and non-negative.
    let the_tolerance_must_be_finite_and_non_negative = {
        let report = optimal(1.0);
        extract(&report, &[("x", 0, 10)], -1e-7).1
            .is_some_and(|message| message.contains("finite and non-negative"))
            && extract(&report, &[("x", 0, 10)], f64::NAN).1
                .is_some_and(|message| message.contains("finite and non-negative"))
            && extract(&report, &[("x", 0, 10)], 0.0).1.is_none()
    };
    // 3) 舍入是"半数远离零"：`2.5→3`、`-2.5→-3`。 
    // 3) Rounding is half away from zero: `2.5→3` and `-2.5→-3`.
    let rounding_is_half_away_from_zero = {
        extract(&optimal(2.5), &[("x", 0, 10)], 0.5) == (vec![("x".to_owned(), 3)], None)
            && extract(&optimal(3.5), &[("x", 0, 10)], 0.5) == (vec![("x".to_owned(), 4)], None)
            && extract(&optimal(-2.5), &[("x", -10, 10)], 0.5)
                == (vec![("x".to_owned(), -3)], None)
    };
    // 4) 容差是绝对差：恰好等于容差通过，略超即拒。 
    // 4) The tolerance is an absolute difference: an exact match passes, a slight excess is refused.
    let the_tolerance_is_an_absolute_difference = {
        let at = extract(&optimal(1.25), &[("x", 0, 10)], 0.25);
        let over = extract(&optimal(1.26), &[("x", 0, 10)], 0.25);
        at == (vec![("x".to_owned(), 1)], None)
            && over.1.is_some_and(|message| message.contains("is not integral"))
    };
    // 5) 两道范围闸：`1e19` 撞 i64 闸，`1e17` 撞精确整数闸。 
    // 5) Two range gates: `1e19` hits the i64 gate, `1e17` the exact-integer gate.
    let there_are_two_range_gates = {
        let i64_gate = extract(&optimal(1e19), &[("x", -EXACT_LIMIT, EXACT_LIMIT)], 0.0).1;
        let exact_gate = extract(&optimal(1e17), &[("x", -EXACT_LIMIT, EXACT_LIMIT)], 0.0).1;
        i64_gate.as_deref().is_some_and(|message| message.contains("outside the i64 range"))
            && exact_gate.as_deref().is_some_and(|message| message
                .contains("outside the exact f64 integer range"))
            && i64_gate != exact_gate
    };
    // 6) 域检查看舍入后的值：`10.4→10` 通过，`10.6→11` 被拒（域 `0..10`）。 
    // 6) The domain check sees the rounded value: `10.4→10` passes, `10.6→11` is refused on `0..10`.
    let the_domain_check_sees_the_rounded_value = {
        let inside = extract(&optimal(10.4), &[("x", 0, 10)], 0.5);
        let outside = extract(&optimal(10.6), &[("x", 0, 10)], 0.5);
        inside == (vec![("x".to_owned(), 10)], None)
            && outside.1.is_some_and(|message| message.contains("outside its declared domain"))
    };
    // 7) 缺 ID 的文案点名最优报告。 / A missing ID names the optimal report.
    let a_missing_id_says_optimal_master_report = {
        let report = report_with(&[("y", 1.0)], true);
        extract(&report, &[("x", 0, 10)], 1e-7).1.is_some_and(|message| message
            .contains("optimal master report is missing stable variable x"))
    };
    Findings { only_optimal_reports_are_accepted,
        the_tolerance_must_be_finite_and_non_negative, rounding_is_half_away_from_zero,
        the_tolerance_is_an_absolute_difference, there_are_two_range_gates,
        the_domain_check_sees_the_rounded_value, a_missing_id_says_optimal_master_report }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MasterAssignmentRequest {
        MasterAssignmentRequest {
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 3.0,
                domain: Some("integer".to_owned()), lower: Some(0), upper: Some(10) }],
            status: None, tolerance: None }
    }

    #[test]
    fn an_optimal_report_yields_the_rounded_assignment() {
        let report = verify_master_assignment(&request()).unwrap();
        assert_eq!(report.assigned, vec![MasterAssignedValue { id: "x".to_owned(), value: 3.0 }]);
        assert_eq!(report.error, None);
        assert!(report.only_optimal_reports_are_accepted);
        assert!(report.the_tolerance_must_be_finite_and_non_negative);
        assert!(report.rounding_is_half_away_from_zero);
        assert!(report.the_tolerance_is_an_absolute_difference);
        assert!(report.there_are_two_range_gates);
        assert!(report.the_domain_check_sees_the_rounded_value);
        assert!(report.a_missing_id_says_optimal_master_report);
    }

    #[test]
    fn a_feasible_but_unproven_report_is_refused() {
        let report = verify_master_assignment(&MasterAssignmentRequest {
            status: Some("feasible".to_owned()), ..request() }).unwrap();
        assert!(report.assigned.is_empty());
        assert!(report.error.as_deref().unwrap_or_default().contains("verified optimal"));
    }

    #[test]
    fn half_away_from_zero_rounding_and_absolute_tolerance() {
        // `2.5` 在容差 `0.5` 下舍入成 `3`。 / `2.5` rounds up to `3` at tolerance `0.5`.
        let rounded = verify_master_assignment(&MasterAssignmentRequest { tolerance: Some(0.5),
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 2.5,
                domain: Some("integer".to_owned()), lower: Some(0), upper: Some(10) }],
            ..request() }).unwrap();
        assert_eq!(rounded.assigned[0].value, 3.0);
        // 恰好等于容差通过，略超被拒。 / An exact match passes, a slight excess is refused.
        let at = verify_master_assignment(&MasterAssignmentRequest { tolerance: Some(0.25),
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 1.25,
                domain: Some("integer".to_owned()), lower: Some(0), upper: Some(10) }],
            ..request() }).unwrap();
        assert_eq!(at.assigned[0].value, 1.0);
        let over = verify_master_assignment(&MasterAssignmentRequest { tolerance: Some(0.25),
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 1.26,
                domain: Some("integer".to_owned()), lower: Some(0), upper: Some(10) }],
            ..request() }).unwrap();
        assert!(over.assigned.is_empty());
        assert!(over.error.as_deref().unwrap_or_default().contains("is not integral"));
    }

    #[test]
    fn the_domain_check_sees_the_rounded_value() {
        let inside = verify_master_assignment(&MasterAssignmentRequest { tolerance: Some(0.5),
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 10.4,
                domain: Some("integer".to_owned()), lower: Some(0), upper: Some(10) }],
            ..request() }).unwrap();
        assert_eq!(inside.assigned[0].value, 10.0);
        let outside = verify_master_assignment(&MasterAssignmentRequest { tolerance: Some(0.5),
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 10.6,
                domain: Some("integer".to_owned()), lower: Some(0), upper: Some(10) }],
            ..request() }).unwrap();
        assert!(outside.assigned.is_empty());
        assert!(outside.error.as_deref().unwrap_or_default()
            .contains("outside its declared domain"));
    }

    #[test]
    fn the_two_range_gates() {
        let huge = verify_master_assignment(&MasterAssignmentRequest { tolerance: Some(0.0),
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 1e19,
                domain: Some("integer".to_owned()), lower: Some(-EXACT_LIMIT),
                upper: Some(EXACT_LIMIT) }], ..request() }).unwrap();
        assert!(huge.error.as_deref().unwrap_or_default().contains("outside the i64 range"));
        let exact = verify_master_assignment(&MasterAssignmentRequest { tolerance: Some(0.0),
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 1e17,
                domain: Some("integer".to_owned()), lower: Some(-EXACT_LIMIT),
                upper: Some(EXACT_LIMIT) }], ..request() }).unwrap();
        assert!(exact.error.as_deref().unwrap_or_default()
            .contains("outside the exact f64 integer range"));
        // 空变量表被拒（`MasterBinding::new` 不接受空绑定）。 
        // An empty variable list is refused (`MasterBinding::new` rejects an empty binding).
        assert!(verify_master_assignment(&MasterAssignmentRequest { variables: Vec::new(),
            ..request() }).unwrap_err().message.contains("between 1 and 32 variables"));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_master_assignment(&MasterAssignmentRequest {
            status: Some("infeasible".to_owned()), ..request() }).unwrap_err().message
            .contains("unknown status"));
        assert!(verify_master_assignment(&MasterAssignmentRequest {
            variables: vec![MasterAssignedVariableSpec { id: "  ".to_owned(), value: 1.0,
                domain: None, lower: None, upper: None }], ..request() }).unwrap_err().message
            .contains("IDs must not be blank"));
        assert!(verify_master_assignment(&MasterAssignmentRequest {
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: f64::NAN,
                domain: None, lower: None, upper: None }], ..request() }).unwrap_err().message
            .contains("values must be finite"));
        assert!(verify_master_assignment(&MasterAssignmentRequest {
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 1.0,
                domain: Some("real".to_owned()), lower: None, upper: None }],
            ..request() }).unwrap_err().message.contains("unknown domain"));
        assert!(verify_master_assignment(&MasterAssignmentRequest {
            variables: vec![MasterAssignedVariableSpec { id: "x".to_owned(), value: 1.0,
                domain: Some("integer".to_owned()), lower: Some(5), upper: Some(1) }],
            ..request() }).unwrap_err().message.contains("lower bound"));
        let too_many = MasterAssignmentRequest {
            variables: (0..33).map(|index| MasterAssignedVariableSpec { id: format!("v{index}"),
                value: 0.0, domain: None, lower: None, upper: None }).collect(), ..request() };
        assert!(verify_master_assignment(&too_many).unwrap_err().message
            .contains("between 1 and 32 variables"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_master_assignment(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"onlyOptimalReportsAreAccepted\""), "{json}");
        assert!(json.contains("\"roundingIsHalfAwayFromZero\""), "{json}");
        assert!(json.contains("\"thereAreTwoRangeGates\""), "{json}");
        assert!(!json.contains("only_optimal"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<MasterAssignmentReport>(&copy.to_string()).is_err());
    }
}
