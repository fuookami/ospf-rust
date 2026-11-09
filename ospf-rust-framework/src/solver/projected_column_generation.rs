//! 同源列投影上的 Phase I、精确定价及受限整数主问题。 / Phase I, certified pricing and restricted integer masters on a shared source projection.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};
use ospf_rust_core::error::{ModelError, Result};
use ospf_rust_core::model::{LinearModelSnapshot, MetaModel};
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{ProofCompleteness, ProofKind, ProofReliability, ProofStatus, SolveHandle, SolveReport, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_core::variable::VariableId;
use super::{ColumnDecision, ColumnMasterOptions, ColumnProjection, ProjectedColumn};

/// f64 求解边界配置；不改变源模型的泛型定义。 / f64 execution-boundary configuration, without changing generic source modeling.
#[derive(Debug, Clone, Copy)]
pub struct ProjectedColumnGenerationConfig {
    /// 最大 RMP 求解轮数，包含 Phase I。 / Maximum RMP rounds, including Phase I.
    pub iteration_limit: usize,
    /// 最大活动列数。 / Maximum active columns.
    pub column_limit: usize,
    /// 含定价与 final 的总求解预算。 / Total solve budget including pricing and final.
    pub time_limit: Duration,
    /// 同域可行性和负检验数容差。 / Same-domain feasibility and negative-reduced-cost tolerance.
    pub tolerance: f64,
}

impl Default for ProjectedColumnGenerationConfig {
    fn default() -> Self { Self { iteration_limit: 128, column_limit: 10_000, time_limit: Duration::from_secs(60), tolerance: 1e-7 } }
}

/// 外层执行终态，不把受限主问题证明当作源模型证明。 / Outer execution terminal, never transferring restricted-master proofs to the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedColumnGenerationStop {
    /// 完成定价及最后整数主问题求解。 / Pricing and final integer-master solve completed.
    Completed,
    /// RMP 轮数耗尽。 / RMP round budget exhausted.
    IterationLimit,
    /// 活动列数耗尽。 / Active-column budget exhausted.
    ColumnLimit,
    /// Phase I 无改进列但人工变量仍非零。 / Phase I has no improving column but positive artificials.
    PhaseOneStall,
    /// 负检验数重复列导致停滞。 / A duplicate negative-reduced-cost column caused a stall.
    PricingStall,
    /// RMP 未形成最优 LP。 / The RMP did not produce an optimal LP.
    MasterTermination,
    /// 定价未形成最优解。 / Pricing did not produce an optimum.
    PricingTermination,
    /// 最后整数主问题未正常最优完成。 / Final integer master did not complete optimally.
    FinalTermination,
    /// 主问题 LP 证书缺失或校验失败。 / Missing or invalid master LP certificate.
    CertificateInvalid,
    /// 合作式取消或中断。 / Cooperative cancellation or interruption.
    Cancelled,
    /// 总预算或后端时限耗尽。 / Total budget or backend time limit exhausted.
    TimeLimit,
}

/// 独立保存最后真实子报告与复验源赋值，不签发源模型最优性证明。
/// Retain last actual child reports and verified source assignments without issuing a source optimality proof.
pub struct ProjectedColumnGenerationResult {
    /// 实际停止原因。 / Actual stop reason.
    pub stop: ProjectedColumnGenerationStop,
    /// Phase I 是否已找到零人工变量 RMP。 / Whether Phase I found a zero-artificial RMP.
    pub phase_one_complete: bool,
    /// Phase II 的全部块是否均完成无负检验数定价。 / Whether every block completed Phase II pricing without negative reduced costs.
    pub pricing_converged: bool,
    /// RMP 轮数。 / RMP rounds.
    pub iterations: usize,
    /// 实际定价求解次数。 / Actual pricing solve count.
    pub pricing_calls: usize,
    /// 已提交列数。 / Committed column count.
    pub column_count: usize,
    /// 源变量的完整 incumbent。 / Complete source-variable incumbent.
    pub values: BTreeMap<VariableId, f64>,
    /// 规范最小化方向的保守 incumbent 目标。 / Conservative incumbent objective in canonical minimization direction.
    pub objective: Option<f64>,
    /// 最后 RMP 原始报告。 / Last original RMP report.
    pub master_report: Option<SolveReport<f64>>,
    /// 最后定价原始报告。 / Last original pricing report.
    pub pricing_report: Option<SolveReport<f64>>,
    /// 最后定价所属块。 / Block of the last pricing report.
    pub pricing_block: Option<String>,
    /// 受限整数主问题原始报告。 / Original restricted-integer-master report.
    pub final_report: Option<SolveReport<f64>>,
}

fn invalid(message: &str) -> ospf_rust_core::error::CoreError { ModelError::InvalidConstraint(format!("column generation: {message}")).into() }

fn checked_report(report: &SolveReport<f64>, model: &LinearTriadModel) -> Result<()> {
    report.validate()?;
    if report.fingerprints.model.as_ref() != Some(&linear_model_fingerprint(model)?)
        || report.model_mapping.as_ref() != Some(&linear_model_mapping(model)?) {
        return Err(invalid("report does not identify the actual solved model"));
    }
    Ok(())
}

fn reliable_optimum(report: &SolveReport<f64>) -> bool {
    report.is_optimal() && report.proof.as_ref().is_some_and(|proof| proof.kind == ProofKind::Optimality
        && proof.status == ProofStatus::Verified && matches!(proof.reliability, ProofReliability::Exact | ProofReliability::Reliable)
        && proof.completeness == ProofCompleteness::Complete)
}

fn assignment(report: &SolveReport<f64>, snapshot: &LinearModelSnapshot<f64>, tolerance: f64) -> Result<BTreeMap<VariableId, f64>> {
    let solution = report.solution.as_ref().ok_or_else(|| invalid("missing incumbent"))?;
    if solution.value.is_some() || solution.values.len() != snapshot.variables.len() { return Err(invalid("incomplete linear incumbent")); }
    let values = snapshot.variables.iter().zip(&solution.values).map(|(state, value)| (state.variable, *value)).collect();
    snapshot.validate_solution(&values, &tolerance)?;
    snapshot.evaluate_objective_flt64(&values)?.validate_reported(
        solution.objective_value.or(solution.objective), tolerance.max(1e-7),
    )?;
    Ok(values)
}

/// 原生列生成策略：直接复用 ColumnProjection 的注册、对偶、定价和源解提取。
/// Native column-generation strategy reusing ColumnProjection registration, duals, pricing and source extraction.
pub struct ProjectedColumnGeneration;

impl ProjectedColumnGeneration {
    /// 从空池开始 Phase I；每块每轮至多一列，最终只求当前受限整数主问题。
    /// Start Phase I from an empty pool; add at most one column per block/round and finalize only the current restricted integer master.
    pub fn solve<F>(
        source: &MetaModel<f64>,
        decisions: Vec<ColumnDecision>,
        blocks: BTreeSet<String>,
        config: ProjectedColumnGenerationConfig,
        handle: &SolveHandle,
        mut solve: F,
    ) -> Result<ProjectedColumnGenerationResult>
    where F: FnMut(&LinearTriadModel, Duration, &SolveHandle) -> Result<SolveReport<f64>> {
        use ProjectedColumnGenerationStop as Stop;
        if config.iteration_limit == 0 || config.column_limit == 0 || !config.tolerance.is_finite() || config.tolerance <= 0.0 || config.tolerance >= 0.5 {
            return Err(invalid("invalid budget or tolerance"));
        }
        let mut projection = ColumnProjection::new(source, decisions.clone(), blocks.clone())?;
        let started = Instant::now();
        let control = |report: Option<&SolveReport<f64>>| {
            if handle.is_cancelled() || report.is_some_and(|report| matches!(report.termination_reason, TerminationReason::Cancelled | TerminationReason::Interrupted)) { Some(Stop::Cancelled) }
            else if started.elapsed() >= config.time_limit || report.is_some_and(|report| report.termination_reason == TerminationReason::TimeLimit) { Some(Stop::TimeLimit) }
            else { None }
        };
        let mut result = ProjectedColumnGenerationResult { stop: Stop::IterationLimit, phase_one_complete: false, pricing_converged: false,
            iterations: 0, pricing_calls: 0, column_count: 0, values: BTreeMap::new(), objective: None,
            master_report: None, pricing_report: None, pricing_block: None, final_report: None };
        'rounds: for _ in 0..config.iteration_limit {
            if let Some(stop) = control(None) { result.stop = stop; break; }
            let phase_one = !result.phase_one_complete;
            let registration = projection.master_for_pricing(source, ColumnMasterOptions { phase_one, tolerance: config.tolerance, ..Default::default() })?;
            let mut actual = None;
            let certified = projection.solve_master_lp(source, &registration, |model| {
                if control(None).is_some() { return Err(invalid("execution stopped before RMP backend entry")); }
                let report = solve(model, config.time_limit.saturating_sub(started.elapsed()), handle)?;
                report.validate()?;
                actual = Some(report.clone());
                Ok(report)
            });
            let report = match actual {
                Some(report) => report,
                None => {
                    if let Some(stop) = control(None) { result.stop = stop; break; }
                    return Err(certified.err().ok_or_else(|| invalid("missing RMP result"))?);
                }
            };
            result.iterations += 1;
            result.master_report = Some(report.clone());
            if let Some(stop) = control(Some(&report)) { result.stop = stop; break; }
            if !report.is_optimal() { result.stop = Stop::MasterTermination; break; }
            if certified.is_err() { result.stop = Stop::CertificateInvalid; break; }
            let anchor = assignment(&report, registration.snapshot(), config.tolerance)?;
            if phase_one && registration.snapshot().evaluate_objective_flt64(&anchor)?.total_value()? <= config.tolerance {
                result.phase_one_complete = true;
                continue;
            }
            let mut batches = Vec::new();
            for block in &blocks {
                if let Some(stop) = control(None) { result.stop = stop; break 'rounds; }
                let request = projection.pricing_request(source, Some(&registration), block, phase_one)?;
                let pricing = projection.pricing_model(source, Some(&registration), &request)?;
                let snapshot = pricing.linear_model_snapshot()?;
                let compiled = snapshot.compile_flt64()?;
                if let Some(stop) = control(None) { result.stop = stop; break 'rounds; }
                let priced = solve(&compiled.model, config.time_limit.saturating_sub(started.elapsed()), handle)?;
                priced.validate()?;
                result.pricing_calls += 1;
                result.pricing_report = Some(priced.clone());
                result.pricing_block = Some(block.clone());
                if let Some(stop) = control(Some(&priced)) { result.stop = stop; break 'rounds; }
                checked_report(&priced, &compiled.model)?;
                if !priced.is_optimal() { result.stop = Stop::PricingTermination; break 'rounds; }
                if !reliable_optimum(&priced) { result.stop = Stop::CertificateInvalid; break 'rounds; }
                let values = assignment(&priced, &snapshot, config.tolerance)?;
                let column = ProjectedColumn { id: format!("column-{}-{}", result.iterations, batches.len()), block_id: block.clone(),
                    values: decisions.iter().filter(|decision| &decision.block_id == block).map(|decision| (decision.id.clone(), values[&decision.variable])).collect() };
                if request.reduced_cost(&column.values)? < -config.tolerance {
                    if projection.columns().iter().any(|existing| existing.block_id == column.block_id && existing.values == column.values) {
                        result.stop = Stop::PricingStall; break 'rounds;
                    }
                    batches.push((request, vec![column]));
                }
            }
            if batches.is_empty() {
                if phase_one { result.stop = Stop::PhaseOneStall; }
                else { result.pricing_converged = true; result.stop = Stop::Completed; }
                break;
            }
            if projection.columns().len().saturating_add(batches.len()) > config.column_limit { result.stop = Stop::ColumnLimit; break; }
            projection.add_priced_batch(source, &registration, batches)?;
            result.column_count = projection.columns().len();
        }
        if let Some(stop) = control(None) { result.stop = stop; }
        if !result.phase_one_complete || !matches!(result.stop, Stop::Completed | Stop::IterationLimit | Stop::ColumnLimit | Stop::PricingStall) { return Ok(result); }
        let registration = projection.master(source, ColumnMasterOptions { integer: true, tolerance: config.tolerance, ..Default::default() })?;
        let compiled = registration.snapshot().compile_flt64()?;
        if let Some(stop) = control(None) { result.stop = stop; return Ok(result); }
        let report = solve(&compiled.model, config.time_limit.saturating_sub(started.elapsed()), handle)?;
        report.validate()?;
        if report.has_incumbent() {
            checked_report(&report, &compiled.model)?;
            assignment(&report, registration.snapshot(), config.tolerance)?;
            result.values = projection.extract_solution(source, &registration, &report.solution.as_ref().ok_or_else(|| invalid("missing final solution"))?.values, &config.tolerance)?;
            result.objective = Some(source.linear_model_snapshot()?.evaluate_objective_flt64(&result.values)?.total_value()?);
        }
        if let Some(stop) = control(Some(&report)) { result.stop = stop; }
        else if !reliable_optimum(&report) { checked_report(&report, &compiled.model)?; result.stop = Stop::FinalTermination; }
        result.final_report = Some(report);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::solver::{CancellationOrigin, ProblemStatus, SolveProof, SolveSolution};
    use ospf_rust_core::variable::Binary;

    fn source() -> (MetaModel<f64>, Vec<ColumnDecision>, BTreeSet<String>) {
        let mut source = MetaModel::new("column-generation-contract");
        source.register_auto_variable::<Binary>("x").unwrap();
        let decisions = vec![ColumnDecision { id: "x".into(), block_id: "block".into(), variable: source.tokens()[0].id() }];
        (source, decisions, BTreeSet::from(["block".into()]))
    }

    #[test]
    fn precancellation_and_zero_deadline_never_enter_a_backend() {
        for cancel in [false, true] {
            let (source, decisions, blocks) = source();
            let handle = SolveHandle::new();
            if cancel { handle.cancel(CancellationOrigin::External); }
            let result = ProjectedColumnGeneration::solve(&source, decisions, blocks,
                ProjectedColumnGenerationConfig { time_limit: if cancel { Duration::from_secs(1) } else { Duration::ZERO }, ..Default::default() },
                &handle, |_, _, _| panic!("must not call backend")).unwrap();
            assert_eq!(result.stop, if cancel { ProjectedColumnGenerationStop::Cancelled } else { ProjectedColumnGenerationStop::TimeLimit });
            assert_eq!(result.iterations, 0);
            assert!(result.final_report.is_none());
        }
    }

    #[test]
    fn time_limit_is_a_report_and_missing_identity_cannot_issue_pricing_duals() {
        for timeout in [false, true] {
            let (source, decisions, blocks) = source();
            let mut calls = 0;
            let result = ProjectedColumnGeneration::solve(&source, decisions, blocks, Default::default(), &SolveHandle::new(), |model, remaining, _| {
                calls += 1;
                assert!(remaining <= Duration::from_secs(60));
                if timeout { SolveReport::<f64>::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit).build() }
                else { SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
                    .solution(SolveSolution::vector(vec![0.0; model.num_variables()])).proof(SolveProof::optimality()).build() }
            }).unwrap();
            assert_eq!(calls, 1);
            assert_eq!(result.stop, if timeout { ProjectedColumnGenerationStop::TimeLimit } else { ProjectedColumnGenerationStop::CertificateInvalid });
            assert!(result.master_report.is_some());
            assert_eq!(result.pricing_calls, 0);
            assert!(result.final_report.is_none());
        }
    }

    #[test]
    fn invalid_budgets_and_backend_errors_are_not_fabricated_success() {
        let (source, decisions, blocks) = source();
        assert!(ProjectedColumnGeneration::solve(&source, decisions.clone(), blocks.clone(),
            ProjectedColumnGenerationConfig { iteration_limit: 0, ..Default::default() }, &SolveHandle::new(), |_, _, _| unreachable!()).is_err());
        assert!(ProjectedColumnGeneration::solve(&source, decisions, blocks, Default::default(), &SolveHandle::new(), |_, _, _| Err(invalid("actual backend error"))).is_err());
    }
}
