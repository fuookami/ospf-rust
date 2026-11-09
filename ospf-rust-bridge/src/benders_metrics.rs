//! 批次 E 第 1 单：Benders 停止原因、运行时指标与逐轮快照 / Batch E, delivery 1: Benders stop reasons, runtime metrics and
//! per-iteration snapshots.
//!
//! `BendersStopReason` / `BendersRuntimeMetrics` / `BendersIterationSnapshot` 在领域里只有 `Debug + Clone`——没有 serde，也就
//! 从来没有一条线格式把它们送到 TS 侧。这一单补上两份东西：
//!
//! 1. **停止原因的词表**：`iterationLimit` / `cutStall` / `objectiveStall` / `masterTermination:<终止原因>` /
//!    `subproblemTermination:<终止原因>`。这是**桥接定义的线格式词表**（领域没有），所以代码里明确标注；嵌套的
//!    `<终止原因>` 直接用核心自己的 `SCREAMING_SNAKE_CASE` 编码（`COMPLETED` / `TIME_LIMIT` / …），不另造一套。
//! 2. **指标的线格式与校验**：把一份指标文档映射成真实的领域结构（这一步由编译器证明映射成立，而不是靠字符串），
//!    再按**字段文档语义**做一致性校验，最后投影成 TS 可读的摘要。
//!
//! 校验规则都从字段含义直接推出，且都属于**桥接层一致性检查**（领域没有 `validate`，代码里逐条注明）：
//! 快照轮次必须从 1 开始、严格递增、互不重复且不超过已执行轮数；`totalCuts` 必须等于最后一轮的累计值，
//! 且等于各轮 `cutsAdded` 之和；`bestSolutionIteration` 必须落在已执行的轮次内；两个停滞计数不得超过已执行轮数；
//! `masterObj` 与可选的 `masterGap` / `masterBound` 必须有限（`f64` 非有限值在 JSON 里会变成 `null`，所以必须在 DTO
//! 边界就拒绝）。
//!
//! In the domain these three types are only `Debug + Clone` — no serde — so no wire format has ever carried them to
//! TypeScript. This delivery adds two things. First, the **stop-reason vocabulary**: `iterationLimit` / `cutStall` /
//! `objectiveStall` / `masterTermination:<reason>` / `subproblemTermination:<reason>`. This vocabulary is **defined by the
//! bridge** (the domain has none) and is marked as such; the nested reason uses core's own `SCREAMING_SNAKE_CASE` encoding
//! rather than inventing a second one. Second, the **metrics wire format and its validation**: a metrics document is mapped
//! onto the real domain structs (so the compiler proves the mapping, not a string), checked against the documented field
//! semantics, and projected into a summary. Every rule follows directly from a field's meaning and is a **bridge-level
//! consistency check** (the domain has no `validate`; the code says so): snapshot iterations start at one, increase
//! strictly, are unique and never exceed the executed count; `totalCuts` equals the last cumulative value and the sum of
//! per-iteration additions; `bestSolutionIteration` falls inside the executed range; both stall counters stay within the
//! executed count; and `masterObj` with the optional gap and bound must be finite (a non-finite `f64` becomes `null` in
//! JSON, so it must be refused at the DTO boundary).

use ospf_rust_core::solver::TerminationReason;
use ospf_rust_framework::solver::column_generation_solver::{
    BendersIterationSnapshot, BendersRuntimeMetrics, BendersStopReason,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// Benders 指标文档。 / The Benders metrics document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersMetricsRequest {
    /// 实际执行迭代轮数。 / The executed iteration count.
    pub executed_iterations: u64,
    /// 最优可行解出现轮次（从 1 开始）。 / The iteration where the best feasible solution appeared (1-based).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub best_solution_iteration: Option<u64>,
    /// 累计新增切割数。 / The total number of newly added cuts.
    pub total_cuts: u64,
    /// 结束时连续无新 cut 轮数。 / Consecutive no-cut iterations at stop.
    pub no_cut_iterations: u64,
    /// 结束时连续目标改进不足轮数。 / Consecutive objective-stall iterations at stop.
    pub no_obj_improvement_iterations: u64,
    /// 触发停止原因（线格式代码，见模块文档）。 / The stop reason (wire code, see the module docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stop_reason: Option<String>,
    /// 逐轮快照。 / Per-iteration snapshots.
    #[serde(default)]
    pub iteration_snapshots: Vec<BendersSnapshotInput>,
}

/// Benders 逐轮快照。 / One Benders per-iteration snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersSnapshotInput {
    /// 当前轮次（从 1 开始）。 / The iteration index (1-based).
    pub iteration: u64,
    /// 当前 master 目标值。 / The current master objective value.
    pub master_obj: f64,
    /// 当前 master gap。 / The current master gap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_gap: Option<f64>,
    /// 当前 master 有效下界。 / The current master best bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_bound: Option<f64>,
    /// 本轮新增 cut 数。 / Newly added cuts in this iteration.
    pub cuts_added: u64,
    /// 截止本轮累计 cut 数。 / Total cuts up to this iteration.
    pub total_cuts: u64,
    /// 本轮结束时连续无新 cut 轮数。 / Consecutive no-cut iterations at the end of this iteration.
    pub no_cut_iterations: u64,
    /// 本轮结束时连续目标停滞轮数。 / Consecutive objective-stall iterations at the end of this iteration.
    pub no_obj_improvement_iterations: u64,
    /// 本轮 master attempt 身份。 / The master-attempt identity for this iteration.
    pub master_attempt_id: String,
    /// 本轮 subproblem attempt 身份。 / The subproblem-attempt identity for this iteration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub subproblem_attempt_id: Option<String>,
    /// 本轮是否给出可供外层使用的有效界。 / Whether this iteration exposes a valid outer bound.
    #[serde(default)]
    pub bound_valid: bool,
    /// 本轮证书引用。 / The certificate reference for this iteration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proof_reference: Option<String>,
    /// 本轮确定的停止原因。 / The stop reason determined for this iteration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stop_reason: Option<String>,
}

/// 一份逐轮快照的投影。 / The projection of one per-iteration snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersSnapshotRow {
    /// 轮次。 / The iteration index.
    pub iteration: u64,
    /// 本轮新增 cut 数。 / Newly added cuts in this iteration.
    pub cuts_added: u64,
    /// 截止本轮累计 cut 数。 / Total cuts up to this iteration.
    pub total_cuts: u64,
    /// 本轮是否给出有效界。 / Whether this iteration exposes a valid outer bound.
    pub bound_valid: bool,
    /// 本轮停止原因（线格式代码）。 / The stop reason (wire code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stop_reason: Option<String>,
    /// 本轮 master attempt 身份。 / The master-attempt identity for this iteration.
    pub master_attempt_id: String,
}

/// Benders 指标校验结论。 / The Benders metrics verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersMetricsReport {
    /// 实际执行迭代轮数。 / The executed iteration count.
    pub executed_iterations: u64,
    /// 累计新增切割数。 / The total number of newly added cuts.
    pub total_cuts: u64,
    /// 最优可行解出现轮次。 / The iteration where the best feasible solution appeared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub best_solution_iteration: Option<u64>,
    /// 结束时连续无新 cut 轮数。 / Consecutive no-cut iterations at stop.
    pub no_cut_iterations: u64,
    /// 结束时连续目标改进不足轮数。 / Consecutive objective-stall iterations at stop.
    pub no_obj_improvement_iterations: u64,
    /// 停止原因（线格式代码）。 / The stop reason (wire code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stop_reason: Option<String>,
    /// 停止原因的粗分类：`executed` / `cutStall` / `objectiveStall` / `masterTermination` /
    /// `subproblemTermination` / `none`。 / The coarse stop-reason category.
    pub stop_category: String,
    /// 嵌套的终止原因代码（仅 `MasterTermination` / `SubproblemTermination` 有）。
    /// The nested termination-reason code (only for master or subproblem termination).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub nested_termination_reason: Option<String>,
    /// 快照条数。 / The snapshot count.
    pub snapshot_count: u32,
    /// 逐轮快照投影。 / The per-iteration snapshot projections.
    pub snapshots: Vec<BendersSnapshotRow>,
    /// 领域接受的**全部**停止原因代码词表（桥接定义）。 / The full stop-reason vocabulary the domain accepts (bridge-defined).
    pub stop_reason_vocabulary: Vec<String>,
}

/// 校验一份 Benders 指标文档，并投影成摘要。非法输入是**错误**，原因原样带出。
/// Validate a Benders metrics document and project it into a summary. An illegal input is an **error** carrying the reason.
pub fn verify_benders_metrics(request: &BendersMetricsRequest) -> Result<BendersMetricsReport> {
    let executed = usize::try_from(request.executed_iterations).map_err(|_| BridgeError::new("InvalidInput",
        "benders metrics failed validation: executed iteration count exceeds the platform index range"))?;
    let snapshots = request.iteration_snapshots.iter()
        .map(snapshot).collect::<Result<Vec<BendersIterationSnapshot>>>()?;
    let stop_reason = match request.stop_reason.as_deref() {
        None => None,
        Some(code) => Some(parse_stop_reason(code)?),
    };
    let metrics = BendersRuntimeMetrics {
        executed_iterations: executed,
        best_solution_iteration: request.best_solution_iteration
            .map(|iteration| usize::try_from(iteration).unwrap_or(usize::MAX)),
        total_cuts: usize::try_from(request.total_cuts).unwrap_or(usize::MAX),
        no_cut_iterations: usize::try_from(request.no_cut_iterations).unwrap_or(usize::MAX),
        no_obj_improvement_iterations: usize::try_from(request.no_obj_improvement_iterations)
            .unwrap_or(usize::MAX),
        stop_reason,
        iteration_snapshots: snapshots,
    };
    // 以下全部是**桥接层一致性检查**（领域没有 `validate`），规则直接来自字段文档语义。
    // Everything below is a **bridge-level consistency check** (the domain has no `validate`); the rules come straight
    // from the documented field semantics.
    if metrics.best_solution_iteration.is_some_and(|iteration|
        iteration == 0 || iteration > metrics.executed_iterations)
    {
        return Err(BridgeError::new("InvalidInput",
            "benders metrics place the best solution outside the executed iterations"));
    }
    if metrics.no_cut_iterations > metrics.executed_iterations
        || metrics.no_obj_improvement_iterations > metrics.executed_iterations
    {
        return Err(BridgeError::new("InvalidInput",
            "benders metrics report more stall iterations than executed iterations"));
    }
    // 快照轮次：从 1 开始、严格递增、互不重复、不超过已执行轮数。
    // Snapshot iterations: start at one, increase strictly, stay unique and never exceed the executed count.
    let mut previous = 0usize;
    for snapshot in &metrics.iteration_snapshots {
        if snapshot.iteration == 0 {
            return Err(BridgeError::new("InvalidInput",
                "benders metrics carry a snapshot with iteration zero (iterations are 1-based)"));
        }
        if snapshot.iteration <= previous {
            return Err(BridgeError::new("InvalidInput",
                "benders metrics snapshots are not in strictly increasing iteration order"));
        }
        if snapshot.iteration > metrics.executed_iterations {
            return Err(BridgeError::new("InvalidInput",
                "benders metrics carry a snapshot beyond the executed iterations"));
        }
        previous = snapshot.iteration;
    }
    if let Some(last) = metrics.iteration_snapshots.last() {
        // `totalCuts` 与最后一轮的累计值必须一致（两者都是"截止本轮"）。
        // `totalCuts` must agree with the last cumulative value (both mean "up to this iteration").
        if last.total_cuts != metrics.total_cuts {
            return Err(BridgeError::new("InvalidInput",
                "benders metrics total cuts disagree with the last snapshot"));
        }
        // 逐轮 `cutsAdded` 之和必须等于累计值。 / The per-iteration additions must sum to the cumulative total.
        let added = metrics.iteration_snapshots.iter()
            .try_fold(0usize, |total, snapshot| total.checked_add(snapshot.cuts_added));
        if added != Some(metrics.total_cuts) {
            return Err(BridgeError::new("InvalidInput",
                "benders metrics per-iteration cut additions do not sum to the total"));
        }
    } else if metrics.total_cuts != 0 {
        // 没有快照就不该有切割记录。 / With no snapshots there can be no recorded cuts.
        return Err(BridgeError::new("InvalidInput",
            "benders metrics report cuts without any iteration snapshot"));
    }
    let (stop_category, nested) = match &metrics.stop_reason {
        None => ("none".to_owned(), None),
        Some(BendersStopReason::IterationLimit) => ("executed".to_owned(), None),
        Some(BendersStopReason::CutStall) => ("cutStall".to_owned(), None),
        Some(BendersStopReason::ObjectiveStall) => ("objectiveStall".to_owned(), None),
        Some(BendersStopReason::MasterTermination(reason)) =>
            ("masterTermination".to_owned(), Some(termination_code(*reason))),
        Some(BendersStopReason::SubproblemTermination(reason)) =>
            ("subproblemTermination".to_owned(), Some(termination_code(*reason))),
    };
    Ok(BendersMetricsReport {
        executed_iterations: request.executed_iterations,
        total_cuts: request.total_cuts,
        best_solution_iteration: request.best_solution_iteration,
        no_cut_iterations: request.no_cut_iterations,
        no_obj_improvement_iterations: request.no_obj_improvement_iterations,
        stop_reason: metrics.stop_reason.map(stop_reason_code),
        stop_category,
        nested_termination_reason: nested,
        snapshot_count: u32::try_from(metrics.iteration_snapshots.len()).unwrap_or(u32::MAX),
        snapshots: metrics.iteration_snapshots.iter().map(|snapshot| BendersSnapshotRow {
            iteration: u64::try_from(snapshot.iteration).unwrap_or(u64::MAX),
            cuts_added: u64::try_from(snapshot.cuts_added).unwrap_or(u64::MAX),
            total_cuts: u64::try_from(snapshot.total_cuts).unwrap_or(u64::MAX),
            bound_valid: snapshot.bound_valid,
            stop_reason: snapshot.stop_reason.map(stop_reason_code),
            master_attempt_id: snapshot.master_attempt_id.clone(),
        }).collect(),
        stop_reason_vocabulary: stop_reason_vocabulary(),
    })
}

/// 把一条快照输入转成领域结构，并在 DTO 边界拒绝非有限数值。
/// Convert one snapshot input into the domain struct and refuse non-finite numbers at the DTO boundary.
fn snapshot(input: &BendersSnapshotInput) -> Result<BendersIterationSnapshot> {
    let label = format!("benders snapshot {}", input.iteration);
    // 非有限 `f64` 在 JSON 里会变成 `null`，所以只能在 DTO 边界拦。
    // A non-finite `f64` becomes `null` in JSON, so it can only be caught at the DTO boundary.
    for (name, value) in [("master objective", Some(input.master_obj)), ("master gap", input.master_gap),
        ("master bound", input.master_bound)]
    {
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(BridgeError::new("InvalidInput",
                &format!("{label} carries a non-finite {name}")));
        }
    }
    if input.master_attempt_id.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            &format!("{label} carries a blank master attempt identity")));
    }
    if input.subproblem_attempt_id.as_deref().is_some_and(|id| id.trim().is_empty()) {
        return Err(BridgeError::new("InvalidInput",
            &format!("{label} carries a blank subproblem attempt identity")));
    }
    let stop = match input.stop_reason.as_deref() {
        None => None,
        Some(code) => Some(parse_stop_reason(code)?),
    };
    Ok(BendersIterationSnapshot {
        iteration: usize::try_from(input.iteration).unwrap_or(usize::MAX),
        master_obj: input.master_obj,
        master_gap: input.master_gap,
        master_bound: input.master_bound,
        cuts_added: usize::try_from(input.cuts_added).unwrap_or(usize::MAX),
        total_cuts: usize::try_from(input.total_cuts).unwrap_or(usize::MAX),
        no_cut_iterations: usize::try_from(input.no_cut_iterations).unwrap_or(usize::MAX),
        no_obj_improvement_iterations: usize::try_from(input.no_obj_improvement_iterations)
            .unwrap_or(usize::MAX),
        master_attempt_id: input.master_attempt_id.clone(),
        subproblem_attempt_id: input.subproblem_attempt_id.clone(),
        master_report: None,
        subproblem_report: None,
        bound_valid: input.bound_valid,
        proof_reference: input.proof_reference.clone(),
        stop_reason: stop,
    })
}

/// 解析停止原因代码（桥接定义的词表）。 / Parse a stop-reason code (the bridge-defined vocabulary).
fn parse_stop_reason(code: &str) -> Result<BendersStopReason> {
    if let Some(rest) = code.strip_prefix("masterTermination:") {
        return Ok(BendersStopReason::MasterTermination(parse_termination(rest)?));
    }
    if let Some(rest) = code.strip_prefix("subproblemTermination:") {
        return Ok(BendersStopReason::SubproblemTermination(parse_termination(rest)?));
    }
    match code {
        "iterationLimit" => Ok(BendersStopReason::IterationLimit),
        "cutStall" => Ok(BendersStopReason::CutStall),
        "objectiveStall" => Ok(BendersStopReason::ObjectiveStall),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "benders metrics failed validation: unknown stop reason `{other}`"))),
    }
}

/// 解析核心自己的终止原因编码。 / Parse core's own termination-reason encoding.
fn parse_termination(code: &str) -> Result<TerminationReason> {
    // 用核心自己的 serde 表示解析，避免桥接再造一套枚举。
    // Parse through core's own serde representation so the bridge never duplicates the enum.
    serde_json::from_value(serde_json::Value::String(code.to_owned())).map_err(|_| {
        BridgeError::new("InvalidInput", &format!(
            "benders metrics failed validation: unknown termination reason `{code}`"))
    })
}

/// 停止原因的线格式代码。 / The wire code of a stop reason.
fn stop_reason_code(reason: BendersStopReason) -> String {
    match reason {
        BendersStopReason::IterationLimit => "iterationLimit".to_owned(),
        BendersStopReason::CutStall => "cutStall".to_owned(),
        BendersStopReason::ObjectiveStall => "objectiveStall".to_owned(),
        BendersStopReason::MasterTermination(nested) =>
            format!("masterTermination:{}", termination_code(nested)),
        BendersStopReason::SubproblemTermination(nested) =>
            format!("subproblemTermination:{}", termination_code(nested)),
    }
}

/// 核心终止原因的编码（SCREAMING_SNAKE_CASE，走领域 serde）。 / Core's termination encoding via domain serde.
fn termination_code(reason: TerminationReason) -> String {
    serde_json::to_value(reason).ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{reason:?}"))
}

/// 领域接受的停止原因词表。 / The stop-reason vocabulary the domain accepts.
fn stop_reason_vocabulary() -> Vec<String> {
    let mut codes = vec!["iterationLimit".to_owned(), "cutStall".to_owned(), "objectiveStall".to_owned()];
    for reason in [TerminationReason::Completed, TerminationReason::TimeLimit,
        TerminationReason::IterationLimit, TerminationReason::Cancelled,
        TerminationReason::Interrupted, TerminationReason::Unknown]
    {
        codes.push(format!("masterTermination:{}", termination_code(reason)));
        codes.push(format!("subproblemTermination:{}", termination_code(reason)));
    }
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_input(iteration: u64, cuts_added: u64, total_cuts: u64) -> BendersSnapshotInput {
        BendersSnapshotInput {
            iteration, master_obj: -1.5, master_gap: Some(0.25), master_bound: Some(-2.0),
            cuts_added, total_cuts, no_cut_iterations: 0, no_obj_improvement_iterations: 0,
            master_attempt_id: format!("master-{iteration}"),
            subproblem_attempt_id: Some(format!("sub-{iteration}")),
            bound_valid: true, proof_reference: None, stop_reason: None,
        }
    }

    fn request() -> BendersMetricsRequest {
        BendersMetricsRequest {
            executed_iterations: 3, best_solution_iteration: Some(2), total_cuts: 5,
            no_cut_iterations: 0, no_obj_improvement_iterations: 1,
            stop_reason: Some("iterationLimit".to_owned()),
            iteration_snapshots: vec![
                snapshot_input(1, 2, 2), snapshot_input(2, 3, 5), snapshot_input(3, 0, 5)],
        }
    }

    #[test]
    fn metrics_are_validated_and_projected() {
        let report = verify_benders_metrics(&request()).unwrap();
        assert_eq!(report.executed_iterations, 3);
        assert_eq!(report.total_cuts, 5);
        assert_eq!(report.best_solution_iteration, Some(2));
        assert_eq!(report.stop_reason.as_deref(), Some("iterationLimit"));
        assert_eq!(report.stop_category, "executed");
        assert_eq!(report.nested_termination_reason, None);
        assert_eq!(report.snapshot_count, 3);
        assert_eq!(report.snapshots.len(), 3);
        assert_eq!(report.snapshots[1].cuts_added, 3);
        assert_eq!(report.snapshots[1].total_cuts, 5);
        assert_eq!(report.snapshots[1].master_attempt_id, "master-2");
        assert!(report.snapshots.iter().all(|row| row.bound_valid));
        // 词表里既有三种纯桥接代码，也有两类嵌套终止原因的完整交叉。
        // The vocabulary holds the three bridge-only codes and both nested families across the reasons.
        assert!(report.stop_reason_vocabulary.contains(&"iterationLimit".to_owned()));
        assert!(report.stop_reason_vocabulary.contains(&"masterTermination:TIME_LIMIT".to_owned()));
        assert!(report.stop_reason_vocabulary.contains(&"subproblemTermination:UNKNOWN".to_owned()));
    }

    #[test]
    fn every_stop_category_is_reachable_and_named() {
        let mut metrics = request();
        metrics.stop_reason = Some("cutStall".to_owned());
        let report = verify_benders_metrics(&metrics).unwrap();
        assert_eq!(report.stop_category, "cutStall");
        metrics.stop_reason = Some("objectiveStall".to_owned());
        assert_eq!(verify_benders_metrics(&metrics).unwrap().stop_category, "objectiveStall");
        // 嵌套终止原因：master 与 subproblem 两条都要能命名。 / Both nested families must name their reason.
        metrics.stop_reason = Some("masterTermination:TIME_LIMIT".to_owned());
        let report = verify_benders_metrics(&metrics).unwrap();
        assert_eq!(report.stop_category, "masterTermination");
        assert_eq!(report.nested_termination_reason.as_deref(), Some("TIME_LIMIT"));
        metrics.stop_reason = Some("subproblemTermination:CANCELLED".to_owned());
        let report = verify_benders_metrics(&metrics).unwrap();
        assert_eq!(report.stop_category, "subproblemTermination");
        assert_eq!(report.nested_termination_reason.as_deref(), Some("CANCELLED"));
        // 无停止原因。 / No stop reason at all.
        metrics.stop_reason = None;
        let report = verify_benders_metrics(&metrics).unwrap();
        assert_eq!(report.stop_category, "none");
        assert_eq!(report.stop_reason, None);
        // 快照自带停止原因也走同一词表。 / A snapshot's own stop reason uses the same vocabulary.
        metrics.iteration_snapshots[2].stop_reason = Some("cutStall".to_owned());
        assert_eq!(verify_benders_metrics(&metrics).unwrap().snapshots[2].stop_reason.as_deref(),
            Some("cutStall"));
    }

    #[test]
    fn inconsistencies_are_refused() {
        // 最优解轮次越界（含 0：轮次从 1 开始）。 / The best-solution iteration out of range (0 included: 1-based).
        for iteration in [0u64, 4] {
            let mut metrics = request();
            metrics.best_solution_iteration = Some(iteration);
            let error = verify_benders_metrics(&metrics).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains("place the best solution outside"), "{}", error.message);
        }
        // 停滞轮数超过已执行轮数。 / Stall iterations beyond the executed count.
        let mut metrics = request();
        metrics.no_cut_iterations = 9;
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("more stall iterations"));
        // 快照轮次：0 / 乱序 / 重复 / 越界。 / Snapshot iterations: zero, out of order, duplicate, beyond the limit.
        let mut metrics = request();
        metrics.iteration_snapshots[0].iteration = 0;
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("iteration zero"));
        let mut metrics = request();
        metrics.iteration_snapshots.swap(0, 1);
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("strictly increasing"));
        let mut metrics = request();
        metrics.iteration_snapshots[1].iteration = 1;
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("strictly increasing"));
        let mut metrics = request();
        metrics.iteration_snapshots[2].iteration = 9;
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("beyond the executed"));
        // 累计 cut 与最后一轮不符 / 与逐轮之和不符。
        // The cumulative total disagreeing with the last snapshot and with the per-iteration sum.
        let mut metrics = request();
        metrics.total_cuts = 6;
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("disagree with the last snapshot"));
        let mut metrics = request();
        metrics.iteration_snapshots[1].cuts_added = 4;
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("do not sum to the total"));
        // 没有快照却有切割记录。 / Cuts recorded without any snapshot.
        let mut metrics = request();
        metrics.iteration_snapshots.clear();
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("without any iteration snapshot"));
        // 有快照但累计为 0 且逐轮也算出 0：合法。 / Snapshots that genuinely add nothing: legal.
        let mut metrics = request();
        metrics.total_cuts = 0;
        metrics.iteration_snapshots = vec![snapshot_input(1, 0, 0)];
        metrics.executed_iterations = 1;
        metrics.best_solution_iteration = None;
        metrics.no_obj_improvement_iterations = 0;
        assert_eq!(verify_benders_metrics(&metrics).unwrap().total_cuts, 0);
        // 非有限数值在 DTO 边界被拒（JSON 会把它变成 `null`）。 / Non-finite numbers are refused at the DTO boundary.
        let mut metrics = request();
        metrics.iteration_snapshots[0].master_obj = f64::INFINITY;
        let error = verify_benders_metrics(&metrics).unwrap_err();
        assert!(error.message.contains("non-finite master objective"), "{}", error.message);
        let mut metrics = request();
        metrics.iteration_snapshots[0].master_gap = Some(f64::NAN);
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("non-finite master gap"));
        // 空白身份 / 未知停止原因 / 未知终止原因。
        // A blank identity, an unknown stop reason and an unknown termination reason.
        let mut metrics = request();
        metrics.iteration_snapshots[0].master_attempt_id = "  ".to_owned();
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("blank master attempt identity"));
        let mut metrics = request();
        metrics.iteration_snapshots[0].subproblem_attempt_id = Some(String::new());
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("blank subproblem attempt identity"));
        let mut metrics = request();
        metrics.stop_reason = Some("timeoutStall".to_owned());
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("unknown stop reason"));
        let mut metrics = request();
        metrics.stop_reason = Some("masterTermination:NOT_A_REASON".to_owned());
        assert!(verify_benders_metrics(&metrics).unwrap_err().message.contains("unknown termination reason"));
    }
}
