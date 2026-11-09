//! 续挖第 96 单：**带状态解的六个状态与两条判据**——`Default`/`new()` 是 `NotSolved`（不是 `Unknown`）、空解、
//! 无目标值；**`with_status` 一定丢掉目标值**；`is_feasible` 只认 `Optimal | Feasible`，于是**`Unbounded` 与
//! `Unknown` 都不算可行**；`is_optimal` 是 `is_feasible` 的真子集；而 **`with_objective` 不校验状态**，
//! 可以给 `Infeasible` 甚至 `NotSolved` 挂上目标值
//! （`model/callback/solution.rs` 的 `SolutionStatus` / `SolutionWithStatus`）
//! Continuation, delivery 96: **the six statuses of the status-carrying solution and its two predicates** —
//! `Default`/`new()` is `NotSolved` (not `Unknown`), empty and without an objective; **`with_status` always drops the
//! objective**; `is_feasible` accepts only `Optimal | Feasible`, so **`Unbounded` and `Unknown` are not feasible**;
//! `is_optimal` is a strict subset of `is_feasible`; and **`with_objective` never checks the status**, so an objective can
//! be attached to `Infeasible` and even `NotSolved` (`SolutionStatus` / `SolutionWithStatus` in
//! `model/callback/solution.rs`).
//!
//! 第 95 单量的是多样性度量，这一单量**带状态解**。八条实测：
//!
//! 1. **默认是 `NotSolved` + 空解 + 无目标值**（`the_default_is_not_solved_with_an_empty_solution()`）。
//! 2. **状态枚举有六个值，默认是 `NotSolved` 而不是 `Unknown`**
//!    （`the_status_enum_has_six_variants_defaulting_to_not_solved()`）。
//! 3. **只有 `optimal()` 自带目标值**（`only_the_optimal_constructor_fills_the_objective()`）。
//! 4. **`with_status` 一定丢掉目标值**（`with_status_drops_the_objective()`）：要目标值必须再链 `with_objective`。
//! 5. **`is_optimal` 是 `is_feasible` 的真子集**（`optimal_is_a_strict_subset_of_feasible()`）。
//! 6. **`Unbounded` 与 `Unknown` 都不算可行**（`unbounded_and_unknown_are_not_feasible()`）。
//! 7. **`infeasible()` 带的是空解**（`the_infeasible_constructor_carries_an_empty_solution()`）。
//! 8. **`with_objective` 不校验状态**（`with_objective_never_checks_the_status()`）：本单最值钱的一条，
//!    `Infeasible` 与 `NotSolved` 都能挂上目标值。
//!
//! Delivery 95 measured diversity; this one measures **the status-carrying solution**. **The default is `NotSolved` with
//! an empty solution and no objective.** **The status enum has six variants and defaults to `NotSolved` rather than
//! `Unknown`.** **Only `optimal` carries an objective.** **`with_status` always drops the objective**, so `with_objective`
//! must be chained. **`is_optimal` is a strict subset of `is_feasible`.** **`Unbounded` and `Unknown` are not feasible.**
//! **`infeasible()` carries an empty solution.** And **`with_objective` never checks the status** — the most valuable
//! finding here, since both `Infeasible` and `NotSolved` accept an objective.

use ospf_rust_core::model::callback::{Solution, SolutionStatus, SolutionWithStatus};
use ospf_rust_core::variable::VariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 带状态解探针请求。 / The status-carrying solution probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SolutionStatusRequest {
    /// 解向量（按变量序号给出的值）。 / The solution vector, as values by variable index.
    #[serde(default)]
    pub solution: Vec<f64>,
    /// 构造方式：`new` / `withStatus` / `optimal` / `feasible` / `infeasible`（默认 `new`）。
    /// The construction route.
    #[serde(default)]
    pub mode: Option<String>,
    /// `withStatus` 用的状态名。 / The status name used by `withStatus`.
    #[serde(default)]
    pub status: Option<String>,
    /// 是否再链一次 `withObjective`。 / Whether to chain `withObjective` afterwards.
    #[serde(default)]
    pub attach_objective: Option<bool>,
    /// `optimal` 与 `withObjective` 用的目标值（默认 0）。 / The objective used by `optimal` and `withObjective`.
    #[serde(default)]
    pub objective: Option<f64>,
}

/// 带状态解探针结论。 / The status-carrying solution probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SolutionStatusReport {
    /// 状态名。 / The status name.
    pub status: String,
    /// `is_optimal()` 的读数。 / What `is_optimal()` says.
    pub is_optimal: bool,
    /// `is_feasible()` 的读数。 / What `is_feasible()` says.
    pub is_feasible: bool,
    /// 解向量的条目数。 / The solution's entry count.
    pub solution_len: u32,
    /// 目标值（`null` 表示没有）。 / The objective, `null` when absent.
    #[serde(default)]
    pub objective: Option<f64>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_default_is_not_solved_with_an_empty_solution: bool,
    pub the_status_enum_has_six_variants_defaulting_to_not_solved: bool,
    pub only_the_optimal_constructor_fills_the_objective: bool,
    pub with_status_drops_the_objective: bool,
    pub optimal_is_a_strict_subset_of_feasible: bool,
    pub unbounded_and_unknown_are_not_feasible: bool,
    pub the_infeasible_constructor_carries_an_empty_solution: bool,
    pub with_objective_never_checks_the_status: bool,
}

/// 六个状态名。 / The six status names.
const STATUS_NAMES: [&str; 6] = ["Optimal", "Feasible", "Infeasible", "Unbounded", "NotSolved",
    "Unknown"];

/// 按名字取状态。 / Resolve a status by name.
fn status_named(name: &str) -> Option<SolutionStatus> {
    match name {
        "Optimal" => Some(SolutionStatus::Optimal),
        "Feasible" => Some(SolutionStatus::Feasible),
        "Infeasible" => Some(SolutionStatus::Infeasible),
        "Unbounded" => Some(SolutionStatus::Unbounded),
        "NotSolved" => Some(SolutionStatus::NotSolved),
        "Unknown" => Some(SolutionStatus::Unknown),
        _ => None,
    }
}

/// 按名字读状态。 / Read a status as its name.
fn status_name(status: SolutionStatus) -> String {
    STATUS_NAMES.iter().copied()
        .find(|name| status_named(name) == Some(status))
        .unwrap_or("Unknown").to_owned()
}

/// 铺出解向量。 / Lay out the solution vector.
fn solution_of(values: &[f64]) -> Solution<f64> {
    values.iter().enumerate()
        .map(|(index, value)| (VariableId::new(1, index), *value)).collect()
}

/// 走一遍带状态解探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_solution_status(request: &SolutionStatusRequest) -> Result<SolutionStatusReport> {
    if request.solution.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "solution status probe failed validation: \
            at most eight entries are accepted"));
    }
    if request.solution.iter().any(|value| !value.is_finite()) {
        return Err(BridgeError::new("InvalidInput", "solution status probe failed validation: \
            non-finite values cannot cross the boundary"));
    }
    let objective = request.objective.unwrap_or(0.0);
    if !objective.is_finite() {
        return Err(BridgeError::new("InvalidInput", "solution status probe failed validation: \
            the objective must be finite"));
    }
    let mode = request.mode.clone().unwrap_or_else(|| "new".to_owned());
    let status = match mode.as_str() {
        "new" | "optimal" | "feasible" | "infeasible" => None,
        "withStatus" => Some(status_named(request.status.as_deref().unwrap_or("Feasible"))
            .ok_or_else(|| BridgeError::new("InvalidInput", "solution status probe failed \
                validation: unknown status name"))?),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "solution status probe failed validation: unknown mode `{other}`"))),
    };
    let findings = canonical_findings();
    let solution = solution_of(&request.solution);
    let mut built = match mode.as_str() {
        "optimal" => SolutionWithStatus::optimal(solution, objective),
        "feasible" => SolutionWithStatus::feasible(solution),
        "infeasible" => SolutionWithStatus::infeasible(),
        _ => match status {
            Some(status) => SolutionWithStatus::with_status(solution, status),
            None => SolutionWithStatus::new(),
        },
    };
    if request.attach_objective.unwrap_or(false) {
        built = built.with_objective(objective);
    }
    Ok(SolutionStatusReport {
        status: status_name(built.status),
        is_optimal: built.is_optimal(),
        is_feasible: built.is_feasible(),
        solution_len: u32::try_from(built.solution.len()).unwrap_or(u32::MAX),
        objective: built.objective_value,
        the_default_is_not_solved_with_an_empty_solution:
            findings.the_default_is_not_solved_with_an_empty_solution,
        the_status_enum_has_six_variants_defaulting_to_not_solved:
            findings.the_status_enum_has_six_variants_defaulting_to_not_solved,
        only_the_optimal_constructor_fills_the_objective:
            findings.only_the_optimal_constructor_fills_the_objective,
        with_status_drops_the_objective: findings.with_status_drops_the_objective,
        optimal_is_a_strict_subset_of_feasible: findings.optimal_is_a_strict_subset_of_feasible,
        unbounded_and_unknown_are_not_feasible: findings.unbounded_and_unknown_are_not_feasible,
        the_infeasible_constructor_carries_an_empty_solution:
            findings.the_infeasible_constructor_carries_an_empty_solution,
        with_objective_never_checks_the_status: findings.with_objective_never_checks_the_status,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_default_is_not_solved_with_an_empty_solution: bool,
    the_status_enum_has_six_variants_defaulting_to_not_solved: bool,
    only_the_optimal_constructor_fills_the_objective: bool,
    with_status_drops_the_objective: bool,
    optimal_is_a_strict_subset_of_feasible: bool,
    unbounded_and_unknown_are_not_feasible: bool,
    the_infeasible_constructor_carries_an_empty_solution: bool,
    with_objective_never_checks_the_status: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let sample = solution_of(&[1.0, 2.0]);
        // 1) 默认是 NotSolved + 空解 + 无目标值。 / The default is NotSolved, empty and without an objective.
        let the_default_is_not_solved_with_an_empty_solution = {
            let fresh = SolutionWithStatus::<f64>::new();
            let defaulted = SolutionWithStatus::<f64>::default();
            fresh.status == SolutionStatus::NotSolved
                && fresh.solution.is_empty() && fresh.objective_value.is_none()
                && defaulted.status == fresh.status && defaulted.solution == fresh.solution
        };
        // 2) 六个状态，默认不是 Unknown。 / Six statuses, and the default is not Unknown.
        let the_status_enum_has_six_variants_defaulting_to_not_solved = {
            let resolved: Vec<SolutionStatus> = STATUS_NAMES.iter()
                .filter_map(|name| status_named(name)).collect();
            // 六个名字都认得、且互不相同。 / All six names resolve and stay distinct.
            resolved.len() == 6
                && STATUS_NAMES.iter().all(|name| status_named(name).is_some())
                && SolutionStatus::default() == SolutionStatus::NotSolved
                && SolutionStatus::default() != SolutionStatus::Unknown
        };
        // 3) 只有 `optimal` 自带目标值。 / Only `optimal` carries an objective.
        let only_the_optimal_constructor_fills_the_objective = {
            SolutionWithStatus::optimal(sample.clone(), 7.0).objective_value == Some(7.0)
                && SolutionWithStatus::feasible(sample.clone()).objective_value.is_none()
                && SolutionWithStatus::<f64>::infeasible().objective_value.is_none()
                && SolutionWithStatus::with_status(sample.clone(), SolutionStatus::Optimal)
                    .objective_value.is_none()
        };
        // 4) `with_status` 一定丢掉目标值。 / `with_status` always drops the objective.
        let with_status_drops_the_objective = {
            // 即使状态是 `Optimal`，目标值也是 `None`；要挂上必须链 `with_objective`。 
            // Even an `Optimal` status carries no objective; `with_objective` must be chained to attach one.
            let bare = SolutionWithStatus::with_status(sample.clone(), SolutionStatus::Optimal);
            let attached = bare.clone().with_objective(3.0);
            bare.objective_value.is_none() && attached.objective_value == Some(3.0)
                // 解与状态一对一保留。 / The solution and the status survive one to one.
                && attached.solution == sample && attached.status == SolutionStatus::Optimal
        };
        // 5) `is_optimal` 是 `is_feasible` 的真子集。 / `is_optimal` is a strict subset of `is_feasible`.
        let optimal_is_a_strict_subset_of_feasible = {
            let optimal = SolutionWithStatus::optimal(sample.clone(), 1.0);
            let feasible = SolutionWithStatus::feasible(sample.clone());
            optimal.is_optimal() && optimal.is_feasible()
                && !feasible.is_optimal() && feasible.is_feasible()
        };
        // 6) `Unbounded` 与 `Unknown` 都不算可行。 / `Unbounded` and `Unknown` are not feasible.
        let unbounded_and_unknown_are_not_feasible = {
            let unbounded = SolutionWithStatus::with_status(sample.clone(),
                SolutionStatus::Unbounded);
            let unknown = SolutionWithStatus::with_status(sample.clone(),
                SolutionStatus::Unknown);
            !unbounded.is_feasible() && !unbounded.is_optimal()
                && !unknown.is_feasible() && !unknown.is_optimal()
                // 对照：四个不可行的状态全是假。 / Control: all four unproven statuses are false.
                && [SolutionStatus::Infeasible, SolutionStatus::NotSolved]
                    .iter().all(|status| !SolutionWithStatus::with_status(sample.clone(), *status)
                        .is_feasible())
        };
        // 7) 空解是构造器的、不是状态的。 / Emptiness belongs to the constructor rather than the status.
        let the_infeasible_constructor_carries_an_empty_solution = {
            let infeasible = SolutionWithStatus::<f64>::infeasible();
            // 同一个状态，`with_status` 却保留了两条解 —— "不可行"不等于"没有解条目"。 
            // The same status keeps two entries through `with_status`, so "infeasible" does not imply "no entries".
            let kept = SolutionWithStatus::with_status(sample.clone(), SolutionStatus::Infeasible);
            infeasible.status == SolutionStatus::Infeasible
                && infeasible.solution.is_empty()
                && infeasible.objective_value.is_none()
                && !infeasible.is_feasible()
                && kept.status == SolutionStatus::Infeasible
                && kept.solution.len() == 2
        };
        // 8) `with_objective` 不校验状态。 / `with_objective` never checks the status.
        let with_objective_never_checks_the_status = {
            // `Infeasible` 与 `NotSolved` 都能挂上目标值，且不改状态。 
            // Both `Infeasible` and `NotSolved` accept an objective without changing the status.
            let infeasible = SolutionWithStatus::<f64>::infeasible().with_objective(-5.0);
            let not_solved = SolutionWithStatus::<f64>::new().with_objective(-5.0);
            infeasible.objective_value == Some(-5.0)
                && infeasible.status == SolutionStatus::Infeasible
                && not_solved.objective_value == Some(-5.0)
                && not_solved.status == SolutionStatus::NotSolved
        };
        Findings {
            the_default_is_not_solved_with_an_empty_solution,
            the_status_enum_has_six_variants_defaulting_to_not_solved,
            only_the_optimal_constructor_fills_the_objective,
            with_status_drops_the_objective,
            optimal_is_a_strict_subset_of_feasible,
            unbounded_and_unknown_are_not_feasible,
            the_infeasible_constructor_carries_an_empty_solution,
            with_objective_never_checks_the_status,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SolutionStatusRequest {
        SolutionStatusRequest { solution: vec![1.0, 2.0], mode: Some("optimal".to_owned()),
            status: None, attach_objective: None, objective: Some(7.0) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_solution_status(&request()).unwrap();
        assert!(report.the_default_is_not_solved_with_an_empty_solution);
        assert!(report.the_status_enum_has_six_variants_defaulting_to_not_solved);
        assert!(report.only_the_optimal_constructor_fills_the_objective);
        assert!(report.with_status_drops_the_objective);
        assert!(report.optimal_is_a_strict_subset_of_feasible);
        assert!(report.unbounded_and_unknown_are_not_feasible);
        assert!(report.the_infeasible_constructor_carries_an_empty_solution);
        assert!(report.with_objective_never_checks_the_status);
        assert_eq!(report.status, "Optimal");
        assert!(report.is_optimal);
        assert!(report.is_feasible);
        assert_eq!(report.objective, Some(7.0));
        assert_eq!(report.solution_len, 2);
    }

    #[test]
    fn the_default_is_not_solved_and_empty() {
        // 必须显式换成 `new`：`request()` 默认走 `optimal`。 
        // The mode must be switched to `new` explicitly, since `request()` defaults to `optimal`.
        let report = verify_solution_status(&SolutionStatusRequest { solution: Vec::new(),
            mode: Some("new".to_owned()), ..request() }).unwrap();
        assert_eq!(report.status, "NotSolved");
        assert!(!report.is_optimal);
        assert!(!report.is_feasible);
        assert_eq!(report.solution_len, 0);
        assert_eq!(report.objective, None);
    }

    #[test]
    fn with_status_drops_the_objective_until_it_is_chained() {
        let bare = verify_solution_status(&SolutionStatusRequest { mode: Some("withStatus"
            .to_owned()), status: Some("Feasible".to_owned()), objective: Some(7.0), ..request() })
            .unwrap();
        assert_eq!(bare.objective, None);
        assert!(bare.is_feasible && !bare.is_optimal);
        let attached = verify_solution_status(&SolutionStatusRequest { mode: Some("withStatus"
            .to_owned()), status: Some("Feasible".to_owned()), attach_objective: Some(true),
            ..request() }).unwrap();
        assert_eq!(attached.objective, Some(7.0));
    }

    #[test]
    fn unbounded_and_unknown_are_not_feasible() {
        for name in ["Unbounded", "Unknown", "Infeasible", "NotSolved"] {
            let report = verify_solution_status(&SolutionStatusRequest { mode: Some("withStatus"
                .to_owned()), status: Some(name.to_owned()), ..request() }).unwrap();
            assert!(!report.is_feasible, "{name}");
            assert!(!report.is_optimal, "{name}");
        }
    }

    #[test]
    fn the_infeasible_constructor_ignores_the_solution_and_objective() {
        let report = verify_solution_status(&SolutionStatusRequest { mode: Some("infeasible"
            .to_owned()), attach_objective: Some(true), ..request() }).unwrap();
        // 解被丢掉，目标值却挂上了。 / The solution is dropped while the objective is attached.
        assert_eq!(report.status, "Infeasible");
        assert_eq!(report.solution_len, 0);
        assert_eq!(report.objective, Some(7.0));
        assert!(!report.is_feasible);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_solution_status(&SolutionStatusRequest { solution: vec![1.0; 9],
            ..request() }).unwrap_err().message.contains("at most eight entries"));
        assert!(verify_solution_status(&SolutionStatusRequest { solution: vec![f64::NAN],
            ..request() }).unwrap_err().message.contains("non-finite values"));
        assert!(verify_solution_status(&SolutionStatusRequest { mode: Some("bogus".to_owned()),
            ..request() }).unwrap_err().message.contains("unknown mode"));
        assert!(verify_solution_status(&SolutionStatusRequest { mode: Some("withStatus"
            .to_owned()), status: Some("Nearly".to_owned()), ..request() }).unwrap_err()
            .message.contains("unknown status name"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_solution_status(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"isOptimal\""), "{json}");
        assert!(json.contains("\"solutionLen\""), "{json}");
        assert!(json.contains("\"withObjectiveNeverChecksTheStatus\""), "{json}");
        assert!(!json.contains("is_optimal"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SolutionStatusReport>(&copy.to_string()).is_err());
    }
}
