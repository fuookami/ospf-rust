//! 续挖第 115 单：**native 终态证明只认"四条件同时成立"的那一种组合，其余一律什么都不写**——
//! `verify_native_terminal_proof` 的四条臂各自要求（native 状态、problem 状态、**incumbent 存在性**、`Completed` 终止）
//! 全部对上才会写证明；**不匹配的组合走 `_ => {}`**，既不写证明也**不直接报错**，把裁判权交给结尾的
//! `report.validate()`；重写证明时走 `..SolveProof::infeasibility()` 的写法，于是**旧证据会被搬到新证明里**；
//! `Optimal` 臂还会把 `solution_presence` 设成 `Optimal`
//! （`solver/report` 的 `verify_native_terminal_proof`）
//! Continuation, delivery 115: **the native terminal proof only recognises the one combination where four conditions hold at
//! once and writes nothing otherwise** — each of the four arms of `verify_native_terminal_proof` demands the native status,
//! the problem status, **whether an incumbent exists** and a `Completed` termination to line up; **a mismatched combination
//! hits `_ => {}`**, which neither writes a proof nor **errors directly**, leaving the verdict to the closing
//! `report.validate()`; a rewritten proof uses `..SolveProof::infeasibility()`, so **the old evidence is carried into the new
//! proof**; and the `Optimal` arm also sets `solution_presence` to `Optimal` (`verify_native_terminal_proof` in
//! `solver/report`).
//!
//! 第 114 单量的是谓词收集，这一单量**native 终态的证明门**。八条实测：
//!
//! 1. **`Optimal` 臂**：`Feasible` + 有 incumbent + `Completed` → 证明是 `Optimality`、presence 变 `Optimal`
//!    （`the_optimal_arm_writes_an_optimality_proof()`）。
//! 2. **`Infeasible` 臂**：`Infeasible` + 无 incumbent + `Completed` → 证明是 `Infeasibility`
//!    （`the_infeasible_arm_writes_an_infeasibility_proof()`）。
//! 3. **旧证据被搬过去**（`the_old_evidence_is_carried_over()`）。
//! 4. **`Unbounded` 臂**（`the_unbounded_arm_writes_an_unboundedness_proof()`）。
//! 5. **`InfeasibleOrUnbounded` 臂**（`the_infeasible_or_unbounded_arm_writes_its_proof()`）。
//! 6. **状态不匹配什么都不写**（`a_mismatched_status_writes_nothing()`）。
//! 7. **终止原因不是 `Completed` 什么都不写**（`a_non_completed_termination_writes_nothing()`）。
//! 8. **有 incumbent 时非 `Optimal` 臂什么都不写**（`an_incumbent_blocks_the_negative_arms()`）。
//!
//! Delivery 114 measured predicate collection; this one measures **the native terminal proof gate**. **The optimal arm writes
//! an optimality proof** and flips the presence. **The infeasible arm writes an infeasibility proof**, and **the old evidence
//! is carried over**. **The unbounded arm** and **the infeasible-or-unbounded arm** write their own proofs. **A mismatched
//! status writes nothing**, **a non-completed termination writes nothing**, and **an incumbent blocks the negative arms.**

use ospf_rust_core::solver::report::{
    verify_native_terminal_proof, ProofCompleteness, ProofKind, ProofReliability, ProofStatus,
    ProblemStatus, SolutionPresence, SolveDiagnostics, SolveFingerprints, SolveProof, SolveReport,
    SolveSolution, SolveStatistics, SolveTrace, SolverProvenance, TerminationReason,
};
use ospf_rust_core::solver::SolverStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// native 终态证明探针请求。 / The native terminal proof probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTerminalProofRequest {
    /// native 状态名（`Optimal` / `Infeasible` / `Unbounded` / `InfeasibleOrUnbounded`）。
    /// The native status name.
    pub native_status: String,
    /// problem 状态名（默认 `Feasible`）。 / The problem status name.
    #[serde(default)]
    pub problem_status: Option<String>,
    /// 是否存在 incumbent（默认 true）。 / Whether an incumbent exists.
    #[serde(default)]
    pub has_incumbent: Option<bool>,
    /// 终止原因名（默认 `Completed`）。 / The termination reason name.
    #[serde(default)]
    pub termination: Option<String>,
    /// 是否带旧证据（默认 false）。 / Whether the previous proof carries evidence.
    #[serde(default)]
    pub has_evidence: Option<bool>,
}

/// native 终态证明探针结论。 / The native terminal proof probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTerminalProofReport {
    /// 门是否通过（只看 `Ok`/`Err`）。 / Whether the gate passed.
    pub gate_ok: bool,
    /// 门的错误文本。 / The gate error text.
    pub gate_error: String,
    /// 写完后的证明种类名。 / The proof kind after the gate.
    pub proof_kind: String,
    /// 写完后的证明状态名。 / The proof status after the gate.
    pub proof_status: String,
    /// 写完后的 solution presence 名。 / The presence after the gate.
    pub solution_presence: String,
    /// 写完后的证据条数。 / The evidence entries after the gate.
    pub evidence_count: u32,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_optimal_arm_writes_an_optimality_proof: bool,
    pub the_infeasible_arm_writes_an_infeasibility_proof: bool,
    pub the_old_evidence_is_carried_over: bool,
    pub the_unbounded_arm_writes_an_unboundedness_proof: bool,
    pub the_infeasible_or_unbounded_arm_writes_its_proof: bool,
    pub a_mismatched_status_writes_nothing: bool,
    pub a_non_completed_termination_writes_nothing: bool,
    pub an_incumbent_blocks_the_negative_arms: bool,
}

/// 按名字取 native 状态。 / Resolve a native status by name.
fn native_named(name: &str) -> Option<SolverStatus> {
    match name {
        "Optimal" => Some(SolverStatus::Optimal),
        "Infeasible" => Some(SolverStatus::Infeasible),
        "Unbounded" => Some(SolverStatus::Unbounded),
        "InfeasibleOrUnbounded" => Some(SolverStatus::InfeasibleOrUnbounded),
        _ => None,
    }
}

/// 按名字取 problem 状态。 / Resolve a problem status by name.
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

/// 按名字取终止原因。 / Resolve a termination reason by name.
fn termination_named(name: &str) -> Option<TerminationReason> {
    match name {
        "Completed" => Some(TerminationReason::Completed),
        "TimeLimit" => Some(TerminationReason::TimeLimit),
        "IterationLimit" => Some(TerminationReason::IterationLimit),
        _ => None,
    }
}

/// 证明种类名。 / The proof kind name.
fn kind_name(kind: ProofKind) -> String {
    match kind {
        ProofKind::Optimality => "Optimality",
        ProofKind::Infeasibility => "Infeasibility",
        ProofKind::Unboundedness => "Unboundedness",
        ProofKind::InfeasibleOrUnbounded => "InfeasibleOrUnbounded",
    }.to_owned()
}

/// presence 名。 / The presence name.
fn presence_name(presence: SolutionPresence) -> String {
    match presence {
        SolutionPresence::Optimal => "Optimal",
        SolutionPresence::Incumbent => "Incumbent",
        SolutionPresence::None => "None",
        _ => "Other",
    }.to_owned()
}

/// 造一份带旧证明的报告。 / Build a report with a previous proof.
fn report_of(native_status: SolverStatus, problem_status: ProblemStatus, has_incumbent: bool,
    termination: TerminationReason, has_evidence: bool) -> SolveReport<f64>
{
    let has_solution = has_incumbent;
    // 数值证据是 `Vec<f64>`。 / The numeric evidence is a `Vec<f64>`.
    let evidence: Option<Vec<f64>> = if has_evidence { Some(vec![1.0]) } else { None };
    // 旧证明：故意用"还没被 native 门认过"的样子。 
    // The previous proof deliberately carries the state before the native gate.
    let previous = SolveProof {
        kind: match native_status {
            SolverStatus::Infeasible => ProofKind::Infeasibility,
            SolverStatus::Unbounded => ProofKind::Unboundedness,
            SolverStatus::InfeasibleOrUnbounded => ProofKind::InfeasibleOrUnbounded,
            _ => ProofKind::Optimality,
        },
        status: ProofStatus::Claimed,
        reliability: ProofReliability::Unknown,
        completeness: ProofCompleteness::Partial,
        reference: None,
        evidence,
    };
    SolveReport {
        schema_version: "native-terminal-proof-probe".to_owned(),
        problem_status,
        termination_reason: termination,
        solution_presence: if has_solution {
            SolutionPresence::Incumbent
        } else {
            SolutionPresence::None
        },
        solution: if has_solution {
            Some(SolveSolution {
                value: Some(0.0),
                values: vec![0.0],
                stable_values: Default::default(),
                objective: Some(0.0),
                objective_value: Some(0.0),
                dual_solution: None,
                quadratic_dual_solution: None,
                pool: Vec::new(),
            })
        } else {
            None
        },
        proof: Some(previous),
        statistics: SolveStatistics::default(),
        diagnostics: SolveDiagnostics::default(),
        provenance: SolverProvenance::default(),
        fingerprints: SolveFingerprints::default(),
        model_mapping: None,
        warnings: Vec::new(),
        trace: SolveTrace::default(),
    }
}

/// 走一遍 native 终态证明探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_native_terminal_proof_probe(
    request: &NativeTerminalProofRequest,
) -> Result<NativeTerminalProofReport> {
    let invalid = |what: &str| BridgeError::new("InvalidInput",
        format!("native terminal proof probe failed validation: unknown {what}"));
    let findings = canonical_findings();
    let native_status = native_named(&request.native_status).ok_or_else(|| invalid("native status"))?;
    let problem_status = match request.problem_status.as_deref() {
        None => ProblemStatus::Feasible,
        Some(name) => problem_named(name).ok_or_else(|| invalid("problem status"))?,
    };
    let termination = match request.termination.as_deref() {
        None => TerminationReason::Completed,
        Some(name) => termination_named(name).ok_or_else(|| invalid("termination"))?,
    };
    let has_incumbent = request.has_incumbent.unwrap_or(true);
    let has_evidence = request.has_evidence.unwrap_or(false);
    let mut report = report_of(native_status, problem_status, has_incumbent, termination,
        has_evidence);
    let gate = verify_native_terminal_proof(&mut report, native_status);
    let proof: SolveProof<f64> = report.proof.clone().unwrap_or_else(SolveProof::optimality);
    Ok(NativeTerminalProofReport {
        gate_ok: gate.is_ok(),
        gate_error: gate.err().map(|error| error.to_string()).unwrap_or_default(),
        proof_kind: kind_name(proof.kind),
        proof_status: match proof.status {
            ProofStatus::Verified => "Verified".to_owned(),
            ProofStatus::Claimed => "Claimed".to_owned(),
            _ => "Other".to_owned(),
        },
        solution_presence: presence_name(report.solution_presence),
        evidence_count: u32::try_from(proof.evidence.as_ref()
            .map(Vec::len).unwrap_or(0)).unwrap_or(u32::MAX),
        the_optimal_arm_writes_an_optimality_proof:
            findings.the_optimal_arm_writes_an_optimality_proof,
        the_infeasible_arm_writes_an_infeasibility_proof:
            findings.the_infeasible_arm_writes_an_infeasibility_proof,
        the_old_evidence_is_carried_over: findings.the_old_evidence_is_carried_over,
        the_unbounded_arm_writes_an_unboundedness_proof:
            findings.the_unbounded_arm_writes_an_unboundedness_proof,
        the_infeasible_or_unbounded_arm_writes_its_proof:
            findings.the_infeasible_or_unbounded_arm_writes_its_proof,
        a_mismatched_status_writes_nothing: findings.a_mismatched_status_writes_nothing,
        a_non_completed_termination_writes_nothing:
            findings.a_non_completed_termination_writes_nothing,
        an_incumbent_blocks_the_negative_arms: findings.an_incumbent_blocks_the_negative_arms,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_optimal_arm_writes_an_optimality_proof: bool,
    the_infeasible_arm_writes_an_infeasibility_proof: bool,
    the_old_evidence_is_carried_over: bool,
    the_unbounded_arm_writes_an_unboundedness_proof: bool,
    the_infeasible_or_unbounded_arm_writes_its_proof: bool,
    a_mismatched_status_writes_nothing: bool,
    a_non_completed_termination_writes_nothing: bool,
    an_incumbent_blocks_the_negative_arms: bool,
}

/// 跑一次门，返回（门的 Ok、写后的证明、写后的 presence）。
/// Run the gate once, returning the gate verdict, the proof and the presence afterwards.
fn gate(native_status: SolverStatus, problem_status: ProblemStatus, has_incumbent: bool,
    termination: TerminationReason, has_evidence: bool)
    -> (bool, SolveProof<f64>, SolutionPresence)
{
    let mut report = report_of(native_status, problem_status, has_incumbent, termination,
        has_evidence);
    let verdict = verify_native_terminal_proof(&mut report, native_status).is_ok();
    let proof: SolveProof<f64> = report.proof.clone().unwrap_or_else(SolveProof::optimality);
    (verdict, proof, report.solution_presence)
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) Optimal 臂。 / The optimal arm.
        let the_optimal_arm_writes_an_optimality_proof = {
            let (verdict, proof, presence) = gate(SolverStatus::Optimal, ProblemStatus::Feasible,
                true, TerminationReason::Completed, false);
            verdict && proof.kind == ProofKind::Optimality
                && proof.status == ProofStatus::Verified
                && presence == SolutionPresence::Optimal
        };
        // 2) Infeasible 臂。 / The infeasible arm.
        let the_infeasible_arm_writes_an_infeasibility_proof = {
            let (verdict, proof, _) = gate(SolverStatus::Infeasible, ProblemStatus::Infeasible,
                false, TerminationReason::Completed, false);
            verdict && proof.kind == ProofKind::Infeasibility
                && proof.status == ProofStatus::Verified
        };
        // 3) 旧证据被搬过去。 / The old evidence is carried over.
        let the_old_evidence_is_carried_over = {
            let (_, without, _) = gate(SolverStatus::Infeasible, ProblemStatus::Infeasible, false,
                TerminationReason::Completed, false);
            let (_, with, _) = gate(SolverStatus::Infeasible, ProblemStatus::Infeasible, false,
                TerminationReason::Completed, true);
            // 有旧证据时新证明也带着它。 / A previous evidence record survives into the new proof.
            without.evidence.is_none() && with.evidence.is_some()
        };
        // 4) Unbounded 臂。 / The unbounded arm.
        let the_unbounded_arm_writes_an_unboundedness_proof = {
            let (verdict, proof, _) = gate(SolverStatus::Unbounded, ProblemStatus::Unbounded,
                false, TerminationReason::Completed, false);
            verdict && proof.kind == ProofKind::Unboundedness
        };
        // 5) InfeasibleOrUnbounded 臂。 / The infeasible-or-unbounded arm.
        let the_infeasible_or_unbounded_arm_writes_its_proof = {
            let (verdict, proof, _) = gate(SolverStatus::InfeasibleOrUnbounded,
                ProblemStatus::InfeasibleOrUnbounded, false, TerminationReason::Completed, false);
            verdict && proof.kind == ProofKind::InfeasibleOrUnbounded
        };
        // 6) 状态不匹配什么都不写。 / A mismatched status writes nothing.
        let a_mismatched_status_writes_nothing = {
            // native 说 Optimal，problem 说 Infeasible：旧证明（Claimed）原样留下。 
            // The native status says optimal while the problem says infeasible: the old proof stays put.
            let (_, proof, _) = gate(SolverStatus::Optimal, ProblemStatus::Infeasible, true,
                TerminationReason::Completed, false);
            proof.kind == ProofKind::Optimality && proof.status != ProofStatus::Verified
        };
        // 7) 非 Completed 什么都不写。 / A non-completed termination writes nothing.
        let a_non_completed_termination_writes_nothing = {
            let (_, timed_out, _) = gate(SolverStatus::Infeasible, ProblemStatus::Infeasible,
                false, TerminationReason::TimeLimit, false);
            let (_, completed, _) = gate(SolverStatus::Infeasible, ProblemStatus::Infeasible,
                false, TerminationReason::Completed, false);
            timed_out.status != ProofStatus::Verified
                && completed.status == ProofStatus::Verified
        };
        // 8) incumbent 挡住负向臂。 / An incumbent blocks the negative arms.
        let an_incumbent_blocks_the_negative_arms = {
            let (_, infeasible, _) = gate(SolverStatus::Infeasible, ProblemStatus::Infeasible,
                true, TerminationReason::Completed, false);
            let (_, unbounded, _) = gate(SolverStatus::Unbounded, ProblemStatus::Unbounded,
                true, TerminationReason::Completed, false);
            let (_, infeasible_or_unbounded, _) = gate(SolverStatus::InfeasibleOrUnbounded,
                ProblemStatus::InfeasibleOrUnbounded, true, TerminationReason::Completed, false);
            infeasible.status != ProofStatus::Verified
                && unbounded.status != ProofStatus::Verified
                && infeasible_or_unbounded.status != ProofStatus::Verified
        };
        Findings {
            the_optimal_arm_writes_an_optimality_proof,
            the_infeasible_arm_writes_an_infeasibility_proof,
            the_old_evidence_is_carried_over,
            the_unbounded_arm_writes_an_unboundedness_proof,
            the_infeasible_or_unbounded_arm_writes_its_proof,
            a_mismatched_status_writes_nothing,
            a_non_completed_termination_writes_nothing,
            an_incumbent_blocks_the_negative_arms,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: NativeTerminalProofRequest) -> NativeTerminalProofReport {
        verify_native_terminal_proof_probe(&request).unwrap()
    }

    fn request(native: &str) -> NativeTerminalProofRequest {
        NativeTerminalProofRequest { native_status: native.to_owned(), problem_status: None,
            has_incumbent: None, termination: None, has_evidence: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request("Optimal"));
        assert!(report.gate_ok, "{}", report.gate_error);
        assert_eq!(report.proof_kind, "Optimality");
        assert_eq!(report.proof_status, "Verified");
        assert_eq!(report.solution_presence, "Optimal");
        assert_eq!(report.evidence_count, 0);
        assert!(report.the_optimal_arm_writes_an_optimality_proof);
        assert!(report.the_infeasible_arm_writes_an_infeasibility_proof);
        assert!(report.the_old_evidence_is_carried_over);
        assert!(report.the_unbounded_arm_writes_an_unboundedness_proof);
        assert!(report.the_infeasible_or_unbounded_arm_writes_its_proof);
        assert!(report.a_mismatched_status_writes_nothing);
        assert!(report.a_non_completed_termination_writes_nothing);
        assert!(report.an_incumbent_blocks_the_negative_arms);
    }

    #[test]
    fn every_arm_writes_its_own_kind() {
        for (native, problem, incumbent, expected) in [
            ("Optimal", "Feasible", true, "Optimality"),
            ("Infeasible", "Infeasible", false, "Infeasibility"),
            ("Unbounded", "Unbounded", false, "Unboundedness"),
            ("InfeasibleOrUnbounded", "InfeasibleOrUnbounded", false, "InfeasibleOrUnbounded"),
        ] {
            let report = ask(NativeTerminalProofRequest {
                problem_status: Some(problem.to_owned()),
                has_incumbent: Some(incumbent), ..request(native) });
            assert_eq!(report.proof_kind, expected, "{native}");
            assert_eq!(report.proof_status, "Verified", "{native}");
        }
    }

    #[test]
    fn the_evidence_survives_the_rewrite() {
        let report = ask(NativeTerminalProofRequest { problem_status: Some("Infeasible".to_owned()),
            has_incumbent: Some(false), has_evidence: Some(true), ..request("Infeasible") });
        assert_eq!(report.proof_kind, "Infeasibility");
        assert_eq!(report.evidence_count, 1);
        let bare = ask(NativeTerminalProofRequest { problem_status: Some("Infeasible".to_owned()),
            has_incumbent: Some(false), ..request("Infeasible") });
        assert_eq!(bare.evidence_count, 0);
    }

    #[test]
    fn a_mismatch_or_a_bad_termination_writes_nothing() {
        // native 说 Optimal、problem 说 Infeasible → 旧证明原样（Claimed）。 
        // Native optimal against an infeasible problem leaves the old claimed proof.
        let mismatched = ask(NativeTerminalProofRequest {
            problem_status: Some("Infeasible".to_owned()), ..request("Optimal") });
        assert_eq!(mismatched.proof_status, "Claimed");
        // 非 Completed 终止同理。 / A non-completed termination behaves the same way.
        let timed_out = ask(NativeTerminalProofRequest {
            problem_status: Some("Infeasible".to_owned()), has_incumbent: Some(false),
            termination: Some("TimeLimit".to_owned()), ..request("Infeasible") });
        assert_eq!(timed_out.proof_status, "Claimed");
    }

    #[test]
    fn an_incumbent_blocks_every_negative_arm() {
        for native in ["Infeasible", "Unbounded", "InfeasibleOrUnbounded"] {
            let problem = native.to_owned();
            let report = ask(NativeTerminalProofRequest { problem_status: Some(problem),
                has_incumbent: Some(true), ..request(native) });
            assert_eq!(report.proof_status, "Claimed", "{native}");
        }
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_native_terminal_proof_probe(&NativeTerminalProofRequest {
            native_status: "Nearly".to_owned(), ..request("Optimal") }).unwrap_err()
            .message.contains("unknown native status"));
        assert!(verify_native_terminal_proof_probe(&NativeTerminalProofRequest {
            problem_status: Some("Nearly".to_owned()), ..request("Optimal") }).unwrap_err()
            .message.contains("unknown problem status"));
        assert!(verify_native_terminal_proof_probe(&NativeTerminalProofRequest {
            termination: Some("Nearly".to_owned()), ..request("Optimal") }).unwrap_err()
            .message.contains("unknown termination"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request("Optimal"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"gateOk\""), "{json}");
        assert!(json.contains("\"proofKind\""), "{json}");
        assert!(json.contains("\"theOldEvidenceIsCarriedOver\""), "{json}");
        assert!(!json.contains("gate_ok"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<NativeTerminalProofReport>(&copy.to_string()).is_err());
    }
}
