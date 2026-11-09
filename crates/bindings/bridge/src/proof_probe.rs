//! 续挖第 103 单：**统一报告的 `validate()` 把"证明"当成一组互相牵制的合同**——已核验的证明必须**同时**完整、
//! 可靠且以 `Completed` 收尾；`Optimal` 的 incumbent 存在性反过来要求**类型对、状态为 `Verified`、可靠、完整**
//! 的最优性证明；证明的 `None` 状态直接被拒；而 `require_optimal_lp_certificate` 这条**精确定价闸门**在报告全部
//! 合法之后还要**对偶解存在且非空**（`solver/report/mod.rs` 的 `SolveReport::validate` / `SolveProof`）
//! Continuation, delivery 103: **the unified report's `validate()` treats a proof as a web of mutual contracts** — a
//! verified proof must be **complete, reliable and finished with `Completed` all at once**; an `Optimal` incumbent in turn
//! demands an optimality proof that is **of the right kind, `Verified`, reliable and complete**; a proof whose status is
//! `None` is refused outright; and the **exact-pricing gate** `require_optimal_lp_certificate` additionally requires a
//! **present, non-empty dual solution** once the whole report is legal
//! (`SolveReport::validate` / `SolveProof` in `solver/report/mod.rs`).
//!
//! 第 102 单量的是证据权威性，这一单量**证明合同与精确定价闸门**。八条实测：
//!
//! 1. **四个构造器给出的证明都是 `Verified` + `Exact` + `Complete`**（`a_fresh_proof_is_verified_exact_and_complete()`），
//!    且不带 reference 与 evidence。
//! 2. **已核验的证明必须完整**（`a_verified_proof_must_be_complete()`）：`Verified` + `Partial` 被拒。
//! 3. **已核验的证明必须可靠**（`a_verified_proof_must_be_reliable()`）：`Heuristic` / `Unknown` 被拒。
//! 4. **已核验的证明必须以 `Completed` 收尾**（`a_verified_proof_needs_a_completed_termination()`）。
//! 5. **状态为 `None` 的证明直接被拒**（`a_none_status_proof_is_refused()`）。
//! 6. **`Optimal` 的 incumbent 需要已核验的最优性证明**（`an_optimal_presence_needs_a_verified_optimality_proof()`）：
//!    `Claimed` 不够，`Infeasibility` 类型也不够。
//! 7. **不可行证明要求不可行的题目状态**（`an_infeasibility_proof_needs_an_infeasible_status()`）。
//! 8. **精确定价闸门要一个非空对偶解**（`the_exact_pricing_gate_needs_a_non_empty_dual()`）：缺对偶、空对偶都被拒，
//!    有对偶才放行。
//!
//! Delivery 102 measured evidence authority; this one measures **the proof contracts and the exact-pricing gate**. **All
//! four constructors yield `Verified` plus `Exact` plus `Complete`** with no reference and no evidence. **A verified proof
//! must be complete**, **must be reliable**, and **must finish with `Completed`**. **A proof whose status is `None` is
//! refused.** **An `Optimal` incumbent needs a verified optimality proof**, where `Claimed` is not enough and the wrong
//! kind is not either. **An infeasibility proof demands an infeasible problem status.** And **the exact-pricing gate needs
//! a non-empty dual solution.**

use ospf_rust_core::solver::report::{
    require_optimal_lp_certificate, ProblemStatus, ProofCompleteness, ProofKind, ProofReliability,
    ProofStatus, SolveDiagnostics, SolveFingerprints, SolveProof, SolveReport, SolveSolution,
    SolveStatistics, SolveTrace, SolutionPresence, SolverProvenance, TerminationReason,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一份报告的描述。 / The description of one report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofSpec {
    /// 证明类型名（默认 `Optimality`）。 / The proof kind name.
    #[serde(default)]
    pub kind: Option<String>,
    /// 证明状态名（默认 `Verified`）。 / The proof status name.
    #[serde(default)]
    pub status: Option<String>,
    /// 可靠性名（默认 `Exact`）。 / The reliability name.
    #[serde(default)]
    pub reliability: Option<String>,
    /// 完整度名（默认 `Complete`）。 / The completeness name.
    #[serde(default)]
    pub completeness: Option<String>,
    /// 题目状态名（默认 `Feasible`）。 / The problem status name.
    #[serde(default)]
    pub problem: Option<String>,
    /// incumbent 存在性名（默认 `Optimal`）。 / The solution-presence name.
    #[serde(default)]
    pub presence: Option<String>,
    /// 收尾原因名（默认 `Completed`）。 / The termination name.
    #[serde(default)]
    pub termination: Option<String>,
    /// 是否带 incumbent（默认真）。 / Whether an incumbent is attached.
    #[serde(default)]
    pub has_solution: Option<bool>,
    /// 对偶解形状：`none` / `empty` / `one`（默认 `one`）。 / The dual shape.
    #[serde(default)]
    pub dual: Option<String>,
}

/// 证明探针请求。 / The proof probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofProbeRequest {
    /// 要校验的报告。 / The report to validate.
    pub report: ProofSpec,
}

/// 证明探针结论。 / The proof probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofProbeReport {
    /// `SolveReport::validate()` 是否通过。 / Whether the report validates.
    pub report_valid: bool,
    /// 报告校验的错误文本（通过时为空）。 / The validation error text, empty when it passes.
    pub report_error: String,
    /// 精确定价闸门是否放行。 / Whether the exact-pricing gate passes.
    pub gate_ok: bool,
    /// 闸门的错误文本（放行时为空）。 / The gate error text, empty when it passes.
    pub gate_error: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub a_fresh_proof_is_verified_exact_and_complete: bool,
    pub a_verified_proof_must_be_complete: bool,
    pub a_verified_proof_must_be_reliable: bool,
    pub a_verified_proof_needs_a_completed_termination: bool,
    pub a_none_status_proof_is_refused: bool,
    pub an_optimal_presence_needs_a_verified_optimality_proof: bool,
    pub an_infeasibility_proof_needs_an_infeasible_status: bool,
    pub the_exact_pricing_gate_needs_a_non_empty_dual: bool,
}

/// 按名字取证明类型。 / Resolve a proof kind by name.
fn kind_named(name: &str) -> Option<ProofKind> {
    match name {
        "Optimality" => Some(ProofKind::Optimality),
        "Infeasibility" => Some(ProofKind::Infeasibility),
        "Unboundedness" => Some(ProofKind::Unboundedness),
        "InfeasibleOrUnbounded" => Some(ProofKind::InfeasibleOrUnbounded),
        _ => None,
    }
}

/// 按名字取证明状态。 / Resolve a proof status by name.
fn status_named(name: &str) -> Option<ProofStatus> {
    match name {
        "None" => Some(ProofStatus::None),
        "Claimed" => Some(ProofStatus::Claimed),
        "Verified" => Some(ProofStatus::Verified),
        _ => None,
    }
}

/// 按名字取题目状态。 / Resolve a problem status by name.
fn problem_named(name: &str) -> Option<ProblemStatus> {
    match name {
        "Feasible" => Some(ProblemStatus::Feasible),
        "Infeasible" => Some(ProblemStatus::Infeasible),
        "Unbounded" => Some(ProblemStatus::Unbounded),
        "InfeasibleOrUnbounded" => Some(ProblemStatus::InfeasibleOrUnbounded),
        "Unknown" => Some(ProblemStatus::Unknown),
        _ => None,
    }
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

/// 按名字取收尾原因。 / Resolve a termination reason by name.
fn termination_named(name: &str) -> Option<TerminationReason> {
    match name {
        "Completed" => Some(TerminationReason::Completed),
        "TimeLimit" => Some(TerminationReason::TimeLimit),
        "NodeLimit" => Some(TerminationReason::NodeLimit),
        "IterationLimit" => Some(TerminationReason::IterationLimit),
        _ => None,
    }
}

/// 按名字取可靠性。 / Resolve a reliability by name.
fn reliability_named(name: &str) -> Option<ProofReliability> {
    match name {
        "Exact" => Some(ProofReliability::Exact),
        "Reliable" => Some(ProofReliability::Reliable),
        "Heuristic" => Some(ProofReliability::Heuristic),
        "Unknown" => Some(ProofReliability::Unknown),
        _ => None,
    }
}

/// 按名字取完整度。 / Resolve a completeness by name.
fn completeness_named(name: &str) -> Option<ProofCompleteness> {
    match name {
        "Complete" => Some(ProofCompleteness::Complete),
        "Partial" => Some(ProofCompleteness::Partial),
        "Unavailable" => Some(ProofCompleteness::Unavailable),
        _ => None,
    }
}

/// 造一个 incumbent。 / Build one incumbent.
fn solution_of(dual: Option<Vec<f64>>) -> SolveSolution<f64> {
    SolveSolution {
        value: Some(0.0),
        values: vec![0.0],
        stable_values: Default::default(),
        objective: Some(0.0),
        objective_value: Some(0.0),
        dual_solution: dual,
        quadratic_dual_solution: None,
        pool: Vec::new(),
    }
}

/// 造一份报告。 / Build one report.
fn report_of(spec: &ProofSpec) -> Result<SolveReport<f64>> {
    let invalid = |what: &str| BridgeError::new("InvalidInput",
        format!("proof probe failed validation: unknown {what}"));
    let proof = SolveProof {
        kind: match spec.kind.as_deref() {
            None => ProofKind::Optimality,
            Some(name) => kind_named(name).ok_or_else(|| invalid("proof kind"))?,
        },
        status: match spec.status.as_deref() {
            None => ProofStatus::Verified,
            Some(name) => status_named(name).ok_or_else(|| invalid("proof status"))?,
        },
        reliability: match spec.reliability.as_deref() {
            None => ProofReliability::Exact,
            Some(name) => reliability_named(name).ok_or_else(|| invalid("reliability"))?,
        },
        completeness: match spec.completeness.as_deref() {
            None => ProofCompleteness::Complete,
            Some(name) => completeness_named(name).ok_or_else(|| invalid("completeness"))?,
        },
        reference: None,
        evidence: None,
    };
    let dual = match spec.dual.as_deref() {
        None | Some("one") => Some(vec![1.0]),
        Some("none") => None,
        Some("empty") => Some(Vec::new()),
        Some(_) => return Err(invalid("dual shape")),
    };
    let has_solution = spec.has_solution.unwrap_or(true);
    Ok(SolveReport {
        schema_version: "proof-probe".to_owned(),
        problem_status: match spec.problem.as_deref() {
            None => ProblemStatus::Feasible,
            Some(name) => problem_named(name).ok_or_else(|| invalid("problem status"))?,
        },
        termination_reason: match spec.termination.as_deref() {
            None => TerminationReason::Completed,
            Some(name) => termination_named(name).ok_or_else(|| invalid("termination"))?,
        },
        solution_presence: match spec.presence.as_deref() {
            None => SolutionPresence::Optimal,
            Some(name) => presence_named(name).ok_or_else(|| invalid("solution presence"))?,
        },
        solution: if has_solution { Some(solution_of(dual)) } else { None },
        proof: Some(proof),
        statistics: SolveStatistics::default(),
        diagnostics: SolveDiagnostics::default(),
        provenance: SolverProvenance::default(),
        fingerprints: SolveFingerprints::default(),
        model_mapping: None,
        warnings: Vec::new(),
        trace: SolveTrace::default(),
    })
}

/// 走一遍证明探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_proof_probe(request: &ProofProbeRequest) -> Result<ProofProbeReport> {
    let findings = canonical_findings();
    let report = report_of(&request.report)?;
    let validation = report.validate();
    let gate = require_optimal_lp_certificate(&report);
    Ok(ProofProbeReport {
        report_valid: validation.is_ok(),
        report_error: validation.err().map(|error| error.to_string()).unwrap_or_default(),
        gate_ok: gate.is_ok(),
        gate_error: gate.err().map(|error| error.to_string()).unwrap_or_default(),
        a_fresh_proof_is_verified_exact_and_complete:
            findings.a_fresh_proof_is_verified_exact_and_complete,
        a_verified_proof_must_be_complete: findings.a_verified_proof_must_be_complete,
        a_verified_proof_must_be_reliable: findings.a_verified_proof_must_be_reliable,
        a_verified_proof_needs_a_completed_termination:
            findings.a_verified_proof_needs_a_completed_termination,
        a_none_status_proof_is_refused: findings.a_none_status_proof_is_refused,
        an_optimal_presence_needs_a_verified_optimality_proof:
            findings.an_optimal_presence_needs_a_verified_optimality_proof,
        an_infeasibility_proof_needs_an_infeasible_status:
            findings.an_infeasibility_proof_needs_an_infeasible_status,
        the_exact_pricing_gate_needs_a_non_empty_dual:
            findings.the_exact_pricing_gate_needs_a_non_empty_dual,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    a_fresh_proof_is_verified_exact_and_complete: bool,
    a_verified_proof_must_be_complete: bool,
    a_verified_proof_must_be_reliable: bool,
    a_verified_proof_needs_a_completed_termination: bool,
    a_none_status_proof_is_refused: bool,
    an_optimal_presence_needs_a_verified_optimality_proof: bool,
    an_infeasibility_proof_needs_an_infeasible_status: bool,
    the_exact_pricing_gate_needs_a_non_empty_dual: bool,
}

/// 一份全默认的请求。 / A fully defaulted request.
fn plain() -> ProofSpec {
    ProofSpec { kind: None, status: None, reliability: None, completeness: None, problem: None,
        presence: None, termination: None, has_solution: None, dual: None }
}

/// 按覆盖项造一份请求。 / Build a request from overrides.
fn spec_with(kind: Option<&str>, status: Option<&str>, reliability: Option<&str>,
    completeness: Option<&str>, problem: Option<&str>, presence: Option<&str>,
    termination: Option<&str>, has_solution: Option<bool>, dual: Option<&str>) -> ProofSpec
{
    ProofSpec {
        kind: kind.map(str::to_owned), status: status.map(str::to_owned),
        reliability: reliability.map(str::to_owned),
        completeness: completeness.map(str::to_owned), problem: problem.map(str::to_owned),
        presence: presence.map(str::to_owned), termination: termination.map(str::to_owned),
        has_solution, dual: dual.map(str::to_owned),
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 报告合法性与闸门放行的便捷判定。 / Convenience checks for validation and the gate.
        let valid = |spec: ProofSpec| {
            report_of(&spec).expect("report builds").validate().is_ok()
        };
        let gate = |spec: ProofSpec| {
            let report = report_of(&spec).expect("report builds");
            require_optimal_lp_certificate(&report).is_ok()
        };
        // 1) 四个构造器都是 Verified + Exact + Complete。 / All four constructors agree.
        let a_fresh_proof_is_verified_exact_and_complete = {
            [SolveProof::<f64>::optimality(), SolveProof::<f64>::infeasibility(),
                SolveProof::<f64>::unboundedness(), SolveProof::<f64>::infeasible_or_unbounded()]
                .iter().all(|proof| proof.status == ProofStatus::Verified
                    && proof.reliability == ProofReliability::Exact
                    && proof.completeness == ProofCompleteness::Complete
                    && proof.reference.is_none() && proof.evidence.is_none())
                // 四者的类型互不相同。 / The four kinds are pairwise distinct.
                && SolveProof::<f64>::optimality().kind == ProofKind::Optimality
                && SolveProof::<f64>::infeasibility().kind == ProofKind::Infeasibility
                && SolveProof::<f64>::unboundedness().kind == ProofKind::Unboundedness
                && SolveProof::<f64>::infeasible_or_unbounded().kind
                    == ProofKind::InfeasibleOrUnbounded
        };
        // 2) 已核验的证明必须完整。 / A verified proof must be complete.
        let a_verified_proof_must_be_complete = {
            valid(plain())
                && !valid(spec_with(None, None, None, Some("Partial"), None, None, None, None,
                    None))
                && !valid(spec_with(None, None, None, Some("Unavailable"), None, None, None,
                    None, None))
        };
        // 3) 已核验的证明必须可靠。 / A verified proof must be reliable.
        let a_verified_proof_must_be_reliable = {
            !valid(spec_with(None, None, Some("Heuristic"), None, None, None, None, None, None))
                && !valid(spec_with(None, None, Some("Unknown"), None, None, None, None, None,
                    None))
                // `Reliable` 与 `Exact` 都可以。 / Both reliable and exact are accepted.
                && valid(spec_with(None, None, Some("Reliable"), None, None, None, None, None,
                    None))
        };
        // 4) 已核验的证明必须以 Completed 收尾。 / A verified proof must finish with Completed.
        let a_verified_proof_needs_a_completed_termination = {
            !valid(spec_with(None, None, None, None, None, None, Some("TimeLimit"), None, None))
                && !valid(spec_with(None, None, None, None, None, None, Some("NodeLimit"), None,
                    None))
                // `Claimed` 状态就不受收尾原因约束。 
                // A `Claimed` proof is not bound by the termination reason.
                && valid(spec_with(None, Some("Claimed"), None, None, None, Some("Incumbent"),
                    Some("TimeLimit"), None, None))
        };
        // 5) None 状态的证明被拒。 / A proof whose status is None is refused.
        let a_none_status_proof_is_refused = {
            !valid(spec_with(None, Some("None"), None, None, None, Some("Incumbent"), None, None,
                None))
        };
        // 6) Optimal 的 incumbent 需要已核验的最优性证明。 / An optimal incumbent needs a verified proof.
        let an_optimal_presence_needs_a_verified_optimality_proof = {
            // `Claimed` 不够。 / Claimed is not enough.
            !valid(spec_with(None, Some("Claimed"), None, None, None, None, None, None, None))
                // 类型错也不够。 / The wrong kind is not enough either.
                && !valid(spec_with(Some("Infeasibility"), None, None, None, None, None, None,
                    None, None))
                // 没有 incumbent 也不够。 / Without an incumbent it is not enough either.
                && !valid(spec_with(None, None, None, None, None, None, None, Some(false), None))
                // 对照组：默认报告合法。 / Control: the default report validates.
                && valid(plain())
        };
        // 7) 不可行证明要求不可行的题目状态。 / An infeasibility proof needs an infeasible status.
        let an_infeasibility_proof_needs_an_infeasible_status = {
            !valid(spec_with(Some("Infeasibility"), None, None, None, None, None, None, None, None))
                && valid(spec_with(Some("Infeasibility"), None, None, None,
                    Some("Infeasible"), Some("None"), None, Some(false), None))
                // 无界证明同理。 / The same holds for an unboundedness proof.
                && valid(spec_with(Some("Unboundedness"), None, None, None, Some("Unbounded"),
                    Some("None"), None, Some(false), None))
        };
        // 8) 精确定价闸门要一个非空对偶解。 / The gate needs a non-empty dual.
        let the_exact_pricing_gate_needs_a_non_empty_dual = {
            // 缺对偶被拒。 / A missing dual is refused.
            let missing = report_of(&spec_with(None, None, None, None, None, None, None, None,
                Some("none"))).expect("report builds");
            // 空对偶也被拒。 / An empty dual is refused too.
            let empty = report_of(&spec_with(None, None, None, None, None, None, None, None,
                Some("empty"))).expect("report builds");
            missing.validate().is_ok() && empty.validate().is_ok()
                && require_optimal_lp_certificate(&missing).is_err()
                && require_optimal_lp_certificate(&empty).is_err()
                && gate(plain())
        };
        Findings {
            a_fresh_proof_is_verified_exact_and_complete,
            a_verified_proof_must_be_complete,
            a_verified_proof_must_be_reliable,
            a_verified_proof_needs_a_completed_termination,
            a_none_status_proof_is_refused,
            an_optimal_presence_needs_a_verified_optimality_proof,
            an_infeasibility_proof_needs_an_infeasible_status,
            the_exact_pricing_gate_needs_a_non_empty_dual,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(spec: ProofSpec) -> ProofProbeReport {
        verify_proof_probe(&ProofProbeRequest { report: spec }).unwrap()
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(plain());
        assert!(report.report_valid, "{}", report.report_error);
        assert!(report.gate_ok, "{}", report.gate_error);
        assert!(report.a_fresh_proof_is_verified_exact_and_complete);
        assert!(report.a_verified_proof_must_be_complete);
        assert!(report.a_verified_proof_must_be_reliable);
        assert!(report.a_verified_proof_needs_a_completed_termination);
        assert!(report.a_none_status_proof_is_refused);
        assert!(report.an_optimal_presence_needs_a_verified_optimality_proof);
        assert!(report.an_infeasibility_proof_needs_an_infeasible_status);
        assert!(report.the_exact_pricing_gate_needs_a_non_empty_dual);
    }

    #[test]
    fn an_incomplete_or_unreliable_verified_proof_is_refused() {
        let partial = ask(spec_with(None, None, None, Some("Partial"), None, None, None, None,
            None));
        assert!(!partial.report_valid);
        assert!(partial.report_error.contains("complete"), "{}", partial.report_error);
        let heuristic = ask(spec_with(None, None, Some("Heuristic"), None, None, None, None,
            None, None));
        assert!(!heuristic.report_valid);
        // `Reliable` 可以。 / Reliable is accepted.
        assert!(ask(spec_with(None, None, Some("Reliable"), None, None, None, None, None, None))
            .report_valid);
    }

    #[test]
    fn a_verified_proof_needs_a_completed_termination() {
        let limited = ask(spec_with(None, None, None, None, None, None, Some("TimeLimit"), None,
            None));
        assert!(!limited.report_valid);
        // `Claimed` 不受收尾原因约束。 / A claimed proof is not bound by it.
        assert!(ask(spec_with(None, Some("Claimed"), None, None, None, Some("Incumbent"),
            Some("TimeLimit"), None, None)).report_valid);
    }

    #[test]
    fn a_none_status_proof_is_refused() {
        let none = ask(spec_with(None, Some("None"), None, None, None, Some("Incumbent"), None,
            None, None));
        assert!(!none.report_valid);
        assert!(none.report_error.contains("Claimed or Verified"), "{}", none.report_error);
    }

    #[test]
    fn an_optimal_presence_needs_a_verified_optimality_proof() {
        assert!(!ask(spec_with(None, Some("Claimed"), None, None, None, None, None, None, None))
            .report_valid);
        assert!(!ask(spec_with(Some("Infeasibility"), None, None, None, None, None, None, None,
            None)).report_valid);
        assert!(!ask(spec_with(None, None, None, None, None, None, None, Some(false), None))
            .report_valid);
    }

    #[test]
    fn an_infeasibility_proof_needs_an_infeasible_status() {
        assert!(!ask(spec_with(Some("Infeasibility"), None, None, None, None, None, None, None,
            None)).report_valid);
        let honest = ask(spec_with(Some("Infeasibility"), None, None, None, Some("Infeasible"),
            Some("None"), None, Some(false), None));
        assert!(honest.report_valid, "{}", honest.report_error);
        // 不可行的报告不能带 incumbent。 / An infeasible report may not carry an incumbent.
        assert!(!ask(spec_with(Some("Infeasibility"), None, None, None, Some("Infeasible"),
            Some("None"), None, Some(true), None)).report_valid);
    }

    #[test]
    fn the_gate_needs_a_non_empty_dual() {
        let missing = ask(spec_with(None, None, None, None, None, None, None, None,
            Some("none")));
        assert!(missing.report_valid);
        assert!(!missing.gate_ok);
        assert!(missing.gate_error.contains("dual"), "{}", missing.gate_error);
        let empty = ask(spec_with(None, None, None, None, None, None, None, None,
            Some("empty")));
        assert!(empty.report_valid);
        assert!(!empty.gate_ok);
        assert!(empty.gate_error.contains("empty dual"), "{}", empty.gate_error);
    }

    #[test]
    fn bad_input_is_refused() {
        let bad = |spec: ProofSpec| verify_proof_probe(&ProofProbeRequest { report: spec })
            .unwrap_err().message;
        assert!(bad(spec_with(Some("Nearly"), None, None, None, None, None, None, None, None))
            .contains("unknown proof kind"));
        assert!(bad(spec_with(None, Some("Nearly"), None, None, None, None, None, None, None))
            .contains("unknown proof status"));
        assert!(bad(spec_with(None, None, None, None, Some("Nearly"), None, None, None, None))
            .contains("unknown problem status"));
        assert!(bad(spec_with(None, None, None, None, None, Some("Nearly"), None, None, None))
            .contains("unknown solution presence"));
        assert!(bad(spec_with(None, None, None, None, None, None, Some("Nearly"), None, None))
            .contains("unknown termination"));
        assert!(bad(spec_with(None, None, Some("Nearly"), None, None, None, None, None, None))
            .contains("unknown reliability"));
        assert!(bad(spec_with(None, None, None, Some("Nearly"), None, None, None, None, None))
            .contains("unknown completeness"));
        assert!(bad(spec_with(None, None, None, None, None, None, None, None, Some("two")))
            .contains("unknown dual shape"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(plain());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"reportValid\""), "{json}");
        assert!(json.contains("\"gateOk\""), "{json}");
        assert!(json.contains("\"theExactPricingGateNeedsANonEmptyDual\""), "{json}");
        assert!(!json.contains("report_valid"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ProofProbeReport>(&copy.to_string()).is_err());
    }
}
