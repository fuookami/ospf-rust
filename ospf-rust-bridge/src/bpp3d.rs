//! BPP3D 默认长方体／轴感知圆柱源协议及原领域物理回放。 / BPP3D default-cuboid and axis-aware cylinder source protocol with original-domain physical replay.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::{CTUnit, derived::{Kilogram, Meter}};
use ospf_rust_framework_bpp3d::{
    application::{ColumnGenerationApplicationState, CsvMaterializedApplicationRequest, LayerTracePair, replay_strict_grouped_packing},
    domain::{
        item::{ActualItem, BinLayer, BinType, Bpp3dDemandKey, Bpp3dDemandMode, Bpp3dLayerDemandCoverage, PackageShapeSpec},
        layer_assignment::{LayerAssignment, PreciseAssignmentSelection},
        layer_generation::{LayerGenerationContext, LayerPlacementTrace},
        packing::{LayerAssignmentTraceReplayRequest, LayerTraceReplayAdapter, PackedBin},
    },
    infrastructure::{geometry::MetricPoint3, orientation::Orientation},
};
use ospf_rust_math::geometry::Axis3;
use crate::{BridgeError, Geometry2d, ModelArtifact, NativeBackend, PhysicalQuantity, QuantityUnit, Result, invalid};

mod execution;
mod pool;
mod stage;
pub use pool::{decode_bpp3d_column_pool, encode_bpp3d_column_pool};
pub use stage::{Bpp3dStageReport, Bpp3dStageReportArtifact, decode_bpp3d_stage_report, encode_bpp3d_stage_report};
pub use execution::{Bpp3dExecutionReport, Bpp3dExecutionReportArtifact, Bpp3dExecutionStageArtifact, Bpp3dExecutionStageKind,
    Bpp3dExecutionStageSummary, Bpp3dExecutionSummary, decode_bpp3d_execution_report, encode_bpp3d_execution_report};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_LAYER_PLACEMENTS: usize = 256;
const MAX_PLACEMENTS: usize = 4096;
const MAX_DEMAND: u64 = 100_000;

/// 原领域六种长方体朝向。 / The six original-domain cuboid orientations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp3dOrientation {
    /// 直立。 / Upright.
    Upright,
    /// 直立旋转。 / Upright rotated.
    UprightRotated,
    /// 侧放。 / Side.
    Side,
    /// 侧放旋转。 / Side rotated.
    SideRotated,
    /// 平放。 / Lie.
    Lie,
    /// 平放旋转。 / Lie rotated.
    LieRotated,
}

impl From<Bpp3dOrientation> for Orientation {
    fn from(value: Bpp3dOrientation) -> Self {
        match value {
            Bpp3dOrientation::Upright => Self::Upright,
            Bpp3dOrientation::UprightRotated => Self::UprightRotated,
            Bpp3dOrientation::Side => Self::Side,
            Bpp3dOrientation::SideRotated => Self::SideRotated,
            Bpp3dOrientation::Lie => Self::Lie,
            Bpp3dOrientation::LieRotated => Self::LieRotated,
        }
    }
}

impl From<Orientation> for Bpp3dOrientation {
    fn from(value: Orientation) -> Self {
        match value {
            Orientation::Upright => Self::Upright,
            Orientation::UprightRotated => Self::UprightRotated,
            Orientation::Side => Self::Side,
            Orientation::SideRotated => Self::SideRotated,
            Orientation::Lie => Self::Lie,
            Orientation::LieRotated => Self::LieRotated,
        }
    }
}

/// 层内带长度单位的左下角坐标。 / Unit-bearing lower-corner coordinates within a layer.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dPoint {
    /// X 坐标。 / X coordinate.
    pub x: PhysicalQuantity,
    /// Y 坐标。 / Y coordinate.
    pub y: PhysicalQuantity,
    /// Z 坐标。 / Z coordinate.
    pub z: PhysicalQuantity,
}

/// 物品与正整数需求；数量在协议中为无损十进制文本，几何形状缺省为长方体。
/// Item with positive integer demand; its wire count is lossless decimal text and the geometry defaults to a cuboid.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dItem {
    /// 唯一物品标识。 / Unique item identity.
    pub id: String,
    /// 显示名称。 / Display name.
    pub name: String,
    /// 原始宽度。 / Original width.
    pub width: PhysicalQuantity,
    /// 原始高度。 / Original height.
    pub height: PhysicalQuantity,
    /// 原始深度。 / Original depth.
    pub depth: PhysicalQuantity,
    /// 质量，边界检查后按千克数值适配原领域。 / Mass, adapted as kilogram values to the original domain after dimension checks.
    pub weight: PhysicalQuantity,
    /// 规范十进制需求。 / Canonical decimal demand.
    pub amount: String,
    /// 非空且不重复的允许朝向。 / Nonempty, distinct allowed orientations.
    pub orientations: Vec<Bpp3dOrientation>,
    /// 几何形状；缺省表示长方体。 / Geometry; omission means a cuboid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shape: Option<Bpp3dShape>,
}

/// 圆柱对齐轴。 / Cylinder alignment axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp3dAxis {
    /// X 轴。 / X axis.
    X,
    /// Y 轴。 / Y axis.
    Y,
    /// Z 轴。 / Z axis.
    Z,
}

impl From<Bpp3dAxis> for Axis3 {
    fn from(axis: Bpp3dAxis) -> Self {
        match axis { Bpp3dAxis::X => Self::X, Bpp3dAxis::Y => Self::Y, Bpp3dAxis::Z => Self::Z }
    }
}

/// 圆柱离散半径候选：按 `min..=max` 以 `step` 展开，供圆装箱层生成选择半径。
/// Discrete cylinder radius candidates expanded from `min..=max` by `step`, used by circle-packing layer generation
/// to pick a radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dRadiusCandidates {
    /// 最小半径，必须不大于最大值。 / Smallest radius, which must not exceed the maximum.
    pub min: PhysicalQuantity,
    /// 最大半径。 / Largest radius.
    pub max: PhysicalQuantity,
    /// 步长，必须为正。 / Step, which must be positive.
    pub step: PhysicalQuantity,
}

/// 物品几何：默认长方体，或带对齐轴与半径的圆柱。 / Item geometry: a default cuboid, or a cylinder with its alignment axis and radius.
///
/// 圆柱沿轴长度取自对应尺寸（X→宽、Y→高、Z→深），占用按真实圆形足迹由原领域几何门禁校验。
/// A cylinder's axial length comes from the matching dimension (X→width, Y→height, Z→depth) and its real circular footprint is checked by the original domain geometry guard.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum Bpp3dShape {
    /// 长方体。 / Cuboid.
    #[default]
    Cuboid,
    /// 轴感知圆柱。 / Axis-aware cylinder.
    Cylinder {
        /// 对齐轴。 / Alignment axis.
        axis: Bpp3dAxis,
        /// 半径。 / Radius.
        radius: PhysicalQuantity,
        /// 可选的离散半径候选；给出时圆装箱层生成按首个候选计算网格容量。
        /// Optional discrete radius candidates; when present, circle-packing layer generation uses the first
        /// candidate for its grid capacity.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        radius_candidates: Option<Bpp3dRadiusCandidates>,
    },
}

/// 连续半径圆柱在求解器中的原型：物品必须已有同轴圆柱形状，边界含单位并归一为米。
/// Solver prototype for a continuous-radius cylinder: the item must already be a cylinder on the same axis and the
/// bounds carry units, normalized to metres.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dRadiusPrototype {
    /// 物品身份，最多一个原型。 / Item identity; at most one prototype per item.
    pub item_id: String,
    /// 来源标签，用于变量名与结果匹配。 / Source label used for the variable name and result matching.
    pub source: String,
    /// 对齐轴，必须与该物品的圆柱轴一致。 / Alignment axis, which must match the item's cylinder axis.
    pub axis: Bpp3dAxis,
    /// 半径下界，正值且不大于上界。 / Radius lower bound, positive and not above the upper bound.
    pub lower_bound: PhysicalQuantity,
    /// 半径上界。 / Radius upper bound.
    pub upper_bound: PhysicalQuantity,
}

/// 半径业务权重函数 `weight = intercept + coefficient * r²`；final 阶段要求每个原型都有对应函数。
/// Business radius weight function `weight = intercept + coefficient * r²`; the final stage requires one for every
/// prototype.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dRadiusWeightFunction {
    /// 函数键，对应原型的 `source`。 / Function key matching a prototype `source`.
    pub key: String,
    /// 截距。 / Intercept.
    pub intercept: f64,
    /// 半径平方系数，必须非零。 / Radius-squared coefficient, which must be nonzero.
    pub radius_squared_coefficient: f64,
    /// 目标权重，必须非零。 / Objective weight, which must be nonzero.
    pub objective_weight: f64,
}

/// 连续半径模型组件声明。 / Continuous-radius model component declaration.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dRadiusComponent {
    /// 非空原型列表。 / Nonempty prototype list.
    pub prototypes: Vec<Bpp3dRadiusPrototype>,
    /// 每个原型 `source` 对应的业务权重函数；缺失会使 final 阶段拒绝该模型。
    /// Business weight functions for each prototype `source`; a missing entry makes the final stage reject the model.
    pub weight_functions: Vec<Bpp3dRadiusWeightFunction>,
    /// 目标策略；缺省为原领域默认的最小化半径平方（权重 1e-6）。
    /// Objective policy; the default is the original domain default minimizing the radius squared with weight 1e-6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub objective: Option<Bpp3dRadiusObjective>,
}

/// 连续半径目标策略；两个变体都由原领域注册阶段真实消费。
/// Continuous-radius objective policy; both variants are really consumed by the original registration stage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp3dRadiusObjective {
    /// 不注册目标项。 / Do not register an objective term.
    None,
    /// 最小化半径平方，权重必须有限且非零。 / Minimize the radius squared; the weight must be finite and nonzero.
    MinimizeRadiusSquared {
        /// 目标权重。 / Objective weight.
        weight: f64,
    },
}

/// 物品的包属性子集：只开放原 CSV 路径实际物化的三项，其余业务规则仍属未绑定。
/// Subset of item package attributes exposing only the three fields the original CSV path materializes; the other
/// business rules remain unbound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dPackageAttribute {
    /// 物品身份，必须存在。 / Item identity, which must exist.
    pub item_id: String,
    /// 禁止混装；`Some(false)` 由原领域业务规则校验记录为诊断（`domain()` 本身不做该步）。
    /// Disallow mixed loading; `Some(false)` is recorded as a diagnostic by the original business-rule validation,
    /// which `domain()` does not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allow_mixed_loading: Option<bool>,
    /// 最大堆叠层数，必须为正。 / Maximum stacking layers, which must be positive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_stack_layers: Option<u64>,
    /// 业务标签。 / Business tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// 独立候选箱；相同 type_code 必须具有相同的归一化箱型。 / An individual candidate bin; identical type codes must share the same normalized type.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dBin {
    /// 唯一候选箱标识。 / Unique candidate-bin identity.
    pub id: String,
    /// 原领域箱型编码，可在相同类型的候选箱间复用。 / Original type code, reusable across identical candidate bins.
    pub type_code: String,
    /// 内部宽度。 / Interior width.
    pub width: PhysicalQuantity,
    /// 内部高度。 / Interior height.
    pub height: PhysicalQuantity,
    /// 内部深度。 / Interior depth.
    pub depth: PhysicalQuantity,
    /// 最大载荷质量。 / Maximum payload mass.
    pub capacity: PhysicalQuantity,
    /// 原领域主箱型标记。 / Original-domain primary-bin-type flag.
    pub is_main: bool,
}

/// 单件真实放置；不将分组数量当成隐含坐标。 / One physical placement; grouped counts never imply coordinates.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dPlacement {
    /// 源物品标识。 / Source item identity.
    pub item_id: String,
    /// 层内坐标。 / Coordinates within the layer.
    pub position: Bpp3dPoint,
    /// 已允许的朝向。 / An allowed orientation.
    pub orientation: Bpp3dOrientation,
}

/// 带完整单件 trace 的初始候选层。 / Initial candidate layer with complete per-item traces.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dLayer {
    /// 唯一源层标识。 / Unique source-layer identity.
    pub id: String,
    /// 对应候选箱标识；相同箱型仍可复用该层。 / Candidate-bin identity; the layer remains reusable by identical bin types.
    pub bin_id: String,
    /// 层深度。 / Layer depth.
    pub depth: PhysicalQuantity,
    /// 完整单件放置。 / Complete individual placements.
    pub placements: Vec<Bpp3dPlacement>,
}

/// 将一个源层分配到独立候选箱；同一层可用于不同箱，但同一箱中不能重复。 / Assign a source layer to a candidate bin; a layer may be reused across bins but not repeated within one bin.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dAssignment {
    /// 源候选箱标识。 / Source candidate-bin identity.
    pub bin_id: String,
    /// 源层标识。 / Source-layer identity.
    pub layer_id: String,
}

/// 已复验的独立箱及其绝对坐标。 / A revalidated candidate bin and its absolute coordinates.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dPackedBin {
    /// 源候选箱标识。 / Source candidate-bin identity.
    pub bin_id: String,
    /// 米制绝对坐标的完整单件放置。 / Complete individual placements with absolute coordinates in meters.
    pub placements: Vec<Bpp3dPlacement>,
}

/// 源绑定的物理方案，不声称求解器来源或最优性。 / Source-bound physical plan, without claims of solver provenance or optimality.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dPacking {
    /// 已复验源指纹。 / Revalidated source fingerprint.
    pub source_fingerprint: String,
    /// 按源箱顺序排列的已使用箱。 / Used bins in source-bin order.
    pub bins: Vec<Bpp3dPackedBin>,
}

/// 已注册的原层生成策略，按显式类型构造，不动态加载任意回调。
/// Registered original layer-generation strategies, constructed by explicit kind without dynamically loaded callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp3dGeneratorKind {
    /// 简单块层。 / Simple block layers.
    Block,
    /// 局部左下 BL。 / Local bottom-left BL.
    BlLocal,
    /// 全局左下 BL。 / Global bottom-left BL.
    BlGlobal,
    /// 圆盘层（仅对圆柱契约生效，长方体源上不产生候选）。 / Circle layers, effective only for cylinder contracts.
    CirclePacking,
    /// 图案层。 / Pattern layers.
    Pattern,
    /// 堆叠层。 / Pile layers.
    Pile,
    /// 历史层。 / Historical layers.
    Historical,
}

impl Bpp3dGeneratorKind {
    /// 全部已注册策略，与原标准链顺序一致。 / Every registered strategy in original standard-chain order.
    pub const ALL: [Self; 7] = [Self::Block, Self::BlLocal, Self::BlGlobal, Self::CirclePacking, Self::Pattern, Self::Pile, Self::Historical];

    /// 严格证据流程的缺省链：原标准链去掉历史层重放。 / Default chain of the strict-evidence flow: the original standard chain without history replay.
    ///
    /// 历史层生成器在源初始层之外重放同一层身份时会产生不带物理 trace 的重复列，严格 final 必须拒绝；
    /// 只有调用方自带历史并确认可回放时才应显式启用。 / The history generator can emit duplicate columns without physical
    /// traces when it replays an initial layer identity, which the strict final must reject; enable it explicitly only with
    /// caller-supplied history that can be replayed.
    pub const DEFAULT_CHAIN: [Self; 6] = [Self::Block, Self::BlLocal, Self::BlGlobal, Self::CirclePacking, Self::Pattern, Self::Pile];
}

/// 单轮 BPP3D 原生执行配置，与源物理问题分开版本化；不接受任意生成器回调。
/// One-round BPP3D native execution configuration, versioned separately from the source physical problem; no arbitrary generator callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dOptions {
    /// 显式原生后端。 / Explicit native backend.
    pub backend: NativeBackend,
    /// 显式层生成链，缺省为原领域标准链；1..=7 个不重复策略。 / Explicit layer-generation chain, defaulting to the original standard chain; 1..=7 distinct strategies.
    #[ts(optional)]
    pub generators: Option<Vec<Bpp3dGeneratorKind>>,
    /// 有效子报告上限，默认 1024，范围 1..=1024。 / Retained child-report limit, default 1024, range 1..=1024.
    #[ts(optional)]
    pub report_limit: Option<u32>,
    /// 严格物理回放展开上限，默认 10000，范围 1..=100000。 / Strict-replay expansion limit, default 10000, range 1..=100000.
    #[ts(optional)]
    pub max_placements: Option<u32>,
}

impl Bpp3dOptions {
    /// 校验有限预算并返回（子报告上限，严格回放展开上限）。 / Validate bounded budgets returning (report limit, strict-replay expansion limit).
    pub fn budgets(&self) -> Result<(usize, usize)> {
        let report_limit = self.report_limit.unwrap_or(1024) as usize;
        let max_placements = self.max_placements.unwrap_or(10_000) as usize;
        if !(1..=1024).contains(&report_limit) || !(1..=execution::MAX_REPLAY_PLACEMENTS).contains(&max_placements) {
            return Err(invalid("invalid BPP3D execution budgets"));
        }
        if let Some(generators) = &self.generators {
            let unique = generators.iter().collect::<BTreeSet<_>>();
            if generators.is_empty() || generators.len() > Bpp3dGeneratorKind::ALL.len() || unique.len() != generators.len() {
                return Err(invalid("BPP3D generator chain must contain 1..=7 unique registered strategies"));
            }
        }
        Ok((report_limit, max_placements))
    }

    /// 构造原层生成上下文，缺省使用严格证据流程的安全标准链。 / Build the original layer-generation context, defaulting to the strict-evidence-safe standard chain.
    pub fn layer_generation(&self) -> Result<LayerGenerationContext<f64, Meter>> {
        use ospf_rust_framework_bpp3d::domain::layer_generation::{
            BLLocalLayerGenerator, BLGlobalLayerGenerator, BlockLayerGenerator, CirclePackingLayerGenerator,
            HistoricalLayerGenerator, PatternLayerGenerator, PileLayerGenerator,
        };
        self.budgets()?;
        let kinds = self.generators.clone().unwrap_or_else(|| Bpp3dGeneratorKind::DEFAULT_CHAIN.to_vec());
        let mut context = LayerGenerationContext::new();
        for kind in &kinds {
            context.add_generator(match kind {
                Bpp3dGeneratorKind::Block => Box::new(BlockLayerGenerator::new()),
                Bpp3dGeneratorKind::BlLocal => Box::new(BLLocalLayerGenerator::new()),
                Bpp3dGeneratorKind::BlGlobal => Box::new(BLGlobalLayerGenerator::new()),
                Bpp3dGeneratorKind::CirclePacking => Box::new(CirclePackingLayerGenerator::new()),
                Bpp3dGeneratorKind::Pattern => Box::new(PatternLayerGenerator::new()),
                Bpp3dGeneratorKind::Pile => Box::new(PileLayerGenerator::new()),
                Bpp3dGeneratorKind::Historical => Box::new(HistoricalLayerGenerator::new()),
            });
        }
        Ok(context)
    }
}

/// 长方体或轴感知圆柱 BPP3D 源，不含策略或隐式回调。 / Cuboid or axis-aware cylinder BPP3D source without strategies or implicit callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dProblem {
    /// 有序且唯一的物品目录。 / Ordered unique item catalog.
    pub items: Vec<Bpp3dItem>,
    /// 有序候选箱，重复箱型为独立候选箱。 / Ordered candidate bins, with repeated types retaining separate instances.
    pub bins: Vec<Bpp3dBin>,
    /// 有序初始候选层。 / Ordered initial candidate layers.
    pub initial_layers: Vec<Bpp3dLayer>,
    /// 可选的连续半径组件；给出时 final 模型会注册半径变量并回填摆放。
    /// Optional continuous-radius component; when present the final model registers radius variables and applies
    /// the selected radii back onto the placements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub radius_component: Option<Bpp3dRadiusComponent>,
    /// 可选的包属性子集，最多每物品一条。 / Optional package-attribute subset, at most one per item.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package_attributes: Vec<Bpp3dPackageAttribute>,
}

fn identifier(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 256 { return Err(invalid("BPP3D identifiers must contain 1..256 UTF-8 bytes")); }
    Ok(())
}

fn count(value: &str) -> Result<u64> {
    let amount = value.parse::<u64>().map_err(invalid)?;
    if amount == 0 || amount > MAX_DEMAND || amount.to_string() != value { return Err(invalid("BPP3D demand must be canonical and within 1..=100000")); }
    Ok(amount)
}

fn mass(value: &PhysicalQuantity) -> Result<f64> {
    let converted = value.quantity()?.to_unit(&Kilogram::INSTANT).map_err(invalid)?.value;
    if !converted.is_finite() || converted < 0.0 || (value.value != 0.0 && converted == 0.0) {
        return Err(invalid("BPP3D mass must be finite, nonnegative and representable in kilograms"));
    }
    Ok(converted)
}

fn volume(width: f64, height: f64, depth: f64) -> Result<()> {
    let value = width * height * depth;
    if !value.is_finite() || value <= 0.0 { return Err(invalid("BPP3D volume overflows or underflows")); }
    Ok(())
}

impl Bpp3dProblem {
    /// 由源声明派生连续半径组件；两条路径（materialize 与池解码）都用它，保证状态与源一致。
    /// Derive the continuous-radius component from the source declaration; both the materialize and pool-decode
    /// paths use it so state and source cannot drift apart.
    pub fn radius_component(&self, item_indices: &HashMap<&str, usize>) -> Result<Option<ospf_rust_framework_bpp3d::domain::item::ContinuousRadiusModelComponent>> {
        let geometry = Geometry2d::new(QuantityUnit::Meter)?;
        let Some(component) = &self.radius_component else { return Ok(None) };
        if component.prototypes.is_empty() || component.prototypes.len() > 64 { return Err(invalid("BPP3D radius component requires 1..=64 prototypes")); }
        let mut seen = HashSet::new();
        let mut prototypes = Vec::with_capacity(component.prototypes.len());
        let mut variable_names = Vec::new();
        for prototype in &component.prototypes {
            identifier(&prototype.source)?;
            if !item_indices.contains_key(prototype.item_id.as_str()) { return Err(invalid("unknown BPP3D radius prototype item")); }
            if !seen.insert(prototype.item_id.as_str()) { return Err(invalid("BPP3D radius component allows one prototype per item")); }
            let shape = self.items.iter().find(|item| item.id == prototype.item_id).and_then(|item| item.shape.as_ref());
            match (shape, prototype.axis) {
                (Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::X, .. }), Bpp3dAxis::X)
                | (Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::Y, .. }), Bpp3dAxis::Y)
                | (Some(Bpp3dShape::Cylinder { axis: Bpp3dAxis::Z, .. }), Bpp3dAxis::Z) => {}
                _ => return Err(invalid("BPP3D radius prototype requires a cylinder item on the same axis")),
            }
            let variable_name = format!("r_{}_{}", prototype.item_id, prototype.source);
            let built = ospf_rust_framework_bpp3d::domain::item::ContinuousCylinderRadiusSolverPrototype {
                item_id: prototype.item_id.clone().into(), source: prototype.source.clone(), axis: prototype.axis.into(),
                variable_name: variable_name.clone(), radius_lower_bound: None, radius_upper_bound: None }
                .with_runtime_quantity_bounds(prototype.lower_bound.quantity()?.to_unit(&ospf_rust_quantities::unit::derived::Meter::INSTANT).map_err(invalid)?,
                    prototype.upper_bound.quantity()?.to_unit(&ospf_rust_quantities::unit::derived::Meter::INSTANT).map_err(invalid)?)
                .map_err(|errors| invalid(errors))?;
            variable_names.push(variable_name);
            prototypes.push(built);
        }
        // 每个原型的 source 必须有对应权重函数，否则原领域会在 final 记为诊断并拒绝整个模型；
        // 在边界直接报错，避免调用方拿到一句"incomplete or inconsistent"却不知原因。
        // Every prototype source needs a weight function; otherwise the original domain records a diagnostic and
        // rejects the whole model at the final stage. Fail at the boundary instead of surfacing an opaque message.
        let mut weight_functions = std::collections::HashMap::new();
        for function in &component.weight_functions {
            if !function.intercept.is_finite() || !function.radius_squared_coefficient.is_finite() || !function.objective_weight.is_finite() {
                return Err(invalid("BPP3D radius weight function coefficients must be finite"));
            }
            if function.radius_squared_coefficient == 0.0 || function.objective_weight == 0.0 {
                return Err(invalid("BPP3D radius weight function coefficients must be nonzero"));
            }
            if weight_functions.insert(function.key.clone(), ospf_rust_framework_bpp3d::domain::item::ContinuousRadiusWeightFunction::new(
                function.key.clone(), function.intercept, function.radius_squared_coefficient, function.objective_weight)).is_some() {
                return Err(invalid("duplicate BPP3D radius weight function key"));
            }
        }
        for prototype in &component.prototypes {
            if !weight_functions.contains_key(&prototype.source) {
                return Err(invalid("BPP3D radius prototype has no weight function for its source"));
            }
        }
        Ok(Some(ospf_rust_framework_bpp3d::domain::item::ContinuousRadiusModelComponent {
            prototypes, registration_plan: ospf_rust_framework_bpp3d::domain::item::ContinuousRadiusRegistrationPlan {
                variable_names: variable_names.clone(), registered_variables: variable_names, blocked_variables: Vec::new() },
            weight_functions,
            // 目标策略按声明映射；缺省保持原领域默认，绝不静默替换成"无目标"。
            // Objective policy maps from the declaration; absence keeps the original domain default and is never
            // silently replaced by "no objective".
            objective_policy: match component.objective {
                None => ospf_rust_framework_bpp3d::domain::item::ContinuousRadiusObjectivePolicy::default(),
                Some(Bpp3dRadiusObjective::None) => ospf_rust_framework_bpp3d::domain::item::ContinuousRadiusObjectivePolicy::None,
                Some(Bpp3dRadiusObjective::MinimizeRadiusSquared { weight }) => {
                    if !weight.is_finite() || weight == 0.0 {
                        return Err(invalid("BPP3D radius objective weight must be finite and nonzero"));
                    }
                    ospf_rust_framework_bpp3d::domain::item::ContinuousRadiusObjectivePolicy::MinimizeRadiusSquared { weight }
                }
            } }))
    }

    /// 物化为原 CSV application 输入并复用严格单层物理回放，不声称满足完整需求或已经求解。
    /// Materialize original CSV-application inputs and replay each layer strictly, without claiming complete demand satisfaction or a solve.
    pub fn domain(&self) -> Result<(CsvMaterializedApplicationRequest, Vec<LayerTracePair>)> {
        if self.items.is_empty() || self.items.len() > 64 || self.bins.is_empty() || self.bins.len() > 64 || self.initial_layers.len() > 256 {
            return Err(invalid("invalid BPP3D source shape or size"));
        }
        let geometry = Geometry2d::new(QuantityUnit::Meter)?;
        let mut item_indices = HashMap::new();
        let mut total_demand = 0_u64;
        let mut items = Vec::with_capacity(self.items.len());
        let mut amounts = Vec::with_capacity(self.items.len());
        for item in &self.items {
            identifier(&item.id)?;
            if item.name.len() > 256 || item_indices.insert(item.id.as_str(), items.len()).is_some() { return Err(invalid("invalid or duplicate BPP3D item")); }
            let amount = count(&item.amount)?;
            total_demand = total_demand.checked_add(amount).ok_or_else(|| invalid("BPP3D demand overflows"))?;
            if total_demand > MAX_DEMAND { return Err(invalid("BPP3D total demand exceeds strict replay limit")); }
            let distinct: HashSet<_> = item.orientations.iter().copied().collect();
            if distinct.is_empty() || distinct.len() != item.orientations.len() { return Err(invalid("BPP3D orientations must be nonempty and distinct")); }
            let width = geometry.length(&item.width, true)?;
            let height = geometry.length(&item.height, true)?;
            let depth = geometry.length(&item.depth, true)?;
            volume(width.value, height.value, depth.value)?;
            let shape_spec = match item.shape.as_ref().unwrap_or(&Bpp3dShape::Cuboid) {
                Bpp3dShape::Cuboid => PackageShapeSpec::Cuboid,
                Bpp3dShape::Cylinder { axis, radius, radius_candidates: candidates } => {
                    let radius = geometry.length(radius, true)?;
                    let cross_section = match axis {
                        Bpp3dAxis::X => height.value.min(depth.value),
                        Bpp3dAxis::Y => width.value.min(depth.value),
                        Bpp3dAxis::Z => width.value.min(height.value),
                    };
                    if radius.value * 2.0 > cross_section {
                        return Err(invalid("BPP3D cylinder radius exceeds its cross-section"));
                    }
                    // 离散候选沿用原 CSV 语义：min..=max 按 step 展开（含 1e-9 容差），首个候选供圆装箱网格使用。
                    // Discrete candidates keep the original CSV semantics: min..=max expanded by step with a 1e-9
                    // tolerance, the first candidate feeding the circle-packing grid.
                    let radius_candidates = match candidates {
                        None => None,
                        Some(candidates) => {
                            let min = geometry.length(&candidates.min, true)?;
                            let max = geometry.length(&candidates.max, true)?;
                            let step = geometry.length(&candidates.step, true)?;
                            if min.value > max.value { return Err(invalid("BPP3D radius candidates require min <= max")); }
                            if step.value <= 0.0 { return Err(invalid("BPP3D radius candidates require a positive step")); }
                            let mut values = Vec::new();
                            let mut value = min.value;
                            while value <= max.value + 1e-9 {
                                if values.len() >= 1024 { return Err(invalid("BPP3D radius candidates exceed 1024 values")); }
                                values.push(Quantity::new_ct(value));
                                value += step.value;
                            }
                            if values.iter().any(|candidate: &Quantity<f64, Meter>| candidate.value * 2.0 > cross_section) {
                                return Err(invalid("BPP3D radius candidate exceeds its cross-section"));
                            }
                            Some(values)
                        }
                    };
                    PackageShapeSpec::Cylinder { axis: (*axis).into(), radius, radius_candidates, radius_lower_bound: None, radius_upper_bound: None }
                }
            };
            items.push(ActualItem { id: item.id.clone().into(), name: item.name.clone(), package_code: None, pack: None,
                width, height, depth, weight: Quantity::new_ct(mass(&item.weight)?),
                enabled_orientations: item.orientations.iter().copied().map(Into::into).collect(), shape_spec_override: Some(shape_spec) });
            amounts.push((item.id.clone(), amount));
        }
        let mut bin_indices = HashMap::new();
        let mut types = HashMap::<&str, (f64, f64, f64, f64, bool)>::new();
        let mut bins = Vec::with_capacity(self.bins.len());
        for bin in &self.bins {
            identifier(&bin.id)?; identifier(&bin.type_code)?;
            if bin_indices.insert(bin.id.as_str(), bins.len()).is_some() { return Err(invalid("duplicate BPP3D bin id")); }
            let width = geometry.length(&bin.width, true)?;
            let height = geometry.length(&bin.height, true)?;
            let depth = geometry.length(&bin.depth, true)?;
            let capacity = mass(&bin.capacity)?;
            volume(width.value, height.value, depth.value)?;
            let signature = (width.value, height.value, depth.value, capacity, bin.is_main);
            if types.insert(&bin.type_code, signature).is_some_and(|previous| previous != signature) { return Err(invalid("same BPP3D bin type has inconsistent dimensions, capacity or main flag")); }
            bins.push(BinType { width, height, depth, capacity: Quantity::new_ct(capacity), type_code: bin.type_code.clone().into(), is_main: bin.is_main });
        }
        let mut layer_ids = HashSet::new();
        let mut identities = HashSet::new();
        let mut layers = Vec::with_capacity(self.initial_layers.len());
        let mut traces = Vec::with_capacity(self.initial_layers.len());
        let mut total_placements = 0_usize;
        for (index, layer) in self.initial_layers.iter().enumerate() {
            identifier(&layer.id)?;
            if !layer_ids.insert(layer.id.as_str()) { return Err(invalid("duplicate BPP3D layer id")); }
            let bin_index = *bin_indices.get(layer.bin_id.as_str()).ok_or_else(|| invalid("BPP3D layer references a foreign bin"))?;
            if layer.placements.is_empty() || layer.placements.len() > MAX_LAYER_PLACEMENTS { return Err(invalid("BPP3D layer requires 1..=256 explicit placements")); }
            total_placements = total_placements.checked_add(layer.placements.len()).ok_or_else(|| invalid("BPP3D placements overflow"))?;
            if total_placements > MAX_PLACEMENTS { return Err(invalid("BPP3D source exceeds 4096 placements")); }
            let mut coverage = BTreeMap::<usize, u64>::new();
            let mut placement_traces = Vec::with_capacity(layer.placements.len());
            for placement in &layer.placements {
                let item_index = *item_indices.get(placement.item_id.as_str()).ok_or_else(|| invalid("BPP3D placement references a foreign item"))?;
                *coverage.entry(item_index).or_default() += 1;
                let coordinate = |value: &PhysicalQuantity| -> Result<Quantity<f64, Meter>> {
                    let value = geometry.length(value, false)?;
                    if value.value < 0.0 { return Err(invalid("BPP3D placement coordinates must be nonnegative")); }
                    Ok(value)
                };
                placement_traces.push(LayerPlacementTrace { item_index, item_id: placement.item_id.clone().into(),
                    position: MetricPoint3 { x: coordinate(&placement.position.x)?, y: coordinate(&placement.position.y)?, z: coordinate(&placement.position.z)? },
                    orientation: placement.orientation.into(), amount: 1 });
            }
            let entry = BinLayer { iteration: 0, from: format!("bridge:{index}"), bin: Some(bins[bin_index].clone()), depth: geometry.length(&layer.depth, true)?,
                demand_coverage: coverage.into_iter().map(|(item_index, amount)| Bpp3dLayerDemandCoverage::new(
                    Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: items[item_index].id.clone() }, amount as f64)).collect() };
            if !identities.insert(entry.identity_key()) { return Err(invalid("ambiguous BPP3D initial-layer identity")); }
            layers.push(entry);
            traces.push((Vec::new(), placement_traces));
            let selection = PreciseAssignmentSelection { activated_bins: vec![bin_index], assignments: vec![LayerAssignment { bin_index, layer_index: index }] };
            let placement_map = HashMap::from([(index, traces[index].1.clone())]);
            let packed = LayerTraceReplayAdapter::new().replay_assignments(LayerAssignmentTraceReplayRequest {
                selection: &selection, layers: &layers, bins: &bins, items: &items, block_traces: &HashMap::new(), placement_traces: &placement_map,
                max_placements: MAX_LAYER_PLACEMENTS, extra_geometry: None,
            }).map_err(|errors| invalid(errors.join("; ")))?;
            let weight = packed[0].total_weight().value;
            if !weight.is_finite() || weight > bins[bin_index].capacity.value + 1e-7 { return Err(invalid("BPP3D layer exceeds bin weight capacity")); }
        }
        let continuous_radius_component = self.radius_component(&item_indices)?;
        // 包属性只映射 CSV 路径实际物化的三项；未知物品、重复条目与非正层数在此直接拒绝。
        // Package attributes map only the three fields the CSV path materializes; unknown items, duplicate entries and
        // non-positive layer counts fail here.
        if self.package_attributes.len() > 1024 { return Err(BridgeError::new("PayloadTooLarge", "BPP3D package attributes exceed the item budget")); }
        let mut seen_attributes = HashSet::new();
        let mut attributes = Vec::with_capacity(self.package_attributes.len());
        for attribute in &self.package_attributes {
            if !item_indices.contains_key(attribute.item_id.as_str()) { return Err(invalid("BPP3D package attribute references an unknown item")); }
            if !seen_attributes.insert(attribute.item_id.as_str()) { return Err(invalid("BPP3D package attributes allow one entry per item")); }
            if attribute.max_stack_layers == Some(0) { return Err(invalid("BPP3D package attribute max_stack_layers must be positive")); }
            if attribute.tags.len() > 64 || attribute.tags.iter().any(|tag| tag.trim().is_empty() || tag.len() > 256) {
                return Err(invalid("BPP3D package attribute tags must be 1..=256 nonempty bytes and at most 64 per item"));
            }
            attributes.push((attribute.item_id.clone(), ospf_rust_framework_bpp3d::domain::item::PackageAttribute {
                allow_mixed_loading: attribute.allow_mixed_loading, max_stack_layers: attribute.max_stack_layers,
                tags: attribute.tags.clone(), ..Default::default() }));
        }
        Ok((CsvMaterializedApplicationRequest { items, item_amounts: amounts, bins, initial_layers: layers, depth_boundary_policy: None,
            patterned_items: Vec::new(), package_attributes: attributes, continuous_radius_component, diagnostics: Vec::new() }, traces))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProblemEnvelope { schema_version: String, problem: Bpp3dProblem }

/// 编码经过领域校验的无损源和独立领域指纹。 / Encode a domain-validated lossless source with a domain-separated fingerprint.
pub fn encode_bpp3d_problem(problem: Bpp3dProblem) -> Result<ModelArtifact> {
    problem.domain()?;
    let model_json = serde_json::to_string(&ProblemEnvelope { schema_version: "1.0".into(), problem }).map_err(invalid)?;
    if model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP3D source exceeds 8 MiB")); }
    let fingerprint = ospf_rust_core::solver::sha256_fingerprint("ospf.bpp3d.problem.v1", model_json.as_bytes()).value;
    Ok(ModelArtifact { model_json, fingerprint })
}

/// 复验版本、canonical 文本、指纹和领域物理数据。 / Revalidate version, canonical text, fingerprint and domain physical data.
pub fn decode_bpp3d_problem(artifact: &ModelArtifact) -> Result<(Bpp3dProblem, CsvMaterializedApplicationRequest, Vec<LayerTracePair>)> {
    if artifact.model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP3D source exceeds 8 MiB")); }
    let envelope: ProblemEnvelope = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP3D source schema")); }
    let encoded = encode_bpp3d_problem(envelope.problem.clone())?;
    if encoded.model_json != artifact.model_json || encoded.fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP3D canonical source or fingerprint mismatch")); }
    let (request, traces) = envelope.problem.domain()?;
    Ok((envelope.problem, request, traces))
}

/// 回放源初始层的完整分配，复验多箱、多层、载重及精确分组需求，不生成列或调用求解器。
/// Replay complete assignments of source initial layers, revalidating multiple bins, layers, loads and exact grouped demands without generating columns or invoking a solver.
pub fn pack_bpp3d_layers(source: &ModelArtifact, assignments: &[Bpp3dAssignment]) -> Result<Bpp3dPacking> {
    if assignments.len() > MAX_PLACEMENTS { return Err(invalid("BPP3D packing exceeds 4096 assignments")); }
    let (problem, request, traces) = decode_bpp3d_problem(source)?;
    let bins = problem.bins.iter().enumerate().map(|(index, bin)| (bin.id.as_str(), index)).collect::<HashMap<_, _>>();
    let layers = problem.initial_layers.iter().enumerate().map(|(index, layer)| (layer.id.as_str(), index)).collect::<HashMap<_, _>>();
    let mut active = BTreeSet::new();
    let mut entries = Vec::with_capacity(assignments.len());
    for assignment in assignments {
        let bin_index = *bins.get(assignment.bin_id.as_str()).ok_or_else(|| invalid("BPP3D assignment references a foreign bin"))?;
        let layer_index = *layers.get(assignment.layer_id.as_str()).ok_or_else(|| invalid("BPP3D assignment references a foreign layer"))?;
        active.insert(bin_index);
        entries.push(LayerAssignment { bin_index, layer_index });
    }
    let selection = PreciseAssignmentSelection { activated_bins: active.into_iter().collect(), assignments: entries };
    let amounts = request.item_amounts.iter().map(|(_, amount)| *amount).collect::<Vec<_>>();
    let state = ColumnGenerationApplicationState {
        items: request.items, bins: request.bins, initial_layers: request.initial_layers.clone(), layers: request.initial_layers,
        iteration: 0, layer_block_traces: HashMap::new(),
        layer_placement_traces: traces.into_iter().enumerate().map(|(index, (_, placements))| (index, placements)).collect(),
        continuous_radius_component: None, additional_shadow_prices: HashMap::new(), info: HashMap::new(),
    };
    let packed = replay_strict_grouped_packing(&state, &selection, &amounts, MAX_PLACEMENTS).map_err(invalid)?;
    Ok(Bpp3dPacking { source_fingerprint: source.fingerprint.clone(), bins: packed_bins(&problem, &selection, packed)? })
}

/// 将严格回放结果与源候选箱绑定为协议装箱，拒绝遗漏或越界的候选箱。
/// Bind strict-replay results to the source candidate bins, rejecting lost or foreign candidates.
pub(super) fn packed_bins(problem: &Bpp3dProblem, selection: &PreciseAssignmentSelection, packed: Vec<PackedBin<f64, Meter>>) -> Result<Vec<Bpp3dPackedBin>> {
    if packed.len() != selection.activated_bins.len() { return Err(invalid("BPP3D replay lost a candidate bin")); }
    let meter = |value| PhysicalQuantity { value, unit: QuantityUnit::Meter };
    selection.activated_bins.iter().zip(packed).map(|(index, bin)| Ok(Bpp3dPackedBin {
        bin_id: problem.bins.get(*index).ok_or_else(|| invalid("BPP3D replay references a foreign candidate bin"))?.id.clone(),
        placements: bin.items.into_iter().map(|item| Bpp3dPlacement {
            item_id: item.item.id.as_str().into(),
            position: Bpp3dPoint { x: meter(item.position.x.value), y: meter(item.position.y.value), z: meter(item.position.z.value) },
            orientation: item.orientation.into(),
        }).collect(),
    })).collect()
}

#[cfg(test)]
mod tests;
