//! 续挖第 68 单：CP subproblem 四态结果的**构造闸门**——`failed` 从不校验、只有 snapshot 入口发证明、
//! `incomplete` 连"最优"都拒（`solver/logic_based_benders.rs` 的 `ConstraintProgrammingSubproblemResult`）
//! Continuation, delivery 68: the **construction gates** of the CP subproblem's four-state result — `failed` never
//! validates, only the snapshot constructors issue proofs, and `incomplete` refuses even an optimal report
//! (`ConstraintProgrammingSubproblemResult` in `solver/logic_based_benders.rs`).
//!
//! §11.14 点名 `logic_based_benders.rs` 剩余的纯数据面。第 59 单量过 `stable_value` 提取、第 60 单量过 master
//! assignment 抽取，这一单量**结果四态本身**：六个构造器各自的闸门与它们留下的可观测痕迹。八条实测：
//!
//! 1. **`failed` 从不校验**（`failed_never_validates_anything()`）：任意 code / message 都收，`report()` 为
//!    `None`，`model_fingerprint()` 也是 `None`——它连"报告"都没有，自然没有指纹。
//! 2. **只有 snapshot 入口发证明**（`only_the_snapshot_constructors_issue_proofs()`）：`feasible()` 走完
//!    `assignment_proof` 仍是 `None`，于是 `model_fingerprint()` 为 `None`；`feasible_from_snapshot` 才有。
//! 3. **`feasible` 要 `Feasible` 状态，有解也不行**（`feasible_demands_the_feasible_status()`）：
//!    `Unknown` 携带完整 solution 仍被拒——判据是 `problem_status == Feasible && has_incumbent()`。
//! 4. **`infeasible` 要真证书**（`infeasible_demands_a_genuine_certificate()`）：`Infeasible` 状态但没 proof
//!    被拒；`Unknown` 即便挂了 infeasibility proof 也被拒。
//! 5. **冲突引用必须与报告引用逐字相等**（`the_conflict_reference_must_match_the_report()`）。
//! 6. **`incomplete` 连 `is_optimal()` 都拒**（`incomplete_refuses_even_an_optimal_report()`）：四个终态
//!    （不可行/无界/二者/最优）全被挡。
//! 7. **`incomplete` 的 incumbent 有两条值域闸门**（`the_incomplete_incumbent_has_two_value_gates()`）：
//!    `i64::MIN` 单值被拒、非有限 objective 被拒；而普通负值、`i64::MAX`、`0` 都收。
//! 8. **`feasible` 的 assignment 是报告的副本**（`the_feasible_assignment_is_a_copy_of_the_report()`）：
//!    构造器从 `solution.stable_values` 拷一份，不重算、不排序。
//!
//! The §11.14 audit names the remaining pure data of `logic_based_benders.rs`. Delivery 59 measured `stable_value`
//! extraction and delivery 60 master-assignment extraction; this one measures **the four-state result itself**: each
//! constructor's gate and the trace it leaves. **`failed` never validates anything** — any code/message is accepted, and both
//! `report()` and `model_fingerprint()` are `None`. **Only the snapshot constructors issue proofs**: `feasible()` leaves
//! `assignment_proof` `None`, so `model_fingerprint()` stays `None`. **`feasible` demands the `Feasible` status** — an
//! `Unknown` report carrying a complete solution is still refused. **`infeasible` demands a genuine certificate.** **The
//! conflict reference must match the report's verbatim.** **`incomplete` refuses even an optimal report.** **The incomplete
//! incumbent has two value gates** (`i64::MIN` and a non-finite objective). And **the feasible assignment is a copy of the
//! report's**, never recomputed or reordered.

use std::collections::BTreeMap;

use ospf_rust_core::model::constraint_programming::{
    ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
    ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerRelation,
    IntegerVariable,
};
use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_core::solver::{
    ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason,
};
use ospf_rust_framework::solver::logic_based_benders::{
    ConstraintProgrammingSubproblemIncumbent, ConstraintProgrammingSubproblemResult,
    VerifiedMasterConflict,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条稳定变量取值。 / One stable-variable value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubproblemValueSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 整数值。 / The integer value.
    pub value: i64,
}

/// subproblem 结果四态探针请求。 / The four-state subproblem probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubproblemResultRequest {
    /// 走哪个构造器：`feasible` / `feasibleFromSnapshot` / `infeasible` / `infeasibleFromSnapshot` /
    /// `incomplete` / `failed`。
    /// Which constructor to walk.
    pub variant: String,
    /// 报告状态名（`Feasible` / `Infeasible` / `Unknown` / `Optimal` 之类）。 / The report status name.
    #[serde(default)]
    pub status: Option<String>,
    /// assignment。 / The assignment.
    #[serde(default)]
    pub assignment: Vec<SubproblemValueSpec>,
    /// 报告是否带 solution（默认带）。 / Whether the report carries a solution (default true).
    #[serde(default)]
    pub with_solution: Option<bool>,
    /// 报告是否带 infeasibility proof。 / Whether the report carries an infeasibility proof.
    #[serde(default)]
    pub with_proof: Option<bool>,
    /// 报告 proof 的引用。 / The report proof's reference.
    #[serde(default)]
    pub proof_reference: Option<String>,
    /// conflict 成员（空表示不给 conflict）。 / Conflict members (empty means no conflict).
    #[serde(default)]
    pub conflict_members: Vec<String>,
    /// conflict 自己的引用。 / The conflict's own reference.
    #[serde(default)]
    pub conflict_reference: Option<String>,
    /// incumbent 的 objective。 / The incumbent objective.
    #[serde(default)]
    pub incumbent_objective: Option<f64>,
    /// `failed` 的 code / message。 / The `failed` code and message.
    #[serde(default)]
    pub issue_code: Option<String>,
    #[serde(default)]
    pub issue_message: Option<String>,
}

/// subproblem 结果四态探针结论。 / The four-state subproblem probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubproblemResultReport {
    /// 请求走的构造器。 / The constructor that was walked.
    pub variant: String,
    /// 构造是否成功。 / Whether construction succeeded.
    pub accepted: bool,
    /// 被拒时的错误文案。 / The refusal message.
    pub refusal: Option<String>,
    /// 成功时的状态名（`Failed` 记 `Failed`）。 / The state name on success.
    pub state: Option<String>,
    /// assignment 条数。 / The assignment size.
    pub assignment_size: Option<u64>,
    /// assignment 的稳定 ID 顺序。 / The assignment's stable-ID order.
    pub assignment_ids: Vec<String>,
    /// `model_fingerprint()` 是否有值。 / Whether `model_fingerprint()` has a value.
    pub has_model_fingerprint: bool,
    /// 报告是否存在。 / Whether a report is present.
    pub has_report: bool,
    /// 报告状态名（若有）。 / The report status name, when present.
    pub report_status: Option<String>,
    /// `failed` 的 code / message。 / The `failed` code and message.
    pub issue_code: Option<String>,
    pub issue_message: Option<String>,
    /// 这六条规则属性的读数。 / The six rule readings.
    pub failed_never_validates_anything: bool,
    pub only_the_snapshot_constructors_issue_proofs: bool,
    pub feasible_demands_the_feasible_status: bool,
    pub infeasible_demands_a_genuine_certificate: bool,
    pub the_conflict_reference_must_match_the_report: bool,
    pub incomplete_refuses_even_an_optimal_report: bool,
    pub the_incomplete_incumbent_has_two_value_gates: bool,
    pub the_feasible_assignment_is_a_copy_of_the_report: bool,
}

/// 状态名。 / A status name.
fn status_of(name: &str) -> Result<ProblemStatus> {
    Ok(match name {
        "Feasible" => ProblemStatus::Feasible,
        "Infeasible" => ProblemStatus::Infeasible,
        "Unbounded" => ProblemStatus::Unbounded,
        "InfeasibleOrUnbounded" => ProblemStatus::InfeasibleOrUnbounded,
        "Unknown" => ProblemStatus::Unknown,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "subproblem result failed validation: unknown status `{other}`"))),
    })
}

/// 状态名（反向）。 / The status name, reversed.
fn status_name(status: ProblemStatus) -> String {
    match status {
        ProblemStatus::Feasible => "Feasible",
        ProblemStatus::Infeasible => "Infeasible",
        ProblemStatus::Unbounded => "Unbounded",
        ProblemStatus::InfeasibleOrUnbounded => "InfeasibleOrUnbounded",
        ProblemStatus::Unknown => "Unknown",
    }
    .to_owned()
}

/// 造一份满足 satisfaction 的 snapshot。 / Build a satisfaction snapshot.
fn snapshot_of() -> ConstraintProgrammingSnapshot {
    let x = IntegerVariable::new("x");
    let mut model = ConstraintProgrammingModel::new("subproblem-probe");
    model.register_variable(x.clone(), IntegerDomain::range(0, 5).expect("domain"))
        .expect("variable");
    model.add_constraint(ConstraintDefinition::new("capacity",
        ConstraintProgrammingConstraint::integer(IntegerExpression::variable(x.clone()),
            IntegerRelation::LessOrEqual, 5))).expect("constraint");
    // 不带目标 → satisfaction 快照：报告也不能带 objective，正好只量"证明标记"这一件事。 
    // No objective, so a satisfaction snapshot: the report must carry none either, which isolates the proof marker.
    model.freeze().expect("snapshot")
}

/// 造一份报告。 / Build a report.
fn report_of(request: &SubproblemResultRequest,
    model: Option<&ospf_rust_core::solver::ModelFingerprint>) -> Result<SolveReport<i64>> {
    let status = match request.status.as_deref() {
        Some(name) => status_of(name)?,
        None => ProblemStatus::Feasible,
    };
    let mut builder = SolveReport::builder(status, TerminationReason::Completed);
    if request.with_solution.unwrap_or(true) {
        let mut solution = SolveSolution::<i64>::vector(Vec::new());
        solution.stable_values = request.assignment.iter()
            .map(|spec| (StableVariableId(spec.id.clone()), spec.value)).collect();
        builder = builder.solution(solution);
    }
    if request.with_proof.unwrap_or(false) {
        let mut proof = SolveProof::infeasibility();
        proof.reference = request.proof_reference.clone();
        builder = builder.proof(proof);
    }
    if let Some(model) = model {
        builder = builder.fingerprints(SolveFingerprints { model: Some(model.clone()),
            ..Default::default() });
    }
    // 报告自身不合法就**如实报错**，不降级成 `Unknown`——否则"四态闸门"会拿一份伪造的报告去测。 
    // A report that is itself invalid errors out instead of degrading to `Unknown`; otherwise the four-state gates
    // would be probed with a forged report.
    builder.build().map_err(|error| BridgeError::new("InvalidInput", &format!(
        "subproblem result failed validation: the probe report is invalid: {error}")))
}

/// 走一遍四态探针。非法输入是**错误**。 / Walk the four-state probe.
pub fn verify_subproblem_result(request: &SubproblemResultRequest) -> Result<SubproblemResultReport> {
    if request.assignment.len() > 16 || request.conflict_members.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "subproblem result failed validation: at most 16 entries are accepted"));
    }
    if request.assignment.iter().any(|spec| spec.id.trim().is_empty()) {
        return Err(BridgeError::new("InvalidInput",
            "subproblem result failed validation: stable IDs must not be blank"));
    }
    let snapshot = matches!(request.variant.as_str(), "feasibleFromSnapshot"
        | "infeasibleFromSnapshot").then(snapshot_of);
    let model = snapshot.as_ref().map(|value| value.fingerprint.clone());
    let report = report_of(request, model.as_ref())?;
    // conflict 的构造错误**不**提前中断：它本身就是一条"被拒"的观测（空成员集会被 `new` 拒）。 
    // A conflict construction error does **not** short-circuit: it is itself a refusal observation (an empty
    // member set is refused by `new`).
    let conflict = if request.conflict_members.is_empty() { Ok(None) } else {
        VerifiedMasterConflict::new(request.conflict_members.iter()
            .map(|id| StableVariableId(id.clone())),
            request.conflict_reference.clone().unwrap_or_default())
            .map(Some).map_err(|error| error.to_string())
    };
    let outcome: std::result::Result<ConstraintProgrammingSubproblemResult, String> = match &conflict {
        Err(refusal) => Err(refusal.clone()),
        Ok(conflict) => match request.variant.as_str() {
            "feasible" => ConstraintProgrammingSubproblemResult::feasible(report)
                .map_err(|error| error.to_string()),
            "feasibleFromSnapshot" => ConstraintProgrammingSubproblemResult::feasible_from_snapshot(
                report, snapshot.as_ref().expect("snapshot"), None, None)
                .map_err(|error| error.to_string()),
            "infeasible" => ConstraintProgrammingSubproblemResult::infeasible(report,
                conflict.clone()).map_err(|error| error.to_string()),
            "infeasibleFromSnapshot" =>
                ConstraintProgrammingSubproblemResult::infeasible_from_snapshot(report,
                    snapshot.as_ref().expect("snapshot"), conflict.clone())
                    .map_err(|error| error.to_string()),
            "incomplete" => ConstraintProgrammingSubproblemResult::incomplete(report,
                Some(ConstraintProgrammingSubproblemIncumbent {
                    assignment: request.assignment.iter()
                        .map(|spec| (StableVariableId(spec.id.clone()), spec.value)).collect(),
                    objective: request.incumbent_objective }))
                .map_err(|error| error.to_string()),
            "failed" => Ok(ConstraintProgrammingSubproblemResult::failed(
                request.issue_code.clone().unwrap_or_default(),
                request.issue_message.clone().unwrap_or_default())),
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "subproblem result failed validation: unknown variant `{other}`"))),
        },
    };
    let findings = canonical_findings();
    match outcome {
        Ok(result) => {
            let (state, assignment_size, assignment_ids, has_report, report_status, code, message) =
                describe(&result);
            Ok(SubproblemResultReport { variant: request.variant.clone(), accepted: true,
                refusal: None, state: Some(state), assignment_size, assignment_ids,
                has_model_fingerprint: result_fingerprint_present(&result), has_report,
                report_status, issue_code: code, issue_message: message,
                failed_never_validates_anything: findings.failed_never_validates_anything,
                only_the_snapshot_constructors_issue_proofs:
                    findings.only_the_snapshot_constructors_issue_proofs,
                feasible_demands_the_feasible_status: findings.feasible_demands_the_feasible_status,
                infeasible_demands_a_genuine_certificate:
                    findings.infeasible_demands_a_genuine_certificate,
                the_conflict_reference_must_match_the_report:
                    findings.the_conflict_reference_must_match_the_report,
                incomplete_refuses_even_an_optimal_report:
                    findings.incomplete_refuses_even_an_optimal_report,
                the_incomplete_incumbent_has_two_value_gates:
                    findings.the_incomplete_incumbent_has_two_value_gates,
                the_feasible_assignment_is_a_copy_of_the_report:
                    findings.the_feasible_assignment_is_a_copy_of_the_report })
        }
        Err(refusal) => Ok(SubproblemResultReport { variant: request.variant.clone(),
            accepted: false, refusal: Some(refusal), state: None, assignment_size: None,
            assignment_ids: Vec::new(), has_model_fingerprint: false, has_report: false,
            report_status: None, issue_code: None, issue_message: None,
            failed_never_validates_anything: findings.failed_never_validates_anything,
            only_the_snapshot_constructors_issue_proofs:
                findings.only_the_snapshot_constructors_issue_proofs,
            feasible_demands_the_feasible_status: findings.feasible_demands_the_feasible_status,
            infeasible_demands_a_genuine_certificate:
                findings.infeasible_demands_a_genuine_certificate,
            the_conflict_reference_must_match_the_report:
                findings.the_conflict_reference_must_match_the_report,
            incomplete_refuses_even_an_optimal_report:
                findings.incomplete_refuses_even_an_optimal_report,
            the_incomplete_incumbent_has_two_value_gates:
                findings.the_incomplete_incumbent_has_two_value_gates,
            the_feasible_assignment_is_a_copy_of_the_report:
                findings.the_feasible_assignment_is_a_copy_of_the_report }),
    }
}

/// 读出状态与载荷。 / Read the state and payload.
fn describe(result: &ConstraintProgrammingSubproblemResult)
    -> (String, Option<u64>, Vec<String>, bool, Option<String>, Option<String>, Option<String>)
{
    match result {
        ConstraintProgrammingSubproblemResult::Feasible { report, assignment, .. } => (
            "Feasible".to_owned(), Some(assignment.len() as u64),
            assignment.keys().map(|id| id.0.clone()).collect(), true,
            Some(status_name(report.problem_status)), None, None),
        ConstraintProgrammingSubproblemResult::Infeasible { report, .. } => (
            "Infeasible".to_owned(), None, Vec::new(), true,
            Some(status_name(report.problem_status)), None, None),
        ConstraintProgrammingSubproblemResult::Incomplete { report, incumbent } => (
            "Incomplete".to_owned(), incumbent.as_ref().map(|value| value.assignment.len() as u64),
            incumbent.as_ref().map(|value| value.assignment.keys().map(|id| id.0.clone())
                .collect()).unwrap_or_default(), true,
            Some(status_name(report.problem_status)), None, None),
        ConstraintProgrammingSubproblemResult::Failed { issue } => (
            "Failed".to_owned(), None, Vec::new(), false, None,
            Some(issue.code.clone()), Some(issue.message.clone())),
    }
}

/// `model_fingerprint()` 是否有值。 / Whether `model_fingerprint()` has a value.
fn result_fingerprint_present(result: &ConstraintProgrammingSubproblemResult) -> bool {
    match result {
        ConstraintProgrammingSubproblemResult::Feasible { assignment_proof, .. } => {
            assignment_proof.is_some()
        }
        ConstraintProgrammingSubproblemResult::Infeasible { snapshot_proof, .. } => {
            snapshot_proof.is_some()
        }
        ConstraintProgrammingSubproblemResult::Incomplete { .. }
        | ConstraintProgrammingSubproblemResult::Failed { .. } => false,
    }
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    failed_never_validates_anything: bool,
    only_the_snapshot_constructors_issue_proofs: bool,
    feasible_demands_the_feasible_status: bool,
    infeasible_demands_a_genuine_certificate: bool,
    the_conflict_reference_must_match_the_report: bool,
    incomplete_refuses_even_an_optimal_report: bool,
    the_incomplete_incumbent_has_two_value_gates: bool,
    the_feasible_assignment_is_a_copy_of_the_report: bool,
}

/// 一份带完整 solution 的可行报告。 / A feasible report with a full solution.
fn feasible_report() -> SolveReport<i64> {
    let mut solution = SolveSolution::<i64>::vector(Vec::new());
    solution.stable_values = BTreeMap::from([(StableVariableId("x".to_owned()), 3)]);
    SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
        .solution(solution).build().expect("feasible report")
}

/// 一份带证书的不可行报告。 / An infeasible report with a certificate.
fn infeasible_report(reference: Option<&str>) -> SolveReport<i64> {
    let mut proof = SolveProof::infeasibility();
    proof.reference = reference.map(|value| value.to_owned());
    SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
        .proof(proof).build().expect("infeasible report")
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `failed` 从不校验。 / `failed` never validates anything.
    let failed_never_validates_anything = {
        let blank = ConstraintProgrammingSubproblemResult::failed("", "");
        let anything = ConstraintProgrammingSubproblemResult::failed("anything", "at all");
        matches!(blank, ConstraintProgrammingSubproblemResult::Failed { .. })
            && matches!(anything, ConstraintProgrammingSubproblemResult::Failed { .. })
    };
    // 2) 只有 snapshot 入口发证明。 / Only the snapshot constructors issue proofs.
    let only_the_snapshot_constructors_issue_proofs = {
        let plain = ConstraintProgrammingSubproblemResult::feasible(feasible_report())
            .expect("plain feasible");
        let snapshot = snapshot_of();
        let mut solution = SolveSolution::<i64>::vector(Vec::new());
        solution.stable_values = BTreeMap::from([(StableVariableId("x".to_owned()), 3)]);
        let bound = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
            .solution(solution)
            .fingerprints(SolveFingerprints { model: Some(snapshot.fingerprint.clone()),
                ..Default::default() }).build().expect("bound report");
        let verified = ConstraintProgrammingSubproblemResult::feasible_from_snapshot(bound,
            &snapshot, None, None).expect("verified feasible");
        // 朴素入口没有证明；snapshot 入口有。 / The plain entry has none; the snapshot entry does.
        matches!(&plain, ConstraintProgrammingSubproblemResult::Feasible {
            assignment_proof: None, .. })
            && matches!(&verified, ConstraintProgrammingSubproblemResult::Feasible {
                assignment_proof: Some(_), .. })
    };
    // 3) `feasible` 要 `Feasible` 状态。 / `feasible` demands the `Feasible` status.
    let feasible_demands_the_feasible_status = {
        let mut solution = SolveSolution::<i64>::vector(Vec::new());
        solution.stable_values = BTreeMap::from([(StableVariableId("x".to_owned()), 3)]);
        let unknown = SolveReport::builder(ProblemStatus::Unknown, TerminationReason::Completed)
            .solution(solution).build().expect("unknown report");
        // 有完整解也不行：判据还要求 `Feasible`。 / Even a complete solution is refused: the status must be Feasible.
        ConstraintProgrammingSubproblemResult::feasible(unknown).is_err()
            && ConstraintProgrammingSubproblemResult::feasible(feasible_report()).is_ok()
    };
    // 4) `infeasible` 要真证书。 / `infeasible` demands a genuine certificate.
    let infeasible_demands_a_genuine_certificate = {
        let bare = SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
            .build().expect("bare infeasible");
        // **报告层先拒**：`Unknown` 状态挂不可行证明连 `build()` 都过不去，所以"假证书"根本到不了
        // 结果层——这条闸门在更下面一层。 
        // **The report layer refuses first**: an `Unknown` status with an infeasibility proof fails `build()`, so a
        // fake certificate never reaches the result layer — that gate lives one layer down.
        let fake_is_unrepresentable = SolveReport::<i64>::builder(ProblemStatus::Unknown,
            TerminationReason::Completed).proof(SolveProof::infeasibility()).build().is_err();
        ConstraintProgrammingSubproblemResult::infeasible(bare, None).is_err()
            && fake_is_unrepresentable
            && ConstraintProgrammingSubproblemResult::infeasible(infeasible_report(None), None)
                .is_ok()
    };
    // 5) 冲突引用逐字相等。 / The conflict reference matches verbatim.
    let the_conflict_reference_must_match_the_report = {
        let matched = VerifiedMasterConflict::new([StableVariableId("x".to_owned())], "proof-1")
            .expect("conflict");
        let wrong = VerifiedMasterConflict::new([StableVariableId("x".to_owned())], "proof-2")
            .expect("conflict");
        // 报告侧没有引用 → 任何 conflict 都对不上。 
        // A report without a reference matches no conflict at all.
        ConstraintProgrammingSubproblemResult::infeasible(infeasible_report(None),
            Some(matched.clone())).is_err()
            && ConstraintProgrammingSubproblemResult::infeasible(
                infeasible_report(Some("proof-1")), Some(matched)).is_ok()
            && ConstraintProgrammingSubproblemResult::infeasible(
                infeasible_report(Some("proof-1")), Some(wrong)).is_err()
    };
    // 6) `incomplete` 连最优都拒。 / `incomplete` refuses even an optimal report.
    let incomplete_refuses_even_an_optimal_report = {
        let mut solution = SolveSolution::<i64>::vector(Vec::new());
        solution.stable_values = BTreeMap::from([(StableVariableId("x".to_owned()), 3)]);
        let optimal = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
            .solution(solution).proof(SolveProof::optimality()).build().expect("optimal");
        let unknown = SolveReport::builder(ProblemStatus::Unknown, TerminationReason::Completed)
            .build().expect("unknown");
        let terminal = [ProblemStatus::Infeasible, ProblemStatus::Unbounded,
            ProblemStatus::InfeasibleOrUnbounded].iter().all(|status| {
            let report = SolveReport::builder(*status, TerminationReason::Completed)
                .build().expect("terminal report");
            ConstraintProgrammingSubproblemResult::incomplete(report, None).is_err()
        });
        optimal.is_optimal()
            && ConstraintProgrammingSubproblemResult::incomplete(optimal, None).is_err()
            && ConstraintProgrammingSubproblemResult::incomplete(unknown, None).is_ok()
            && terminal
    };
    // 7) `incomplete` 的 incumbent 两条闸门。 / The incomplete incumbent's two gates.
    let the_incomplete_incumbent_has_two_value_gates = {
        let unknown = || SolveReport::builder(ProblemStatus::Unknown, TerminationReason::Completed)
            .build().expect("unknown");
        let incumbent = |value: i64, objective: Option<f64>| {
            Some(ConstraintProgrammingSubproblemIncumbent {
                assignment: BTreeMap::from([(StableVariableId("x".to_owned()), value)]),
                objective })
        };
        ConstraintProgrammingSubproblemResult::incomplete(unknown(),
            incumbent(i64::MIN, None)).is_err()
            && ConstraintProgrammingSubproblemResult::incomplete(unknown(),
                incumbent(1, Some(f64::INFINITY))).is_err()
            && ConstraintProgrammingSubproblemResult::incomplete(unknown(),
                incumbent(i64::MAX, Some(-2.5))).is_ok()
            && ConstraintProgrammingSubproblemResult::incomplete(unknown(),
                incumbent(-4, None)).is_ok()
    };
    // 8) `feasible` 的 assignment 是报告副本。 / The feasible assignment is a copy of the report's.
    let the_feasible_assignment_is_a_copy_of_the_report = {
        let mut solution = SolveSolution::<i64>::vector(Vec::new());
        solution.stable_values = BTreeMap::from([(StableVariableId("b".to_owned()), 2),
            (StableVariableId("a".to_owned()), 1)]);
        let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
            .solution(solution).build().expect("report");
        let expected = report.solution.as_ref().expect("solution").stable_values.clone();
        let result = ConstraintProgrammingSubproblemResult::feasible(report).expect("feasible");
        match result {
            ConstraintProgrammingSubproblemResult::Feasible { assignment, .. } => {
                // `BTreeMap` 的键序由 ID 决定，与插入顺序无关。 
                // The key order comes from the IDs, not the insertion order.
                assignment == expected
                    && assignment.keys().map(|id| id.0.clone()).collect::<Vec<_>>()
                        == vec!["a".to_owned(), "b".to_owned()]
            }
            _ => false,
        }
    };
    Findings { failed_never_validates_anything, only_the_snapshot_constructors_issue_proofs,
        feasible_demands_the_feasible_status, infeasible_demands_a_genuine_certificate,
        the_conflict_reference_must_match_the_report, incomplete_refuses_even_an_optimal_report,
        the_incomplete_incumbent_has_two_value_gates, the_feasible_assignment_is_a_copy_of_the_report }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(variant: &str) -> SubproblemResultRequest {
        SubproblemResultRequest { variant: variant.to_owned(), status: Some("Feasible".to_owned()),
            assignment: vec![SubproblemValueSpec { id: "x".to_owned(), value: 3 }],
            with_solution: None, with_proof: None, proof_reference: None,
            conflict_members: Vec::new(), conflict_reference: None, incumbent_objective: None,
            issue_code: None, issue_message: None }
    }

    #[test]
    fn the_feasible_entry_copies_the_report_assignment() {
        let report = verify_subproblem_result(&request("feasible")).unwrap();
        assert!(report.accepted);
        assert_eq!(report.state.as_deref(), Some("Feasible"));
        assert_eq!(report.assignment_size, Some(1));
        assert_eq!(report.assignment_ids, vec!["x".to_owned()]);
        // 朴素入口没有证明 → 没有模型指纹。 / The plain entry issues no proof, so no model fingerprint.
        assert!(!report.has_model_fingerprint);
        assert!(report.has_report);
        assert_eq!(report.report_status.as_deref(), Some("Feasible"));
        assert!(report.failed_never_validates_anything);
        assert!(report.only_the_snapshot_constructors_issue_proofs);
        assert!(report.feasible_demands_the_feasible_status);
        assert!(report.infeasible_demands_a_genuine_certificate);
        assert!(report.the_conflict_reference_must_match_the_report);
        assert!(report.incomplete_refuses_even_an_optimal_report);
        assert!(report.the_incomplete_incumbent_has_two_value_gates);
        assert!(report.the_feasible_assignment_is_a_copy_of_the_report);
    }

    #[test]
    fn the_snapshot_entry_issues_a_proof() {
        let report = verify_subproblem_result(&request("feasibleFromSnapshot")).unwrap();
        assert!(report.accepted);
        assert!(report.has_model_fingerprint);
    }

    #[test]
    fn the_unknown_status_is_refused_even_with_a_full_solution() {
        let report = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Unknown".to_owned()), ..request("feasible") }).unwrap();
        assert!(!report.accepted);
        assert!(report.refusal.as_deref().unwrap().contains("feasible incumbent report"));
        assert!(report.state.is_none());
    }

    #[test]
    fn infeasible_needs_a_certificate_and_a_matching_reference() {
        let bare = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Infeasible".to_owned()), with_solution: Some(false),
            ..request("infeasible") }).unwrap();
        assert!(!bare.accepted);
        let good = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Infeasible".to_owned()), with_solution: Some(false),
            with_proof: Some(true), ..request("infeasible") }).unwrap();
        assert!(good.accepted);
        assert_eq!(good.state.as_deref(), Some("Infeasible"));
        let matched = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Infeasible".to_owned()), with_solution: Some(false),
            with_proof: Some(true), proof_reference: Some("proof-1".to_owned()),
            conflict_members: vec!["x".to_owned()],
            conflict_reference: Some("proof-1".to_owned()), ..request("infeasible") }).unwrap();
        assert!(matched.accepted);
        let wrong = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Infeasible".to_owned()), with_solution: Some(false),
            with_proof: Some(true), proof_reference: Some("proof-1".to_owned()),
            conflict_members: vec!["x".to_owned()],
            conflict_reference: Some("proof-2".to_owned()), ..request("infeasible") }).unwrap();
        assert!(!wrong.accepted);
        assert!(wrong.refusal.as_deref().unwrap().contains("does not match"));
    }

    #[test]
    fn incomplete_refuses_terminal_statuses_and_gates_the_incumbent() {
        let unknown = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Unknown".to_owned()), ..request("incomplete") }).unwrap();
        assert!(unknown.accepted);
        assert_eq!(unknown.state.as_deref(), Some("Incomplete"));
        // 终态报告**不能带 incumbent**，否则连 `build()` 都过不去——所以这里必须先去掉 solution。 
        // A terminal report **cannot carry an incumbent**, so the solution must be dropped first or `build()` fails.
        let terminal = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Infeasible".to_owned()), with_solution: Some(false),
            ..request("incomplete") }).unwrap();
        assert!(!terminal.accepted);
        let floor = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Unknown".to_owned()),
            assignment: vec![SubproblemValueSpec { id: "x".to_owned(), value: i64::MIN }],
            ..request("incomplete") }).unwrap();
        assert!(!floor.accepted);
        let infinite = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Unknown".to_owned()),
            incumbent_objective: Some(f64::INFINITY), ..request("incomplete") }).unwrap();
        assert!(!infinite.accepted);
    }

    #[test]
    fn failed_accepts_anything_and_carries_no_report() {
        let report = verify_subproblem_result(&SubproblemResultRequest { issue_code: Some(String::new()),
            issue_message: Some(String::new()), ..request("failed") }).unwrap();
        assert!(report.accepted);
        assert_eq!(report.state.as_deref(), Some("Failed"));
        assert!(!report.has_report);
        assert!(!report.has_model_fingerprint);
        assert_eq!(report.issue_code, Some(String::new()));
    }

    #[test]
    fn a_blank_conflict_member_set_is_refused_by_the_conflict_constructor() {
        let report = verify_subproblem_result(&SubproblemResultRequest {
            status: Some("Infeasible".to_owned()), with_solution: Some(false),
            with_proof: Some(true), conflict_members: vec!["x".to_owned()],
            conflict_reference: Some("   ".to_owned()), ..request("infeasible") }).unwrap();
        assert!(!report.accepted);
        assert!(report.refusal.as_deref().unwrap().contains("proof reference"));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_subproblem_result(&SubproblemResultRequest { variant: "nope".to_owned(),
            ..request("feasible") }).unwrap_err().message.contains("unknown variant"));
        assert!(verify_subproblem_result(&SubproblemResultRequest { status: Some("Maybe".to_owned()),
            ..request("feasible") }).unwrap_err().message.contains("unknown status"));
        assert!(verify_subproblem_result(&SubproblemResultRequest {
            assignment: (0..17).map(|index| SubproblemValueSpec { id: format!("x{index}"),
                value: 0 }).collect(), ..request("feasible") }).unwrap_err().message
            .contains("at most 16 entries"));
        assert!(verify_subproblem_result(&SubproblemResultRequest { assignment: vec![
            SubproblemValueSpec { id: "  ".to_owned(), value: 0 }], ..request("feasible") })
            .unwrap_err().message.contains("stable IDs must not be blank"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_subproblem_result(&request("feasible")).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"assignmentSize\""), "{json}");
        assert!(json.contains("\"failedNeverValidatesAnything\""), "{json}");
        assert!(json.contains("\"hasModelFingerprint\""), "{json}");
        assert!(!json.contains("assignment_size"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SubproblemResultReport>(&copy.to_string()).is_err());
    }
}
