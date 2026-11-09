//! BPP2D 完整原生流程的源绑定 codec；受限模型结论不提升为源问题证明。
//! Source-bound codec for complete native BPP2D workflows, without promoting restricted-model conclusions to source proofs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_framework::solver::remote::{CompiledLinearModelArtifact, encode_compiled_linear_model};
use ospf_rust_framework_bpp2d::{application::{Bpp2dColumnGenerationConfig, Bpp2dColumnGenerationResult, Bpp2dColumnGenerationStop},
    domain::{layer_assignment::LayerAssignmentStage, layer_generation::Bpp2dLayerGenerator}};
use crate::{Bpp2dLayer, BridgeError, ModelArtifact, NativeBackend, Result, decode_bpp2d_layers, decode_bpp2d_problem, encode_bpp2d_layer, invalid};
use crate::bpp2d::{BPP2D_MAX_BYTES, BPP2D_MAX_POOL};

mod production;
mod stage;
mod trace;
pub use production::{Bpp2dProduction, Bpp2dUsage};

/// 宿主可预注册的既有生成器；不接受任意 JS 回调。 / Existing generators that hosts may preregister, without arbitrary JS callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp2dGeneratorKind {
    /// 局部块。 / Local blocks.
    BlockLocal,
    /// 全局块。 / Global blocks.
    BlockGlobal,
    /// 空间切分。 / Space splitting.
    SpaceSplit,
    /// 底部向上左对齐。 / Bottom-up left-justified.
    Bla,
    /// 规则图样。 / Regular patterns.
    Pattern,
    /// 圆形网格。 / Circle grids.
    CircleGrid,
    /// 混合形状。 / Mixed shapes.
    MixedShape,
    /// 历史列回放。 / Historical column replay.
    History,
}

/// 默认规则原生列生成配置，与源物理问题分开版本化。 / Default-rule native column-generation configuration, versioned separately from the source physical problem.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dOptions {
    /// 显式原生后端。 / Explicit native backend.
    pub backend: NativeBackend,
    /// 默认使用原领域完整链；指定时保留链顺序，不按名字筛选自定义形状。 / Defaults to the original complete chain; explicit chains preserve order without name-based shape selection.
    #[ts(optional)]
    pub generators: Option<Vec<Bpp2dGeneratorKind>>,
    /// 默认 8，范围 0..64 个定价批次。 / Default 8, range 0..64 pricing batches.
    #[ts(optional)]
    pub iteration_limit: Option<u32>,
    /// 默认 128，范围 1..512 个观察候选。 / Default 128, range 1..512 observed candidates.
    #[ts(optional)]
    pub candidates_per_iteration: Option<u32>,
    /// 默认 512，必须容纳全部初始列。 / Default 512, must accommodate all initial columns.
    #[ts(optional)]
    pub pool_limit: Option<u32>,
    /// 默认 true；时间／取消／报告预算耗尽不会强制启动 final。 / Default true; time/cancellation/report exhaustion never forces final execution.
    #[ts(optional)]
    pub final_milp_enabled: Option<bool>,
    /// 默认 1e-6，有限且位于 [0, 0.5)。 / Default 1e-6, finite and in [0, 0.5).
    #[ts(optional)]
    pub reduced_cost_tolerance: Option<f64>,
    /// 默认 196，范围 1..1024。 / Default 196, range 1..1024.
    #[ts(optional)]
    pub report_limit: Option<u32>,
}

impl Bpp2dOptions {
    /// 校验有限预算并构造原领域配置，不改变 Rust 默认入口。 / Validate bounded budgets and construct original-domain configuration without changing Rust defaults.
    pub fn config(&self) -> Result<Bpp2dColumnGenerationConfig> {
        let config = Bpp2dColumnGenerationConfig { iteration_limit: self.iteration_limit.unwrap_or(8) as usize,
            max_candidates_per_iteration: self.candidates_per_iteration.unwrap_or(128) as usize, column_limit: self.pool_limit.unwrap_or(512) as usize,
            final_milp_enabled: self.final_milp_enabled.unwrap_or(true), reduced_cost_tolerance: self.reduced_cost_tolerance.unwrap_or(1e-6) };
        if config.iteration_limit > 64 || !(1..=512).contains(&config.max_candidates_per_iteration) || !(1..=BPP2D_MAX_POOL).contains(&config.column_limit)
            || !config.reduced_cost_tolerance.is_finite() || !(0.0..0.5).contains(&config.reduced_cost_tolerance)
            || !(1..=1024).contains(&self.report_limit.unwrap_or(196)) {
            return Err(invalid("invalid BPP2D execution budgets or tolerance"));
        }
        if let Some(generators) = &self.generators {
            let unique = generators.iter().collect::<std::collections::BTreeSet<_>>();
            if generators.is_empty() || generators.len() > 8 || unique.len() != generators.len() { return Err(invalid("BPP2D generator chain must contain 1..8 unique registered strategies")); }
        }
        Ok(config)
    }

    /// 按显式类型构造原生成器，不动态加载任意回调。 / Construct original generators by explicit kind without dynamically loading arbitrary callbacks.
    pub fn generators(&self) -> Result<Vec<Box<dyn Bpp2dLayerGenerator<f64, ospf_rust_quantities::unit::derived::Meter>>>> {
        use ospf_rust_framework_bpp2d::domain::layer_generation::*;
        self.config()?;
        let Some(kinds) = &self.generators else { return Ok(LayerGenerationContext::default_generators()); };
        Ok(kinds.iter().map(|kind| match kind {
            Bpp2dGeneratorKind::BlockLocal => Box::new(BlockLocalLayerGenerator) as Box<dyn Bpp2dLayerGenerator<f64, ospf_rust_quantities::unit::derived::Meter>>,
            Bpp2dGeneratorKind::BlockGlobal => Box::new(BlockGlobalLayerGenerator), Bpp2dGeneratorKind::SpaceSplit => Box::new(SpaceSplitLayerGenerator),
            Bpp2dGeneratorKind::Bla => Box::new(BlaLayerGenerator), Bpp2dGeneratorKind::Pattern => Box::new(PatternLayerGenerator),
            Bpp2dGeneratorKind::CircleGrid => Box::new(CircleGridLayerGenerator), Bpp2dGeneratorKind::MixedShape => Box::new(MixedShapeLayerGenerator),
            Bpp2dGeneratorKind::History => Box::new(HistoryLayerGenerator),
        }).collect())
    }
}

/// 原领域阶段，不能由类型标签冒充不同变量域。 / Original-domain stage, whose tag cannot impersonate a different variable domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp2dStageKind {
    /// 普通受限主 LP。 / Ordinary restricted master LP.
    Rmp,
    /// 归一化短缺 LP。 / Normalized-deficit LP.
    FeasibilityLp,
    /// 当前池整数主问题。 / Integer master over the current pool.
    Integer,
}

impl Bpp2dStageKind {
    pub(crate) fn domain(self) -> LayerAssignmentStage {
        match self { Self::Rmp => LayerAssignmentStage::Rmp, Self::FeasibilityLp => LayerAssignmentStage::FeasibilityLp, Self::Integer => LayerAssignmentStage::Integer }
    }
}

impl From<LayerAssignmentStage> for Bpp2dStageKind {
    fn from(value: LayerAssignmentStage) -> Self { match value { LayerAssignmentStage::Rmp => Self::Rmp, LayerAssignmentStage::FeasibilityLp => Self::FeasibilityLp, LayerAssignmentStage::Integer => Self::Integer } }
}

/// 定价停止原因，不是源问题最优性或可行性结论。 / Pricing stop reason, not a source-problem optimality or feasibility conclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp2dExecutionStop {
    /// 观察候选没有新的改进列。 / Observed candidates contain no new improving columns.
    NoImprovingColumns,
    /// 批次预算。 / Batch budget.
    IterationLimit,
    /// 完整池预算。 / Complete-pool budget.
    ColumnLimit,
    /// 有效报告预算。 / Accepted-report budget.
    ReportLimit,
    /// 总预算或子求解限时。 / Total budget or child time limit.
    TimeLimit,
    /// 合作取消。 / Cooperative cancellation.
    Cancelled,
    /// 没有可继续定价的认证 LP。 / No certified LP suitable for further pricing.
    MasterStopped,
}

impl From<Bpp2dColumnGenerationStop> for Bpp2dExecutionStop {
    fn from(value: Bpp2dColumnGenerationStop) -> Self { match value {
        Bpp2dColumnGenerationStop::NoImprovingColumns => Self::NoImprovingColumns, Bpp2dColumnGenerationStop::IterationLimit => Self::IterationLimit,
        Bpp2dColumnGenerationStop::ColumnLimit => Self::ColumnLimit, Bpp2dColumnGenerationStop::ReportLimit => Self::ReportLimit,
        Bpp2dColumnGenerationStop::TimeLimit => Self::TimeLimit, Bpp2dColumnGenerationStop::Cancelled => Self::Cancelled,
        Bpp2dColumnGenerationStop::MasterStopped => Self::MasterStopped,
    } }
}

/// 实际编译模型及原报告，内部精确 u64 从不经 JS number。 / Actual compiled model and original report, whose exact u64 values never pass through JS number.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dStageReportArtifact {
    /// 明确变量域和目标阶段。 / Explicit variable-domain/objective stage.
    pub stage: Bpp2dStageKind,
    /// 完整池前缀，与份数向量同序。 / Complete-pool prefix in copy-vector order.
    pub pool_size: u32,
    /// 保留真实行来源、类型及映射的模型封装。 / Model artifact retaining actual row origins, types and mapping.
    pub model: CompiledLinearModelArtifact,
    /// 原统一报告文本，不修复外部身份或证明。 / Original unified-report text, without repairing external identity or proof.
    pub solver_report_json: String,
    /// 原始独立目标常数。 / Original separate objective constant.
    pub objective_constant: f64,
}

/// 新增列引用与实算检验数。 / Appended-column reference and recomputed reduced cost.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dPricedColumn {
    /// 完整池索引。 / Complete-pool index.
    pub index: u32,
    /// 必须与已认证阶段对偶及源列重新计算完全一致。 / Must exactly match recomputation from certified stage duals and the source column.
    pub reduced_cost: f64,
}

/// 源绑定定价批次，不声称记录了未观察候选。 / Source-bound pricing batch, without claiming to record unobserved candidates.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dPricingIteration {
    /// 所用 LP 在 stages 中的索引。 / Index of the LP used in stages.
    pub stage_index: u32,
    /// 实际观察候选数。 / Observed candidate count.
    pub candidates_evaluated: u32,
    /// 观察预算是否触顶，不等价于搜索完备。 / Whether the observation budget saturated, not search completeness.
    pub candidate_limit_reached: bool,
    /// 连续追加的新列及检验数，保持接纳顺序。 / Contiguously appended columns and reduced costs, retaining admission order.
    pub added_columns: Vec<Bpp2dPricedColumn>,
}

/// 完整源绑定执行封装，不携带可伪造的全局最优／生产声明。
/// Complete source-bound execution artifact without forgeable global-optimality/production assertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dExecutionReportArtifact {
    /// 独立主版本。 / Independent major version.
    pub schema_version: String,
    /// 源 canonical 问题身份。 / Canonical source-problem identity.
    pub source_fingerprint: String,
    /// 有界预注册策略配置。 / Bounded preregistered-strategy configuration.
    pub configuration: Bpp2dOptions,
    /// 完整物理列池，含未使用列。 / Complete physical pool, including unused columns.
    pub columns: Vec<Bpp2dLayer>,
    /// 全部实际阶段，按执行顺序。 / All actual stages in execution order.
    pub stages: Vec<Bpp2dStageReportArtifact>,
    /// 已完成的定价批次。 / Completed pricing batches.
    pub iterations: Vec<Bpp2dPricingIteration>,
    /// 定价停止原因，final 终态另读。 / Pricing stop reason; read final termination separately.
    pub stop: Bpp2dExecutionStop,
    /// 实际 final 子报告索引，未执行则为空。 / Actual final-child index, absent when not executed.
    pub final_stage: Option<u32>,
}

/// 经源模型及实际报告共同复验的阶段摘要。 / Stage summary jointly revalidated against the source model and actual report.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bpp2dStageSummary {
    /// 阶段。 / Stage.
    pub stage: Bpp2dStageKind,
    /// 实际池前缀。 / Actual pool prefix.
    pub pool_size: u32,
    /// 实际子模型指纹，不是重新编译模型身份。 / Actual child fingerprint, not the recompiled model's identity.
    pub model_fingerprint: String,
    /// 原子问题状态。 / Original child problem status.
    pub problem_status: String,
    /// 原终止原因。 / Original termination reason.
    pub termination_reason: String,
    /// 仅原池内最优，不是源问题最优。 / Optimal over the original pool only, not source-wide optimality.
    pub restricted_optimal: bool,
    /// 原池内认证不可行，不是源问题不可行。 / Certified infeasibility over the original pool only, not source-wide infeasibility.
    pub restricted_infeasible: bool,
    /// 是否有通过原模型复验的完整解。 / Whether a complete incumbent passed original-model validation.
    pub has_incumbent: bool,
    /// 派生目标，独立常数仅计一次。 / Derived objective counting the separate constant once.
    pub objective_value: Option<f64>,
}

/// 全流程派生摘要，不能通过输入字段冒充全局证明。 / Derived workflow summary that cannot impersonate global proofs through input fields.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bpp2dExecutionSummary {
    /// 源问题身份。 / Source-problem identity.
    pub source_fingerprint: String,
    /// 定价停止原因。 / Pricing stop reason.
    pub stop: Bpp2dExecutionStop,
    /// 始终 false，既有生成器是启发式。 / Always false, as existing generators are heuristic.
    pub candidate_complete: bool,
    /// 完整池大小。 / Complete-pool size.
    pub column_count: u32,
    /// 实际阶段数。 / Actual stage count.
    pub stage_count: u32,
    /// 定价批次数。 / Pricing-batch count.
    pub iteration_count: u32,
    /// 实际最终整数阶段摘要。 / Actual final integer-stage summary.
    pub integer: Option<Bpp2dStageSummary>,
    /// 是否由真实整数向量导出了源可行物理产出。 / Whether source-feasible physical production was derived from the actual integer vector.
    pub has_production: bool,
}

/// 独立拥有、经源及全部子阶段复验的执行报告；仍不认证求解器来源。
/// Independently owned execution report revalidated against its source and every child stage, still without authenticating solver provenance.
#[derive(Debug, Clone)]
pub struct Bpp2dExecutionReport {
    /// 原始封装，内部模型／报告文本保留字节。 / Original artifact retaining inner model/report text bytes.
    pub artifact: Bpp2dExecutionReportArtifact,
    /// 验证器派生摘要。 / Validator-derived summary.
    pub summary: Bpp2dExecutionSummary,
    /// 与 stages 同序的派生摘要。 / Derived summaries in stages order.
    pub stages: Vec<Bpp2dStageSummary>,
    /// 仅实际整数 incumbent 可导出，不从 LP 份数构造生产。 / Derived only from an actual integer incumbent, never from LP copies.
    pub production: Option<Bpp2dProduction>,
}

/// 编码真实完整流程后立即用源问题及原报告复验，不写回身份或替代证明。
/// Encode an actual complete workflow and immediately revalidate it against its source and original reports, without rewriting identity or substituting proofs.
pub fn encode_bpp2d_execution_report(
    source: &ModelArtifact,
    configuration: Bpp2dOptions,
    result: &Bpp2dColumnGenerationResult,
) -> Result<String> {
    if result.candidate_complete { return Err(invalid("BPP2D heuristic execution cannot claim complete pricing")); }
    let artifact = Bpp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), configuration,
        columns: result.columns.iter().map(encode_bpp2d_layer).collect::<Result<_>>()?,
        stages: result.stages.iter().map(|record| Ok(Bpp2dStageReportArtifact { stage: record.result.stage_report.stage.into(), pool_size: u32::try_from(record.pool_size).map_err(invalid)?,
            model: encode_compiled_linear_model(&record.result.stage_report.model).map_err(invalid)?, solver_report_json: serde_json::to_string(&record.result.stage_report.report).map_err(invalid)?,
            objective_constant: record.result.stage_report.objective_constant })).collect::<Result<_>>()?,
        iterations: result.iterations.iter().map(|record| Ok(Bpp2dPricingIteration { stage_index: u32::try_from(record.stage_index).map_err(invalid)?,
            candidates_evaluated: u32::try_from(record.candidates_evaluated).map_err(invalid)?, candidate_limit_reached: record.candidate_limit_reached,
            added_columns: record.added_columns.iter().map(|(index, cost)| Ok(Bpp2dPricedColumn { index: u32::try_from(*index).map_err(invalid)?, reduced_cost: *cost })).collect::<Result<_>>()?,
        })).collect::<Result<_>>()?, stop: result.stop.into(), final_stage: result.final_stage.map(u32::try_from).transpose().map_err(invalid)? };
    let text = serde_json::to_string(&artifact).map_err(invalid)?;
    decode_bpp2d_execution_report(&text, source)?;
    Ok(text)
}

/// 复验全池、每个源主问题、真实报告和定价生命周期，再从实际 final 向量提取精确产出。
/// Revalidate the full pool, every source master, actual report and pricing lifecycle, then extract exact production from the actual final vector.
pub fn decode_bpp2d_execution_report(text: &str, source: &ModelArtifact) -> Result<Bpp2dExecutionReport> {
    if text.len() > BPP2D_MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP2D execution report exceeds 8 MiB")); }
    let artifact: Bpp2dExecutionReportArtifact = serde_json::from_str(text).map_err(invalid)?;
    if artifact.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP2D execution schema")); }
    let (problem, mut input) = decode_bpp2d_problem(source)?;
    if artifact.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP2D execution belongs to a different source problem")); }
    input.config = artifact.configuration.config()?;
    if input.request.existing_layers.len() > input.config.column_limit || artifact.columns.len() > input.config.column_limit
        || artifact.stages.len() > artifact.configuration.report_limit.unwrap_or(196) as usize || artifact.iterations.len() > input.config.iteration_limit {
        return Err(invalid("BPP2D execution exceeds configured pool, stage or batch budgets"));
    }
    let layers = decode_bpp2d_layers(&input.request, &input.demands, &artifact.columns)?;
    let decoded = artifact.stages.iter().map(|record| stage::decode(record, &input, &layers)).collect::<Result<Vec<_>>>()?;
    trace::validate(&artifact, &input, &layers, &decoded)?;
    let production = artifact.final_stage.map(|index| production::derive(&problem, &input, &layers, &decoded[index as usize])).transpose()?.flatten();
    let stages = decoded.iter().map(|stage| stage.summary.clone()).collect::<Vec<_>>();
    let summary = Bpp2dExecutionSummary { source_fingerprint: source.fingerprint.clone(), stop: artifact.stop, candidate_complete: false,
        column_count: layers.len() as u32, stage_count: stages.len() as u32, iteration_count: artifact.iterations.len() as u32,
        integer: artifact.final_stage.map(|index| stages[index as usize].clone()), has_production: production.is_some() };
    Ok(Bpp2dExecutionReport { artifact, summary, stages, production })
}

#[cfg(test)]
mod tests;
