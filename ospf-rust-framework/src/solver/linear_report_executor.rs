//! 领域共用的报告驱动线性执行边界，不提供启发式回退。 / Shared report-driven linear execution boundary for domains, without heuristic fallback.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use ospf_rust_core::model::{LinearObjectiveEvaluationFlt64, MetaModel};
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{CancellationOrigin, LinearSolver, ProblemStatus, SolveFingerprints, SolveHandle, SolveOptions, SolveProgressReporter, SolveReport, SolverProvenance, TerminationReason, cancelled_solve_report, linear_model_fingerprint, linear_model_mapping, require_infeasibility_certificate_for_model, require_optimal_lp_certificate_for_linear_model};
use ospf_rust_core::variable::VariableId;

/// 一次领域执行的总预算及合作取消；每个调用须独立创建 executor。 / Total budget and cooperative cancellation for one domain execution; create an independent executor per invocation.
#[derive(Clone)]
pub struct LinearExecutionOptions {
    /// 含生成、编译和全部子求解的总预算，默认 60 秒，不是硬抢占截止。 / Total budget including generation, compilation and every child solve, default 60 seconds, not a hard preemptive deadline.
    pub time_limit: Duration,
    /// 原快照可行性绝对容差，有限且位于 [0, 0.5)。 / Absolute original-snapshot feasibility tolerance, finite and in [0, 0.5).
    pub tolerance: f64,
    /// 外部取消句柄；子求解使用独立完成标记。 / External cancellation handle; child solves use independent completion markers.
    pub cancellation: SolveHandle,
    /// 原始子求解进度，不代表总体百分比。 / Raw child progress, not an overall percentage.
    pub progress: Option<SolveProgressReporter>,
    /// 保留的有效子报告上限，默认 1024，最大 100000。 / Retained valid child-report limit, default 1024, maximum 100000.
    pub report_limit: usize,
}

impl Default for LinearExecutionOptions {
    fn default() -> Self {
        Self { time_limit: Duration::from_secs(60), tolerance: 1e-7, cancellation: SolveHandle::new(), progress: None, report_limit: 1024 }
    }
}

/// LP 定价或整数候选执行；不改变输入模型的变量类型。 / LP pricing or integer-candidate execution, without changing input variable types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearExecutionMode {
    /// 拒绝整数变量；只有经认证的最优 LP 才交付原始行对偶。 / Reject integer variables; only a certified optimal LP delivers original-row duals.
    Lp,
    /// 保留整数域，允许已有可行解的非最优终态，不伪造对偶。 / Retain integer domains and allow non-optimal terminals with an incumbent, without fabricating duals.
    Milp,
}

/// 保留实际编译模型与原报告，不能通过重新编译另一个模型替代身份。 / Retain the actual compiled model and original report; recompiling another model cannot replace its identity.
#[derive(Debug, Clone)]
pub struct LinearStageReport<S> {
    /// 领域定义的阶段。 / Domain-defined stage.
    pub stage: S,
    /// 实际提交的编译模型，保留类型及全部行元数据。 / Actual submitted compiled model retaining types and all row metadata.
    pub model: LinearTriadModel,
    /// 经身份、完整向量及原模型复验的报告。 / Report revalidated for identity, complete vector and original-model feasibility.
    pub report: SolveReport<f64>,
    /// 独立保留的目标常数，不修改原报告的无常数目标。 / Separately retained objective constant, without modifying the report's variable-only objective.
    pub objective_constant: f64,
}

/// 受检执行结果；无 incumbent 或非最优 LP 时不填补缺失数据。 / Checked execution result; missing data is not filled for absent incumbents or non-optimal LPs.
#[derive(Debug, Clone)]
pub struct LinearExecutionResult<S> {
    /// 实际子报告及模型。 / Actual child report and model.
    pub stage_report: LinearStageReport<S>,
    /// 包含函数辅助变量的完整稳定 ID 解，仅 incumbent 存在时提供。 / Complete stable-ID solution including function helpers, present only for an incumbent.
    pub source_values: Option<BTreeMap<VariableId, f64>>,
    /// 原快照的最小化目标，常数仅计一次。 / Original snapshot's minimizing objective, counting the constant once.
    pub objective: Option<f64>,
    /// 与 MetaModel 原始约束同序的完整对偶；已还原符号并汇总等式拆行，仅最优 LP 提供。 / Complete duals in original MetaModel constraint order, signs restored and equality splits aggregated, present only for an optimal LP.
    pub original_duals: Option<Vec<f64>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> { mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) }

fn invalid(message: impl Into<String>) -> ospf_rust_core::error::CoreError {
    ospf_rust_core::error::SolverError::ContractViolation(message.into()).into()
}

/// 检查实际模型身份、完整向量与目标；原快照可行性由 executor 进一步复验。
/// Check actual-model identity, complete vector and objective; the executor additionally revalidates original-snapshot feasibility.
///
/// 仅无解且无证明的取消／中断报告可补齐缺失身份；已有冲突身份、限时报告及证明不允许修复。
/// Only proof-free, incumbent-free cancellation/interruption reports may have missing identity completed; conflicting identity, timeout reports and proofs cannot be repaired.
pub fn validate_linear_execution_report(
    model: &LinearTriadModel,
    report: &mut SolveReport<f64>,
    tolerance: f64,
) -> ospf_rust_core::Result<()> {
    if !tolerance.is_finite() || !(0.0..0.5).contains(&tolerance) { return Err(invalid("invalid linear execution report tolerance")); }
    let fingerprint = linear_model_fingerprint(model)?;
    let mapping = linear_model_mapping(model)?;
    if !report.has_incumbent() && report.proof.is_none() && matches!(report.termination_reason, TerminationReason::Cancelled | TerminationReason::Interrupted) {
        if report.fingerprints.model.is_none() { report.fingerprints.model = Some(fingerprint.clone()); }
        if report.model_mapping.is_none() { report.model_mapping = Some(mapping.clone()); }
    }
    report.validate()?;
    if report.fingerprints.model.as_ref() != Some(&fingerprint) || report.model_mapping.as_ref() != Some(&mapping) {
        return Err(invalid("linear execution report model mapping/fingerprint mismatch"));
    }
    if let Some(solution) = &report.solution {
        if solution.value.is_some() || solution.values.len() != model.num_variables() {
            return Err(invalid("linear execution solution is not a complete variable vector"));
        }
        let objective = LinearObjectiveEvaluationFlt64::new(model.c.iter().copied().zip(solution.values.iter().copied()), 0.0)?;
        objective.validate_reported(solution.objective_value.or(solution.objective), tolerance.max(1e-7))?;
        if solution.objective.is_some() { objective.validate_reported(solution.objective, tolerance.max(1e-7))?; }
        for (variable, value) in &solution.stable_values {
            let column = mapping.columns_by_variable.get(variable).ok_or_else(|| invalid("foreign linear execution stable variable"))?;
            if *value != solution.values[*column] { return Err(invalid("linear execution stable and vector values disagree")); }
        }
    }
    Ok(())
}

/// 同一领域调用共用一个显式后端，串行执行子模型并保留无损报告。
/// One domain invocation shares an explicit backend, serially executes child models and retains lossless reports.
pub struct LinearReportExecutor<S> {
    solver: Arc<dyn LinearSolver>,
    options: LinearExecutionOptions,
    started: Instant,
    reports: Mutex<Vec<LinearStageReport<S>>>,
    execution: Mutex<()>,
}

impl<S> std::fmt::Debug for LinearReportExecutor<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LinearReportExecutor").field("backend", &self.solver.name()).field("reports", &lock(&self.reports).len()).finish()
    }
}

impl<S: Clone> LinearReportExecutor<S> {
    /// 不自动选择后端、不允许非法配置。 / Do not automatically choose a backend or accept invalid options.
    pub fn new(solver: Arc<dyn LinearSolver>, options: LinearExecutionOptions) -> ospf_rust_core::Result<Self> {
        if !options.tolerance.is_finite() || !(0.0..0.5).contains(&options.tolerance) || options.report_limit == 0 || options.report_limit > 100_000 {
            return Err(invalid("invalid linear execution options"));
        }
        Ok(Self { solver, options, started: Instant::now(), reports: Mutex::new(Vec::new()), execution: Mutex::new(()) })
    }

    /// 实际后端名。 / Actual backend name.
    pub fn name(&self) -> &str { self.solver.name() }

    /// 原快照复验容差。 / Original-snapshot validation tolerance.
    pub fn tolerance(&self) -> f64 { self.options.tolerance }

    /// 包含生成及前序子求解耗时的剩余预算。 / Remaining budget including generation and previous child solves.
    pub fn remaining_time(&self) -> Duration { self.options.time_limit.saturating_sub(self.started.elapsed()) }

    /// 是否收到外部取消；不把子求解完成当作整个领域调用完成。 / Whether external cancellation was requested; child completion is not whole-invocation completion.
    pub fn is_cancelled(&self) -> bool { self.options.cancellation.is_cancelled() }

    /// 已接受的原始子报告副本；观察不等待正在运行的求解。 / Copies of accepted original child reports; observation does not wait for a running solve.
    pub fn reports(&self) -> Vec<LinearStageReport<S>> { lock(&self.reports).clone() }

    /// 编译、求解及完整复验；契约失败返回错误，绝不生成替代解或接受不可靠定价。
    /// Compile, solve and fully revalidate; contract failures return errors, never replacement solutions or unreliable pricing.
    pub fn solve(
        &self,
        model: &MetaModel<f64>,
        stage: S,
        mode: LinearExecutionMode,
    ) -> ospf_rust_core::Result<LinearExecutionResult<S>> {
        let _execution = self.execution.try_lock().map_err(|_| invalid("linear backend is already executing; use one executor per domain invocation"))?;
        if lock(&self.reports).len() >= self.options.report_limit { return Err(invalid("linear child report limit reached")); }
        let snapshot = model.linear_model_snapshot()?;
        let compiled = snapshot.compile_flt64()?;
        if mode == LinearExecutionMode::Lp && compiled.model.basic.variables.iter().any(|token| token.var_type().is_integer()) {
            return Err(invalid("pricing master must be an LP"));
        }
        let remaining = self.remaining_time();
        let mut report = if self.is_cancelled() {
            cancelled_solve_report(SolverProvenance::default(), &self.options.cancellation)?
        } else if remaining.is_zero() {
            SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit)
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model)?), ..Default::default() })
                .model_mapping(Some(linear_model_mapping(&compiled.model)?)).build()?
        } else {
            // 弱引用防止遗留子句柄；每轮独立完成标记，避免首轮完成吞掉后续取消。
            // Weak references avoid retaining old child handles; independent completion markers prevent a completed first round from swallowing later cancellation.
            let child = Arc::new(SolveHandle::new());
            let weak = Arc::downgrade(&child);
            self.options.cancellation.register_interrupter(move || {
                if let Some(child) = weak.upgrade() { child.cancel(CancellationOrigin::External); }
            });
            self.solver.solve_linear_report_with_options(&compiled.model, &SolveOptions {
                time_limit: Some(remaining), cancellation_handle: Some(&child), progress_reporter: self.options.progress.as_ref(), ..Default::default()
            })?
        };
        validate_linear_execution_report(&compiled.model, &mut report, self.options.tolerance)?;
        let (source_values, objective) = if let Some(solution) = &report.solution {
            let values = compiled.model.basic.variables.iter().zip(&solution.values).map(|(token, value)| (token.id(), *value)).collect();
            snapshot.validate_solution(&values, &self.options.tolerance)?;
            let objective = snapshot.evaluate_objective_flt64(&values)?;
            objective.validate_reported(solution.objective_value.or(solution.objective), self.options.tolerance.max(1e-7))?;
            (Some(values), Some(objective.total_value()?))
        } else { (None, None) };
        if report.problem_status == ProblemStatus::Infeasible {
            require_infeasibility_certificate_for_model(&report, &linear_model_fingerprint(&compiled.model)?)?;
        }
        let original_duals = if mode == LinearExecutionMode::Lp && report.is_optimal() {
            let certificate = require_optimal_lp_certificate_for_linear_model(&report, &compiled.model)?;
            let mut prices = BTreeMap::<&str, f64>::new();
            for (row, dual) in compiled.rows.iter().zip(certificate.dual) {
                let price = prices.entry(&row.source).or_default();
                *price += f64::from(row.multiplier) * dual;
                if !price.is_finite() { return Err(invalid("linear original-row dual overflows")); }
            }
            if snapshot.rows.len() < model.constraints().len() { return Err(invalid("LP lost original row origins")); }
            Some(snapshot.rows.iter().zip(model.constraints()).map(|(row, constraint)| {
                if row.source_name != constraint.name { return Err(invalid("LP original row order changed")); }
                prices.get(row.id.as_str()).copied().ok_or_else(|| invalid("LP row lacks a dual origin"))
            }).collect::<ospf_rust_core::Result<Vec<_>>>()?)
        } else { None };
        let stage_report = LinearStageReport { stage, model: compiled.model, report, objective_constant: compiled.objective_constant };
        lock(&self.reports).push(stage_report.clone());
        Ok(LinearExecutionResult { stage_report, source_values, objective, original_duals })
    }
}

#[cfg(test)]
mod tests;
