//! 续挖第 105 单：**报告的诊断面完全不受 `validate()` 约束**——把 `satisfied = true` 配上一个巨大的 `violation`、
//! 空白的 warning code、空白的 issue message 塞进报告，`validate()` 照样通过；而读侧的三个谓词**各看各的字段**：
//! `has_incumbent()` 只看 `solution_presence`（于是它可以为真而 `solution` 为空）、`is_optimal()` 只看是否为
//! `Optimal`、`incumbent()` 读的是 `solution.value` 而不是 `objective_value`
//! （`solver/report/mod.rs` 的 `SolveDiagnostics` / `ConstraintEvaluation` / `SolveWarning` / `has_incumbent`）
//! Continuation, delivery 105: **the report's diagnostic face is not constrained by `validate()` at all** — stuffing a
//! `satisfied` evaluation with a huge `violation`, a blank warning code and a blank issue message into a report still
//! validates; and the three read-side predicates **each look at their own field**: `has_incumbent()` reads only
//! `solution_presence`, so it can be true while `solution` is empty, `is_optimal()` reads only the presence, and
//! `incumbent()` reads `solution.value` rather than `objective_value`
//! (`SolveDiagnostics` / `ConstraintEvaluation` / `SolveWarning` / `has_incumbent` in `solver/report/mod.rs`).
//!
//! 第 104 单量的是 gap 算术，这一单量**诊断面与读侧谓词的边界**。八条实测：
//!
//! 1. **`has_incumbent()` 只看 `solution_presence`**（`has_incumbent_reads_only_the_presence()`）：
//!    `Incumbent` 与 `Optimal` 为真，`None` 为假。
//! 2. **`is_optimal()` 只认 `Optimal`**（`is_optimal_reads_only_the_presence()`）。
//! 3. **谓词不校验报告**（`the_predicates_never_validate_the_report()`），而且两个方向都能矛盾：
//!    `presence: Incumbent` 配 `solution: None` 时 `has_incumbent()` 为真而 `incumbent()` 为空；
//!    `presence: None` 配 incumbent 时 `has_incumbent()` 为假而 `incumbent()` 却有值——两种报告 `validate()` 都拒。
//! 4. **`incumbent()` 读 `solution.value`**（`the_incumbent_reads_the_typed_value()`）：
//!    `value: None` 而 `objective_value: Some(..)` 时 `incumbent()` 是 `None`。
//! 5. **诊断面不被 `validate()` 检查**（`the_diagnostics_are_never_validated()`）：
//!    荒谬的求值记录、空白 warning 与空白 issue 全都能通过。
//! 6. **`satisfied` 与 `violation` 从不互相校验**（`satisfied_and_violation_are_never_cross_checked()`）。
//! 7. **边界求值的 `satisfied` 与上下界也不互相校验**（`the_bound_evaluation_flags_are_never_cross_checked()`）。
//! 8. **空白的 warning / issue 文本被接受**（`blank_warnings_and_issues_are_accepted()`）。
//!
//! Delivery 104 measured the gap arithmetic; this one measures **the diagnostic face and the read-side predicates**. **The
//! predicates read only the presence.** **They never validate the report**, so `has_incumbent()` can be true with no
//! solution. **`incumbent()` reads the typed value.** **The diagnostics are never validated**, **`satisfied` is never
//! cross-checked against `violation`**, **the bound flags are never cross-checked against the bounds**, and **blank
//! warnings and issues are accepted.**

use ospf_rust_core::solver::report::{
    ConstraintEvaluation, ProblemStatus, ProofCompleteness, ProofKind, ProofReliability, ProofStatus,
    SolveDiagnostics, SolveFingerprints, SolveIssue, SolveProof, SolveReport, SolveSolution,
    SolveStatistics, SolveTrace, SolveWarning, SolutionPresence, SolverProvenance,
    StableVariableId, TerminationReason, VariableBoundEvaluation,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条约束求值的描述。 / The description of one constraint evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationSpec {
    /// 是否标记为满足。 / Whether it is flagged satisfied.
    pub satisfied: bool,
    /// 违反量。 / The violation.
    pub violation: f64,
}

/// 一条 warning / issue 的描述。 / The description of one warning or issue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageSpec {
    /// 稳定 code。 / The stable code.
    pub code: String,
    /// 脱敏说明。 / The redacted message.
    pub message: String,
}

/// 诊断探针请求。 / The diagnostics probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticsProbeRequest {
    /// incumbent 存在性名（默认 `Optimal`）。 / The solution-presence name.
    #[serde(default)]
    pub presence: Option<String>,
    /// 是否带 incumbent（默认真）。 / Whether a solution is attached.
    #[serde(default)]
    pub has_solution: Option<bool>,
    /// 是否填 `solution.value`（默认真）。 / Whether `solution.value` is filled in.
    #[serde(default)]
    pub typed_value: Option<bool>,
    /// 约束求值记录。 / The constraint evaluations.
    #[serde(default)]
    pub evaluations: Vec<EvaluationSpec>,
    /// warning 列表。 / The warnings.
    #[serde(default)]
    pub warnings: Vec<MessageSpec>,
    /// issue 列表。 / The issues.
    #[serde(default)]
    pub issues: Vec<MessageSpec>,
}

/// 诊断探针结论。 / The diagnostics probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticsProbeReport {
    /// 报告是否通过 `validate()`。 / Whether the report validates.
    pub report_valid: bool,
    /// 校验错误文本（通过时为空）。 / The validation error text, empty when it passes.
    pub report_error: String,
    /// `has_incumbent()` 的读数。 / What `has_incumbent()` says.
    pub has_incumbent: bool,
    /// `is_optimal()` 的读数。 / What `is_optimal()` says.
    pub is_optimal: bool,
    /// `incumbent()` 是否给出 payload。 / Whether `incumbent()` yields a payload.
    pub incumbent_present: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub has_incumbent_reads_only_the_presence: bool,
    pub is_optimal_reads_only_the_presence: bool,
    pub the_predicates_never_validate_the_report: bool,
    pub the_incumbent_reads_the_typed_value: bool,
    pub the_diagnostics_are_never_validated: bool,
    pub satisfied_and_violation_are_never_cross_checked: bool,
    pub the_bound_evaluation_flags_are_never_cross_checked: bool,
    pub blank_warnings_and_issues_are_accepted: bool,
}

/// 按名字取 incumbent 存在性。 / Resolve a solution presence by name.
fn presence_named(name: &str) -> Option<SolutionPresence> {
    match name {
        "None" => Some(SolutionPresence::None),
        "Incumbent" => Some(SolutionPresence::Incumbent),
        "Optimal" => Some(SolutionPresence::Optimal),
        _ => None,
    }
}

/// 造一条约束求值。 / Build one constraint evaluation.
fn evaluation_of(index: usize, spec: &EvaluationSpec) -> ConstraintEvaluation<f64> {
    ConstraintEvaluation {
        constraint_id: format!("c{index}"),
        lhs: 0.0,
        rhs: 1.0,
        residual: -spec.violation,
        violation: spec.violation,
        tolerance: 1e-9,
        satisfied: spec.satisfied,
    }
}

/// 造一份报告。 / Build one report.
fn report_of(request: &DiagnosticsProbeRequest) -> Result<SolveReport<f64, u32>> {
    let presence = match request.presence.as_deref() {
        None => SolutionPresence::Optimal,
        Some(name) => presence_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "diagnostics probe failed validation: unknown solution presence"))?,
    };
    let has_solution = request.has_solution.unwrap_or(true);
    let typed_value = request.typed_value.unwrap_or(true);
    Ok(SolveReport {
        schema_version: "diagnostics-probe".to_owned(),
        problem_status: ProblemStatus::Feasible,
        termination_reason: TerminationReason::Completed,
        solution_presence: presence,
        solution: if has_solution {
            Some(SolveSolution {
                value: if typed_value { Some(0.0) } else { None },
                values: vec![0.0],
                stable_values: Default::default(),
                objective: Some(0.0),
                objective_value: Some(0.0),
                dual_solution: Some(vec![1.0]),
                quadratic_dual_solution: None,
                pool: Vec::new(),
            })
        } else {
            None
        },
        proof: Some(SolveProof {
            kind: ProofKind::Optimality,
            status: ProofStatus::Verified,
            reliability: ProofReliability::Exact,
            completeness: ProofCompleteness::Complete,
            reference: None,
            evidence: None,
        }),
        statistics: SolveStatistics::default(),
        diagnostics: SolveDiagnostics {
            constraint_evaluations: request.evaluations.iter().enumerate()
                .map(|(index, spec)| evaluation_of(index, spec)).collect(),
            variable_bound_evaluations: request.evaluations.iter().enumerate().map(
                |(index, spec)| VariableBoundEvaluation {
                    variable_id: StableVariableId(format!("v{index}")),
                    value: 0.0,
                    lower: Some(1.0),
                    upper: Some(2.0),
                    satisfied: spec.satisfied,
                }).collect(),
            infeasibility_evidence: None,
            warnings: request.warnings.iter().map(|spec| SolveWarning::new(
                spec.code.clone(), spec.message.clone())).collect(),
            issues: request.issues.iter().map(|spec| SolveIssue::new(
                spec.code.clone(), spec.message.clone())).collect(),
            extensions: Default::default(),
        },
        provenance: SolverProvenance::default(),
        fingerprints: SolveFingerprints::default(),
        model_mapping: None,
        warnings: Vec::new(),
        trace: SolveTrace::default(),
    })
}

/// 走一遍诊断探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_diagnostics_probe(request: &DiagnosticsProbeRequest) -> Result<DiagnosticsProbeReport> {
    if request.evaluations.len() > 4 || request.warnings.len() > 4 || request.issues.len() > 4 {
        return Err(BridgeError::new("InvalidInput", "diagnostics probe failed validation: \
            at most four entries per list"));
    }
    let findings = canonical_findings();
    let report = report_of(request)?;
    let validation = report.validate();
    Ok(DiagnosticsProbeReport {
        report_valid: validation.is_ok(),
        report_error: validation.err().map(|error| error.to_string()).unwrap_or_default(),
        has_incumbent: report.has_incumbent(),
        is_optimal: report.is_optimal(),
        incumbent_present: report.incumbent().is_some(),
        has_incumbent_reads_only_the_presence: findings.has_incumbent_reads_only_the_presence,
        is_optimal_reads_only_the_presence: findings.is_optimal_reads_only_the_presence,
        the_predicates_never_validate_the_report:
            findings.the_predicates_never_validate_the_report,
        the_incumbent_reads_the_typed_value: findings.the_incumbent_reads_the_typed_value,
        the_diagnostics_are_never_validated: findings.the_diagnostics_are_never_validated,
        satisfied_and_violation_are_never_cross_checked:
            findings.satisfied_and_violation_are_never_cross_checked,
        the_bound_evaluation_flags_are_never_cross_checked:
            findings.the_bound_evaluation_flags_are_never_cross_checked,
        blank_warnings_and_issues_are_accepted: findings.blank_warnings_and_issues_are_accepted,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    has_incumbent_reads_only_the_presence: bool,
    is_optimal_reads_only_the_presence: bool,
    the_predicates_never_validate_the_report: bool,
    the_incumbent_reads_the_typed_value: bool,
    the_diagnostics_are_never_validated: bool,
    satisfied_and_violation_are_never_cross_checked: bool,
    the_bound_evaluation_flags_are_never_cross_checked: bool,
    blank_warnings_and_issues_are_accepted: bool,
}

/// 一份全默认的请求。 / A fully defaulted request.
fn plain() -> DiagnosticsProbeRequest {
    DiagnosticsProbeRequest { presence: None, has_solution: None, typed_value: None,
        evaluations: Vec::new(), warnings: Vec::new(), issues: Vec::new() }
}

/// 一条求值。 / One evaluation.
fn evaluation(satisfied: bool, violation: f64) -> EvaluationSpec {
    EvaluationSpec { satisfied, violation }
}

/// 一条消息。 / One message.
fn message(code: &str, body: &str) -> MessageSpec {
    MessageSpec { code: code.to_owned(), message: body.to_owned() }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let build = |request: DiagnosticsProbeRequest| report_of(&request).expect("report builds");
        let presence = |name: &str| DiagnosticsProbeRequest {
            presence: Some(name.to_owned()), ..plain() };
        // 1) has_incumbent 只看 presence。 / has_incumbent reads only the presence.
        let has_incumbent_reads_only_the_presence = {
            build(presence("Incumbent")).has_incumbent()
                && build(presence("Optimal")).has_incumbent()
                && !build(presence("None")).has_incumbent()
        };
        // 2) is_optimal 只认 Optimal。 / is_optimal only accepts Optimal.
        let is_optimal_reads_only_the_presence = {
            build(presence("Optimal")).is_optimal()
                && !build(presence("Incumbent")).is_optimal()
                && !build(presence("None")).is_optimal()
        };
        // 3) 谓词不校验报告。 / The predicates never validate.
        let the_predicates_never_validate_the_report = {
            // `presence: Incumbent` 配 `solution: None` 报告非法，但谓词照样为真。 
            // An `Incumbent` presence with no solution is illegal, yet the predicate still says true.
            let broken = build(DiagnosticsProbeRequest { has_solution: Some(false),
                ..presence("Incumbent") });
            // 两个方向都能矛盾：没有 solution 时谓词为真、`incumbent()` 为空；
            // `presence: None` 配 incumbent 时谓词为假、`incumbent()` 却有值。
            // Both directions contradict: without a solution the predicate is true while `incumbent()` is empty, and
            // with a `None` presence the predicate is false while `incumbent()` still yields a value.
            broken.validate().is_err() && broken.has_incumbent()
                && broken.incumbent().is_none()
                && { let orphan = build(DiagnosticsProbeRequest {
                        presence: Some("None".to_owned()), ..plain() });
                    orphan.validate().is_err() && !orphan.has_incumbent()
                        && orphan.incumbent().is_some() }
        };
        // 4) incumbent 读 solution.value。 / The incumbent reads the typed value.
        let the_incumbent_reads_the_typed_value = {
            let untyped = build(DiagnosticsProbeRequest { typed_value: Some(false), ..plain() });
            // 报告仍然合法（`objective_value` 在），但 `incumbent()` 是 None。 
            // The report stays legal since `objective_value` is present, yet `incumbent()` is none.
            untyped.validate().is_ok() && untyped.has_incumbent() && untyped.incumbent().is_none()
        };
        // 5) 诊断面不被校验。 / The diagnostics are never validated.
        let the_diagnostics_are_never_validated = {
            let absurd = build(DiagnosticsProbeRequest {
                evaluations: vec![evaluation(true, 1e9)],
                warnings: vec![message("", "")],
                issues: vec![message("  ", "   ")],
                ..plain() });
            absurd.validate().is_ok()
                && absurd.diagnostics.constraint_evaluations.len() == 1
                && absurd.diagnostics.issues.len() == 1
        };
        // 6) satisfied 与 violation 从不互相校验。 / satisfied is never cross-checked with violation.
        let satisfied_and_violation_are_never_cross_checked = {
            // 标记满足却违反一个亿；标记不满足却零违反。两者都合法。 
            // Satisfied with a huge violation, and unsatisfied with none: both are legal.
            let contradictory = build(DiagnosticsProbeRequest {
                evaluations: vec![evaluation(true, 1e8), evaluation(false, 0.0)], ..plain() });
            contradictory.validate().is_ok()
        };
        // 7) 边界求值的标记也不与上下界校验。 / The bound flags are not cross-checked either.
        let the_bound_evaluation_flags_are_never_cross_checked = {
            // 值 0 落在 [1, 2] 之外，却标成满足。 / A value of zero sits outside [1, 2] yet is flagged satisfied.
            let outside = build(DiagnosticsProbeRequest { evaluations: vec![evaluation(true, 0.0)],
                ..plain() });
            let bound = &outside.diagnostics.variable_bound_evaluations[0];
            outside.validate().is_ok() && bound.satisfied
                && (bound.value < bound.lower.unwrap() || bound.value > bound.upper.unwrap())
        };
        // 8) 空白 warning / issue 被接受。 / Blank warnings and issues are accepted.
        let blank_warnings_and_issues_are_accepted = {
            let blank = build(DiagnosticsProbeRequest { warnings: vec![message("", "")],
                issues: vec![message("", "")], ..plain() });
            blank.validate().is_ok()
                && blank.diagnostics.warnings[0].code.is_empty()
                && blank.diagnostics.issues[0].message.is_empty()
        };
        Findings {
            has_incumbent_reads_only_the_presence,
            is_optimal_reads_only_the_presence,
            the_predicates_never_validate_the_report,
            the_incumbent_reads_the_typed_value,
            the_diagnostics_are_never_validated,
            satisfied_and_violation_are_never_cross_checked,
            the_bound_evaluation_flags_are_never_cross_checked,
            blank_warnings_and_issues_are_accepted,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: DiagnosticsProbeRequest) -> DiagnosticsProbeReport {
        verify_diagnostics_probe(&request).unwrap()
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(plain());
        assert!(report.report_valid, "{}", report.report_error);
        assert!(report.has_incumbent);
        assert!(report.is_optimal);
        assert!(report.incumbent_present);
        assert!(report.has_incumbent_reads_only_the_presence);
        assert!(report.is_optimal_reads_only_the_presence);
        assert!(report.the_predicates_never_validate_the_report);
        assert!(report.the_incumbent_reads_the_typed_value);
        assert!(report.the_diagnostics_are_never_validated);
        assert!(report.satisfied_and_violation_are_never_cross_checked);
        assert!(report.the_bound_evaluation_flags_are_never_cross_checked);
        assert!(report.blank_warnings_and_issues_are_accepted);
    }

    #[test]
    fn the_predicates_read_only_the_presence() {
        let incumbent = ask(DiagnosticsProbeRequest { presence: Some("Incumbent".to_owned()),
            ..plain() });
        assert!(incumbent.has_incumbent);
        assert!(!incumbent.is_optimal);
        let none = ask(DiagnosticsProbeRequest { presence: Some("None".to_owned()),
            ..plain() });
        assert!(!none.has_incumbent);
        assert!(!none.is_optimal);
        // `None` 的 presence 配 incumbent 报告非法。 / A `None` presence with an incumbent is illegal.
        assert!(!none.report_valid);
    }

    #[test]
    fn the_predicates_never_validate() {
        let broken = ask(DiagnosticsProbeRequest { presence: Some("Incumbent".to_owned()),
            has_solution: Some(false), ..plain() });
        assert!(!broken.report_valid);
        assert!(broken.has_incumbent);
        assert!(!broken.incumbent_present);
    }

    #[test]
    fn the_incumbent_reads_the_typed_value() {
        let untyped = ask(DiagnosticsProbeRequest { typed_value: Some(false), ..plain() });
        assert!(untyped.report_valid);
        assert!(untyped.has_incumbent);
        assert!(!untyped.incumbent_present);
    }

    #[test]
    fn absurd_diagnostics_still_validate() {
        let absurd = ask(DiagnosticsProbeRequest {
            evaluations: vec![evaluation(true, 1e9)],
            warnings: vec![message("", "")],
            issues: vec![message("  ", "   ")], ..plain() });
        assert!(absurd.report_valid, "{}", absurd.report_error);
        let contradictory = ask(DiagnosticsProbeRequest {
            evaluations: vec![evaluation(true, 1e8), evaluation(false, 0.0)], ..plain() });
        assert!(contradictory.report_valid);
        let blank = ask(DiagnosticsProbeRequest { warnings: vec![message("", "")],
            issues: vec![message("", "")], ..plain() });
        assert!(blank.report_valid);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_diagnostics_probe(&DiagnosticsProbeRequest {
            evaluations: (0..5).map(|index| evaluation(index % 2 == 0, 0.0)).collect(),
            ..plain() }).unwrap_err().message.contains("at most four entries"));
        assert!(verify_diagnostics_probe(&DiagnosticsProbeRequest {
            presence: Some("Nearly".to_owned()), ..plain() }).unwrap_err()
            .message.contains("unknown solution presence"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(plain());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"hasIncumbent\""), "{json}");
        assert!(json.contains("\"incumbentPresent\""), "{json}");
        assert!(json.contains("\"thePredicatesNeverValidateTheReport\""), "{json}");
        assert!(!json.contains("has_incumbent"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<DiagnosticsProbeReport>(&copy.to_string()).is_err());
    }
}
