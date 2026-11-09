//! 续挖第 104 单：**报告的 gap 是算出来的，不是抄进来的**——`absolute_gap` 必须等于
//! `|incumbent − best_bound|`，`relative_gap` 的分母**以 1 为地板**（`max(|objective|, 1)`，于是目标值小于 1 时
//! 相对 gap 可以**大于 1**）；负数与非有限值一律被拒；逐轮快照还有四条独立不变量：**编号从 1 起且连续**、
//! **阶段名不得为空**、**累计条目数不得下降**、**每轮的 gap 必须与该轮自己的目标值和下界自洽**
//! （`solver/report/mod.rs` 的 `SolveReport::validate` 的 gap 与 trace 两段）
//! Continuation, delivery 104: **the report's gap is computed, never copied in** — `absolute_gap` must equal
//! `|incumbent - best_bound|`, the `relative_gap` denominator **floors at one** (`max(|objective|, 1)`), so with an
//! objective below one the relative gap is **capped at the absolute gap**, and it can exceed one only when the absolute
//! gap does; **when either side is missing a gap is forbidden outright rather than skipped**; negative and non-finite values are refused; and the iteration
//! trace carries four independent invariants: **one-based contiguous numbering**, **a non-blank stage**, **a cumulative
//! item count that never decreases**, and **a per-snapshot gap consistent with that snapshot's own objective and bound**
//! (the gap and trace sections of `SolveReport::validate` in `solver/report/mod.rs`).
//!
//! 第 103 单量的是证明合同，这一单量**报告里的算术自洽**。八条实测：
//!
//! 1. **绝对 gap 必须等于 incumbent 与下界之差**，且**缺任一侧时 gap 一律不许出现**
//!    （`the_absolute_gap_must_match_the_incumbent_and_bound()`）。
//! 2. **相对 gap 的分母以 1 为地板**（`the_relative_gap_floors_its_denominator_at_one()`）：
//!    目标值 `0.5`、下界 `0` → 相对 gap `0.5`（不设地板会是 `1.0`，被拒）；目标 `0.5` 配下界 `-3` →
//!    绝对 `3.5`、相对 `3.5`，**可以大于 1**。
//! 3. **负 gap 被拒**（`a_negative_gap_is_refused()`）。
//! 4. **非有限 gap 被拒**（`a_non_finite_gap_is_refused()`）。
//! 5. **逐轮编号从 1 起且连续**（`the_iteration_numbers_are_one_based_and_contiguous()`）。
//! 6. **阶段名不得为空**（`a_blank_iteration_stage_is_refused()`）。
//! 7. **累计条目数不得下降**（`the_cumulative_item_count_never_decreases()`）。
//! 8. **每轮 gap 与该轮自己的目标值和下界自洽**（`the_snapshot_gap_must_match_its_own_numbers()`）。
//!
//! Delivery 103 measured the proof contracts; this one measures **the arithmetic coherence inside the report**. **The
//! absolute gap must match the incumbent and the bound.** **The relative gap floors its denominator at one**, so it is capped at the
//! absolute gap and exceeds one only when that does. **A negative gap is refused.** **A non-finite gap is refused.** **Iteration numbers are one-based and
//! contiguous.** **A blank stage is refused.** **The cumulative item count never decreases.** And **each snapshot's gap
//! must agree with that snapshot's own objective and bound.**

use ospf_rust_core::solver::report::{
    ProblemStatus, ProofKind, ProofReliability, ProofCompleteness, ProofStatus, SolveDiagnostics,
    SolveFingerprints, SolveIterationSnapshot, SolveProof, SolveReport, SolveSolution,
    SolveStatistics, SolveTrace, SolutionPresence, SolverProvenance, TerminationReason,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条逐轮快照的描述。 / The description of one iteration snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotSpec {
    /// 轮次编号。 / The iteration number.
    pub iteration: u32,
    /// 阶段名。 / The stage name.
    pub stage: String,
    /// 该轮目标值。 / The objective value at that iteration.
    #[serde(default)]
    pub objective_value: Option<f64>,
    /// 该轮下界。 / The best bound at that iteration.
    #[serde(default)]
    pub best_bound: Option<f64>,
    /// 该轮相对 gap。 / The relative gap at that iteration.
    #[serde(default)]
    pub relative_gap: Option<f64>,
    /// 该轮新增条目。 / Items added at that iteration.
    #[serde(default)]
    pub items_added: Option<u32>,
    /// 该轮累计条目。 / The cumulative item count at that iteration.
    #[serde(default)]
    pub items_total: Option<u32>,
}

/// gap 探针请求。 / The gap probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GapProbeRequest {
    /// incumbent 的目标值（默认 0）。 / The incumbent objective value.
    #[serde(default)]
    pub objective_value: Option<f64>,
    /// 统计里的下界（默认缺省）。 / The statistics best bound.
    #[serde(default)]
    pub best_bound_value: Option<f64>,
    /// 绝对 gap（缺省表示不填）。 / The absolute gap; absent means omitted.
    #[serde(default)]
    pub absolute_gap: Option<f64>,
    /// 相对 gap（缺省表示不填）。 / The relative gap; absent means omitted.
    #[serde(default)]
    pub relative_gap: Option<f64>,
    /// 逐轮快照。 / The iteration snapshots.
    #[serde(default)]
    pub snapshots: Vec<SnapshotSpec>,
}

/// gap 探针结论。 / The gap probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GapProbeReport {
    /// 报告是否通过 `validate()`。 / Whether the report validates.
    pub report_valid: bool,
    /// 校验错误文本（通过时为空）。 / The validation error text, empty when it passes.
    pub report_error: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_absolute_gap_must_match_the_incumbent_and_bound: bool,
    pub the_relative_gap_floors_its_denominator_at_one: bool,
    pub a_negative_gap_is_refused: bool,
    pub a_non_finite_gap_is_refused: bool,
    pub the_iteration_numbers_are_one_based_and_contiguous: bool,
    pub a_blank_iteration_stage_is_refused: bool,
    pub the_cumulative_item_count_never_decreases: bool,
    pub the_snapshot_gap_must_match_its_own_numbers: bool,
}

/// 造一条快照。 / Build one snapshot.
fn snapshot_of(spec: &SnapshotSpec) -> SolveIterationSnapshot<u32> {
    SolveIterationSnapshot {
        iteration: spec.iteration,
        stage: spec.stage.clone(),
        objective_value: spec.objective_value,
        best_bound: spec.best_bound,
        relative_gap: spec.relative_gap,
        items_added: spec.items_added.unwrap_or(0),
        items_total: spec.items_total.unwrap_or(0),
        proof_reference: None,
        master_attempt_id: None,
        subproblem_attempt_id: None,
        master_problem_status: None,
        master_termination_reason: None,
        subproblem_problem_status: None,
        subproblem_termination_reason: None,
        bound_valid: None,
    }
}

/// 造一份报告：证明与状态都合法，只让 gap 与快照可变。
/// Build a report whose proof and statuses are legal, varying only the gap and the snapshots.
fn report_of(request: &GapProbeRequest) -> SolveReport<f64, u32> {
    let objective = request.objective_value.unwrap_or(0.0);
    SolveReport {
        schema_version: "gap-probe".to_owned(),
        problem_status: ProblemStatus::Feasible,
        termination_reason: TerminationReason::Completed,
        solution_presence: SolutionPresence::Optimal,
        solution: Some(SolveSolution {
            value: Some(objective),
            values: vec![objective],
            stable_values: Default::default(),
            objective: Some(objective),
            objective_value: Some(objective),
            dual_solution: Some(vec![1.0]),
            quadratic_dual_solution: None,
            pool: Vec::new(),
        }),
        proof: Some(SolveProof {
            kind: ProofKind::Optimality,
            status: ProofStatus::Verified,
            reliability: ProofReliability::Exact,
            completeness: ProofCompleteness::Complete,
            reference: None,
            evidence: None,
        }),
        statistics: SolveStatistics {
            solve_time: std::time::Duration::ZERO,
            iterations: None,
            nodes: None,
            best_bound: None,
            best_bound_value: request.best_bound_value,
            absolute_gap: request.absolute_gap,
            relative_gap: request.relative_gap,
            solution_count: None,
            extensions: Default::default(),
        },
        diagnostics: SolveDiagnostics::default(),
        provenance: SolverProvenance::default(),
        fingerprints: SolveFingerprints::default(),
        model_mapping: None,
        warnings: Vec::new(),
        trace: SolveTrace {
            iteration_snapshots: request.snapshots.iter().map(snapshot_of).collect(),
            ..SolveTrace::default()
        },
    }
}

/// 走一遍 gap 探针。 / Walk the probe.
pub fn verify_gap_probe(request: &GapProbeRequest) -> Result<GapProbeReport> {
    if request.snapshots.len() > 4 {
        return Err(BridgeError::new("InvalidInput", "gap probe failed validation: \
            at most four snapshots are accepted"));
    }
    let findings = canonical_findings();
    let report = report_of(request);
    let validation = report.validate();
    Ok(GapProbeReport {
        report_valid: validation.is_ok(),
        report_error: validation.err().map(|error| error.to_string()).unwrap_or_default(),
        the_absolute_gap_must_match_the_incumbent_and_bound:
            findings.the_absolute_gap_must_match_the_incumbent_and_bound,
        the_relative_gap_floors_its_denominator_at_one:
            findings.the_relative_gap_floors_its_denominator_at_one,
        a_negative_gap_is_refused: findings.a_negative_gap_is_refused,
        a_non_finite_gap_is_refused: findings.a_non_finite_gap_is_refused,
        the_iteration_numbers_are_one_based_and_contiguous:
            findings.the_iteration_numbers_are_one_based_and_contiguous,
        a_blank_iteration_stage_is_refused: findings.a_blank_iteration_stage_is_refused,
        the_cumulative_item_count_never_decreases:
            findings.the_cumulative_item_count_never_decreases,
        the_snapshot_gap_must_match_its_own_numbers:
            findings.the_snapshot_gap_must_match_its_own_numbers,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_absolute_gap_must_match_the_incumbent_and_bound: bool,
    the_relative_gap_floors_its_denominator_at_one: bool,
    a_negative_gap_is_refused: bool,
    a_non_finite_gap_is_refused: bool,
    the_iteration_numbers_are_one_based_and_contiguous: bool,
    a_blank_iteration_stage_is_refused: bool,
    the_cumulative_item_count_never_decreases: bool,
    the_snapshot_gap_must_match_its_own_numbers: bool,
}

/// 一份全默认的请求。 / A fully defaulted request.
fn plain() -> GapProbeRequest {
    GapProbeRequest { objective_value: Some(2.0), best_bound_value: Some(0.0),
        absolute_gap: Some(2.0), relative_gap: Some(1.0), snapshots: Vec::new() }
}

/// 一条全默认的快照。 / A fully defaulted snapshot.
fn snapshot(iteration: u32) -> SnapshotSpec {
    SnapshotSpec { iteration, stage: "master".to_owned(), objective_value: None,
        best_bound: None, relative_gap: None, items_added: Some(1), items_total: Some(1) }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let valid = |request: GapProbeRequest| report_of(&request).validate().is_ok();
        // 1) 绝对 gap 必须等于 incumbent 与下界之差。 / The absolute gap must match.
        let the_absolute_gap_must_match_the_incumbent_and_bound = {
            valid(plain())
                && !valid(GapProbeRequest { absolute_gap: Some(3.0), ..plain() })
                // 缺任一侧时 gap 不是"跳过算术"，而是**整体不许出现**。 
                // When either side is missing the gap is not skipped but **forbidden outright**.
                && !valid(GapProbeRequest { best_bound_value: None, absolute_gap: Some(2.0),
                    ..plain() })
                && !valid(GapProbeRequest { objective_value: None, absolute_gap: Some(2.0),
                    ..plain() })
                // 两侧都缺、gap 也缺 → 合法。 / Both sides and the gap absent is fine.
                && valid(GapProbeRequest { objective_value: None, best_bound_value: None,
                    absolute_gap: None, relative_gap: None, ..plain() })
        };
        // 2) 相对 gap 的分母以 1 为地板。 / The relative gap floors its denominator at one.
        let the_relative_gap_floors_its_denominator_at_one = {
            // 目标值 2、下界 0 → 相对 gap 1。 / Objective two and bound zero give a gap of one.
            valid(plain())
                // 目标值 0.5、下界 0 → 分母取 1，相对 gap 0.5；不设地板会是 1.0，被拒。 
                // With objective a half the denominator floors at one, so the gap is 0.5; without the floor it
                // would be one, which is refused.
                && valid(GapProbeRequest { objective_value: Some(0.5),
                    absolute_gap: Some(0.5), relative_gap: Some(0.5), ..plain() })
                && !valid(GapProbeRequest { objective_value: Some(0.5),
                    absolute_gap: Some(0.5), relative_gap: Some(1.0), ..plain() })
                // 目标 0.5、下界 −3 → 绝对 3.5、相对 3.5，**可以大于 1**。 
                // Objective a half against a bound of minus three gives 3.5 for both, which exceeds one.
                && valid(GapProbeRequest { objective_value: Some(0.5), best_bound_value: Some(-3.0),
                    absolute_gap: Some(3.5), relative_gap: Some(3.5), ..plain() })
        };
        // 3) 负 gap 被拒。 / A negative gap is refused.
        let a_negative_gap_is_refused = {
            !valid(GapProbeRequest { absolute_gap: Some(-1.0), ..plain() })
                && !valid(GapProbeRequest { relative_gap: Some(-0.5), ..plain() })
        };
        // 4) 非有限 gap 被拒。 / A non-finite gap is refused.
        let a_non_finite_gap_is_refused = {
            !valid(GapProbeRequest { absolute_gap: Some(f64::INFINITY), ..plain() })
                && !valid(GapProbeRequest { relative_gap: Some(f64::NAN), ..plain() })
        };
        // 5) 逐轮编号从 1 起且连续。 / Iteration numbers are one-based and contiguous.
        let the_iteration_numbers_are_one_based_and_contiguous = {
            valid(GapProbeRequest { snapshots: vec![snapshot(1), snapshot(2)], ..plain() })
                // 从 0 起被拒。 / Starting at zero is refused.
                && !valid(GapProbeRequest { snapshots: vec![snapshot(0)], ..plain() })
                // 跳号被拒。 / A gap in the numbering is refused.
                && !valid(GapProbeRequest { snapshots: vec![snapshot(1), snapshot(3)], ..plain() })
        };
        // 6) 阶段名不得为空。 / A blank stage is refused.
        let a_blank_iteration_stage_is_refused = {
            let blank = SnapshotSpec { stage: "   ".to_owned(), ..snapshot(1) };
            !valid(GapProbeRequest { snapshots: vec![blank], ..plain() })
        };
        // 7) 累计条目数不得下降。 / The cumulative item count never decreases.
        let the_cumulative_item_count_never_decreases = {
            // 单轮里"累计小于新增"被拒。 / Within one snapshot the total may not be below the increment.
            let inverted = SnapshotSpec { items_added: Some(3), items_total: Some(2),
                ..snapshot(1) };
            !valid(GapProbeRequest { snapshots: vec![inverted], ..plain() })
                // 跨轮下降被拒。 / A decrease across iterations is refused.
                && !valid(GapProbeRequest { snapshots: vec![
                    SnapshotSpec { items_added: Some(2), items_total: Some(5), ..snapshot(1) },
                    SnapshotSpec { items_added: Some(1), items_total: Some(3), ..snapshot(2) },
                ], ..plain() })
                // 持平与上升都可以。 / Holding steady and rising are both fine.
                && valid(GapProbeRequest { snapshots: vec![
                    SnapshotSpec { items_added: Some(2), items_total: Some(5), ..snapshot(1) },
                    SnapshotSpec { items_added: Some(1), items_total: Some(5), ..snapshot(2) },
                ], ..plain() })
        };
        // 8) 每轮 gap 与该轮自己的数字自洽。 / Each snapshot's gap agrees with its own numbers.
        let the_snapshot_gap_must_match_its_own_numbers = {
            // 目标 4、下界 2 → gap 0.5。 / Objective four and bound two give a gap of a half.
            let honest = SnapshotSpec { objective_value: Some(4.0), best_bound: Some(2.0),
                relative_gap: Some(0.5), ..snapshot(1) };
            let lying = SnapshotSpec { relative_gap: Some(0.25), ..honest.clone() };
            valid(GapProbeRequest { snapshots: vec![honest], ..plain() })
                && !valid(GapProbeRequest { snapshots: vec![lying], ..plain() })
                // 三缺一时不做这条算术。 / With any of the three missing the arithmetic is skipped.
                && valid(GapProbeRequest { snapshots: vec![
                    SnapshotSpec { objective_value: Some(4.0), best_bound: Some(2.0),
                        relative_gap: None, ..snapshot(1) }], ..plain() })
        };
        Findings {
            the_absolute_gap_must_match_the_incumbent_and_bound,
            the_relative_gap_floors_its_denominator_at_one,
            a_negative_gap_is_refused,
            a_non_finite_gap_is_refused,
            the_iteration_numbers_are_one_based_and_contiguous,
            a_blank_iteration_stage_is_refused,
            the_cumulative_item_count_never_decreases,
            the_snapshot_gap_must_match_its_own_numbers,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: GapProbeRequest) -> GapProbeReport {
        verify_gap_probe(&request).unwrap()
    }

    fn snapshot(iteration: u32) -> SnapshotSpec {
        SnapshotSpec { iteration, stage: "master".to_owned(), objective_value: None,
            best_bound: None, relative_gap: None, items_added: Some(1), items_total: Some(1) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(plain());
        assert!(report.report_valid, "{}", report.report_error);
        assert!(report.the_absolute_gap_must_match_the_incumbent_and_bound);
        assert!(report.the_relative_gap_floors_its_denominator_at_one);
        assert!(report.a_negative_gap_is_refused);
        assert!(report.a_non_finite_gap_is_refused);
        assert!(report.the_iteration_numbers_are_one_based_and_contiguous);
        assert!(report.a_blank_iteration_stage_is_refused);
        assert!(report.the_cumulative_item_count_never_decreases);
        assert!(report.the_snapshot_gap_must_match_its_own_numbers);
    }

    #[test]
    fn the_absolute_gap_must_match() {
        let wrong = ask(GapProbeRequest { absolute_gap: Some(3.0), ..plain() });
        assert!(!wrong.report_valid);
        assert!(wrong.report_error.contains("absolute gap"), "{}", wrong.report_error);
        // 缺任一侧时 gap 整体不许出现。 / A gap is forbidden when either side is missing.
        let orphan = ask(GapProbeRequest { best_bound_value: None, absolute_gap: Some(2.0),
            ..plain() });
        assert!(!orphan.report_valid);
        assert!(orphan.report_error.contains("requires both"), "{}", orphan.report_error);
        assert!(ask(GapProbeRequest { objective_value: None, best_bound_value: None,
            absolute_gap: None, relative_gap: None, ..plain() }).report_valid);
    }

    #[test]
    fn the_relative_gap_floors_its_denominator_at_one() {
        // 目标 0.5、下界 0 → 分母取 1，相对 gap 0.5。 
        // With objective a half the denominator floors at one, so the gap is 0.5.
        assert!(ask(GapProbeRequest { objective_value: Some(0.5), absolute_gap: Some(0.5),
            relative_gap: Some(0.5), ..plain() }).report_valid);
        // 按"不设地板"算成 1 就被拒。 / Computed without the floor it is refused.
        let floored = ask(GapProbeRequest { objective_value: Some(0.5), absolute_gap: Some(0.5),
            relative_gap: Some(1.0), ..plain() });
        assert!(!floored.report_valid);
        assert!(floored.report_error.contains("relative gap"), "{}", floored.report_error);
        // 目标 0.5 配下界 −3 → 绝对与相对都是 3.5，可以大于 1。 
        // Objective a half against a bound of minus three gives 3.5 twice, which exceeds one.
        assert!(ask(GapProbeRequest { objective_value: Some(0.5), best_bound_value: Some(-3.0),
            absolute_gap: Some(3.5), relative_gap: Some(3.5), ..plain() }).report_valid);
    }

    #[test]
    fn negative_and_non_finite_gaps_are_refused() {
        assert!(!ask(GapProbeRequest { absolute_gap: Some(-1.0), ..plain() }).report_valid);
        assert!(!ask(GapProbeRequest { relative_gap: Some(-0.5), ..plain() }).report_valid);
        assert!(!ask(GapProbeRequest { absolute_gap: Some(f64::INFINITY), ..plain() })
            .report_valid);
        assert!(!ask(GapProbeRequest { relative_gap: Some(f64::NAN), ..plain() }).report_valid);
    }

    #[test]
    fn iteration_numbers_are_one_based_and_contiguous() {
        assert!(ask(GapProbeRequest { snapshots: vec![snapshot(1), snapshot(2)], ..plain() })
            .report_valid);
        let zero = ask(GapProbeRequest { snapshots: vec![snapshot(0)], ..plain() });
        assert!(!zero.report_valid);
        assert!(zero.report_error.contains("one-based"), "{}", zero.report_error);
        assert!(!ask(GapProbeRequest { snapshots: vec![snapshot(1), snapshot(3)], ..plain() })
            .report_valid);
    }

    #[test]
    fn a_blank_stage_is_refused() {
        let blank = ask(GapProbeRequest { snapshots: vec![SnapshotSpec {
            stage: "  ".to_owned(), ..snapshot(1) }], ..plain() });
        assert!(!blank.report_valid);
        assert!(blank.report_error.contains("blank"), "{}", blank.report_error);
    }

    #[test]
    fn the_cumulative_item_count_never_decreases() {
        let inverted = ask(GapProbeRequest { snapshots: vec![SnapshotSpec {
            items_added: Some(3), items_total: Some(2), ..snapshot(1) }], ..plain() });
        assert!(!inverted.report_valid);
        let falling = ask(GapProbeRequest { snapshots: vec![
            SnapshotSpec { items_added: Some(2), items_total: Some(5), ..snapshot(1) },
            SnapshotSpec { items_added: Some(1), items_total: Some(3), ..snapshot(2) },
        ], ..plain() });
        assert!(!falling.report_valid);
        assert!(ask(GapProbeRequest { snapshots: vec![
            SnapshotSpec { items_added: Some(2), items_total: Some(5), ..snapshot(1) },
            SnapshotSpec { items_added: Some(1), items_total: Some(5), ..snapshot(2) },
        ], ..plain() }).report_valid);
    }

    #[test]
    fn a_snapshot_gap_must_match_its_own_numbers() {
        let honest = SnapshotSpec { objective_value: Some(4.0), best_bound: Some(2.0),
            relative_gap: Some(0.5), ..snapshot(1) };
        assert!(ask(GapProbeRequest { snapshots: vec![honest.clone()], ..plain() }).report_valid);
        let lying = ask(GapProbeRequest { snapshots: vec![SnapshotSpec {
            relative_gap: Some(0.25), ..honest }], ..plain() });
        assert!(!lying.report_valid);
        assert!(lying.report_error.contains("iteration trace gap"), "{}", lying.report_error);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_gap_probe(&GapProbeRequest {
            snapshots: (0..5).map(snapshot).collect(), ..plain() }).unwrap_err()
            .message.contains("at most four snapshots"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(plain());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"reportValid\""), "{json}");
        assert!(json.contains("\"theRelativeGapFloorsItsDenominatorAtOne\""), "{json}");
        assert!(!json.contains("report_valid"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<GapProbeReport>(&copy.to_string()).is_err());
    }
}
