//! 一维装箱的无损协议、领域适配及共享校验。 / Lossless one-dimensional packing protocol, domain adapters and shared validation.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::solver::{CURRENT_SOLVE_REPORT_SCHEMA_VERSION, ProblemStatus, ProofKind, ProofStatus, SolveReport, sha256_fingerprint};
use ospf_rust_framework_bpp::infrastructure::RuntimePackingValueAdapter;
use ospf_rust_framework_bpp1d::application::{Bpp1dSolution, Bpp1dSolveConfig};
use ospf_rust_framework_bpp1d::domain as domain;
use ospf_rust_quantities::unit::Unit;
use crate::{BridgeError, ModelArtifact, NativeBackend, PhysicalQuantity, QuantityUnit, Result, invalid};

/// 当前绑定的运行时单位领域上下文。 / Current binding's runtime-unit domain context.
pub type Bpp1dContext = domain::PackingContext<f64, Unit, Unit>;

/// 物品输入，所有精确整数使用 canonical u64 十进制文本。 / Item input, with all exact integers encoded as canonical decimal u64 text.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dItem {
    /// 稳定物品身份。 / Stable item identity.
    pub id: String,
    /// 单件长度。 / Piece length.
    pub length: PhysicalQuantity,
    /// 单件重量。 / Piece weight.
    pub weight: PhysicalQuantity,
    /// 精确需求文本。 / Exact demand text.
    pub amount: String,
    /// 每件物料贡献的精确文本。 / Exact per-piece material contributions.
    pub materials: BTreeMap<String, String>,
}

/// 箱型输入。 / Bin-type input.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dBinType {
    /// 稳定箱型身份。 / Stable bin-type identity.
    pub id: String,
    /// 长度容量。 / Length capacity.
    pub length: PhysicalQuantity,
    /// 重量容量。 / Weight capacity.
    pub max_weight: PhysicalQuantity,
    /// 正有限成本。 / Positive finite cost.
    pub cost: f64,
    /// 可选库存的 canonical u64 文本，空表示无限。 / Optional canonical u64 inventory text, absent for unlimited stock.
    pub inventory: Option<String>,
}

/// 装箱目标，不在 SDK 中重算目标系数。 / Packing objective, without recomputing coefficients in the SDK.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp1dObjective {
    /// 最少箱数。 / Minimum bin count.
    BinCount,
    /// 最少箱长。 / Minimum consumed bin length.
    ConsumedLength,
    /// 最小成本。 / Minimum cost.
    Cost,
}

/// 有界数量的领域输入，单位归一由原生领域 context 完成。 / Size-bounded domain input, normalized by the existing domain context.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dProblem {
    /// 最多 1000 种物品。 / Up to 1000 item types.
    pub items: Vec<Bpp1dItem>,
    /// 最多 128 种箱型。 / Up to 128 bin types.
    pub bin_types: Vec<Bpp1dBinType>,
    /// 仅相邻件间距，无尾间距。 / Inter-piece spacing only, without trailing spacing.
    pub spacing: PhysicalQuantity,
    /// 长度归一单位。 / Length normalization unit.
    pub length_unit: QuantityUnit,
    /// 重量归一单位。 / Weight normalization unit.
    pub weight_unit: QuantityUnit,
    /// 目标。 / Objective.
    pub objective: Bpp1dObjective,
}

/// CSV 导入的声明口径：输出单位、间距与目标；表头必须是闭合 schema。
/// Declared conventions for CSV import: output units, spacing and objective, with a closed schema header.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dCsvOptions {
    /// 长度单位。 / Length unit.
    pub length_unit: QuantityUnit,
    /// 重量单位。 / Weight unit.
    pub weight_unit: QuantityUnit,
    /// 仅相邻件间距。 / Inter-piece spacing only.
    pub spacing: PhysicalQuantity,
    /// 目标口径。 / Objective metric.
    pub objective: Bpp1dObjective,
}

/// 严格读取两张 CSV 并返回常规 BPP1D 问题 DTO；后续校验与编码完全复用既有链路。
/// Read the two strict CSV tables and return an ordinary BPP1D problem DTO; later validation and encoding
/// reuse the existing path unchanged.
pub fn load_bpp1d_csv(items_text: &str, bins_text: &str, options: &Bpp1dCsvOptions) -> Result<Bpp1dProblem> {
    if items_text.len() + bins_text.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "BPP1D CSV input exceeds 8 MiB")); }
    let declared = ospf_rust_framework_bpp1d::infrastructure::PackingCsvOptions { length_unit: options.length_unit.unit(), weight_unit: options.weight_unit.unit(),
        spacing: options.spacing.quantity()?, objective: match options.objective { Bpp1dObjective::BinCount => domain::PackingObjective::BinCount,
            Bpp1dObjective::ConsumedLength => domain::PackingObjective::ConsumedLength, Bpp1dObjective::Cost => domain::PackingObjective::Cost } };
    // CSV 文本里是十进制数字，先按 f64 解析；量纲与有限性仍由原域 context 复核。
    // CSV text carries decimal numbers, parsed as f64 first; the original domain context still checks dimensions and finiteness.
    let parse = |text: &str| text.parse::<f64>().map_err(|_| -> ospf_rust_core::error::CoreError {
        ospf_rust_core::error::ModelError::InvalidConstraint("invalid BPP1D CSV number".into()).into()
    });
    let context = ospf_rust_framework_bpp1d::infrastructure::load_packing_csv(items_text, bins_text, declared, parse,
        &RuntimePackingValueAdapter, &RuntimePackingValueAdapter).map_err(invalid)?;
    let problem = context.problem();
    let quantity = |value: f64, unit: QuantityUnit| PhysicalQuantity { value, unit };
    Ok(Bpp1dProblem {
        items: problem.items.iter().map(|item| Bpp1dItem { id: item.id.clone(), length: quantity(item.length.value, options.length_unit),
            weight: quantity(item.weight.value, options.weight_unit), amount: item.amount.to_string(),
            materials: item.materials.iter().map(|(id, value)| (id.clone(), value.to_string())).collect() }).collect(),
        bin_types: problem.bin_types.iter().map(|bin| Bpp1dBinType { id: bin.id.clone(), length: quantity(bin.length.value, options.length_unit),
            max_weight: quantity(bin.max_weight.value, options.weight_unit), cost: bin.cost,
            inventory: bin.inventory.map(|value| value.to_string()) }).collect(),
        spacing: quantity(problem.spacing.value, options.length_unit),
        length_unit: options.length_unit, weight_unit: options.weight_unit, objective: options.objective,
    })
}

fn count(value: &str) -> Result<u64> {
    let parsed = value.parse::<u64>().map_err(invalid)?;
    if parsed.to_string() != value { return Err(invalid("expected canonical decimal u64 text")); }
    Ok(parsed)
}

fn identity(id: &str) -> Result<()> {
    if id.trim().is_empty() || id.len() > 256 { return Err(invalid("blank or oversized BPP1D identity")); }
    Ok(())
}

#[cfg(test)]
mod csv_tests {
    use super::*;

    fn options() -> Bpp1dCsvOptions {
        Bpp1dCsvOptions { length_unit: QuantityUnit::Centimeter, weight_unit: QuantityUnit::Kilogram,
            spacing: PhysicalQuantity { value: 1.0, unit: QuantityUnit::Centimeter }, objective: Bpp1dObjective::BinCount }
    }

    #[test]
    fn csv_tables_become_an_ordinary_problem_and_keep_strict_schema() {
        let items = "id,length,weight,amount
a,4,0.1,2
b,2,0.1,1
";
        let bins = "id,length,max_weight,cost,inventory
bin,10,1,2,2
";
        let problem = load_bpp1d_csv(items, bins, &options()).unwrap();
        assert_eq!(problem.items.len(), 2);
        assert_eq!(problem.items[0].amount, "2");
        assert_eq!(problem.bin_types[0].inventory.as_deref(), Some("2"));
        assert_eq!(problem.length_unit, QuantityUnit::Centimeter);
        // 空库存表示无限 / An empty inventory means unlimited stock.
        let unlimited = load_bpp1d_csv(items, "id,length,max_weight,cost,inventory
bin,10,1,2,
", &options()).unwrap();
        assert!(unlimited.bin_types[0].inventory.is_none());
        // 表头必须闭合且不能重复 / The header must be closed and free of duplicates.
        assert!(load_bpp1d_csv("id,length,weight,amount,extra
a,4,0.1,2,1
", bins, &options()).is_err());
        assert!(load_bpp1d_csv("id,length,weight
a,4,0.1
", bins, &options()).is_err());
        // 引号未闭合与非数字必须失败，不静默截断 / An unclosed quote or a non-numeric cell must fail without silent truncation.
        assert!(load_bpp1d_csv("id,length,weight,amount
\"a,4,0.1,2
", bins, &options()).is_err());
        assert!(load_bpp1d_csv("id,length,weight,amount
a,four,0.1,2
", bins, &options()).is_err());
        // 产物必须能走既有指纹编码 / The result must encode through the existing fingerprint path.
        assert!(encode_bpp1d_problem(load_bpp1d_csv(items, bins, &options()).unwrap()).is_ok());
    }
}

impl Bpp1dProblem {
    /// 调用既有单位 adapter 和 PackingContext，保留所有领域校验。 / Call the existing unit adapter and PackingContext, retaining all domain validation.
    pub fn context(&self) -> Result<Bpp1dContext> {
        if self.items.len() > 1000 || self.bin_types.len() > 128 { return Err(BridgeError::new("PayloadTooLarge", "BPP1D input exceeds item/bin limits")); }
        let items = self.items.iter().map(|item| {
            identity(&item.id)?;
            if item.materials.len() > 1000 { return Err(invalid("too many material contributions")); }
            let materials = item.materials.iter().map(|(id, value)| { identity(id)?; Ok((id.clone(), count(value)?)) }).collect::<Result<_>>()?;
            Ok(domain::Item { id: item.id.clone(), length: item.length.quantity()?, weight: item.weight.quantity()?, amount: count(&item.amount)?, materials })
        }).collect::<Result<_>>()?;
        let bin_types = self.bin_types.iter().map(|bin| {
            identity(&bin.id)?;
            Ok(domain::BinType { id: bin.id.clone(), length: bin.length.quantity()?, max_weight: bin.max_weight.quantity()?, cost: bin.cost,
                inventory: bin.inventory.as_deref().map(count).transpose()? })
        }).collect::<Result<_>>()?;
        domain::PackingContext::new(domain::PackingProblem { items, bin_types, spacing: self.spacing.quantity()?, length_unit: self.length_unit.unit(),
            weight_unit: self.weight_unit.unit(), objective: match self.objective { Bpp1dObjective::BinCount => domain::PackingObjective::BinCount,
                Bpp1dObjective::ConsumedLength => domain::PackingObjective::ConsumedLength, Bpp1dObjective::Cost => domain::PackingObjective::Cost } },
            &RuntimePackingValueAdapter, &RuntimePackingValueAdapter).map_err(invalid)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProblemEnvelope { schema_version: String, problem: Bpp1dProblem }

/// 用共享审计指纹编码 canonical 问题文本，不经 JS 摘要或整数解析。 / Encode canonical problem text with shared audit fingerprints, without JS hashing or integer parsing.
pub fn encode_bpp1d_problem(problem: Bpp1dProblem) -> Result<ModelArtifact> {
    problem.context()?;
    let model_json = serde_json::to_string(&ProblemEnvelope { schema_version: "1.0".into(), problem }).map_err(invalid)?;
    if model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "BPP1D problem exceeds 8 MiB")); }
    let fingerprint = sha256_fingerprint("ospf.bpp1d.problem.v1", model_json.as_bytes()).value;
    Ok(ModelArtifact { model_json, fingerprint })
}

/// 复验 canonical 字节、协议主版本、领域输入和外层指纹。 / Recheck canonical bytes, protocol major, domain input and outer fingerprint.
pub fn decode_bpp1d_problem(artifact: &ModelArtifact) -> Result<(Bpp1dProblem, Bpp1dContext)> {
    if artifact.model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "BPP1D problem exceeds 8 MiB")); }
    let envelope: ProblemEnvelope = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP1D problem schema")); }
    let encoded = encode_bpp1d_problem(envelope.problem.clone())?;
    if encoded.model_json != artifact.model_json || encoded.fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP1D canonical problem or fingerprint mismatch")); }
    let context = envelope.problem.context()?;
    Ok((envelope.problem, context))
}

/// 稳定去重的组合列，件数为 canonical u64 文本。 / Stably deduplicated pattern with canonical u64 piece-count text.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dPattern {
    /// 箱型身份。 / Bin-type identity.
    pub bin_type_id: String,
    /// 每种物品件数。 / Piece count by item identity.
    pub amounts: BTreeMap<String, String>,
}

impl Bpp1dPattern {
    /// 解码精确整数，不给缺失项补零。 / Decode exact integers without filling absent entries with zero.
    pub fn pattern(&self) -> Result<domain::PackingPattern> {
        Ok(domain::PackingPattern { bin_type_id: self.bin_type_id.clone(), amounts: self.amounts.iter().map(|(id, value)| Ok((id.clone(), count(value)?))).collect::<Result<_>>()? })
    }

    fn from_pattern(pattern: &domain::PackingPattern) -> Self {
        Self { bin_type_id: pattern.bin_type_id.clone(), amounts: pattern.amounts.iter().map(|(id, value)| (id.clone(), value.to_string())).collect() }
    }
}

/// 组合使用次数。 / Pattern usage count.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dUsage {
    /// 完整组合。 / Complete pattern.
    pub pattern: Bpp1dPattern,
    /// 正 canonical u64 次数。 / Positive canonical u64 usage count.
    pub count: String,
}

/// 拒绝重复组合，避免集合收集时静默覆盖。 / Reject duplicate patterns instead of silently overwriting during collection.
pub fn bpp1d_usages(usages: &[Bpp1dUsage]) -> Result<BTreeMap<domain::PackingPattern, u64>> {
    if usages.len() > 10_000 { return Err(invalid("too many BPP1D usages")); }
    let mut result = BTreeMap::new();
    for usage in usages {
        if result.insert(usage.pattern.pattern()?, count(&usage.count)?).is_some() { return Err(invalid("duplicate BPP1D usage pattern")); }
    }
    Ok(result)
}

/// 有界候选生成选项。 / Bounded pattern-generation options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dGenerationOptions {
    /// 默认和最大均为 10000。 / Default and maximum are 10000.
    #[ts(optional)]
    pub max_patterns: Option<u32>,
    /// 默认和最大均为 100000。 / Default and maximum are 100000.
    #[ts(optional)]
    pub max_nodes: Option<u32>,
}

impl Bpp1dGenerationOptions {
    /// 保留 Rust 默认值并限制纯计算工作量。 / Retain Rust defaults and bound portable computation work.
    pub fn config(&self) -> Result<domain::PatternGenerationConfig> {
        let defaults = domain::PatternGenerationConfig::default();
        let config = domain::PatternGenerationConfig { max_patterns: self.max_patterns.map_or(defaults.max_patterns, |value| value as usize),
            max_nodes: self.max_nodes.map_or(defaults.max_nodes, |value| value as usize) };
        if config.max_patterns == 0 || config.max_patterns > 10_000 || config.max_nodes == 0 || config.max_nodes > 100_000 { return Err(invalid("invalid BPP1D generation budgets")); }
        Ok(config)
    }
}

/// 候选结果，完整性只针对当前默认策略的有界搜索。 / Candidate results, with completeness scoped to bounded search under the current default policy.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dPatterns {
    /// 候选。 / Candidates.
    pub patterns: Vec<Bpp1dPattern>,
    /// 是否穷尽当前候选空间。 / Whether the current candidate space was exhausted.
    pub complete: bool,
    /// 实际访问节点数。 / Actual visited nodes.
    pub visited_nodes: u32,
}

/// 委托原有领域搜索，明确保留截断状态。 / Delegate to the existing domain search, retaining explicit truncation state.
pub fn generate_bpp1d_patterns(context: &Bpp1dContext, options: &Bpp1dGenerationOptions) -> Result<Bpp1dPatterns> {
    let generated = context.generate_patterns(options.config()?, &domain::AcceptAllPatterns).map_err(invalid)?;
    Ok(Bpp1dPatterns { patterns: generated.patterns.iter().map(Bpp1dPattern::from_pattern).collect(), complete: generated.complete,
        visited_nodes: u32::try_from(generated.visited_nodes).map_err(invalid)? })
}

/// 单件坐标。 / Piece coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dPlacement {
    /// 物品身份。 / Item identity.
    pub item_id: String,
    /// 起点。 / Start.
    pub start: PhysicalQuantity,
    /// 长度。 / Length.
    pub length: PhysicalQuantity,
}

/// 单箱完整放置。 / Complete single-bin placements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dPlan {
    /// 箱实例身份。 / Bin instance identity.
    pub bin_id: String,
    /// 箱型身份。 / Bin-type identity.
    pub bin_type_id: String,
    /// 单件放置。 / Piece placements.
    pub placements: Vec<Bpp1dPlacement>,
}

/// 空闲区间。 / Free interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dFreeInterval {
    /// 箱实例身份。 / Bin instance identity.
    pub bin_id: String,
    /// 起点。 / Start.
    pub start: PhysicalQuantity,
    /// 长度。 / Length.
    pub length: PhysicalQuantity,
}

/// 原数值域 KPI 投影，精确计数仍为十进制文本。 / Native-value KPI projection with exact decimal-text counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dKpi {
    /// 箱数。 / Bin count.
    pub bin_count: String,
    /// 件数。 / Item count.
    pub item_count: String,
    /// 消耗长度。 / Consumed length.
    pub consumed_length: PhysicalQuantity,
    /// 尾余长度，不含内部间距。 / Tail remnant length, excluding internal spacing.
    pub remnant_length: PhysicalQuantity,
    /// 净长度利用率。 / Net-length utilization.
    pub utilization: f64,
    /// 实际总箱成本。 / Actual total bin cost.
    pub cost: f64,
}

/// 独立复验的物理装箱，不是求解器最优性证书。 / Independently verified physical packing, not a solver optimality certificate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dPacking {
    /// 全部箱方案。 / All bin plans.
    pub plans: Vec<Bpp1dPlan>,
    /// 包含内部间距的空闲区间。 / Free intervals including internal spacing.
    pub free_intervals: Vec<Bpp1dFreeInterval>,
    /// 精确物料总量文本。 / Exact material-total text.
    pub materials: BTreeMap<String, String>,
    /// 实际目标。 / Actual objective.
    pub objective: f64,
    /// 原域 KPI。 / Native-domain KPIs.
    pub kpi: Bpp1dKpi,
}

/// 归一并独立复核输入坐标，拒绝重叠、尺寸伪造与超额需求/库存。 / Normalize and independently verify coordinates, rejecting overlaps, forged sizes and excess demand/stock.
pub fn verify_bpp1d_plans(context: &Bpp1dContext, plans: &[Bpp1dPlan]) -> Result<domain::PackingResult<f64, Unit>> {
    if plans.len() > 100_000 || plans.iter().map(|plan| plan.placements.len()).sum::<usize>() > 100_000 { return Err(invalid("BPP1D plan expansion exceeds 100000 pieces")); }
    let unit = &context.problem().length_unit;
    let plans = plans.iter().map(|plan| {
        Ok(domain::PackingPlan { bin_id: plan.bin_id.clone(), bin_type_id: plan.bin_type_id.clone(), placements: plan.placements.iter().map(|placement| {
            Ok(domain::Placement { item_id: placement.item_id.clone(), start: placement.start.quantity()?.to_unit(unit).map_err(invalid)?,
                length: placement.length.quantity()?.to_unit(unit).map_err(invalid)? })
        }).collect::<Result<_>>()? })
    }).collect::<Result<_>>()?;
    context.verify_plans(plans).map_err(invalid)
}

/// 使用领域提取和 KPI，不在桥接层重写几何或目标算法。 / Use domain extraction and KPIs without rewriting geometry or objectives in the bridge.
pub fn encode_bpp1d_packing(context: &Bpp1dContext, result: &domain::PackingResult<f64, Unit>, unit: QuantityUnit) -> Result<Bpp1dPacking> {
    if context.problem().length_unit != unit.unit() { return Err(invalid("packing output unit differs from the normalized context")); }
    let result = context.verify_plans(result.plans.clone()).map_err(invalid)?;
    let kpi = context.kpi(&result).map_err(invalid)?;
    let quantity = |value| PhysicalQuantity { value, unit };
    Ok(Bpp1dPacking { plans: result.plans.iter().map(|plan| Bpp1dPlan { bin_id: plan.bin_id.clone(), bin_type_id: plan.bin_type_id.clone(),
        placements: plan.placements.iter().map(|placement| Bpp1dPlacement { item_id: placement.item_id.clone(), start: quantity(placement.start.value), length: quantity(placement.length.value) }).collect() }).collect(),
        free_intervals: result.free_intervals.iter().map(|interval| Bpp1dFreeInterval { bin_id: interval.bin_id.clone(), start: quantity(interval.start.value), length: quantity(interval.length.value) }).collect(),
        materials: result.materials.iter().map(|(id, value)| (id.clone(), value.to_string())).collect(), objective: result.objective,
        kpi: Bpp1dKpi { bin_count: kpi.bin_count.to_string(), item_count: kpi.item_count.to_string(), consumed_length: quantity(kpi.consumed_length.value),
            remnant_length: quantity(kpi.remnant_length.value), utilization: kpi.utilization, cost: kpi.cost } })
}

/// 渲染单件，只含展示数值。 / Rendered piece containing display numbers only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bpp1dRenderItem {
    /// 物品身份。 / Item identity.
    pub item_id: String,
    /// 展示单位起点。 / Start in display units.
    pub start: f64,
    /// 展示单位长度。 / Length in display units.
    pub length: f64,
}

/// 渲染箱。 / Rendered bin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bpp1dRenderBin {
    /// 箱实例身份。 / Bin instance identity.
    pub id: String,
    /// 箱型身份。 / Bin-type identity.
    pub bin_type_id: String,
    /// 展示单位箱长。 / Bin length in display units.
    pub length: f64,
    /// 实际放置。 / Actual placements.
    pub items: Vec<Bpp1dRenderItem>,
}

/// 一维渲染结果；展示单位下的数值，不是精度或最优性证据。
/// 1D render result; these are display-unit numbers and never precision or optimality evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bpp1dRenderSchema {
    /// 显式展示长度单位。 / Explicit display length unit.
    pub length_unit: QuantityUnit,
    /// 箱体。 / Bins.
    pub bins: Vec<Bpp1dRenderBin>,
    /// 展示精度下的净长度利用率。 / Net-length utilization at display precision.
    pub utilization: f64,
}

/// 用原域渲染适配器生成展示方案：先复核完整放置，再显式换单位。
/// Render through the original domain adapter: complete placements are revalidated before any unit conversion.
pub fn render_bpp1d_packing(context: &Bpp1dContext, plans: &[Bpp1dPlan], unit: QuantityUnit) -> Result<Bpp1dRenderSchema> {
    let result = verify_bpp1d_plans(context, plans)?;
    let target = unit.unit();
    let schema = ospf_rust_framework_bpp1d::infrastructure::PackingRendererAdapter::to_schema(context, &result, &target,
        &ospf_rust_framework_bpp::infrastructure::RuntimePackingValueAdapter, |value: &f64| Ok(*value)).map_err(invalid)?;
    Ok(Bpp1dRenderSchema {
        length_unit: unit,
        bins: schema.bins.into_iter().map(|bin| Bpp1dRenderBin { id: bin.id, bin_type_id: bin.bin_type_id, length: bin.length,
            items: bin.items.into_iter().map(|item| Bpp1dRenderItem { item_id: item.item_id, start: item.start, length: item.length }).collect() }).collect(),
        utilization: schema.utilization,
    })
}

/// 执行方式，列生成是既有有限池列激活而非任意定价回调。 / Execution mode; column generation is existing finite-pool activation, not arbitrary pricing callbacks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp1dAlgorithm {
    /// 普通候选 MILP。 / Ordinary candidate MILP.
    Milp,
    /// 有限池列激活加完整候选池 final。 / Finite-pool column activation followed by full-pool final.
    ColumnGeneration,
}

/// 宿主预注册的默认领域规则及有界配置。 / Host-registered default domain rules and bounded configuration.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dOptions {
    /// 显式后端。 / Explicit backend.
    pub backend: NativeBackend,
    /// 执行方式。 / Execution mode.
    pub algorithm: Bpp1dAlgorithm,
    /// 候选搜索选项。 / Candidate search options.
    #[ts(optional)]
    pub generation: Option<Bpp1dGenerationOptions>,
    /// 默认 32，最大 10000。 / Default 32, maximum 10000.
    #[ts(optional)]
    pub iteration_limit: Option<u32>,
    /// 默认 32，最大 10000。 / Default 32, maximum 10000.
    #[ts(optional)]
    pub columns_per_iteration: Option<u32>,
    /// 初始列与结果展开上限，默认/最大 100000。 / Seed/output expansion limit, default/maximum 100000.
    #[ts(optional)]
    pub expansion_limit: Option<u32>,
    /// 有限且位于 [0, 0.5)，默认 1e-7。 / Finite and in [0, 0.5), default 1e-7.
    #[ts(optional)]
    pub tolerance: Option<f64>,
    /// 尾箱优化窗口：只重排最后 N 个箱，默认不启用（1..=1000）。 / Tail window re-optimizing only the last N
    /// bins; disabled by default, 1..=1000.
    #[ts(optional)]
    pub tail_window: Option<u32>,
}

impl Bpp1dOptions {
    /// 复用原配置默认值并校验工作量。 / Reuse original configuration defaults and validate work bounds.
    pub fn config(&self) -> Result<Bpp1dSolveConfig> {
        let defaults = Bpp1dSolveConfig::default();
        let config = Bpp1dSolveConfig { generation: self.generation.clone().unwrap_or_default().config()?,
            iteration_limit: self.iteration_limit.map_or(defaults.iteration_limit, |value| value as usize),
            columns_per_iteration: self.columns_per_iteration.map_or(defaults.columns_per_iteration, |value| value as usize),
            expansion_limit: self.expansion_limit.map_or(defaults.expansion_limit, u64::from), tolerance: self.tolerance.unwrap_or(defaults.tolerance), ..defaults };
        if self.tail_window.is_some_and(|window| window == 0 || window > 1000) { return Err(invalid("BPP1D tail window must be in [1, 1000]")); }
        if config.iteration_limit == 0 || config.iteration_limit > 10_000 || config.columns_per_iteration == 0 || config.columns_per_iteration > 10_000
            || config.expansion_limit == 0 || config.expansion_limit > 100_000 || !config.tolerance.is_finite() || !(0.0..0.5).contains(&config.tolerance) { return Err(invalid("invalid BPP1D execution options")); }
        Ok(config)
    }
}

/// 原生领域结果的安全摘要，完整性与证明范围由既有 Rust 流程决定。 / Safe native-domain summary, with completeness/proof scope decided by the existing Rust workflow.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dSummary {
    /// 执行方式。 / Execution mode.
    pub algorithm: Bpp1dAlgorithm,
    /// 完备候选搜索。 / Complete candidate search.
    pub candidate_complete: bool,
    /// 实际候选列数，列生成模式包含独立 seeds。 / Actual candidate count, including independent seeds in column-generation mode.
    pub column_count: u32,
    /// 已验证 LP 轮数。 / Validated LP rounds.
    pub pricing_iterations: u32,
    /// 当前有限池定价收敛。 / Current finite-pool pricing converged.
    pub pricing_converged: bool,
    /// 原生默认规则流程判定的全局最优性，不是导入器独立重放证明。 / Global optimality classified by the native default-rule workflow, not independently replayed by the importer.
    pub globally_optimal: bool,
    /// 生效的尾箱优化窗口；未启用时为 null。 / Active tail window, or null when disabled.
    #[serde(default)]
    #[ts(optional)]
    pub tail_window: Option<u32>,
    /// 尾箱优化是否真的改动了最终排样。 / Whether tail optimization actually changed the final packing.
    #[serde(default)]
    pub tail_optimized: bool,
    /// 原生流程判定的全局不可行性。 / Global infeasibility classified by the native workflow.
    pub globally_infeasible: bool,
    /// 实际 core 问题状态。 / Actual core problem status.
    pub problem_status: String,
    /// 实际 core 终止原因。 / Actual core termination reason.
    pub termination_reason: String,
    /// 已验证物理结果的目标。 / Objective of the verified physical result.
    pub objective_value: Option<f64>,
}

/// 独立保留原始求解报告文本和物理结果。 / Independently retain original solver-report text and physical results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp1dReport {
    schema_version: String,
    fingerprint: String,
    /// 生成契约定义的摘要。 / Summary defined by generated contracts.
    pub summary: Bpp1dSummary,
    /// 原始候选模型报告，无损计数。 / Original candidate-model report with lossless counts.
    pub solver_report_json: String,
    /// 独立复核的物理结果。 / Independently verified physical result.
    pub packing: Option<Bpp1dPacking>,
}

/// 编码既有领域执行结果，不伪造源问题身份到候选模型报告。 / Encode existing domain results without relabeling candidate-model reports with source-problem identities.
pub fn encode_bpp1d_result(result: &Bpp1dSolution<Unit>, artifact: &ModelArtifact, algorithm: Bpp1dAlgorithm) -> Result<String> {
    encode_bpp1d_result_with_tail(result, artifact, algorithm, None, false)
}

/// 附带尾箱优化来源的编码：`tail_optimized` 为真时不再主张全局最优性。
/// Encode with tail-optimization provenance: when `tail_optimized` is true no global optimality is claimed.
pub fn encode_bpp1d_result_with_tail(result: &Bpp1dSolution<Unit>, artifact: &ModelArtifact, algorithm: Bpp1dAlgorithm,
    tail_window: Option<u32>, tail_optimized: bool) -> Result<String> {
    let (problem, context) = decode_bpp1d_problem(artifact)?;
    let packing = result.packing.as_ref().map(|packing| encode_bpp1d_packing(&context, packing, problem.length_unit)).transpose()?;
    let enum_text = |value| serde_json::to_value(value).map_err(invalid)?.as_str().map(str::to_owned).ok_or_else(|| invalid("expected a core enum string"));
    let report = Bpp1dReport { schema_version: "1.0".into(), fingerprint: artifact.fingerprint.clone(),
        summary: Bpp1dSummary { algorithm, candidate_complete: result.candidate_complete, column_count: u32::try_from(result.column_count).map_err(invalid)?,
            pricing_iterations: u32::try_from(result.pricing_iterations).map_err(invalid)?, pricing_converged: result.pricing_converged,
            tail_window, tail_optimized,
            // 尾箱优化改动了排样后，原全局最优性主张只覆盖首轮模型，必须收回。
            // Once tail optimization changed the packing, the original optimality claim only covered the first
            // model and must be withdrawn.
            globally_optimal: result.globally_optimal && !tail_optimized, globally_infeasible: result.globally_infeasible,
            problem_status: enum_text(result.report.problem_status)?, termination_reason: serde_json::to_value(result.report.termination_reason).map_err(invalid)?.as_str().ok_or_else(|| invalid("expected a termination string"))?.into(),
            objective_value: packing.as_ref().map(|packing| packing.objective) },
        solver_report_json: serde_json::to_string(&result.report).map_err(invalid)?, packing };
    let json = serde_json::to_string(&report).map_err(invalid)?;
    decode_bpp1d_report(&json, artifact)?;
    Ok(json)
}

/// 复核 schema、问题身份、完整物理方案/KPI 和 core 契约；不重放候选完整性证明。
/// Recheck schema, problem identity, complete physical plans/KPIs and core contracts, without replaying candidate-completeness proofs.
pub fn decode_bpp1d_report(json: &str, artifact: &ModelArtifact) -> Result<Bpp1dReport> {
    if json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "BPP1D report exceeds 8 MiB")); }
    let report: Bpp1dReport = serde_json::from_str(json).map_err(invalid)?;
    if report.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP1D report schema")); }
    let (problem, context) = decode_bpp1d_problem(artifact)?;
    if report.fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP1D report belongs to another problem")); }
    let solver: SolveReport<f64, u64> = serde_json::from_str(&report.solver_report_json).map_err(invalid)?;
    if solver.schema_version != CURRENT_SOLVE_REPORT_SCHEMA_VERSION { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported solver report schema")); }
    solver.validate().map_err(invalid)?;
    let summary = &report.summary;
    if summary.column_count > 210_000 || summary.pricing_iterations > 10_000 || (summary.pricing_converged && summary.pricing_iterations == 0)
        || (matches!(summary.algorithm, Bpp1dAlgorithm::Milp) && (summary.pricing_iterations != 0 || summary.pricing_converged))
        || serde_json::to_value(solver.problem_status).map_err(invalid)?.as_str() != Some(&summary.problem_status)
        || serde_json::to_value(solver.termination_reason).map_err(invalid)?.as_str() != Some(&summary.termination_reason)
        || solver.has_incumbent() != report.packing.is_some() || summary.objective_value != report.packing.as_ref().map(|packing| packing.objective)
        || (summary.globally_optimal && (!summary.candidate_complete || !solver.is_optimal() || report.packing.is_none()))
        || (summary.globally_infeasible && (!summary.candidate_complete || solver.problem_status != ProblemStatus::Infeasible || report.packing.is_some()
            || !solver.proof.as_ref().is_some_and(|proof| proof.kind == ProofKind::Infeasibility && proof.status == ProofStatus::Verified))) {
        return Err(invalid("inconsistent BPP1D result metadata"));
    }
    if let Some(packing) = &report.packing {
        let objective = solver.solution.as_ref().and_then(|solution| solution.objective_value.or(solution.objective));
        if !objective.is_some_and(|value| (value - packing.objective).abs() <= 1e-7 * value.abs().max(packing.objective.abs()).max(1.0)) {
            return Err(invalid("BPP1D solver objective differs from the physical packing"));
        }
        let actual = verify_bpp1d_plans(&context, &packing.plans)?;
        let verified = encode_bpp1d_packing(&context, &actual, problem.length_unit)?;
        if &verified != packing { return Err(invalid("BPP1D physical result, material totals or KPIs were altered")); }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem() -> Bpp1dProblem {
        serde_json::from_value(serde_json::json!({
            "items": [{"id":"a","length":{"value":40,"unit":"mm"},"weight":{"value":100,"unit":"g"},"amount":"2","materials":{"steel":"3"}},
                {"id":"b","length":{"value":0.02,"unit":"m"},"weight":{"value":0.1,"unit":"kg"},"amount":"1","materials":{"steel":"1"}}],
            "binTypes":[{"id":"bin","length":{"value":10,"unit":"cm"},"maxWeight":{"value":1,"unit":"kg"},"cost":2,"inventory":"2"}],
            "spacing":{"value":0,"unit":"mm"},"lengthUnit":"cm","weightUnit":"kg","objective":"binCount"
        })).unwrap()
    }

    fn usages() -> Vec<Bpp1dUsage> {
        vec![Bpp1dUsage { pattern: Bpp1dPattern { bin_type_id: "bin".into(), amounts: BTreeMap::from([("a".into(), "2".into()), ("b".into(), "1".into())]) }, count: "1".into() }]
    }

    #[test]
    fn domain_normalizes_units_and_recomputes_physics_and_kpis() {
        let problem = problem();
        let artifact = encode_bpp1d_problem(problem).unwrap();
        let (problem, context) = decode_bpp1d_problem(&artifact).unwrap();
        let result = context.pack(&bpp1d_usages(&usages()).unwrap(), 3).unwrap();
        let packing = encode_bpp1d_packing(&context, &result, problem.length_unit).unwrap();
        assert_eq!(packing.kpi.bin_count, "1");
        assert_eq!(packing.kpi.item_count, "3");
        assert_eq!(packing.materials["steel"], "7");
        assert_eq!(packing.kpi.utilization, 1.0);
        assert_eq!(packing.kpi.cost, 2.0);
        assert_eq!(packing.kpi.remnant_length.value, 0.0);
        assert!(encode_bpp1d_packing(&context, &result, QuantityUnit::Meter).is_err());
        let mut plans = packing.plans.clone();
        plans[0].placements[0].length.value = 3.0;
        assert!(verify_bpp1d_plans(&context, &plans).is_err());
        plans = packing.plans.clone();
        plans[0].placements[1].start.value = 0.0;
        assert!(verify_bpp1d_plans(&context, &plans).is_err());
        plans = packing.plans.clone();
        plans[0].placements[0].length.unit = QuantityUnit::Second;
        assert!(verify_bpp1d_plans(&context, &plans).is_err());
    }

    #[test]
    fn exact_counts_do_not_pass_through_f64_and_reject_noncanonical_or_duplicate_inputs() {
        let mut problem = problem();
        problem.items.truncate(1);
        problem.items[0].amount = u64::MAX.to_string();
        problem.items[0].materials.clear();
        problem.bin_types[0].inventory = None;
        let context = problem.context().unwrap();
        let usage = Bpp1dUsage { pattern: Bpp1dPattern { bin_type_id: "bin".into(), amounts: BTreeMap::from([("a".into(), "1".into())]) }, count: u64::MAX.to_string() };
        context.verify_usages(&bpp1d_usages(&[usage.clone()]).unwrap()).unwrap();
        assert!(context.pack(&bpp1d_usages(&[usage.clone()]).unwrap(), 100_000).is_err());
        assert!(bpp1d_usages(&[usage.clone(), usage]).is_err());
        for count in ["01", "+1", "-1", "18446744073709551616", "1.0"] {
            problem.items[0].amount = count.into();
            assert!(problem.context().is_err(), "{count}");
        }
    }

    #[test]
    fn budgets_units_spacing_and_fingerprints_are_enforced() {
        let mut problem = problem();
        let context = problem.context().unwrap();
        let full = generate_bpp1d_patterns(&context, &Bpp1dGenerationOptions::default()).unwrap();
        assert!(full.complete);
        assert!(!generate_bpp1d_patterns(&context, &Bpp1dGenerationOptions { max_patterns: Some(1), max_nodes: Some(1) }).unwrap().complete);
        assert!(Bpp1dGenerationOptions { max_nodes: Some(100_001), ..Default::default() }.config().is_err());
        problem.spacing.value = 10.0;
        assert!(problem.context().unwrap().validate_pattern(&usages()[0].pattern.pattern().unwrap()).is_err());
        problem.weight_unit = QuantityUnit::Second;
        assert!(problem.context().is_err());
        let mut artifact = encode_bpp1d_problem(self::problem()).unwrap();
        artifact.fingerprint.push('0');
        assert_eq!(decode_bpp1d_problem(&artifact).err().unwrap().code, "FingerprintMismatch");
    }
}
