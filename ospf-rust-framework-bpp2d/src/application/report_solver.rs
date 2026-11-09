//! 显式报告驱动的 BPP2D 原生主问题入口，不改写旧闭包执行器。 / Explicit report-driven native BPP2D master entry, without replacing legacy closure executors.

use std::sync::Arc;
use std::time::Duration;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::LinearSolver;
use ospf_rust_framework::solver::{LinearExecutionMode, LinearExecutionOptions, LinearReportExecutor, LinearStageReport};
use crate::domain::layer_assignment::{Bpp2dDemandShadowPriceMap, LayerAssignmentModel, LayerAssignmentStage};

/// 同一次领域调用共享的总预算、进度与取消选项。 / Total budget, progress and cancellation options shared by one domain invocation.
pub type Bpp2dExecutionOptions = LinearExecutionOptions;

/// 实际提交的主问题及原报告，不是源装箱问题的全局证明。 / Actual submitted master and original report, not a global proof of the source packing problem.
pub type Bpp2dStageReport = LinearStageReport<LayerAssignmentStage>;

/// 经原模型及报告复验的主问题结果；缺失的解、目标或对偶不以零填充。
/// Master result revalidated against the original model and report; missing solutions, objectives or duals are not filled with zeros.
#[derive(Debug, Clone)]
pub struct Bpp2dMasterResult {
    /// 实际编译模型、阶段、目标常数与无损原报告。 / Actual compiled model, stage, objective constant and lossless original report.
    pub stage_report: Bpp2dStageReport,
    /// 与原列池同序的份数；不 round，不展开为大量板材。 / Copies in original-pool order, without rounding or expanding into large sheet collections.
    pub copies: Option<Vec<f64>>,
    /// 含一次常数的原主问题最小化目标。 / Original master's minimizing objective, counting its constant once.
    pub objective: Option<f64>,
    /// 仅有 incumbent 的 FeasibilityLp 提供原需求单位的人工短缺。 / Artificial deficits in original demand units, only for FeasibilityLp with an incumbent.
    pub deficits: Option<Vec<f64>>,
    /// 仅经完整证书复验的最优 LP 提供；包含需求上下界的贡献，不含附加规则的定价修正。
    /// Only a fully certificate-validated optimal LP provides these prices; they include demand lower/upper contributions, not extra-rule pricing corrections.
    pub shadow_prices: Option<Bpp2dDemandShadowPriceMap>,
}

/// 每次领域调用独立创建；LP、Phase I 和最终整数模型共用实际后端及总预算。
/// Create independently per domain invocation; LP, Phase I and final integer models share the actual backend and total budget.
#[derive(Debug)]
pub struct Bpp2dNativeBackend {
    executor: LinearReportExecutor<LayerAssignmentStage>,
}

impl Bpp2dNativeBackend {
    /// 后端必须显式注入；不会选择默认后端或启发式替代。 / A backend must be explicitly injected; never choose a default backend or heuristic substitute.
    pub fn new(solver: Arc<dyn LinearSolver>, options: Bpp2dExecutionOptions) -> ospf_rust_core::Result<Self> {
        Ok(Self { executor: LinearReportExecutor::new(solver, options)? })
    }

    /// 实际后端名。 / Actual backend name.
    pub fn name(&self) -> &str { self.executor.name() }

    /// 剩余总预算，供生成编排在子求解之间检查。 / Remaining total budget for generation orchestration to check between child solves.
    pub fn remaining_time(&self) -> Duration { self.executor.remaining_time() }

    /// 外部合作取消状态。 / External cooperative cancellation state.
    pub fn is_cancelled(&self) -> bool { self.executor.is_cancelled() }

    /// 原快照复验容差。 / Original-snapshot validation tolerance.
    pub fn tolerance(&self) -> f64 { self.executor.tolerance() }

    /// 已通过执行门禁的原始子报告副本，包括不可行、限时和取消终态。 / Copies of original child reports accepted by the execution gates, including infeasible, timeout and cancellation terminals.
    pub fn reports(&self) -> Vec<Bpp2dStageReport> { self.executor.reports() }

    /// 求解已由领域 context 注册的主问题；允许下游在调用前追加业务 pipeline。
    /// Solve a master registered by the domain context; downstream code may append business pipelines before this call.
    ///
    /// 本入口不进行列生成或全局证明提升；非最优 LP 保留报告与可行解，但绝不交付定价对偶。
    /// This entry neither performs column generation nor promotes proofs globally; non-optimal LPs retain their report and incumbent but never deliver pricing duals.
    pub fn solve_master(
        &self,
        model: &MetaModel<f64>,
        assignment: &LayerAssignmentModel,
    ) -> ospf_rust_core::Result<Bpp2dMasterResult> {
        assignment.require_model(model)?;
        let stage = assignment.stage();
        let execution = self.executor.solve(
            model,
            stage,
            if stage == LayerAssignmentStage::Integer { LinearExecutionMode::Milp } else { LinearExecutionMode::Lp },
        )?;
        let copies = execution.source_values.as_ref().map(|values| assignment.extract_copies(model, values, self.tolerance())).transpose()?;
        let deficits = if stage == LayerAssignmentStage::FeasibilityLp {
            execution.source_values.as_ref().map(|values| assignment.extract_deficits(model, values, self.tolerance())).transpose()?
        } else { None };
        let shadow_prices = execution.original_duals.as_ref().map(|duals| assignment.extract_shadow_prices(model, duals)).transpose()?;
        Ok(Bpp2dMasterResult { stage_report: execution.stage_report, copies, objective: execution.objective, deficits, shadow_prices })
    }
}

#[cfg(test)]
mod tests;
