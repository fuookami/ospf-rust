//! BPP2D 的源目录与无损物理排布协议，模型注册委托原领域。 / BPP2D source catalog and lossless physical-layout protocol, delegating model registration to the original domain.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::{CTUnit, derived::{Meter, SquareMeter}};
use ospf_rust_core::model::MetaModel;
use ospf_rust_framework_bpp2d::{application::Bpp2dColumnGenerationInput, domain::{item::model::{ActualItem, ItemId, Layer, SheetType, SheetTypeId},
    layer_assignment::{Bpp2dAssignmentDemandEntry, Bpp2dAssignmentDemandValue, LayerAssignmentContext, LayerAssignmentModelInput, LayerAssignmentObjectiveConfig, LayerAssignmentStage},
    layer_generation::{Bpp2dLayerGenerationRequest, LayerGenerationContext}}};
use crate::{BridgeError, Geometry2d, Geometry2dPlacement, Geometry2dShape, Geometry2dSheet, ModelArtifact, PhysicalQuantity, QuantityUnit, Result, invalid};

pub(crate) const BPP2D_MAX_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const BPP2D_MAX_POOL: usize = 512;
pub(crate) const BPP2D_MAX_PLACEMENTS: usize = 4096;
pub(crate) const BPP2D_TOLERANCE: f64 = 1e-7;

pub(crate) fn count(value: &str) -> Result<u64> {
    let number = value.parse::<u64>().map_err(invalid)?;
    if number.to_string() != value || number > 9_007_199_254_740_991 { return Err(invalid("BPP2D count must be canonical and within 0..=2^53-1")); }
    Ok(number)
}

fn identifier(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 256 { return Err(invalid("BPP2D identifiers must contain 1..256 UTF-8 bytes")); }
    Ok(())
}

fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for value in values { identifier(value)?; if !seen.insert(value) { return Err(invalid("duplicate BPP2D identifier")); } }
    Ok(())
}

fn area(value: &PhysicalQuantity) -> Result<f64> {
    let converted = value.quantity()?.to_unit(&SquareMeter::INSTANT).map_err(invalid)?.value;
    if !converted.is_finite() || converted < 0.0 || (value.value != 0.0 && converted == 0.0) { return Err(invalid("BPP2D requires finite nonnegative square area without underflow")); }
    Ok(converted)
}

/// 米制求解边界前的带单位源物品，不接受未绑定的重量／物料规则。 / Unit-bearing source item before the meter-based solver boundary, rejecting unbound mass/material rules.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dItem {
    /// 稳定物品身份。 / Stable item identity.
    pub id: String,
    /// 显示名，最多 256 UTF-8 字节。 / Display name, at most 256 UTF-8 bytes.
    pub name: String,
    /// 原形状，旋转尚未应用。 / Original shape before rotation.
    pub shape: Geometry2dShape,
    /// 原领域旋转许可。 / Original-domain rotation permission.
    pub allow_rotate: bool,
    /// 生成器输入件数，范围 0..4096，不是额外库存或需求上界。 / Generator input count in 0..4096, not an additional inventory or demand upper bound.
    pub generation_count: String,
}

/// 带单位源板材，尚不支持原类型中量纲不明确的 max_load。 / Unit-bearing source sheet, without the original type's dimensionally ambiguous max_load.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dSheetType {
    /// 稳定板材身份。 / Stable sheet identity.
    pub id: String,
    /// 原点起算的板材尺寸。 / Sheet extents anchored at the origin.
    pub geometry: Geometry2dSheet,
    /// 原领域主板材标记。 / Original-domain primary-sheet flag.
    pub is_main: bool,
}

/// 严格区分件数及面积的需求，不将两种口径混算。 / Demand strictly distinguishing count and area without mixing the two bases.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Bpp2dDemand {
    /// 精确件数。 / Exact item count.
    Count {
        /// 源物品身份。 / Source item identity.
        item_id: String,
        /// canonical 非负十进制文本。 / Canonical nonnegative decimal text.
        count: String,
        /// 可选硬上界，不是短缺松弛。 / Optional hard upper bound, not deficit slack.
        upper_bound: Option<String>,
    },
    /// 带面积单位的覆盖量。 / Coverage carrying an area unit.
    Area {
        /// 源物品身份。 / Source item identity.
        item_id: String,
        /// 有限非负面积。 / Finite nonnegative area.
        area: PhysicalQuantity,
        /// 可选同量纲硬上界。 / Optional dimension-compatible hard upper bound.
        upper_bound: Option<PhysicalQuantity>,
    },
}

/// 仅保存物理排布，来源及迭代不是数学身份或来源认证。 / Physical layout only; provenance and iteration are not mathematical identity or authentication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dLayer {
    /// 源板材身份。 / Source sheet identity.
    pub sheet_id: String,
    /// 层包围尺寸，不能越出板材或缩小到遗漏放置。 / Layer envelope, never exceeding its sheet or omitting placements.
    pub envelope: Geometry2dSheet,
    /// 实际朝向的完整放置，不隐式再旋转。 / Complete physically oriented placements, without implicit re-rotation.
    pub placements: Vec<Geometry2dPlacement>,
}

/// 已开放默认规则的目标配置；RMP 定价目标和 final 业务目标明确分离。 / Objective configuration for exposed default rules, explicitly separating RMP pricing and final business objectives.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dObjective {
    /// final 整板面积权重。 / Final whole-sheet-area weight.
    pub sheet_area_weight: f64,
    /// final 板数权重。 / Final sheet-count weight.
    pub sheet_amount_weight: f64,
    /// final 类型成本权重。 / Final type-cost weight.
    pub sheet_type_cost_weight: f64,
    /// 仅引用源板材 ID 的成本表。 / Costs referencing source sheet IDs only.
    pub sheet_type_costs: BTreeMap<String, f64>,
    /// RMP 每平方米成本，不含 final 次目标。 / RMP cost per square meter, excluding final secondary objectives.
    pub rmp_area_coefficient: f64,
}

impl Default for Bpp2dObjective {
    fn default() -> Self {
        Self { sheet_area_weight: 1.0, sheet_amount_weight: 1e-6, sheet_type_cost_weight: 0.0, sheet_type_costs: BTreeMap::new(), rmp_area_coefficient: 1.0 }
    }
}

/// 有界 BPP2D 源问题；原域采用 f64/Meter，精确文本不代表任意 u64 可求解。 / Bounded BPP2D source problem; the original domain uses f64/Meter, and exact text does not imply arbitrary-u64 solving.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dProblem {
    /// 最多 64 个唯一物品。 / At most 64 unique items.
    pub items: Vec<Bpp2dItem>,
    /// 最多 16 种唯一板材，保留输入顺序。 / At most 16 unique sheets, retaining input order.
    pub sheets: Vec<Bpp2dSheetType>,
    /// 最多 128 条不同口径／物品需求。 / At most 128 distinct basis/item demands.
    pub demands: Vec<Bpp2dDemand>,
    /// 最多 256 个初始列，保留全部位置，允许物理重复。 / At most 256 initial columns, retaining all positions and allowing physical duplicates.
    pub initial_layers: Vec<Bpp2dLayer>,
    /// 非负带长度单位刀缝，按原几何守卫校验。 / Nonnegative length-bearing kerf, checked by the original geometry guard.
    pub kerf: PhysicalQuantity,
    /// 原领域默认目标配置。 / Original-domain default objective configuration.
    pub objective: Bpp2dObjective,
}

impl Bpp2dProblem {
    /// 复用几何量纲 adapter 和领域注册路径；不运行生成器、求解器或宿主时钟。
    /// Reuse geometry dimension adapters and domain registration, without invoking generators, solvers or host clocks.
    pub fn domain(&self) -> Result<Bpp2dColumnGenerationInput> {
        if self.items.is_empty() || self.items.len() > 64 || self.sheets.is_empty() || self.sheets.len() > 16
            || self.demands.is_empty() || self.demands.len() > 128 || self.initial_layers.len() > 256 {
            return Err(invalid("invalid BPP2D source shape or size"));
        }
        unique(self.items.iter().map(|item| item.id.as_str()))?;
        unique(self.sheets.iter().map(|sheet| sheet.id.as_str()))?;
        let geometry = Geometry2d::new(QuantityUnit::Meter)?;
        let items = self.items.iter().map(|item| {
            if item.name.len() > 256 { return Err(invalid("BPP2D item name exceeds 256 UTF-8 bytes")); }
            let amount = count(&item.generation_count)?;
            if amount > BPP2D_MAX_PLACEMENTS as u64 { return Err(invalid("BPP2D generation count exceeds 4096")); }
            Ok((ActualItem::new(ItemId::new(&item.id), &item.name, geometry.read_shape(&item.shape)?, item.allow_rotate, Quantity::<f64, Meter>::new_ct(0.0)),
                amount as usize))
        }).collect::<Result<Vec<_>>>()?;
        let sheets = self.sheets.iter().map(|sheet| Ok(SheetType::new(geometry.length(&sheet.geometry.width, true)?, geometry.length(&sheet.geometry.height, true)?,
            Quantity::<f64, Meter>::new_ct(0.0), SheetTypeId::new(&sheet.id)).with_main(sheet.is_main))).collect::<Result<Vec<_>>>()?;
        for sheet in &sheets {
            for (item, _) in &items {
                let bound = sheet.area().value / item.shape.bounding_area().value;
                if !bound.is_finite() || bound > BPP2D_MAX_PLACEMENTS as f64 { return Err(invalid("BPP2D geometric generation bound exceeds 4096 placements per sheet")); }
            }
        }
        let demands = self.demands.iter().map(|demand| Ok(match demand {
            Bpp2dDemand::Count { item_id, count: amount, upper_bound } => {
                let mut demand = Bpp2dAssignmentDemandEntry::item_amount(item_id, count(amount)?);
                if let Some(upper) = upper_bound { demand = demand.with_upper_bound(Bpp2dAssignmentDemandValue::Amount(count(upper)?)); }
                demand
            },
            Bpp2dDemand::Area { item_id, area: amount, upper_bound } => {
                let mut demand = Bpp2dAssignmentDemandEntry::item_area(item_id, area(amount)?);
                if let Some(upper) = upper_bound { demand = demand.with_upper_bound(Bpp2dAssignmentDemandValue::Area(area(upper)?)); }
                demand
            },
        })).collect::<Result<Vec<_>>>()?;
        let kerf = geometry.length(&self.kerf, false)?;
        if kerf.value < 0.0 { return Err(invalid("negative BPP2D kerf")); }
        let request = Bpp2dLayerGenerationRequest::new(0, items).with_kerf(kerf);
        let mut request = Bpp2dLayerGenerationRequest { sheets, ..request };
        request.existing_layers = decode_bpp2d_layers(&request, &demands, &self.initial_layers)?;
        let objective = &self.objective;
        if objective.sheet_type_costs.keys().any(|id| !self.sheets.iter().any(|sheet| &sheet.id == id)) { return Err(invalid("BPP2D cost references a foreign sheet")); }
        let objective = LayerAssignmentObjectiveConfig { sheet_area_weight: objective.sheet_area_weight, sheet_amount_weight: objective.sheet_amount_weight,
            sheet_type_cost_weight: objective.sheet_type_cost_weight, rmp_area_coefficient: objective.rmp_area_coefficient,
            sheet_type_costs: objective.sheet_type_costs.iter().map(|(id, cost)| (SheetTypeId::new(id), *cost)).collect() };
        LayerAssignmentContext::new(objective.clone()).register_model(&mut MetaModel::new("bpp2d-source-validation"), LayerAssignmentModelInput {
            layers: &request.existing_layers, demands: &demands, stage: LayerAssignmentStage::FeasibilityLp,
        }).map_err(invalid)?;
        Ok(Bpp2dColumnGenerationInput { request, demands, objective, config: Default::default() })
    }
}

/// 将完整物理排布绑定到已有源目录，不信任单独传入的物品尺寸或板材定义。
/// Bind complete layouts to an existing source catalog, without trusting independently supplied item sizes or sheet definitions.
pub fn decode_bpp2d_layers(
    request: &Bpp2dLayerGenerationRequest<f64, Meter>,
    demands: &[Bpp2dAssignmentDemandEntry],
    layers: &[Bpp2dLayer],
) -> Result<Vec<Layer<f64, Meter>>> {
    if layers.len() > BPP2D_MAX_POOL || layers.iter().map(|layer| layer.placements.len()).sum::<usize>() > BPP2D_MAX_PLACEMENTS {
        return Err(invalid("BPP2D pool exceeds layer or total-placement limits"));
    }
    let geometry = Geometry2d::new(QuantityUnit::Meter)?;
    let output = layers.iter().map(|layer| {
        let sheet = request.sheets.iter().chain(request.sheet.iter()).find(|sheet| sheet.type_code.as_str() == layer.sheet_id)
            .ok_or_else(|| invalid("BPP2D layer references a foreign source sheet"))?;
        Ok(Layer::new(0, "bridge", Some(sheet.clone()), ospf_rust_framework_geometry2d::Sheet2Shape::new(
            geometry.length(&layer.envelope.width, true)?, geometry.length(&layer.envelope.height, true)?,
        )).with_units(layer.placements.iter().map(|placement| geometry.read_placement(placement)).collect::<Result<Vec<_>>>()?))
    }).collect::<Result<Vec<_>>>()?;
    LayerGenerationContext::new().validate_columns(request, demands, &output).map_err(invalid)?;
    Ok(output)
}

/// 编码原领域物理排布为米制 DTO；业务来源元数据不作为认证证据。
/// Encode an original-domain physical layout as a meter-based DTO; business provenance metadata is not authentication evidence.
pub fn encode_bpp2d_layer(layer: &Layer<f64, Meter>) -> Result<Bpp2dLayer> {
    let sheet = layer.sheet.as_ref().ok_or_else(|| invalid("BPP2D layer lacks a sheet"))?;
    let geometry = Geometry2d::new(QuantityUnit::Meter)?;
    Ok(Bpp2dLayer { sheet_id: sheet.type_code.as_str().into(), envelope: Geometry2dSheet {
        width: PhysicalQuantity { value: layer.shape.width.value, unit: QuantityUnit::Meter }, height: PhysicalQuantity { value: layer.shape.height.value, unit: QuantityUnit::Meter },
    }, placements: layer.units.iter().cloned().map(|placement| geometry.write_placement(placement)).collect::<Result<Vec<_>>>()? })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProblemEnvelope { schema_version: String, problem: Bpp2dProblem }

/// 编码已验证源问题；整数保持文本，摘要使用独立领域标签。
/// Encode a validated source problem, retaining integer text and a domain-separated digest.
pub fn encode_bpp2d_problem(problem: Bpp2dProblem) -> Result<ModelArtifact> {
    problem.domain()?;
    let model_json = serde_json::to_string(&ProblemEnvelope { schema_version: "1.0".into(), problem }).map_err(invalid)?;
    if model_json.len() > BPP2D_MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP2D source exceeds 8 MiB")); }
    let fingerprint = ospf_rust_core::solver::sha256_fingerprint("ospf.bpp2d.problem.v1", model_json.as_bytes()).value;
    Ok(ModelArtifact { model_json, fingerprint })
}

/// 复验协议主版本、canonical 原文、指纹及全部源规则。
/// Revalidate protocol major version, canonical original text, fingerprint and all source rules.
pub fn decode_bpp2d_problem(artifact: &ModelArtifact) -> Result<(Bpp2dProblem, Bpp2dColumnGenerationInput)> {
    if artifact.model_json.len() > BPP2D_MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP2D source exceeds 8 MiB")); }
    let envelope: ProblemEnvelope = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP2D source schema")); }
    let encoded = encode_bpp2d_problem(envelope.problem.clone())?;
    if encoded.model_json != artifact.model_json || encoded.fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP2D canonical source or fingerprint mismatch")); }
    let domain = envelope.problem.domain()?;
    Ok((envelope.problem, domain))
}

#[cfg(test)]
pub(crate) mod tests;
