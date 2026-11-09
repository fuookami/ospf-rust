//! 报告驱动的原生 LP/MILP 边界，禁止伪造对偶和启发式终态。 / Report-driven native LP/MILP boundary, without fabricated duals or heuristic terminal states.

use std::collections::BTreeMap;
use std::sync::Arc;
use ospf_rust_base::error::{Error, ErrorCode, Ret};
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{LinearSolver, ProblemStatus, SolveReport};
use ospf_rust_framework::solver::{LinearExecutionMode, LinearExecutionOptions, LinearReportExecutor, validate_linear_execution_report};
use crate::application::lp_solver::{Csp1dLpSolver, LpMasterSolution, LpTerminalStatus};

/// 实际求解子阶段；报告只属于其接收的模型。 / Actual solve stage; a report belongs only to its submitted model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Csp1dSolveStage {
    /// 受限 LP，包括可行性阶段。 / Restricted LP, including the feasibility phase.
    Lp,
    /// 普通候选 MILP。 / Ordinary candidate MILP.
    Milp,
    /// 当前列池的最终整数模型。 / Final integer model over the current column pool.
    FinalMilp,
}

/// 独立保留原始子报告，不转写为源问题证明。 / Independently retained child report, never relabeled as a source-problem proof.
#[derive(Debug, Clone)]
pub struct Csp1dStageReport {
    /// 实际子阶段。 / Actual child stage.
    pub stage: Csp1dSolveStage,
    /// 实际提交的编译模型，保留类型、行来源和元数据，不能通过重编译替代。 / Actual submitted compiled model retaining types, row origins and metadata, never replaced by recompilation.
    pub model: LinearTriadModel,
    /// 完整实际报告及模型身份。 / Complete actual report and model identity.
    pub report: SolveReport<f64>,
    /// 编译时独立保留、未进入子模型目标的常数。 / Separately retained compilation constant, excluded from the child model objective.
    pub objective_constant: f64,
}

/// 同一领域调用的合作取消、总预算及报告上限，复用共享执行选项。 / Cooperative cancellation, total budget and report limit for one domain invocation, reusing shared execution options.
pub type Csp1dExecutionOptions = LinearExecutionOptions;

/// 可注入的真实 MILP 报告入口，不接受启发式向量冒充报告。 / Injectable actual MILP-report entry, not heuristic vectors masquerading as reports.
pub trait Csp1dMilpBackend: std::fmt::Debug + Send + Sync {
    /// 接收原始模型，返回其编译模型的真实报告，保留原始可行性校验。 / Receive the original model and return its compiled model's actual report, retaining original feasibility validation.
    fn solve_milp(&self, model: &MetaModel<f64>, final_milp: bool) -> ospf_rust_core::Result<SolveReport<f64>>;
}

/// 兼容原 CSP1D 报告门禁；实际校验由领域共用边界实现。
/// Preserve the CSP1D report-gate API; actual validation is implemented by the shared domain boundary.
pub fn validate_csp1d_report(
    model: &LinearTriadModel,
    report: &mut SolveReport<f64>,
    tolerance: f64,
) -> ospf_rust_core::Result<()> {
    validate_linear_execution_report(model, report, tolerance)
}

/// 每次领域执行独立创建，LP 和 MILP 共用真实后端、预算、取消及无损报告。
/// Create independently per domain invocation; LP and MILP share the actual backend, budget, cancellation and lossless reports.
#[derive(Debug)]
pub struct Csp1dNativeBackend {
    executor: LinearReportExecutor<Csp1dSolveStage>,
}

impl Csp1dNativeBackend {
    /// 校验配置；不会选择或替换调用方后端。 / Validate options without choosing or substituting the caller's backend.
    pub fn new(solver: Arc<dyn LinearSolver>, options: Csp1dExecutionOptions) -> ospf_rust_core::Result<Self> {
        Ok(Self { executor: LinearReportExecutor::new(solver, options)? })
    }

    /// 原始子报告副本，含最终不可行、限时或取消报告。 / Copies of original child reports, including final infeasibility, timeout or cancellation.
    pub fn reports(&self) -> Vec<Csp1dStageReport> {
        self.executor.reports().into_iter().map(|record| Csp1dStageReport {
            stage: record.stage, model: record.model, report: record.report, objective_constant: record.objective_constant,
        }).collect()
    }

    fn solve_lp_model(&self, model: &MetaModel<f64>) -> ospf_rust_core::Result<LpMasterSolution> {
        let execution = self.executor.solve(model, Csp1dSolveStage::Lp, LinearExecutionMode::Lp)?;
        let report = &execution.stage_report.report;
        let mut result = match report.problem_status {
            ProblemStatus::Infeasible => LpMasterSolution::infeasible(),
            ProblemStatus::Unbounded => LpMasterSolution { status: LpTerminalStatus::Unbounded, objective: 0.0, duals: Vec::new(), primal: Vec::new(), info: BTreeMap::new() },
            _ if report.is_optimal() => LpMasterSolution::optimal(
                execution.objective.ok_or_else(|| ospf_rust_core::error::SolverError::ContractViolation("optimal CSP1D LP lacks an objective".into()))?,
                execution.original_duals.ok_or_else(|| ospf_rust_core::error::SolverError::ContractViolation("optimal CSP1D LP lacks certified duals".into()))?,
                report.solution.as_ref().ok_or_else(|| ospf_rust_core::error::SolverError::ContractViolation("optimal CSP1D LP lacks an incumbent".into()))?.values.clone(),
            ),
            _ => LpMasterSolution { status: LpTerminalStatus::Other, objective: 0.0, duals: Vec::new(), primal: Vec::new(), info: BTreeMap::new() },
        };
        result.info.insert("terminationReason".into(), format!("{:?}", report.termination_reason));
        Ok(result)
    }
}

impl Csp1dMilpBackend for Csp1dNativeBackend {
    fn solve_milp(&self, model: &MetaModel<f64>, final_milp: bool) -> ospf_rust_core::Result<SolveReport<f64>> {
        Ok(self.executor.solve(
            model,
            if final_milp { Csp1dSolveStage::FinalMilp } else { Csp1dSolveStage::Milp },
            LinearExecutionMode::Milp,
        )?.stage_report.report)
    }
}

#[derive(Debug)]
struct LpError(String);
impl std::fmt::Display for LpError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.0.fmt(f) } }
impl Error for LpError { fn code(&self) -> ErrorCode { ErrorCode::OREngineSolvingException } fn msg(&self) -> String { self.0.clone() } }

impl<V: ospf_rust_core::solver::SolveValue> Csp1dLpSolver<V> for Csp1dNativeBackend {
    fn name(&self) -> &str { self.executor.name() }
    fn solve_lp(&self, _: &str, model: &MetaModel<f64>) -> Ret<LpMasterSolution> {
        self.solve_lp_model(model).map_err(|error| Box::new(LpError(error.to_string())) as Box<dyn Error>)
    }
}

#[cfg(test)]
mod tests;
