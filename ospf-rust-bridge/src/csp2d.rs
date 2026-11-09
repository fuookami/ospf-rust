//! CSP2D 源目录、平坦切分树及无损问题 codec，数学与物理校验委托原领域。 / CSP2D source catalog, flat cut trees and lossless problem codec, delegating mathematical/physical validation to the original domain.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::unit::{CTUnit, derived::{Meter, SquareMeter}};
use ospf_rust_framework_csp2d::application::Csp2dColumnGenerationInput;
use ospf_rust_framework_csp2d::domain::material::{CuttingPlan2, DemandValue2, Material2, MaterialId2, Pattern2, PatternId2, PlanId2, Product2, ProductId2};
use ospf_rust_framework_csp2d::domain::pattern_generation::PatternGenerationRequest2;
use ospf_rust_framework_csp2d::domain::produce::{Csp2dMasterDemand, ProduceObjective};
use ospf_rust_framework_csp2d::infrastructure::{CuttingProcess2, Region2};
use crate::{BridgeError, Geometry2d, Geometry2dPlacement, Geometry2dPoint, Geometry2dSheet, ModelArtifact, PhysicalQuantity, QuantityUnit, Result, invalid};

mod tree;
pub use tree::{Csp2dCutNode, Csp2dCutOrientation, Csp2dCutTree};

pub(crate) const CSP2D_MAX_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const CSP2D_MAX_POOL: usize = 512;
pub(crate) const CSP2D_MAX_PLACEMENTS: usize = 4096;
pub(crate) const CSP2D_TOLERANCE: f64 = 1e-6;
const CSP2D_MAX_TREE_NODES: usize = 65536;

fn identifier(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 256 { return Err(invalid("CSP2D identifiers must contain 1..256 UTF-8 bytes")); }
    Ok(())
}

fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for value in values { identifier(value)?; if !seen.insert(value) { return Err(invalid("duplicate CSP2D identifier")); } }
    Ok(())
}

pub(crate) fn exact_count_text(value: &str) -> Result<u64> {
    let number = value.parse::<u64>().map_err(invalid)?;
    if number.to_string() != value || number > 9_007_199_254_740_991 { return Err(invalid("CSP2D count must be canonical decimal text within 0..=2^53-1")); }
    Ok(number)
}

fn area(value: &PhysicalQuantity) -> Result<f64> {
    let converted = value.quantity()?.to_unit(&SquareMeter::INSTANT).map_err(invalid)?.value;
    if !converted.is_finite() || converted < 0.0 || (value.value != 0.0 && converted == 0.0) { return Err(invalid("CSP2D requires finite nonnegative square area without underflow")); }
    Ok(converted)
}

/// 原领域矩形产品目录，不声明未绑定的重量规则。 / Original-domain rectangular product catalog, without claiming unbound weight rules.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dProduct {
    /// 稳定产品身份。 / Stable product identity.
    pub id: String,
    /// 显示名称，最多 256 UTF-8 字节。 / Display name, at most 256 UTF-8 bytes.
    pub name: String,
    /// 原方向正尺寸。 / Positive original-orientation extents.
    pub geometry: Geometry2dSheet,
    /// 是否允许旋转九十度。 / Whether ninety-degree rotation is allowed.
    pub allow_rotate: bool,
}

/// 原领域工艺级数随刀向变化，不等于树深度。 / Original-domain process stages follow orientation changes, not tree depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp2dCuttingProcess {
    /// 任意排样。 / Freeform nesting.
    Freeform,
    /// 两级断头台。 / Two-stage guillotine.
    GuillotineTwoStage,
    /// 三级断头台。 / Three-stage guillotine.
    GuillotineThreeStage,
}

impl Csp2dCuttingProcess {
    fn domain(self) -> CuttingProcess2 {
        match self { Self::Freeform => CuttingProcess2::Freeform, Self::GuillotineTwoStage => CuttingProcess2::GuillotineTwoStage, Self::GuillotineThreeStage => CuttingProcess2::GuillotineThreeStage }
    }
}

/// 互斥的定尺板／卷材几何，不允许同时声明板高和卷长。 / Mutually exclusive sheet/coil geometry, never declaring both sheet height and coil length.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Csp2dMaterialShape {
    /// 定尺板材。 / Fixed-size sheet.
    Sheet {
        /// 完整板材尺寸；图案仍可使用部分 region。 / Complete sheet extents; a pattern may still consume a partial region.
        geometry: Geometry2dSheet,
    },
    /// 定宽卷材。 / Fixed-width coil.
    Coil {
        /// 正幅宽。 / Positive width.
        width: PhysicalQuantity,
        /// 可选正卷长，限制单次段高，不表示多段共享卷长优化。 / Optional positive length bounding one segment, not shared-roll-length optimization.
        length: Option<PhysicalQuantity>,
    },
}

/// 每物料的生成提示，不是模型需求或份数上界。 / Per-material generation hint, not a model demand or usage upper bound.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dGenerationProduct {
    /// 源产品身份。 / Source product identity.
    pub product_id: String,
    /// canonical 正件数文本，每物料全部提示合计最多 4096。 / Canonical positive count text, totaling at most 4096 across one material's hints.
    pub count: String,
}

/// 显式原生源物料，不接受尚未绑定的质量／设备／副产物规则。 / Explicit native source material, excluding unbound mass/machine/by-product rules.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dMaterial {
    /// 稳定物料身份。 / Stable material identity.
    pub id: String,
    /// 显示名，最多 256 UTF-8 字节。 / Display name, at most 256 UTF-8 bytes.
    pub name: String,
    /// 定尺或卷材形态。 / Sheet or coil shape.
    pub shape: Csp2dMaterialShape,
    /// canonical 批次文本；每次图案使用消耗一个批次。 / Canonical batch count text; each pattern use consumes one batch.
    pub available_batches: String,
    /// 裁切工艺。 / Cutting process.
    pub process: Csp2dCuttingProcess,
    /// 仅卷材可配置的正段高限制。 / Positive segment-height cap configurable only for a coil.
    pub max_segment_height: Option<PhysicalQuantity>,
    /// 此物料允许的唯一产品及生成件数，不能为空。 / Unique products allowed for this material and their generation counts, never empty.
    pub products: Vec<Csp2dGenerationProduct>,
}

/// 严格独立的件数／面积需求。 / Strictly distinct count/area demand bases.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Csp2dDemand {
    /// 精确件数。 / Exact piece count.
    Count {
        /// 源产品身份。 / Source product identity.
        product_id: String,
        /// canonical 非负件数文本。 / Canonical nonnegative count text.
        count: String,
        /// 可选硬件数上界。 / Optional hard count upper bound.
        upper_bound: Option<String>,
    },
    /// 有限非负面积。 / Finite nonnegative area.
    Area {
        /// 源产品身份。 / Source product identity.
        product_id: String,
        /// 带单位面积下界。 / Unit-bearing area lower bound.
        area: PhysicalQuantity,
        /// 可选同量纲硬上界。 / Optional dimension-compatible hard upper bound.
        upper_bound: Option<PhysicalQuantity>,
    },
}

/// 任意原点的正矩形料域。 / Positive rectangular material region with an arbitrary origin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dRegion {
    /// 左下角。 / Lower-left corner.
    pub origin: Geometry2dPoint,
    /// 正宽高。 / Positive extents.
    pub size: Geometry2dSheet,
}

impl Csp2dRegion {
    fn from_domain(region: &Region2<f64, Meter>) -> Self {
        let length = |value| PhysicalQuantity { value, unit: QuantityUnit::Meter };
        Self { origin: Geometry2dPoint { x: length(region.x.value), y: length(region.y.value) },
            size: Geometry2dSheet { width: length(region.width.value), height: length(region.height.value) } }
    }

    fn domain(&self, geometry: &Geometry2d) -> Result<Region2<f64, Meter>> {
        Ok(Region2::new(geometry.length(&self.origin.x, false)?, geometry.length(&self.origin.y, false)?,
            geometry.length(&self.size.width, true)?, geometry.length(&self.size.height, true)?))
    }
}

/// 带完整物理排布及可选切分树的列，不携带伪造物料或产品定义。 / Column with complete physical layout/optional cut tree, without independently forgeable material/product definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dColumn {
    /// 方案身份，不用于物理去重。 / Plan identity, not used for physical deduplication.
    pub id: String,
    /// 源物料身份，工艺沿用源物料。 / Source material identity, also determining the process.
    pub material_id: String,
    /// 实际消费料域，不自动扩大为整板。 / Actual consumed region, not implicitly expanded to the whole sheet.
    pub region: Csp2dRegion,
    /// 已体现实际朝向的矩形放置，保留顺序。 / Physically oriented rectangular placements, retaining order.
    pub placements: Vec<Geometry2dPlacement>,
    /// 有界平坦后序树；断头台工艺必须提供。 / Bounded flat postorder tree, required for guillotine processes.
    pub cut_tree: Option<Csp2dCutTree>,
}

/// 米／平方米归一后的原领域目标；Phase I 仍只优化短缺。 / Original-domain objective after meter/square-meter normalization; Phase I still optimizes deficits only.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dObjective {
    /// 料域消耗面积权重，默认 1。 / Consumed-region area weight, default 1.
    pub area_weight: f64,
    /// 原余料面积权重，默认 0。 / Original waste-area weight, default 0.
    pub waste_weight: f64,
    /// 卷材段高权重，默认 0。 / Coil segment-height weight, default 0.
    pub height_weight: f64,
    /// 每次使用权重，默认 0。 / Per-use weight, default 0.
    pub usage_weight: f64,
}

impl Default for Csp2dObjective {
    fn default() -> Self { Self { area_weight: 1.0, waste_weight: 0.0, height_weight: 0.0, usage_weight: 0.0 } }
}

/// 有界源问题，整数使用文本；底层求解边界仍是 f64/Meter。 / Bounded source problem with textual integers; the underlying solver boundary remains f64/Meter.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dProblem {
    /// 最多 64 个唯一产品。 / At most 64 unique products.
    pub products: Vec<Csp2dProduct>,
    /// 最多 16 个唯一物料。 / At most 16 unique materials.
    pub materials: Vec<Csp2dMaterial>,
    /// 最多 128 项独立产品／口径需求。 / At most 128 distinct product/basis demands.
    pub demands: Vec<Csp2dDemand>,
    /// 最多 256 个初始列，保留重复位置。 / At most 256 initial columns, retaining duplicate positions.
    pub initial_columns: Vec<Csp2dColumn>,
    /// 全部物料共用的非负长度刀缝。 / Nonnegative length-bearing kerf shared by every material.
    pub kerf: PhysicalQuantity,
    /// 原领域目标权重。 / Original-domain objective weights.
    pub objective: Csp2dObjective,
}

impl Csp2dProblem {
    /// 通过原领域注册及几何门禁验证，不启动生成器、求解器或宿主时钟。 / Validate through original-domain registration/geometry gates, without generators, solvers or host clocks.
    pub fn domain(&self) -> Result<Csp2dColumnGenerationInput> {
        if self.products.is_empty() || self.products.len() > 64 || self.materials.is_empty() || self.materials.len() > 16
            || self.demands.is_empty() || self.demands.len() > 128 || self.initial_columns.len() > 256 {
            return Err(invalid("invalid CSP2D source shape or size"));
        }
        unique(self.products.iter().map(|product| product.id.as_str()))?;
        unique(self.materials.iter().map(|material| material.id.as_str()))?;
        let geometry = Geometry2d::new(QuantityUnit::Meter)?;
        let products = self.products.iter().map(|product| {
            if product.name.len() > 256 { return Err(invalid("CSP2D product name exceeds 256 UTF-8 bytes")); }
            let value = Product2::new(ProductId2::new(&product.id), &product.name,
                geometry.length(&product.geometry.width, true)?, geometry.length(&product.geometry.height, true)?).with_allow_rotate(product.allow_rotate);
            if !value.area().value.is_finite() || value.area().value <= 0.0 { return Err(invalid("CSP2D source product area overflows or underflows")); }
            Ok((product.id.as_str(), value))
        }).collect::<Result<BTreeMap<_, _>>>()?;
        let kerf = geometry.length(&self.kerf, false)?;
        let requests = self.materials.iter().map(|source| {
            if source.name.len() > 256 || source.products.is_empty() || source.products.len() > 64 { return Err(invalid("invalid CSP2D material name or product hints")); }
            unique(source.products.iter().map(|product| product.product_id.as_str()))?;
            let batches = exact_count_text(&source.available_batches)?;
            let material = match &source.shape {
                Csp2dMaterialShape::Sheet { geometry: size } => Material2::sheet(MaterialId2::new(&source.id), &source.name,
                    geometry.length(&size.width, true)?, geometry.length(&size.height, true)?, batches),
                Csp2dMaterialShape::Coil { width, length } => {
                    let mut material = Material2::coil(MaterialId2::new(&source.id), &source.name, geometry.length(width, true)?, batches);
                    material.length = length.as_ref().map(|length| geometry.length(length, true)).transpose()?;
                    material
                },
            };
            let mut total = 0u64;
            let generated = source.products.iter().map(|hint| {
                let product = products.get(hint.product_id.as_str()).ok_or_else(|| invalid("CSP2D generation hint references a foreign product"))?;
                let amount = exact_count_text(&hint.count)?;
                total = total.checked_add(amount).ok_or_else(|| invalid("CSP2D generation count overflows"))?;
                if amount == 0 || total > CSP2D_MAX_PLACEMENTS as u64 { return Err(invalid("CSP2D per-material generation counts must be positive and total at most 4096")); }
                Ok((product.clone(), amount))
            }).collect::<Result<Vec<_>>>()?;
            let mut request = PatternGenerationRequest2::new(material, generated, kerf.clone()).with_process(source.process.domain());
            request.max_segment_height = source.max_segment_height.as_ref().map(|height| geometry.length(height, true)).transpose()?;
            Ok(request)
        }).collect::<Result<Vec<_>>>()?;
        let demands = self.demands.iter().map(|demand| {
            let (id, value, upper) = match demand {
                Csp2dDemand::Count { product_id, count: amount, upper_bound } => (product_id, DemandValue2::Amount(exact_count_text(amount)?), upper_bound.as_ref().map(|upper| exact_count_text(upper).map(DemandValue2::Amount)).transpose()?),
                Csp2dDemand::Area { product_id, area: amount, upper_bound } => (product_id, DemandValue2::Area(area(amount)?), upper_bound.as_ref().map(|upper| area(upper).map(DemandValue2::Area)).transpose()?),
            };
            let mut demand = Csp2dMasterDemand::new(ProductId2::new(id), value); demand.upper_bound = upper; Ok(demand)
        }).collect::<Result<Vec<_>>>()?;
        let objective = ProduceObjective { area_weight: self.objective.area_weight, waste_weight: self.objective.waste_weight,
            height_weight: self.objective.height_weight, usage_weight: self.objective.usage_weight };
        let mut input = Csp2dColumnGenerationInput { requests, demands, initial_columns: Vec::new(), objective, config: Default::default() };
        input.initial_columns = decode_csp2d_columns(&input, &self.initial_columns)?;
        input.validate().map_err(invalid)?;
        Ok(input)
    }
}

/// 用已有源目录构造并复验完整物理池；不认证其业务来源或用量可行性。 / Construct/revalidate a complete physical pool against existing sources, without authenticating provenance or usage feasibility.
pub fn decode_csp2d_columns(input: &Csp2dColumnGenerationInput, columns: &[Csp2dColumn]) -> Result<Vec<CuttingPlan2<f64, Meter>>> {
    if columns.len() > CSP2D_MAX_POOL || columns.iter().map(|column| column.placements.len()).sum::<usize>() > CSP2D_MAX_PLACEMENTS
        || columns.iter().filter_map(|column| column.cut_tree.as_ref()).map(|tree| tree.nodes.len()).sum::<usize>() > CSP2D_MAX_TREE_NODES {
        return Err(invalid("CSP2D pool exceeds column, total-placement or total-tree-node limits"));
    }
    let geometry = Geometry2d::new(QuantityUnit::Meter)?;
    let output = columns.iter().map(|column| {
        identifier(&column.id)?;
        let request = input.requests.iter().find(|request| request.material.id.as_str() == column.material_id)
            .ok_or_else(|| invalid("CSP2D column references a foreign material"))?;
        let mut pattern = Pattern2::new(PatternId2::new(&column.id), request.material.clone(), column.region.domain(&geometry)?,
            column.placements.iter().map(|placement| geometry.read_placement(placement)).collect::<Result<Vec<_>>>()?, request.process);
        pattern.cut_tree = column.cut_tree.as_ref().map(|tree| tree.domain(&geometry, &request.kerf)).transpose()?;
        Ok(CuttingPlan2::new(PlanId2::new(&column.id), pattern))
    }).collect::<Result<Vec<_>>>()?;
    input.validate_columns(&output).map_err(invalid)?;
    Ok(output)
}

/// 编码米制物理列，源目录及未绑定字段须由调用方另行复验；不将来源元数据当作认证。 / Encode a meter-based physical column; callers separately revalidate source catalogs/unbound fields, without treating provenance metadata as authentication.
pub fn encode_csp2d_column(column: &CuttingPlan2<f64, Meter>) -> Result<Csp2dColumn> {
    identifier(column.id.as_str())?;
    let geometry = Geometry2d::new(QuantityUnit::Meter)?;
    Ok(Csp2dColumn { id: column.id.as_str().into(), material_id: column.pattern.material.id.as_str().into(),
        region: Csp2dRegion::from_domain(&column.pattern.region),
        placements: column.pattern.placements.iter().cloned().map(|placement| geometry.write_placement(placement)).collect::<Result<_>>()?,
        cut_tree: column.pattern.cut_tree.as_ref().map(Csp2dCutTree::from_domain).transpose()? })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProblemEnvelope { schema_version: String, problem: Csp2dProblem }

/// 编码已验证的 canonical 源问题，整数文本与独立领域指纹无损保留。 / Encode a validated canonical source problem, retaining integer text and a domain-separated fingerprint losslessly.
pub fn encode_csp2d_problem(problem: Csp2dProblem) -> Result<ModelArtifact> {
    problem.domain()?;
    let model_json = serde_json::to_string(&ProblemEnvelope { schema_version: "1.0".into(), problem }).map_err(invalid)?;
    if model_json.len() > CSP2D_MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "CSP2D source exceeds 8 MiB")); }
    let fingerprint = ospf_rust_core::solver::sha256_fingerprint("ospf.csp2d.problem.v1", model_json.as_bytes()).value;
    Ok(ModelArtifact { model_json, fingerprint })
}

/// 复验 schema、canonical 原文、指纹及原领域数学／物理契约。 / Revalidate schema, canonical original text, fingerprint and original-domain mathematical/physical contracts.
pub fn decode_csp2d_problem(artifact: &ModelArtifact) -> Result<(Csp2dProblem, Csp2dColumnGenerationInput)> {
    if artifact.model_json.len() > CSP2D_MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "CSP2D source exceeds 8 MiB")); }
    let envelope: ProblemEnvelope = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported CSP2D source schema")); }
    let domain = envelope.problem.domain()?;
    let canonical = serde_json::to_string(&envelope).map_err(invalid)?;
    let fingerprint = ospf_rust_core::solver::sha256_fingerprint("ospf.csp2d.problem.v1", canonical.as_bytes()).value;
    if canonical != artifact.model_json || fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "CSP2D canonical source or fingerprint mismatch")); }
    Ok((envelope.problem, domain))
}

#[cfg(test)]
pub(crate) mod tests;
