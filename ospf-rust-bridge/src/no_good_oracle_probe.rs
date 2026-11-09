//! 续挖第 69 单：默认 feasibility no-good oracle 真正生成出来的 cuts——二进制域一条、有界整数域
//! `3n+1` 条、空 conflict 退化为"排除整份赋值"、而 context 入口**不校验**引用
//! （`solver/logic_based_benders.rs` 的 `DefaultFeasibilityCutOracle` + `LogicBasedBendersProofContext`）
//! Continuation, delivery 69: the cuts the default feasibility no-good oracle actually emits — one for binary domains,
//! `3n+1` for bounded integers, an empty conflict degrading to "exclude the whole assignment", and a context entry that
//! **never validates** its reference (`DefaultFeasibilityCutOracle` + `LogicBasedBendersProofContext`).
//!
//! 第 68 单量过结果四态；这一单量**吃结果的那个 oracle**：`LogicBasedBendersCutOracle` 的两个入口。
//! 八条实测：
//!
//! 1. **二进制域只出 1 条 cut**（`binary_domains_yield_a_single_no_good()`）：`GreaterOrEqual`、`rhs = 1 - ones`，
//!    系数里 1 变成 `-1`、0 变成 `1`。
//! 2. **有界整数域出 `3n+1` 条**（`integer_domains_yield_three_rows_per_variable_plus_one()`）：每个变量三行
//!    （`lowerSide` / `upperSide` / `atMostOne`）外加一条全族共享的 `requireDifference`。
//! 3. **每行都声明完整 auxiliary 集合**（`every_row_declares_the_whole_auxiliary_set()`）：不是"我这行用到几个
//!    就声明几个"，而是整个 family 的 `2n` 个二进制 auxiliary 逐行全给。
//! 4. **空 conflict 退化成排除整份赋值**（`an_empty_conflict_excludes_the_whole_assignment()`）：成员集为空时
//!    取 `assignment.values` 的**全部**键。
//! 5. **conflict 成员必须都在赋值里**（`a_conflict_outside_the_assignment_is_refused()`）。
//! 6. **两边都没引用就拒**（`a_missing_proof_reference_is_refused()`）：conflict 与报告都没有引用时，oracle
//!    报"default feasibility cut requires a proof reference"。
//! 7. **context 入口不校验引用**（`the_context_entry_does_not_validate_its_reference()`）：同一个结果，走
//!    `cuts_for_infeasible` 拿到的是**已验证**引用，走 `cuts_for_infeasible_with_context` 拿到的是 context
//!    自己带的那串——两条路径给同一份 cut 打上不同引用。
//! 8. **`failed` / 可行结果都被拒**（`only_an_infeasible_result_reaches_the_oracle()`）。
//!
//! 附带一条**结构性**结论：`ConstraintProgrammingSubproblemResult::infeasible` 要求
//! `report.proof.reference == conflict.proof_reference`，所以"conflict 引用覆盖报告引用"这条 `or_else` 分支在
//! **构造出来的**结果上永远走不到——两条引用必然相等。
//!
//! Delivery 68 measured the four-state result; this one measures **the oracle that consumes it**. **Binary domains yield a
//! single cut** (`GreaterOrEqual`, `rhs = 1 - ones`, ones negated). **Bounded-integer domains yield `3n+1`** (three rows
//! per variable plus one family-wide `requireDifference`). **Every row declares the whole auxiliary set**, not just what it
//! uses. **An empty conflict degrades to excluding the whole assignment.** **Conflict members must lie inside the
//! assignment.** **With no reference on either side the oracle refuses.** **The context entry does not validate its
//! reference** — the same result gets two different proof references depending on the entry. And **only an infeasible
//! result reaches the oracle**. One structural note: `infeasible` forces `report.proof.reference ==
//! conflict.proof_reference`, so the "conflict reference overrides the report's" fallback is unreachable for constructed
//! results.

use std::collections::{BTreeMap, BTreeSet};

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_core::solver::{
    ProblemStatus, SolveProof, SolveReport, TerminationReason,
};
use ospf_rust_framework::solver::logic_based_benders::{
    ConstraintProgrammingSubproblemResult, DefaultFeasibilityCutOracle, LogicBasedBendersCutOracle,
    LogicBasedBendersProofContext, MasterAssignment, MasterBinding, MasterCut, MasterCutEncoding,
    MasterCutProofStatus, MasterCutValidity, MasterVariableBinding, MasterVariableDomain,
    VerifiedMasterConflict,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个 master 绑定的字面量。 / One master binding literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OracleBindingLiteral {
    /// 稳定 ID。 / The stable ID.
    pub id: String,
    /// `binary` 或 `integer`。 / `binary` or `integer`.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub lower: Option<i64>,
    #[serde(default)]
    pub upper: Option<i64>,
}

/// 一个赋值条目。 / One assignment entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OracleAssignmentEntry {
    pub id: String,
    pub value: i64,
}

/// no-good oracle 探针请求。 / The no-good oracle probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoGoodOracleRequest {
    /// master 绑定。 / The master bindings.
    #[serde(default)]
    pub bindings: Vec<OracleBindingLiteral>,
    /// 当前 master 赋值。 / The current master assignment.
    #[serde(default)]
    pub assignment: Vec<OracleAssignmentEntry>,
    /// 已验证冲突的成员（空表示不给冲突）。 / The verified conflict's members (empty means no conflict).
    #[serde(default)]
    pub conflict_members: Vec<String>,
    /// 冲突自己的证明引用。 / The conflict's own proof reference.
    #[serde(default)]
    pub conflict_reference: Option<String>,
    /// 报告 proof 的引用。 / The report proof's reference.
    #[serde(default)]
    pub report_reference: Option<String>,
    /// 走哪个入口：`plain` 或 `withContext`。 / Which entry to walk: `plain` or `withContext`.
    #[serde(default)]
    pub entry: Option<String>,
    /// context 自带的证明引用（`withContext` 才用）。 / The context's own proof reference (`withContext` only).
    #[serde(default)]
    pub context_reference: Option<String>,
    /// 结果的四态：`infeasible`（默认）/ `feasible` / `failed`。
    /// The result's four-state: `infeasible` (default) / `feasible` / `failed`.
    #[serde(default)]
    pub result_variant: Option<String>,
}

/// 一条 cut 的读面。 / One cut's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OracleCutView {
    /// 稳定 cut ID。 / The stable cut ID.
    pub id: String,
    /// 关系方向（`LessOrEqual` / `GreaterOrEqual` / `Equal`）。 / The relation sense.
    pub sense: String,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
    /// 系数条数。 / The coefficient count.
    pub coefficient_count: u32,
    /// 系数列表（按稳定 ID 排序）。 / The coefficients, ordered by stable ID.
    pub coefficients: Vec<OracleAssignmentEntry>,
    /// 有效性（`Global` / `Local`）。 / The validity scope.
    pub validity: String,
    /// 证明状态（`Verified` / `Claimed` / `None`）。 / The proof status.
    pub proof_status: String,
    /// 证明引用。 / The proof reference.
    #[serde(default)]
    pub proof_reference: Option<String>,
    /// 来源标签。 / The origin label.
    pub origin: String,
    /// 原因标签。 / The reason label.
    pub reason: String,
    /// auxiliary 声明条数。 / The auxiliary declaration count.
    pub auxiliary_count: u32,
    /// 编码类型（`linear` / `binaryNoGood` / `boundedIntegerNoGood`）。 / The encoding kind.
    pub encoding_kind: String,
    /// 多行族的行类型（非多行时为空）。 / The multi-row family's row kind (empty when not multi-row).
    #[serde(default)]
    pub row_kind: Option<String>,
}

/// no-good oracle 探针结论。 / The no-good oracle probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoGoodOracleReport {
    /// 请求走的入口。 / The entry that was walked.
    pub entry: String,
    /// 是否生成了 cuts。 / Whether cuts were produced.
    pub accepted: bool,
    /// 被拒时的错误文案。 / The refusal message.
    #[serde(default)]
    pub refusal: Option<String>,
    /// cut 条数。 / The cut count.
    pub cut_count: u32,
    /// 每条 cut 的读面。 / Each cut's readable face.
    pub cuts: Vec<OracleCutView>,
    /// 这些 cut 用到的证明引用（去重后升序）。 / The proof references these cuts carry (deduplicated, sorted).
    pub proof_references: Vec<String>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub binary_domains_yield_a_single_no_good: bool,
    pub integer_domains_yield_three_rows_per_variable_plus_one: bool,
    pub every_row_declares_the_whole_auxiliary_set: bool,
    pub an_empty_conflict_excludes_the_whole_assignment: bool,
    pub a_conflict_outside_the_assignment_is_refused: bool,
    pub a_missing_proof_reference_is_refused: bool,
    pub the_context_entry_does_not_validate_its_reference: bool,
    pub only_an_infeasible_result_reaches_the_oracle: bool,
}

/// 关系方向名。 / The relation sense name.
fn sense_name(sense: ospf_rust_framework::solver::CutSense) -> String {
    match sense {
        ospf_rust_framework::solver::CutSense::LessOrEqual => "LessOrEqual",
        ospf_rust_framework::solver::CutSense::GreaterOrEqual => "GreaterOrEqual",
        ospf_rust_framework::solver::CutSense::Equal => "Equal",
    }
    .to_owned()
}

/// 有效性名。 / The validity name.
fn validity_name(validity: MasterCutValidity) -> String {
    match validity {
        MasterCutValidity::Global => "Global",
        MasterCutValidity::Local => "Local",
    }
    .to_owned()
}

/// 证明状态名。 / The proof status name.
fn proof_status_name(status: MasterCutProofStatus) -> String {
    match status {
        MasterCutProofStatus::Verified => "Verified",
        MasterCutProofStatus::Claimed => "Claimed",
        MasterCutProofStatus::None => "None",
    }
    .to_owned()
}

/// 编码类型名。 / The encoding kind name.
fn encoding_kind_name(encoding: &MasterCutEncoding) -> String {
    match encoding {
        MasterCutEncoding::Linear => "linear",
        MasterCutEncoding::BinaryNoGood { .. } => "binaryNoGood",
        MasterCutEncoding::BoundedIntegerNoGood { .. } => "boundedIntegerNoGood",
    }
    .to_owned()
}

/// 多行族的行类型名。 / The multi-row family's row kind name.
fn row_kind_name(encoding: &MasterCutEncoding) -> Option<String> {
    match encoding {
        MasterCutEncoding::BoundedIntegerNoGood { row, .. } => Some(match row {
            ospf_rust_framework::solver::logic_based_benders::BoundedIntegerNoGoodRow::LowerSide(_) =>
                "lowerSide",
            ospf_rust_framework::solver::logic_based_benders::BoundedIntegerNoGoodRow::UpperSide(_) =>
                "upperSide",
            ospf_rust_framework::solver::logic_based_benders::BoundedIntegerNoGoodRow::AtMostOne(_) =>
                "atMostOne",
            ospf_rust_framework::solver::logic_based_benders::BoundedIntegerNoGoodRow::RequireDifference =>
                "requireDifference",
        }
        .to_owned()),
        _ => None,
    }
}

/// 把一条 cut 读成 DTO。 / Read one cut into the DTO.
fn cut_view(cut: &MasterCut) -> OracleCutView {
    OracleCutView {
        id: cut.id.clone(),
        sense: sense_name(cut.sense),
        rhs: cut.rhs,
        coefficient_count: u32::try_from(cut.coefficients.len()).unwrap_or(u32::MAX),
        coefficients: cut.coefficients.iter()
            .map(|(id, value)| OracleAssignmentEntry { id: id.0.clone(), value: *value as i64 })
            .collect(),
        validity: validity_name(cut.validity),
        proof_status: proof_status_name(cut.proof_status),
        proof_reference: cut.proof_reference.clone(),
        origin: cut.origin.clone(),
        reason: cut.reason.clone(),
        auxiliary_count: u32::try_from(cut.auxiliary_variables.len()).unwrap_or(u32::MAX),
        encoding_kind: encoding_kind_name(&cut.encoding),
        row_kind: row_kind_name(&cut.encoding),
    }
}

/// 造一份绑定。 / Build a binding set.
fn binding_of(literals: &[OracleBindingLiteral]) -> Result<MasterBinding> {
    let mut values = Vec::with_capacity(literals.len());
    for literal in literals {
        if literal.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "no-good oracle failed validation: binding IDs must not be blank"));
        }
        let id = StableVariableId(literal.id.clone());
        values.push(match literal.kind.as_deref().unwrap_or("binary") {
            "binary" => MasterVariableBinding::binary(id),
            "integer" => {
                let lower = literal.lower.ok_or_else(|| BridgeError::new("InvalidInput",
                    "no-good oracle failed validation: an integer binding needs a lower bound"))?;
                let upper = literal.upper.ok_or_else(|| BridgeError::new("InvalidInput",
                    "no-good oracle failed validation: an integer binding needs an upper bound"))?;
                MasterVariableBinding::integer(id, lower, upper)
            }
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "no-good oracle failed validation: unknown binding kind `{other}`"))),
        });
    }
    MasterBinding::new(values).map_err(|error| BridgeError::new("InvalidInput", &format!(
        "no-good oracle failed validation: {error}")))
}

/// 造一份赋值。 / Build an assignment.
fn assignment_of(entries: &[OracleAssignmentEntry], binding: &MasterBinding) -> Result<MasterAssignment> {
    let mut values = BTreeMap::new();
    for entry in entries {
        if entry.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "no-good oracle failed validation: assignment IDs must not be blank"));
        }
        if values.insert(StableVariableId(entry.id.clone()), entry.value).is_some() {
            return Err(BridgeError::new("InvalidInput",
                "no-good oracle failed validation: the assignment repeats a stable ID"));
        }
    }
    MasterAssignment::new(values, binding).map_err(|error| BridgeError::new("InvalidInput", &format!(
        "no-good oracle failed validation: {error}")))
}

/// 造一份不可行报告。 / Build an infeasible report.
fn infeasible_report(reference: Option<&str>) -> Result<SolveReport<i64>> {
    let mut proof = SolveProof::infeasibility();
    proof.reference = reference.map(|value| value.to_owned());
    SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
        .proof(proof).build().map_err(|error| BridgeError::new("InvalidInput", &format!(
            "no-good oracle failed validation: the probe report is invalid: {error}")))
}

/// 走一遍 no-good oracle 探针。非法输入是**错误**。 / Walk the no-good oracle probe.
pub fn verify_no_good_oracle(request: &NoGoodOracleRequest) -> Result<NoGoodOracleReport> {
    if request.bindings.len() > 16 || request.assignment.len() > 16
        || request.conflict_members.len() > 16
    {
        return Err(BridgeError::new("InvalidInput",
            "no-good oracle failed validation: at most 16 entries are accepted"));
    }
    let entry = request.entry.clone().unwrap_or_else(|| "plain".to_owned());
    if !matches!(entry.as_str(), "plain" | "withContext") {
        return Err(BridgeError::new("InvalidInput", &format!(
            "no-good oracle failed validation: unknown entry `{entry}`")));
    }
    let binding = binding_of(&request.bindings)?;
    let assignment = assignment_of(&request.assignment, &binding)?;
    let conflict = if request.conflict_members.is_empty() {
        None
    } else {
        Some(VerifiedMasterConflict::new(request.conflict_members.iter()
            .map(|id| StableVariableId(id.clone())),
            request.conflict_reference.clone().unwrap_or_default())
            .map_err(|error| BridgeError::new("InvalidInput", &format!(
                "no-good oracle failed validation: {error}")))?)
    };
    let result_variant = request.result_variant.clone().unwrap_or_else(|| "infeasible".to_owned());
    let result = match result_variant.as_str() {
        "infeasible" => ConstraintProgrammingSubproblemResult::infeasible(
            infeasible_report(request.report_reference.as_deref())?, conflict),
        "feasible" => {
            // 可行结果也走 snapshot 无关的朴素入口，只为量"oracle 拒它"。
            // A feasible result goes through the plain entry too, only to measure that the oracle refuses it.
            let mut solution = ospf_rust_core::solver::SolveSolution::<i64>::vector(Vec::new());
            solution.stable_values = assignment.values.iter()
                .map(|(id, value)| (id.clone(), *value)).collect();
            ConstraintProgrammingSubproblemResult::feasible(
                SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
                    .solution(solution).build().map_err(|error| BridgeError::new("InvalidInput",
                        &format!("no-good oracle failed validation: the probe report is invalid: {error}")))?)
        }
        "failed" => Ok(ConstraintProgrammingSubproblemResult::failed("probe", "probe")),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "no-good oracle failed validation: unknown result variant `{other}`"))),
    }
    .map_err(|error| BridgeError::new("InvalidInput", &format!(
        "no-good oracle failed validation: the probe result is invalid: {error}")))?;
    let mut oracle = DefaultFeasibilityCutOracle::new(binding.clone());
    let context = LogicBasedBendersProofContext {
        subproblem_attempt_id: "attempt-1".to_owned(),
        report_proof_reference: request.report_reference.clone(),
        proof_reference: request.context_reference.clone().unwrap_or_default(),
    };
    let cuts: std::result::Result<Vec<MasterCut>, String> = match entry.as_str() {
        "withContext" => oracle.cuts_for_infeasible_with_context(&assignment, &result, &context)
            .map_err(|error| error.to_string()),
        _ => oracle.cuts_for_infeasible(&assignment, &result).map_err(|error| error.to_string()),
    };
    let findings = canonical_findings();
    let (accepted, refusal, views, references) = match cuts {
        Ok(cuts) => {
            let mut references = cuts.iter().filter_map(|cut| cut.proof_reference.clone())
                .collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
            references.sort();
            (true, None, cuts.iter().map(cut_view).collect::<Vec<_>>(), references)
        }
        Err(refusal) => (false, Some(refusal), Vec::new(), Vec::new()),
    };
    Ok(NoGoodOracleReport {
        entry,
        accepted,
        refusal,
        cut_count: u32::try_from(views.len()).unwrap_or(u32::MAX),
        cuts: views,
        proof_references: references,
        binary_domains_yield_a_single_no_good: findings.binary_domains_yield_a_single_no_good,
        integer_domains_yield_three_rows_per_variable_plus_one:
            findings.integer_domains_yield_three_rows_per_variable_plus_one,
        every_row_declares_the_whole_auxiliary_set:
            findings.every_row_declares_the_whole_auxiliary_set,
        an_empty_conflict_excludes_the_whole_assignment:
            findings.an_empty_conflict_excludes_the_whole_assignment,
        a_conflict_outside_the_assignment_is_refused:
            findings.a_conflict_outside_the_assignment_is_refused,
        a_missing_proof_reference_is_refused: findings.a_missing_proof_reference_is_refused,
        the_context_entry_does_not_validate_its_reference:
            findings.the_context_entry_does_not_validate_its_reference,
        only_an_infeasible_result_reaches_the_oracle:
            findings.only_an_infeasible_result_reaches_the_oracle,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    binary_domains_yield_a_single_no_good: bool,
    integer_domains_yield_three_rows_per_variable_plus_one: bool,
    every_row_declares_the_whole_auxiliary_set: bool,
    an_empty_conflict_excludes_the_whole_assignment: bool,
    a_conflict_outside_the_assignment_is_refused: bool,
    a_missing_proof_reference_is_refused: bool,
    the_context_entry_does_not_validate_its_reference: bool,
    only_an_infeasible_result_reaches_the_oracle: bool,
}

/// 一份二进制绑定（`a`、`b`）。 / A binary binding (`a`, `b`).
fn binary_binding() -> MasterBinding {
    MasterBinding::new([MasterVariableBinding::binary(StableVariableId("a".to_owned())),
        MasterVariableBinding::binary(StableVariableId("b".to_owned()))])
        .expect("binary binding")
}

/// 一份整数绑定（`a`、`b` 都在 `0..=4`）。 / An integer binding (`a`, `b` in `0..=4`).
fn integer_binding() -> MasterBinding {
    MasterBinding::new([
        MasterVariableBinding::integer(StableVariableId("a".to_owned()), 0, 4),
        MasterVariableBinding::integer(StableVariableId("b".to_owned()), 0, 4),
    ]).expect("integer binding")
}

/// 一份赋值（`a=1, b=0`）。 / An assignment (`a=1, b=0`).
fn assignment_with(binding: &MasterBinding) -> MasterAssignment {
    MasterAssignment::new(BTreeMap::from([
        (StableVariableId("a".to_owned()), 1),
        (StableVariableId("b".to_owned()), 0),
    ]), binding).expect("assignment")
}

/// 一份不可行结果（引用 `proof-1`，无冲突）。 / An infeasible result (reference `proof-1`, no conflict).
fn plain_infeasible() -> ConstraintProgrammingSubproblemResult {
    ConstraintProgrammingSubproblemResult::infeasible(
        infeasible_report(Some("proof-1")).expect("report"), None).expect("infeasible")
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 二进制域只出 1 条 cut。 / Binary domains yield a single cut.
    let binary_domains_yield_a_single_no_good = {
        let binding = binary_binding();
        let assignment = assignment_with(&binding);
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        let cuts = oracle.cuts_for_infeasible(&assignment, &plain_infeasible()).expect("cuts");
        // 系数里 1 → `-1`、0 → `1`，`rhs = 1 - ones = 0`。 
        // Coefficients turn 1 into `-1` and 0 into `1`, and `rhs = 1 - ones = 0`.
        cuts.len() == 1
            && matches!(cuts[0].encoding, MasterCutEncoding::BinaryNoGood { .. })
            && cuts[0].sense == ospf_rust_framework::solver::CutSense::GreaterOrEqual
            && cuts[0].rhs == 0.0
            && cuts[0].coefficients.get(&StableVariableId("a".to_owned())) == Some(&-1.0)
            && cuts[0].coefficients.get(&StableVariableId("b".to_owned())) == Some(&1.0)
    };
    // 2) 有界整数域出 `3n+1` 条。 / Bounded-integer domains yield `3n+1`.
    let integer_domains_yield_three_rows_per_variable_plus_one = {
        let binding = integer_binding();
        let assignment = assignment_with(&binding);
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        let cuts = oracle.cuts_for_infeasible(&assignment, &plain_infeasible()).expect("cuts");
        let mut rows = std::collections::BTreeMap::new();
        for cut in &cuts {
            *rows.entry(row_kind_name(&cut.encoding).unwrap_or_default()).or_insert(0u32) += 1;
        }
        cuts.len() == 7
            && rows.get("lowerSide") == Some(&2)
            && rows.get("upperSide") == Some(&2)
            && rows.get("atMostOne") == Some(&2)
            && rows.get("requireDifference") == Some(&1)
    };
    // 3) 每行都声明完整 auxiliary 集合。 / Every row declares the whole auxiliary set.
    let every_row_declares_the_whole_auxiliary_set = {
        let binding = integer_binding();
        let assignment = assignment_with(&binding);
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        let cuts = oracle.cuts_for_infeasible(&assignment, &plain_infeasible()).expect("cuts");
        // `2n` 个 auxiliary，逐行全给——包括只用到一个 auxiliary 的 `atMostOne`。 
        // `2n` auxiliaries, all declared on every row — including `atMostOne`, which uses only one.
        cuts.len() == 7 && cuts.iter().all(|cut| cut.auxiliary_variables.len() == 4)
    };
    // 4) 空 conflict 退化成排除整份赋值。 / An empty conflict degrades to excluding the whole assignment.
    let an_empty_conflict_excludes_the_whole_assignment = {
        let binding = binary_binding();
        let assignment = assignment_with(&binding);
        let mut oracle = DefaultFeasibilityCutOracle::new(binding.clone());
        let whole = oracle.cuts_for_infeasible(&assignment, &plain_infeasible()).expect("cuts");
        // 只给 `a` 的冲突：系数只剩一项。 / A conflict on `a` alone leaves one coefficient.
        let narrow = ConstraintProgrammingSubproblemResult::infeasible(
            infeasible_report(Some("proof-1")).expect("report"),
            Some(VerifiedMasterConflict::new([StableVariableId("a".to_owned())], "proof-1")
                .expect("conflict"))).expect("infeasible");
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        let single = oracle.cuts_for_infeasible(&assignment, &narrow).expect("cuts");
        whole[0].coefficients.len() == 2 && single[0].coefficients.len() == 1
            && single[0].rhs == 0.0
    };
    // 5) conflict 成员必须都在赋值里。 / Conflict members must lie inside the assignment.
    let a_conflict_outside_the_assignment_is_refused = {
        let binding = binary_binding();
        let assignment = assignment_with(&binding);
        // 冲突里有个不在赋值里的 `c`；注意 `MasterBinding` 也不认它，所以先建 binding 再取 assignment。
        // The conflict names a `c` that is not assigned; the binding is built first, then the assignment.
        let stray = ConstraintProgrammingSubproblemResult::infeasible(
            infeasible_report(Some("proof-1")).expect("report"),
            Some(VerifiedMasterConflict::new([StableVariableId("c".to_owned())], "proof-1")
                .expect("conflict"))).expect("infeasible");
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        oracle.cuts_for_infeasible(&assignment, &stray)
            .is_err_and(|error| error.to_string().contains("outside the current assignment"))
    };
    // 6) 两边都没引用就拒。 / With no reference on either side the oracle refuses.
    let a_missing_proof_reference_is_refused = {
        let binding = binary_binding();
        let assignment = assignment_with(&binding);
        let bare = ConstraintProgrammingSubproblemResult::infeasible(
            infeasible_report(None).expect("report"), None).expect("infeasible");
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        oracle.cuts_for_infeasible(&assignment, &bare)
            .is_err_and(|error| error.to_string().contains("requires a proof reference"))
    };
    // 7) context 入口不校验引用。 / The context entry does not validate its reference.
    let the_context_entry_does_not_validate_its_reference = {
        let binding = binary_binding();
        let assignment = assignment_with(&binding);
        let result = plain_infeasible();
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        let plain = oracle.cuts_for_infeasible(&assignment, &result).expect("cuts");
        let context = LogicBasedBendersProofContext {
            subproblem_attempt_id: "attempt-1".to_owned(),
            report_proof_reference: Some("proof-1".to_owned()),
            proof_reference: "attempt-1::not-the-verified-reference".to_owned(),
        };
        let with_context = oracle.cuts_for_infeasible_with_context(&assignment, &result, &context)
            .expect("cuts");
        // 同一个结果、同一份 cut，两条路径打出不同引用；context 那串**没有**被拿去和报告比对。
        // The same result and the same cut carry different references, and the context string is never checked against
        // the report.
        plain[0].proof_reference.as_deref() == Some("proof-1")
            && with_context[0].proof_reference.as_deref()
                == Some("attempt-1::not-the-verified-reference")
            && plain[0].id == with_context[0].id
    };
    // 8) 只有不可行结果能进 oracle。 / Only an infeasible result reaches the oracle.
    let only_an_infeasible_result_reaches_the_oracle = {
        let binding = binary_binding();
        let assignment = assignment_with(&binding);
        let mut solution = ospf_rust_core::solver::SolveSolution::<i64>::vector(Vec::new());
        solution.stable_values = assignment.values.iter()
            .map(|(id, value)| (id.clone(), *value)).collect();
        let feasible = ConstraintProgrammingSubproblemResult::feasible(
            SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
                .solution(solution).build().expect("feasible report")).expect("feasible");
        let failed = ConstraintProgrammingSubproblemResult::failed("probe", "probe");
        let mut oracle = DefaultFeasibilityCutOracle::new(binding);
        let feasible_refused = oracle.cuts_for_infeasible(&assignment, &feasible).is_err();
        let failed_refused = oracle.cuts_for_infeasible(&assignment, &failed).is_err();
        // 默认的可行入口**总是**空——包括拿到一份真的可行结果。
        // The default feasible entry is **always** empty, even with a genuinely feasible result.
        let feasible_cuts = oracle.cuts_for_feasible(&assignment, &feasible)
            .is_ok_and(|cuts| cuts.is_empty());
        feasible_refused && failed_refused && feasible_cuts
    };
    Findings {
        binary_domains_yield_a_single_no_good,
        integer_domains_yield_three_rows_per_variable_plus_one,
        every_row_declares_the_whole_auxiliary_set,
        an_empty_conflict_excludes_the_whole_assignment,
        a_conflict_outside_the_assignment_is_refused,
        a_missing_proof_reference_is_refused,
        the_context_entry_does_not_validate_its_reference,
        only_an_infeasible_result_reaches_the_oracle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binary_request() -> NoGoodOracleRequest {
        NoGoodOracleRequest {
            bindings: vec![OracleBindingLiteral { id: "a".to_owned(), kind: Some("binary".to_owned()),
                lower: None, upper: None },
                OracleBindingLiteral { id: "b".to_owned(), kind: Some("binary".to_owned()),
                    lower: None, upper: None }],
            assignment: vec![OracleAssignmentEntry { id: "a".to_owned(), value: 1 },
                OracleAssignmentEntry { id: "b".to_owned(), value: 0 }],
            conflict_members: Vec::new(), conflict_reference: None,
            report_reference: Some("proof-1".to_owned()), entry: None, context_reference: None,
            result_variant: None }
    }

    fn integer_request() -> NoGoodOracleRequest {
        NoGoodOracleRequest {
            bindings: vec![OracleBindingLiteral { id: "a".to_owned(),
                kind: Some("integer".to_owned()), lower: Some(0), upper: Some(4) },
                OracleBindingLiteral { id: "b".to_owned(), kind: Some("integer".to_owned()),
                    lower: Some(0), upper: Some(4) }],
            assignment: vec![OracleAssignmentEntry { id: "a".to_owned(), value: 1 },
                OracleAssignmentEntry { id: "b".to_owned(), value: 0 }],
            ..binary_request() }
    }

    #[test]
    fn a_binary_domain_yields_one_no_good() {
        let report = verify_no_good_oracle(&binary_request()).unwrap();
        assert!(report.accepted);
        assert_eq!(report.cut_count, 1);
        let cut = &report.cuts[0];
        assert_eq!(cut.encoding_kind, "binaryNoGood");
        assert_eq!(cut.sense, "GreaterOrEqual");
        assert_eq!(cut.rhs, 0.0);
        assert_eq!(cut.proof_status, "Verified");
        assert_eq!(cut.validity, "Global");
        assert_eq!(cut.proof_reference.as_deref(), Some("proof-1"));
        assert_eq!(report.proof_references, vec!["proof-1".to_owned()]);
        // 1 → -1，0 → 1。 / 1 becomes -1 and 0 becomes 1.
        assert_eq!(cut.coefficients, vec![
            OracleAssignmentEntry { id: "a".to_owned(), value: -1 },
            OracleAssignmentEntry { id: "b".to_owned(), value: 1 }]);
        assert!(report.binary_domains_yield_a_single_no_good);
        assert!(report.integer_domains_yield_three_rows_per_variable_plus_one);
        assert!(report.every_row_declares_the_whole_auxiliary_set);
        assert!(report.an_empty_conflict_excludes_the_whole_assignment);
        assert!(report.a_conflict_outside_the_assignment_is_refused);
        assert!(report.a_missing_proof_reference_is_refused);
        assert!(report.the_context_entry_does_not_validate_its_reference);
        assert!(report.only_an_infeasible_result_reaches_the_oracle);
    }

    #[test]
    fn an_integer_domain_yields_seven_rows() {
        let report = verify_no_good_oracle(&integer_request()).unwrap();
        assert_eq!(report.cut_count, 7);
        let rows = report.cuts.iter().filter_map(|cut| cut.row_kind.clone()).collect::<Vec<_>>();
        assert_eq!(rows.iter().filter(|kind| *kind == "lowerSide").count(), 2);
        assert_eq!(rows.iter().filter(|kind| *kind == "upperSide").count(), 2);
        assert_eq!(rows.iter().filter(|kind| *kind == "atMostOne").count(), 2);
        assert_eq!(rows.iter().filter(|kind| *kind == "requireDifference").count(), 1);
        // 每行都声明 2n = 4 个 auxiliary。 / Every row declares all 2n = 4 auxiliaries.
        assert!(report.cuts.iter().all(|cut| cut.auxiliary_count == 4));
        assert!(report.cuts.iter().all(|cut| cut.encoding_kind == "boundedIntegerNoGood"));
    }

    #[test]
    fn a_narrow_conflict_shrinks_the_no_good() {
        let report = verify_no_good_oracle(&NoGoodOracleRequest {
            conflict_members: vec!["a".to_owned()],
            conflict_reference: Some("proof-1".to_owned()), ..binary_request() }).unwrap();
        assert!(report.accepted);
        assert_eq!(report.cut_count, 1);
        assert_eq!(report.cuts[0].coefficient_count, 1);
    }

    #[test]
    fn a_conflict_outside_the_assignment_is_refused() {
        let report = verify_no_good_oracle(&NoGoodOracleRequest {
            conflict_members: vec!["c".to_owned()],
            conflict_reference: Some("proof-1".to_owned()), ..binary_request() }).unwrap();
        assert!(!report.accepted);
        assert!(report.refusal.as_deref().unwrap().contains("outside the current assignment"));
    }

    #[test]
    fn a_missing_reference_is_refused() {
        let report = verify_no_good_oracle(&NoGoodOracleRequest {
            report_reference: None, ..binary_request() }).unwrap();
        assert!(!report.accepted);
        assert!(report.refusal.as_deref().unwrap().contains("requires a proof reference"));
    }

    #[test]
    fn the_context_entry_uses_its_own_reference() {
        let report = verify_no_good_oracle(&NoGoodOracleRequest {
            entry: Some("withContext".to_owned()),
            context_reference: Some("attempt-1::anything".to_owned()),
            ..binary_request() }).unwrap();
        assert!(report.accepted);
        assert_eq!(report.proof_references, vec!["attempt-1::anything".to_owned()]);
        // **按实测纠正**：context 入口不拿引用去和报告比对，但空白引用仍被 `MasterCut::normalize`
        // 拒掉（"verified master cut must carry a proof reference"）——所以它"不校验来源"，不是"什么都收"。
        // **Corrected by measurement**: the context entry never compares the reference against the report, yet a blank
        // one is still refused by `MasterCut::normalize` ("verified master cut must carry a proof reference"), so it
        // "skips provenance", not "accepts anything".
        let blank = verify_no_good_oracle(&NoGoodOracleRequest {
            entry: Some("withContext".to_owned()),
            context_reference: Some(String::new()), ..binary_request() }).unwrap();
        assert!(!blank.accepted);
        assert!(blank.refusal.as_deref().unwrap().contains("must carry a proof reference"));
    }

    #[test]
    fn only_an_infeasible_result_reaches_the_oracle() {
        let feasible = verify_no_good_oracle(&NoGoodOracleRequest {
            result_variant: Some("feasible".to_owned()), ..binary_request() }).unwrap();
        assert!(!feasible.accepted);
        assert!(feasible.refusal.as_deref().unwrap().contains("requires an infeasible subproblem"));
        let failed = verify_no_good_oracle(&NoGoodOracleRequest {
            result_variant: Some("failed".to_owned()), ..binary_request() }).unwrap();
        assert!(!failed.accepted);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_no_good_oracle(&NoGoodOracleRequest { entry: Some("nope".to_owned()),
            ..binary_request() }).unwrap_err().message.contains("unknown entry"));
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            result_variant: Some("nope".to_owned()), ..binary_request() }).unwrap_err().message
            .contains("unknown result variant"));
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            bindings: vec![OracleBindingLiteral { id: "a".to_owned(),
                kind: Some("nope".to_owned()), lower: None, upper: None }],
            ..binary_request() }).unwrap_err().message.contains("unknown binding kind"));
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            bindings: vec![OracleBindingLiteral { id: "a".to_owned(),
                kind: Some("integer".to_owned()), lower: None, upper: None }],
            ..binary_request() }).unwrap_err().message.contains("needs a lower bound"));
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            bindings: vec![OracleBindingLiteral { id: "  ".to_owned(), kind: None,
                lower: None, upper: None }], ..binary_request() }).unwrap_err().message
            .contains("binding IDs must not be blank"));
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            assignment: vec![OracleAssignmentEntry { id: "a".to_owned(), value: 0 },
                OracleAssignmentEntry { id: "a".to_owned(), value: 1 }],
            ..binary_request() }).unwrap_err().message.contains("repeats a stable ID"));
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            assignment: (0..17).map(|index| OracleAssignmentEntry { id: format!("x{index}"),
                value: 0 }).collect(), ..binary_request() }).unwrap_err().message
            .contains("at most 16 entries"));
        // 赋值必须**恰好**覆盖绑定集合：只给一个 ID 时，先撞上"集合不匹配"这条，而不是"超域"。
        // The assignment must cover **exactly** the binding set: with a single ID the set-mismatch check fires first, not
        // the domain check.
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            assignment: vec![OracleAssignmentEntry { id: "a".to_owned(), value: 1 }],
            ..binary_request() }).unwrap_err().message.contains("does not match the stable binding set"));
        assert!(verify_no_good_oracle(&NoGoodOracleRequest {
            assignment: vec![OracleAssignmentEntry { id: "a".to_owned(), value: 9 },
                OracleAssignmentEntry { id: "b".to_owned(), value: 0 }],
            ..binary_request() }).unwrap_err().message.contains("outside its domain"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_no_good_oracle(&integer_request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"cutCount\""), "{json}");
        assert!(json.contains("\"binaryDomainsYieldASingleNoGood\""), "{json}");
        assert!(json.contains("\"proofReferences\""), "{json}");
        assert!(json.contains("\"rowKind\""), "{json}");
        assert!(!json.contains("cut_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<NoGoodOracleReport>(&copy.to_string()).is_err());
    }
}
