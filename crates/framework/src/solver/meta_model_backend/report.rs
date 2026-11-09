//! 原 MetaModel 执行器的报告驱动后端，向量兼容层不填补缺失证据。
//! Report-driven backend for existing MetaModel executors; vector compatibility never fills missing evidence.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::LinearSolver;
use super::{MetaModelExecutionDiagnostics, MetaModelExecutorSolveResult, MetaModelSolverBackend};
use crate::solver::{LinearExecutionMode, LinearExecutionOptions, LinearExecutionResult, LinearReportExecutor, LinearStageReport};

/// MetaModel 执行阶段，不把受限主问题证明提升为领域证明。 / MetaModel execution stage, without promoting restricted-master proofs to domain proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaModelStage {
    /// 经认证后才提供原行对偶的 LP 主问题。 / LP master delivering original-row duals only after certification.
    RestrictedMaster,
    /// 保留实际整数域及原报告的 final 主问题。 / Final master retaining actual integer domains and original reports.
    FinalMilp,
}

/// 共用总预算、原始进度和合作取消的真实报告后端。
/// Actual-report backend sharing one total budget, original progress and cooperative cancellation.
/// 每次领域执行创建一个实例；不会选择 solver、放松整数域、使用 no-op 配置或伪造解。
/// Create one instance per domain execution; never select solvers, relax integers, use no-op settings or fabricate solutions.
/// clone 共享同一预算与报告，适合 RMP/final 执行器，不代表独立调用。
/// Clones share the same budget and reports for RMP/final executors, not independent invocations.
#[derive(Debug, Clone)]
pub struct ReportMetaModelBackend {
    executor: Arc<LinearReportExecutor<MetaModelStage>>,
}

impl ReportMetaModelBackend {
    /// 显式注入真实后端与本次调用预算。 / Explicitly inject the backend and this invocation's budget.
    pub fn new(solver: Arc<dyn LinearSolver>, options: LinearExecutionOptions) -> ospf_rust_core::Result<Self> {
        Ok(Self { executor: Arc::new(LinearReportExecutor::new(solver, options)?) })
    }

    /// 包含模型构造和前序子求解的剩余总预算。 / Remaining total budget including construction and previous children.
    pub fn remaining_time(&self) -> Duration { self.executor.remaining_time() }

    /// 外部取消状态；子求解完成不取消后续子求解。 / External cancellation; child completion never cancels subsequent children.
    pub fn is_cancelled(&self) -> bool { self.executor.is_cancelled() }

    /// 已通过门禁的实际模型和原报告副本，包含无解终态。 / Copies of accepted actual models and original reports, including no-incumbent terminals.
    pub fn reports(&self) -> Vec<LinearStageReport<MetaModelStage>> { self.executor.reports() }

    /// 报告原生入口：无解、取消、超时及非最优 LP 仍返回真实结果，但不补对偶或方案。
    /// Report-native entry: absent incumbents, cancellation, timeout and nonoptimal LPs retain actual results without invented duals or plans.
    pub fn execute(
        &self,
        model: &MetaModel<f64>,
        stage: MetaModelStage,
    ) -> ospf_rust_core::Result<LinearExecutionResult<MetaModelStage>> {
        self.executor.solve(model, stage, match stage {
            MetaModelStage::RestrictedMaster => LinearExecutionMode::Lp,
            MetaModelStage::FinalMilp => LinearExecutionMode::Milp,
        })
    }

    fn legacy(
        &self,
        model: &MetaModel<f64>,
        stage: MetaModelStage,
    ) -> Result<MetaModelExecutorSolveResult, String> {
        let execution = self.execute(model, stage).map_err(|error| error.to_string())?;
        let report = &execution.stage_report.report;
        let values = execution.source_values.as_ref().ok_or_else(|| format!(
            "{:?} has no incumbent ({:?}); actual report retained, no vector fallback", stage, report.termination_reason))?;
        let primal_solution = model.tokens_in_solver_order().into_iter().map(|token| {
            values.get(&token.id()).copied().ok_or_else(|| "actual report is missing an original model token".to_string())
        }).collect::<Result<Vec<_>, _>>()?;
        // 旧执行器把空向量当作 no-op 哨兵；原生 execute 仍可表示合法的空模型。
        // Legacy executors treat empty vectors as no-op sentinels; native execute still represents valid empty models.
        if primal_solution.is_empty() {
            return Err("empty actual primal cannot enter legacy vector fallback; use report-native execute".into());
        }
        let dual_solution = match stage {
            MetaModelStage::RestrictedMaster => execution.original_duals.ok_or_else(|| format!(
                "restricted master lacks a certified optimal LP ({:?}); actual report retained, no pricing fallback", report.termination_reason))?,
            MetaModelStage::FinalMilp => Vec::new(),
        };
        if stage == MetaModelStage::RestrictedMaster && dual_solution.is_empty() {
            return Err("empty certified row duals cannot enter legacy pricing fallback; use report-native execute".into());
        }
        Ok(MetaModelExecutorSolveResult { objective: execution.objective, primal_solution, dual_solution,
            additional_shadow_prices: HashMap::new(), info: HashMap::from([
                ("backend_kind".into(), "linear_report".into()),
                ("backend_phase".into(), if stage == MetaModelStage::RestrictedMaster { "rmp" } else { "final" }.into()),
            ]) })
    }
}

impl MetaModelSolverBackend for ReportMetaModelBackend {
    fn name(&self) -> &str { self.executor.name() }

    fn solve_rmp(&self, model: &MetaModel<f64>, _: &MetaModelExecutionDiagnostics) -> Result<MetaModelExecutorSolveResult, String> {
        self.legacy(model, MetaModelStage::RestrictedMaster)
    }

    fn solve_final(&self, model: &MetaModel<f64>, _: &MetaModelExecutionDiagnostics) -> Result<MetaModelExecutorSolveResult, String> {
        self.legacy(model, MetaModelStage::FinalMilp)
    }
}

#[cfg(test)]
mod tests;
