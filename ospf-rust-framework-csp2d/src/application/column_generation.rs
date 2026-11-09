//! 原生报告驱动 CSP2D 列生成；受限主问题结论不提升为源问题全局证明。 / Native report-driven CSP2D column generation; restricted-master conclusions are not promoted to global source proofs.

use std::collections::BTreeSet;
use std::sync::Arc;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::{LinearSolver, ProblemStatus, TerminationReason};
use crate::application::report_solver::{Csp2dExecutionOptions, Csp2dMasterResult, Csp2dNativeBackend};
use crate::domain::material::CuttingPlan2;
use crate::domain::pattern_generation::{PatternGenerationRequest2, PatternGenerator2};
use crate::domain::pattern_generation::catalog::{ColumnCatalog, PatternColumnKey, invalid};
use crate::domain::produce::{Csp2dMasterDemand, ProduceContext, ProduceModelInput, ProduceObjective, ProducePricingInput, ProduceStage};

/// 列池与候选观察预算，不是任意用户生成器的抢占或硬内存配额。 / Pool/candidate observation budgets, not preemption or hard memory quotas for arbitrary user generators.
#[derive(Debug, Clone)]
pub struct Csp2dColumnGenerationConfig {
    /// 定价批次上限，含无改进批次；默认 20，最大 100000。 / Pricing-batch cap including non-improving batches; default 20, maximum 100000.
    pub iteration_limit: usize,
    /// 全物料／生成器合计每批最多观察的候选，默认 32，最大 100000。 / Per-batch observed-candidate cap across all materials/generators; default 32, maximum 100000.
    pub max_candidates_per_iteration: usize,
    /// 完整列池上限，含保留原位置的重复初始列；默认 10000，最大 100000。 / Complete-pool cap including duplicate initial columns at original positions; default 10000, maximum 100000.
    pub column_limit: usize,
    /// 正常停止后在完整池执行同后端整数主问题，默认启用。 / Run a same-backend integer master on the complete pool after a normal stop, enabled by default.
    pub final_integer_enabled: bool,
    /// 仅接收严格低于负容差的有限检验数，默认 1e-6。 / Accept only finite reduced costs strictly below the negative tolerance, default 1e-6.
    pub reduced_cost_tolerance: f64,
}

impl Default for Csp2dColumnGenerationConfig {
    fn default() -> Self {
        Self { iteration_limit: 20, max_candidates_per_iteration: 32, column_limit: 10_000, final_integer_enabled: true, reduced_cost_tolerance: 1e-6 }
    }
}

/// 显式原生边界，使用米制物理目录及默认需求／库存规则，不改变旧 service。 / Explicit native boundary using meter-based catalogs and default demand/stock rules, without changing the legacy service.
#[derive(Debug, Clone)]
pub struct Csp2dColumnGenerationInput {
    /// 每物料一个生成请求；产品件数仅控制候选生成，不是主问题需求或使用上界。 / One request per material; product counts control candidate generation, not master demands or usage bounds.
    pub requests: Vec<PatternGenerationRequest2<f64, Meter>>,
    /// 主问题完整需求及可选硬上界，件数与面积口径保持独立。 / Complete master demands and optional hard upper bounds, keeping count/area bases distinct.
    pub demands: Vec<Csp2dMasterDemand>,
    /// 完整初始物理池，允许为空并保留重复位置。 / Complete initial physical pool, allowing empty input and preserving duplicate positions.
    pub initial_columns: Vec<CuttingPlan2<f64, Meter>>,
    /// RMP 与 final 共用原料域面积／余料／卷段高／次数目标。 / Shared RMP/final objective using original region area/waste/coil height/use costs.
    pub objective: ProduceObjective,
    /// 迭代及池预算；时间、取消、进度与报告预算来自 execution options。 / Iteration/pool budgets; time, cancellation, progress and report budgets come from execution options.
    pub config: Csp2dColumnGenerationConfig,
}

impl Csp2dColumnGenerationInput {
    /// 运行时无关的源目录／物理列／模型校验，不启动生成器、后端或时钟。 / Runtime-independent source/physical-column/model validation without generators, backends or clocks.
    pub fn validate(&self) -> ospf_rust_core::Result<()> { self.prepare().map(|_| ()) }

    /// 按当前源目录、需求及目标复验完整候选池，不声称其用量满足需求。 / Revalidate a complete pool against current sources/demands/objective, without claiming any usage satisfies demand.
    pub fn validate_columns(&self, columns: &[CuttingPlan2<f64, Meter>]) -> ospf_rust_core::Result<()> {
        let catalog = ColumnCatalog::new(&self.requests)?;
        catalog.require_demands(&self.demands)?;
        catalog.keys(columns)?;
        let master = catalog.master(columns, &self.demands);
        ProduceContext { objective: self.objective.clone() }.register_model(&mut MetaModel::new("csp2d-column-validation"),
            ProduceModelInput { master: &master, stage: ProduceStage::FeasibilityLp, kerf: catalog.kerf() })?;
        Ok(())
    }

    /// 按原顺序返回默认行语义下的物理身份，保留重复位置；不包含方案名或生成来源。 / Return physical identities in original order under default-row semantics, retaining duplicate positions and excluding plan names/generator provenance.
    pub fn column_keys(&self, columns: &[CuttingPlan2<f64, Meter>]) -> ospf_rust_core::Result<Vec<PatternColumnKey>> {
        let catalog = ColumnCatalog::new(&self.requests)?;
        catalog.require_demands(&self.demands)?;
        columns.iter().map(|column| catalog.validate(column)).collect()
    }

    fn prepare(&self) -> ospf_rust_core::Result<(ColumnCatalog<'_>, ProduceContext, BTreeSet<PatternColumnKey>)> {
        let config = &self.config;
        if config.iteration_limit > 100_000 || config.max_candidates_per_iteration == 0 || config.max_candidates_per_iteration > 100_000
            || config.column_limit == 0 || config.column_limit > 100_000 || self.initial_columns.len() > config.column_limit
            || !config.reduced_cost_tolerance.is_finite() || config.reduced_cost_tolerance < 0.0 {
            return Err(invalid("invalid CSP2D column-generation limits or reduced-cost tolerance"));
        }
        let catalog = ColumnCatalog::new(&self.requests)?;
        catalog.require_demands(&self.demands)?;
        let keys = catalog.keys(&self.initial_columns)?;
        let context = ProduceContext { objective: self.objective.clone() };
        let initial = catalog.master(&self.initial_columns, &self.demands);
        context.register_model(&mut MetaModel::new("csp2d-preflight"), ProduceModelInput { master: &initial, stage: ProduceStage::FeasibilityLp, kerf: catalog.kerf() })?;
        Ok((catalog, context, keys))
    }
}

/// 定价循环停止原因，不是源问题求解状态。 / Pricing-loop stop reason, not source-problem solve status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Csp2dColumnGenerationStop {
    /// 观察到的启发式候选没有新改进列，不证明 exact pricing 收敛。 / No new improving observed heuristic candidate, not proof of exact-pricing convergence.
    NoImprovingColumns,
    /// 已耗尽定价批次。 / Pricing batches exhausted.
    IterationLimit,
    /// 完整列池已达上限。 / Complete pool reached its cap.
    ColumnLimit,
    /// 已达原始子报告上限。 / Original child-report cap reached.
    ReportLimit,
    /// 总预算耗尽或子求解限时。 / Total budget exhausted or child timed out.
    TimeLimit,
    /// 外部合作取消或子求解取消。 / External cooperative cancellation or child cancellation.
    Cancelled,
    /// 没有可继续定价的认证最优 LP。 / No certified optimal LP suitable for continued pricing.
    MasterStopped,
}

/// 实际子模型及其完整池前缀。 / Actual child model and its complete-pool prefix.
#[derive(Debug, Clone)]
pub struct Csp2dColumnGenerationStage {
    /// 注册时的池前缀长度，用量向量严格对应此前缀。 / Registered pool-prefix length, matched exactly by the usage vector.
    pub pool_size: usize,
    /// 实际模型、原报告及复验结果。 / Actual model, original report and revalidated result.
    pub result: Csp2dMasterResult,
}

/// 由已认证 LP 对偶驱动的一批候选。 / A candidate batch driven by certified LP duals.
#[derive(Debug, Clone)]
pub struct Csp2dPricingIteration {
    /// 所用实际 LP 在 stages 中的位置。 / Position of the actual LP used within stages.
    pub stage_index: usize,
    /// 通过源目录／几何门禁的观察候选数，含重复及非改进候选。 / Observed candidates passing source/geometry gates, including duplicates/non-improving candidates.
    pub candidates_evaluated: usize,
    /// 是否达到全物料合计观察上限，不代表候选完备性。 / Whether the cross-material observation cap was reached, not candidate completeness.
    pub candidate_limit_reached: bool,
    /// 实际新增池位置及重算检验数，不信任生成器价格或历史。 / Actual new pool positions and recomputed reduced costs, not generator prices/history.
    pub added_columns: Vec<(usize, f64)>,
}

/// 完整执行轨迹；整数 incumbent 只属于 final 实际子模型，不构造源问题全局结论。 / Complete trace; integer incumbents belong only to the actual final child model, without global source conclusions.
#[derive(Debug, Clone)]
pub struct Csp2dColumnGenerationResult {
    /// 完整候选池，final 选中子集不替换该池。 / Complete candidate pool, never replaced by the final selected subset.
    pub columns: Vec<CuttingPlan2<f64, Meter>>,
    /// 所有有效实际子模型与原报告，含 final。 / Every valid actual child model/original report, including final.
    pub stages: Vec<Csp2dColumnGenerationStage>,
    /// 已完成的定价批次。 / Completed pricing batches.
    pub iterations: Vec<Csp2dPricingIteration>,
    /// 定价停止原因，final 的真实报告终态单独保留。 / Pricing stop reason, with the actual final terminal retained separately.
    pub stop: Csp2dColumnGenerationStop,
    /// final 子模型位置，未执行时为 None。 / Final child index, or None when not executed.
    pub final_stage: Option<usize>,
    /// 当前始终 false，原启发式生成器不提供 exact pricing 证明。 / Always false currently; original heuristic generators supply no exact-pricing proof.
    pub candidate_complete: bool,
}

fn halted(backend: &Csp2dNativeBackend) -> Option<Csp2dColumnGenerationStop> {
    if backend.is_cancelled() { Some(Csp2dColumnGenerationStop::Cancelled) }
    else if backend.remaining_time().is_zero() { Some(Csp2dColumnGenerationStop::TimeLimit) }
    else { None }
}

fn terminal(result: &Csp2dMasterResult) -> Option<Csp2dColumnGenerationStop> {
    match result.stage_report.report.termination_reason {
        TerminationReason::Cancelled => Some(Csp2dColumnGenerationStop::Cancelled),
        TerminationReason::TimeLimit => Some(Csp2dColumnGenerationStop::TimeLimit),
        TerminationReason::Completed => None,
        _ => Some(Csp2dColumnGenerationStop::MasterStopped),
    }
}

fn solve_stage(
    backend: &Csp2dNativeBackend,
    context: &ProduceContext,
    catalog: &ColumnCatalog<'_>,
    input: &Csp2dColumnGenerationInput,
    columns: &[CuttingPlan2<f64, Meter>],
    stage: ProduceStage,
) -> ospf_rust_core::Result<Csp2dMasterResult> {
    let mut model = MetaModel::new("csp2d-column-generation");
    let master = catalog.master(columns, &input.demands);
    let registration = context.register_model(&mut model, ProduceModelInput { master: &master, stage, kerf: catalog.kerf() })?;
    backend.solve_master(&model, &registration)
}

/// 显式后端和生成器驱动的 Phase I／认证 LP 定价／最终整数流程，不调用旧启发式主问题。
/// Phase I/certified-LP pricing/final-integer workflow driven by explicit backend/generators, never invoking the legacy heuristic master.
///
/// 所有实际子求解共用执行预算。默认行外的业务 pipeline 仍需通过 solve_master 及匹配定价修正扩展，不能静默混入本循环。
/// All actual children share execution budgets. Extra business pipelines still require solve_master plus matching pricing corrections and cannot silently enter this loop.
pub fn solve_csp2d_column_generation(
    solver: Arc<dyn LinearSolver>,
    execution: Csp2dExecutionOptions,
    input: &Csp2dColumnGenerationInput,
    generators: &[Box<dyn PatternGenerator2<f64, Meter>>],
) -> ospf_rust_core::Result<Csp2dColumnGenerationResult> {
    let report_limit = execution.report_limit;
    let backend = Csp2dNativeBackend::new(solver, execution)?;
    let config = &input.config;
    let (catalog, context, mut keys) = input.prepare()?;
    let mut output = Csp2dColumnGenerationResult {
        columns: input.initial_columns.clone(), stages: Vec::new(), iterations: Vec::new(),
        stop: Csp2dColumnGenerationStop::MasterStopped, final_stage: None, candidate_complete: false,
    };
    let mut stage = if output.columns.is_empty() { ProduceStage::FeasibilityLp } else { ProduceStage::Rmp };
    let mut recovered_pool = None;
    output.stop = loop {
        if let Some(stop) = halted(&backend) { break stop; }
        if output.stages.len() >= report_limit { break Csp2dColumnGenerationStop::ReportLimit; }
        let result = solve_stage(&backend, &context, &catalog, input, &output.columns, stage)?;
        let stage_index = output.stages.len();
        output.stages.push(Csp2dColumnGenerationStage { pool_size: output.columns.len(), result: result.clone() });
        if let Some(stop) = terminal(&result).or_else(|| halted(&backend)) { break stop; }
        if result.stage_report.report.problem_status == ProblemStatus::Infeasible {
            if stage == ProduceStage::Rmp && recovered_pool != Some(output.columns.len()) { stage = ProduceStage::FeasibilityLp; continue; }
            break Csp2dColumnGenerationStop::MasterStopped;
        }
        let Some(prices) = result.shadow_prices.as_ref() else { break Csp2dColumnGenerationStop::MasterStopped; };
        // 归一化目标很小不代表原单位短缺为零，不能据此绕过恢复阶段。 / A tiny normalized objective does not imply zero original-unit deficits and must not bypass recovery.
        if stage == ProduceStage::FeasibilityLp && result.deficits.as_ref().is_some_and(|values| values.iter().all(|value| value.abs() <= backend.tolerance())) {
            recovered_pool = Some(output.columns.len()); stage = ProduceStage::Rmp; continue;
        }
        if output.iterations.len() >= config.iteration_limit { break Csp2dColumnGenerationStop::IterationLimit; }
        if output.columns.len() >= config.column_limit { break Csp2dColumnGenerationStop::ColumnLimit; }
        let mut improving = Vec::new();
        let mut evaluated = 0usize;
        for source in &input.requests {
            for generator in generators {
                let remaining = config.max_candidates_per_iteration - evaluated;
                if remaining == 0 { break; }
                if let Some(stop) = halted(&backend) { return Ok(Csp2dColumnGenerationResult { stop, ..output }); }
                let mut request = catalog.generation_request(source, &output.columns, prices, output.iterations.len(), remaining)?;
                request.waste_weight = Some(input.objective.waste_weight);
                let candidates = generator.generate(&request);
                if let Some(stop) = halted(&backend) { return Ok(Csp2dColumnGenerationResult { stop, ..output }); }
                for pattern in candidates.into_iter().take(remaining) {
                    if let Some(stop) = halted(&backend) { return Ok(Csp2dColumnGenerationResult { stop, ..output }); }
                    let column = CuttingPlan2::from_pattern(pattern);
                    let key = catalog.validate(&column)?;
                    let cost = context.reduced_cost(ProducePricingInput { candidate: &column, demands: &input.demands, prices, stage })?;
                    evaluated += 1;
                    if cost < -config.reduced_cost_tolerance && !keys.contains(&key) { improving.push((cost, key, column)); }
                }
            }
        }
        improving.sort_by(|left, right| left.0.total_cmp(&right.0));
        if let Some(stop) = halted(&backend) { break stop; }
        let mut iteration = Csp2dPricingIteration {
            stage_index, candidates_evaluated: evaluated, candidate_limit_reached: evaluated == config.max_candidates_per_iteration, added_columns: Vec::new(),
        };
        for (cost, key, column) in improving {
            if output.columns.len() >= config.column_limit { break; }
            if keys.insert(key) { iteration.added_columns.push((output.columns.len(), cost)); output.columns.push(column); }
        }
        let added = !iteration.added_columns.is_empty();
        output.iterations.push(iteration);
        if !added { break Csp2dColumnGenerationStop::NoImprovingColumns; }
    };
    if config.final_integer_enabled && !matches!(output.stop, Csp2dColumnGenerationStop::MasterStopped | Csp2dColumnGenerationStop::TimeLimit | Csp2dColumnGenerationStop::Cancelled | Csp2dColumnGenerationStop::ReportLimit) {
        if let Some(stop) = halted(&backend) { output.stop = stop; }
        else if output.stages.len() >= report_limit { output.stop = Csp2dColumnGenerationStop::ReportLimit; }
        else {
            let result = solve_stage(&backend, &context, &catalog, input, &output.columns, ProduceStage::Integer)?;
            if result.usages.as_ref().is_some_and(|values| values.iter().any(|value| *value > 9_007_199_254_740_991.0)) {
                return Err(invalid("CSP2D final usages exceed the exact solver-integer range"));
            }
            output.final_stage = Some(output.stages.len());
            output.stages.push(Csp2dColumnGenerationStage { pool_size: output.columns.len(), result });
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
