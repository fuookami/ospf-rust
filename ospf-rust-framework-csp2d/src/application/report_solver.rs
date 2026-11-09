//! 显式 CSP2D 原生主问题报告路径，不调用旧启发式回退。 / Explicit CSP2D native-master report path, without invoking legacy heuristic fallbacks.

use std::sync::Arc;
use std::time::Duration;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::LinearSolver;
use ospf_rust_framework::solver::{LinearExecutionMode, LinearExecutionOptions, LinearReportExecutor, LinearStageReport};
use crate::domain::produce::{Csp2dMasterShadowPrices, ProduceModel, ProduceStage};

/// 单次调用共享的总时间、进度、取消与报告预算。 / Total time, progress, cancellation and report budgets shared by one invocation.
pub type Csp2dExecutionOptions = LinearExecutionOptions;

/// 实际编译模型、阶段、原始报告和一次目标常数。 / Actual compiled model, stage, original report and single-counted objective constant.
pub type Csp2dStageReport = LinearStageReport<ProduceStage>;

/// 复验实际原模型的主问题结果，不提升为源套裁问题全局证明。 / Master result revalidated against the actual original model, without promotion to a global cutting-stock proof.
#[derive(Debug, Clone)]
pub struct Csp2dMasterResult {
    /// 原始实际子报告。 / Original actual child report.
    pub stage_report: Csp2dStageReport,
    /// 与完整候选池同序，不 round 或展开。 / Complete-pool order, without rounding or expansion.
    pub usages: Option<Vec<f64>>,
    /// 仅 Phase I incumbent 提供原需求单位短缺。 / Original-demand-unit deficits, provided only for a Phase I incumbent.
    pub deficits: Option<Vec<f64>>,
    /// 含一次常数的原快照目标。 / Original-snapshot objective including its constant once.
    pub objective: Option<f64>,
    /// 仅经证书复验的最优 LP 提供需求与库存价格。 / Demand/stock prices provided only by a certificate-validated optimal LP.
    pub shadow_prices: Option<Csp2dMasterShadowPrices>,
}

/// 每次领域执行独立创建，同一后端及预算覆盖全部实际主问题。 / Created independently per domain execution, sharing one backend/budget across actual masters.
#[derive(Debug)]
pub struct Csp2dNativeBackend { executor: LinearReportExecutor<ProduceStage> }

impl Csp2dNativeBackend {
    /// 显式注入后端，不选择默认或启发式替代。 / Explicitly inject a backend, never selecting a default or heuristic substitute.
    pub fn new(solver: Arc<dyn LinearSolver>, options: Csp2dExecutionOptions) -> ospf_rust_core::Result<Self> {
        Ok(Self { executor: LinearReportExecutor::new(solver, options)? })
    }
    /// 实际后端名。 / Actual backend name.
    pub fn name(&self) -> &str { self.executor.name() }
    /// 含前序生成／编译／子求解的剩余预算。 / Remaining budget including earlier generation/compilation/child solves.
    pub fn remaining_time(&self) -> Duration { self.executor.remaining_time() }
    /// 原始合作取消状态，不把子求解完成视为父调用完成。 / Original cooperative cancellation state; child completion is not parent completion.
    pub fn is_cancelled(&self) -> bool { self.executor.is_cancelled() }
    /// 原快照复验容差。 / Original-snapshot validation tolerance.
    pub fn tolerance(&self) -> f64 { self.executor.tolerance() }
    /// 已接受的实际原报告副本。 / Copies of accepted actual original reports.
    pub fn reports(&self) -> Vec<Csp2dStageReport> { self.executor.reports() }

    /// 求解由领域 context 注册的原模型，可追加 pipeline；此入口本身不生成或定价列。
    /// Solve the original domain-context-registered model with optional extra pipelines; this entry itself neither generates nor prices columns.
    pub fn solve_master(&self, model: &MetaModel<f64>, registration: &ProduceModel) -> ospf_rust_core::Result<Csp2dMasterResult> {
        registration.require_model(model)?;
        let stage = registration.stage();
        let execution = self.executor.solve(model, stage, if stage == ProduceStage::Integer { LinearExecutionMode::Milp } else { LinearExecutionMode::Lp })?;
        let usages = execution.source_values.as_ref().map(|values| registration.extract_usages(model, values, self.tolerance())).transpose()?;
        let deficits = if stage == ProduceStage::FeasibilityLp {
            execution.source_values.as_ref().map(|values| registration.extract_deficits(model, values, self.tolerance())).transpose()?
        } else { None };
        let shadow_prices = execution.original_duals.as_ref().map(|duals| registration.extract_shadow_prices(model, duals)).transpose()?;
        Ok(Csp2dMasterResult { stage_report: execution.stage_report, usages, deficits, objective: execution.objective, shadow_prices })
    }
}

#[cfg(test)]
mod tests;
