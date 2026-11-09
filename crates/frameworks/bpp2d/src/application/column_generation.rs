//! 报告驱动的原生列生成，不提升受限模型证明。 / Report-driven native column generation without promoting restricted-model proofs.

use std::sync::Arc;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::{LinearSolver, ProblemStatus, TerminationReason};
use crate::application::report_solver::{Bpp2dExecutionOptions, Bpp2dMasterResult, Bpp2dNativeBackend};
use crate::domain::item::model::Layer;
use crate::domain::layer_assignment::{Bpp2dAssignmentDemandEntry, LayerAssignmentContext, LayerAssignmentModelInput, LayerAssignmentObjectiveConfig, LayerAssignmentStage, layer_reduced_cost};
use crate::domain::layer_generation::{Bpp2dLayerGenerationRequest, Bpp2dLayerGenerator, LayerGenerationContext};
use crate::domain::layer_generation::catalog::{ColumnCatalog, generation_demands, generation_prices, invalid};

/// 列池及启发式生成预算；唯一时间预算来自 execution options。 / Column-pool and heuristic generation budgets; the sole time budget comes from execution options.
#[derive(Debug, Clone)]
pub struct Bpp2dColumnGenerationConfig {
    /// 允许的定价批次数，包含没有改进列的批次。 / Allowed pricing batches, including batches without improving columns.
    pub iteration_limit: usize,
    /// 每批观察的候选上限，不是任意用户生成器的硬内存限制。 / Observed candidate cap per batch, not a hard memory limit for arbitrary user generators.
    pub max_candidates_per_iteration: usize,
    /// 完整列池上限，含保留原位置的全部初始列。 / Complete pool cap, including all initial columns at their original positions.
    pub column_limit: usize,
    /// 正常停止后是否在完整池运行同后端整数模型。 / Whether to run a same-backend integer master on the complete pool after a normal stop.
    pub final_milp_enabled: bool,
    /// 仅接收严格低于负容差的有限检验数。 / Accept only finite reduced costs strictly below the negative tolerance.
    pub reduced_cost_tolerance: f64,
}

impl Default for Bpp2dColumnGenerationConfig {
    fn default() -> Self {
        Self { iteration_limit: 50, max_candidates_per_iteration: 256, column_limit: 10_000, final_milp_enabled: true, reduced_cost_tolerance: 1e-6 }
    }
}

/// 原生求解器边界；只支持物品件数／平方米需求和既有包围盒几何语义。
/// Native solver boundary; supports item-count/square-meter demands and the existing bounding-box geometry semantics only.
#[derive(Debug, Clone)]
pub struct Bpp2dColumnGenerationInput {
    /// 米制目录、刀缝、生成件数和初始池；初始影子价格与迭代不被信任。 / Meter-based catalogs, kerf, generation counts and initial pool; initial prices and iteration are not trusted.
    pub request: Bpp2dLayerGenerationRequest<f64, Meter>,
    /// 主问题需求及可选硬上界；非空 request.demand_entries 必须与这些下界一致。 / Master demands and optional hard upper bounds; nonempty request.demand_entries must match these lower bounds.
    pub demands: Vec<Bpp2dAssignmentDemandEntry>,
    /// Phase II 使用 RMP 面积系数；final 使用完整配置目标，两者可以不同。 / Phase II uses the RMP area coefficient; final uses the complete configured objective, which may differ.
    pub objective: LayerAssignmentObjectiveConfig,
    /// 迭代、候选与列池预算。 / Iteration, candidate and pool budgets.
    pub config: Bpp2dColumnGenerationConfig,
}

/// 生成停止原因，不是源问题求解状态或全局最优证明。 / Generation stop reason, not source-problem solve status or global optimality proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bpp2dColumnGenerationStop {
    /// 观察到的启发式候选中没有新的改进列。 / No new improving column among observed heuristic candidates.
    NoImprovingColumns,
    /// 已耗尽定价批次预算。 / Pricing-batch budget exhausted.
    IterationLimit,
    /// 完整池已达上限。 / Complete pool reached its cap.
    ColumnLimit,
    /// 已达原始子报告保存上限。 / Original child-report cap reached.
    ReportLimit,
    /// 总预算耗尽或子求解限时。 / Total budget exhausted or a child timed out.
    TimeLimit,
    /// 收到合作取消或子求解取消。 / Cooperative cancellation requested or a child cancelled.
    Cancelled,
    /// 主问题未提供可用于继续定价的已认证最优 LP。 / Master did not provide a certified optimal LP suitable for further pricing.
    MasterStopped,
}

/// 某次实际子模型对应完整列池的前缀。 / Complete-pool prefix corresponding to an actual child model.
#[derive(Debug, Clone)]
pub struct Bpp2dColumnGenerationStage {
    /// 此次注册的列池前缀长度，份数向量与此前缀严格同序。 / Registered pool-prefix length; copies are in exact prefix order.
    pub pool_size: usize,
    /// 原始实际模型、报告以及经验证的派生结果。 / Original actual model/report and checked derived results.
    pub result: Bpp2dMasterResult,
}

/// 一次已认证对偶驱动的定价批次。 / One pricing batch driven by certified duals.
#[derive(Debug, Clone)]
pub struct Bpp2dPricingIteration {
    /// 所用 LP 在 stages 中的位置。 / Index of the LP used in stages.
    pub stage_index: usize,
    /// 实际观察并认证目录身份的候选数。 / Number of observed candidates whose catalog identity was validated.
    pub candidates_evaluated: usize,
    /// 是否达到观察上限；即使未达到也不代表生成完备。 / Whether the observation cap was reached; not reaching it still does not imply complete generation.
    pub candidate_limit_reached: bool,
    /// 新列在完整池中的位置及重算检验数，不信任生成器自报分数。 / New-column complete-pool indices and recomputed reduced costs, not generator-reported scores.
    pub added_columns: Vec<(usize, f64)>,
}

/// 完整原生执行轨迹；不展开大量板材，不构造全局源问题结论。
/// Complete native execution trace; does not expand large sheet counts or construct global source-problem conclusions.
#[derive(Debug, Clone)]
pub struct Bpp2dColumnGenerationResult {
    /// 完整池，初始列位置不变；final 选中子集不覆盖此字段。 / Complete pool with unchanged initial positions; final selected subsets never overwrite this field.
    pub columns: Vec<Layer<f64, Meter>>,
    /// 按实际执行顺序保留的所有有效子模型与报告，含 final。 / All valid child models/reports in execution order, including final.
    pub stages: Vec<Bpp2dColumnGenerationStage>,
    /// 已完成的定价批次。 / Completed pricing batches.
    pub iterations: Vec<Bpp2dPricingIteration>,
    /// 定价停止原因；final 的真实状态单独保留。 / Pricing stop reason; final's actual status is retained separately.
    pub stop: Bpp2dColumnGenerationStop,
    /// final 子模型在 stages 中的位置；没有执行时为 None。 / Final child index in stages, or None when not executed.
    pub final_stage: Option<usize>,
    /// 当前始终为 false：既有生成器不提供 exact pricing 证明。 / Always false currently: existing generators provide no exact-pricing proof.
    pub candidate_complete: bool,
}

fn halted(backend: &Bpp2dNativeBackend) -> Option<Bpp2dColumnGenerationStop> {
    if backend.is_cancelled() { Some(Bpp2dColumnGenerationStop::Cancelled) }
    else if backend.remaining_time().is_zero() { Some(Bpp2dColumnGenerationStop::TimeLimit) }
    else { None }
}

fn terminal(result: &Bpp2dMasterResult) -> Option<Bpp2dColumnGenerationStop> {
    match result.stage_report.report.termination_reason {
        TerminationReason::Cancelled => Some(Bpp2dColumnGenerationStop::Cancelled),
        TerminationReason::TimeLimit => Some(Bpp2dColumnGenerationStop::TimeLimit),
        TerminationReason::Completed => None,
        _ => Some(Bpp2dColumnGenerationStop::MasterStopped),
    }
}

fn solve_stage(
    backend: &Bpp2dNativeBackend,
    context: &LayerAssignmentContext,
    input: &Bpp2dColumnGenerationInput,
    columns: &[Layer<f64, Meter>],
    stage: LayerAssignmentStage,
) -> ospf_rust_core::Result<Bpp2dMasterResult> {
    let mut model = MetaModel::new("bpp2d-column-generation");
    let assignment = context.register_model(&mut model, LayerAssignmentModelInput { layers: columns, demands: &input.demands, stage })?;
    backend.solve_master(&model, &assignment)
}

/// 使用显式后端完成 Phase I、受限 LP 定价和 final MILP；每次调用独立创建预算与执行器。
/// Complete Phase I, restricted-LP pricing and final MILP with an explicit backend; each invocation creates independent budgets and executor.
///
/// 不调用旧无报告求解器，不自动选择后端，不把启发式候选穷尽当作全局收敛。
/// Does not invoke legacy report-free solvers, select a backend automatically, or equate exhausted heuristic candidates with global convergence.
/// 本入口只注册默认需求模型；附加规则仍可通过 solve_master 注册，但没有匹配定价修正时不得混入此循环。
/// This entry registers the default demand model only; extra rules remain available through solve_master but must not enter this loop without matching pricing corrections.
pub fn solve_bpp2d_column_generation(
    solver: Arc<dyn LinearSolver>,
    execution: Bpp2dExecutionOptions,
    input: &Bpp2dColumnGenerationInput,
    generators: &[Box<dyn Bpp2dLayerGenerator<f64, Meter>>],
) -> ospf_rust_core::Result<Bpp2dColumnGenerationResult> {
    let report_limit = execution.report_limit;
    let backend = Bpp2dNativeBackend::new(solver, execution)?;
    let config = &input.config;
    if config.iteration_limit > 100_000 || config.max_candidates_per_iteration == 0 || config.max_candidates_per_iteration > 100_000
        || config.column_limit == 0 || config.column_limit > 100_000 || input.request.existing_layers.len() > config.column_limit
        || !config.reduced_cost_tolerance.is_finite() || config.reduced_cost_tolerance < 0.0 {
        return Err(invalid("invalid BPP2D column-generation limits or reduced-cost tolerance"));
    }
    let catalog = ColumnCatalog::new(&input.request)?;
    catalog.require_demands(&input.demands)?;
    let mut keys = catalog.keys(&input.request.existing_layers)?;
    let context = LayerAssignmentContext::new(input.objective.clone());
    // 即使预取消，也先拒绝非法需求与目标配置。 / Reject invalid demands/objective options even when precancelled.
    context.register_model(&mut MetaModel::new("bpp2d-preflight"), LayerAssignmentModelInput {
        layers: &input.request.existing_layers, demands: &input.demands, stage: LayerAssignmentStage::FeasibilityLp,
    })?;
    let mut output = Bpp2dColumnGenerationResult {
        columns: input.request.existing_layers.clone(), stages: Vec::new(), iterations: Vec::new(),
        stop: Bpp2dColumnGenerationStop::MasterStopped, final_stage: None, candidate_complete: false,
    };
    let mut stage = if output.columns.is_empty() { LayerAssignmentStage::FeasibilityLp } else { LayerAssignmentStage::Rmp };
    let mut recovered_pool = None;
    output.stop = loop {
        if let Some(stop) = halted(&backend) { break stop; }
        if output.stages.len() >= report_limit { break Bpp2dColumnGenerationStop::ReportLimit; }
        let result = solve_stage(&backend, &context, input, &output.columns, stage)?;
        let stage_index = output.stages.len();
        output.stages.push(Bpp2dColumnGenerationStage { pool_size: output.columns.len(), result: result.clone() });
        if let Some(stop) = terminal(&result).or_else(|| halted(&backend)) { break stop; }
        if result.stage_report.report.problem_status == ProblemStatus::Infeasible {
            if stage == LayerAssignmentStage::Rmp && recovered_pool != Some(output.columns.len()) {
                stage = LayerAssignmentStage::FeasibilityLp;
                continue;
            }
            break Bpp2dColumnGenerationStop::MasterStopped;
        }
        let Some(prices) = result.shadow_prices.as_ref() else { break Bpp2dColumnGenerationStop::MasterStopped; };
        if stage == LayerAssignmentStage::FeasibilityLp && result.objective.is_some_and(|value| value <= backend.tolerance()) {
            recovered_pool = Some(output.columns.len());
            stage = LayerAssignmentStage::Rmp;
            continue;
        }
        if output.iterations.len() >= config.iteration_limit { break Bpp2dColumnGenerationStop::IterationLimit; }
        if output.columns.len() >= config.column_limit { break Bpp2dColumnGenerationStop::ColumnLimit; }
        let request = Bpp2dLayerGenerationRequest {
            iteration: output.iterations.len() as i64, existing_layers: output.columns.clone(),
            demand_entries: generation_demands(&input.demands), shadow_prices: generation_prices(prices),
            max_candidates: config.max_candidates_per_iteration, ..input.request.clone()
        };
        if let Some(stop) = halted(&backend) { break stop; }
        let candidates = LayerGenerationContext::new().generate_explicit(&request, generators);
        if let Some(stop) = halted(&backend) { break stop; }
        let mut improving = Vec::new();
        for candidate in &candidates {
            let key = catalog.validate(&candidate.layer)?;
            let cost = layer_reduced_cost(&candidate.layer, &input.demands, prices,
                if stage == LayerAssignmentStage::FeasibilityLp { 0.0 } else { input.objective.rmp_area_coefficient });
            if !cost.is_finite() { return Err(invalid("BPP2D candidate reduced cost overflows")); }
            if cost < -config.reduced_cost_tolerance && !keys.contains(&key) { improving.push((cost, key, candidate.layer.clone())); }
        }
        improving.sort_by(|lhs, rhs| lhs.0.total_cmp(&rhs.0));
        if let Some(stop) = halted(&backend) { break stop; }
        let mut iteration = Bpp2dPricingIteration {
            stage_index, candidates_evaluated: candidates.len(), candidate_limit_reached: candidates.len() == config.max_candidates_per_iteration,
            added_columns: Vec::new(),
        };
        for (cost, key, layer) in improving {
            if output.columns.len() >= config.column_limit { break; }
            if keys.insert(key) {
                iteration.added_columns.push((output.columns.len(), cost));
                output.columns.push(layer);
            }
        }
        let added = !iteration.added_columns.is_empty();
        output.iterations.push(iteration);
        if !added { break Bpp2dColumnGenerationStop::NoImprovingColumns; }
    };
    if config.final_milp_enabled && !matches!(output.stop, Bpp2dColumnGenerationStop::MasterStopped | Bpp2dColumnGenerationStop::TimeLimit | Bpp2dColumnGenerationStop::Cancelled | Bpp2dColumnGenerationStop::ReportLimit) {
        if let Some(stop) = halted(&backend) { output.stop = stop; }
        else if output.stages.len() >= report_limit { output.stop = Bpp2dColumnGenerationStop::ReportLimit; }
        else {
            let result = solve_stage(&backend, &context, input, &output.columns, LayerAssignmentStage::Integer)?;
            if result.copies.as_ref().is_some_and(|copies| copies.iter().any(|value| *value > 9_007_199_254_740_991.0)) {
                return Err(invalid("BPP2D final copies exceed the exact solver-integer range"));
            }
            output.final_stage = Some(output.stages.len());
            output.stages.push(Bpp2dColumnGenerationStage { pool_size: output.columns.len(), result });
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
