//! 共享稀疏离散装载协议，所有领域判断及模型注册委托原 BPP context。
//! Shared sparse discrete-loading protocol, delegating domain rules and model registration to the original BPP context.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::{CTUnit, Unit};
use ospf_rust_framework_bpp::{AssignmentKey, DiscreteLoadingProblem, DiscreteLoadingSolution, LayoutSelectionGroup, LoadingInterval, LoadingItem, LoadingLayout, LoadingPosition, PositionConflict, RuntimePackingValueAdapter, loading_conflicts};
use crate::{BridgeError, PhysicalQuantity, QuantityUnit, Result, invalid};

mod context;
pub use context::{BppLoadingContext, BppLoadingReport};
mod objective;
pub use objective::{BppLoadingLayoutCost, BppLoadingObjective};
mod resources;
pub use resources::{BppLoadingResource, BppLoadingResourceResult};
mod benders;
pub use benders::{BppLoadingBendersEnvelope, BppLoadingBendersOptions, BppLoadingBendersReport, decode_bpp_loading_benders, encode_bpp_loading_benders};
mod finishing;
pub use finishing::{BppLoadingCertificationEvidence, BppLoadingSourceFinishingOptions, encode_bpp_loading_certification};
mod partition;
pub use partition::*;
mod partition_execution;
pub use partition_execution::*;
mod execution;
pub use execution::{BppLoadingApplicationEnvelope, BppLoadingApplicationReport, BppLoadingOptions, decode_bpp_loading_application, encode_bpp_loading_application};

pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const TOLERANCE: f64 = 1e-7;

fn identifier(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 256 { return Err(invalid("BPP loading identifiers require 1..256 UTF-8 bytes")); }
    Ok(())
}

fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Result<BTreeSet<String>> {
    let mut output = BTreeSet::new();
    for value in values { identifier(value)?; if !output.insert(value.to_owned()) { return Err(invalid("duplicate BPP loading identity")); } }
    Ok(output)
}

fn count(value: &str) -> Result<u64> {
    let count = value.parse::<u64>().map_err(invalid)?;
    if count.to_string() != value || count == 0 { return Err(invalid("BPP maxCount requires canonical positive u64 text")); }
    Ok(count)
}

fn mass(value: &PhysicalQuantity, unit: QuantityUnit) -> Result<Quantity<f64, Unit>> {
    if !matches!(unit, QuantityUnit::Kilogram | QuantityUnit::Gram) { return Err(invalid("BPP loading requires kg or g as its explicit model weight unit")); }
    let quantity = value.quantity()?;
    let normalized = quantity.to_unit(&unit.unit()).map_err(invalid)?.value;
    if !normalized.is_finite() || normalized < 0.0 || (value.value != 0.0 && normalized == 0.0) {
        return Err(invalid("BPP loading mass must be finite and nonnegative without conversion overflow or underflow"));
    }
    Ok(quantity)
}

/// 独立货物，不按尺寸或重量合并。 / Independent cargo item, never merged by size or weight.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingItem {
    /// 源货物身份。 / Source cargo identity.
    pub id: String,
    /// 带单位单件重量。 / Unit-bearing individual weight.
    pub weight: PhysicalQuantity,
    /// 是否必须装载。 / Whether loading is mandatory.
    pub required: bool,
    /// 可选固定舱位，必须属于合法稀疏分配。 / Optional fixed position, required to belong to the legal sparse assignments.
    pub fixed_position_id: Option<String>,
}

/// 舱位真实容量及外部固定占用，不引入任意 big M。 / True position capacity and external fixed occupancy, without arbitrary big M.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPosition {
    /// 稳定舱位身份。 / Stable position identity.
    pub id: String,
    /// canonical 正 u64 文本；注册按实际稀疏邻接数收紧。 / Canonical positive u64 text; registration tightens it by actual sparse adjacency.
    pub max_count: String,
    /// 带单位最大重量。 / Unit-bearing maximum weight.
    pub max_weight: PhysicalQuantity,
    /// 外部固定占用。 / External fixed occupancy.
    pub fixed_occupied: bool,
    /// 是否允许决策装载，不移除合法固定货物。 / Allow decision loading without removing legal fixed cargo.
    pub enabled: bool,
}

/// 显式稀疏合法分配，不补全笛卡尔积。 / Explicit sparse legal assignment, without Cartesian-product completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingAssignment {
    /// 货物身份。 / Cargo identity.
    pub item_id: String,
    /// 舱位身份。 / Position identity.
    pub position_id: String,
}

/// 非传递冲突边，反向重复也明确拒绝。 / Non-transitive conflict edge, rejecting reversed duplicates too.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingConflict {
    /// 首个舱位。 / First position.
    pub first: String,
    /// 第二个舱位。 / Second position.
    pub second: String,
}

/// 显式布局及允许舱位，不从冲突连通分量推导。 / Explicit layout and permitted positions, never inferred from conflict components.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingLayout {
    /// 组内布局身份。 / Layout identity within its group.
    pub id: String,
    /// 不重复的舱位集合。 / Duplicate-free position set.
    pub position_ids: Vec<String>,
}

/// 显式作用域内恰选一种布局，可与其他组共享舱位。 / Select exactly one layout within an explicit scope; groups may share positions.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingLayoutGroup {
    /// 唯一选择组身份。 / Unique selection-group identity.
    pub id: String,
    /// 完整作用域，必须等于所有布局舱位的并集。 / Complete scope, required to equal the union of all layout positions.
    pub position_ids: Vec<String>,
    /// 显式候选布局。 / Explicit candidate layouts.
    pub layouts: Vec<BppLoadingLayout>,
}

/// 舱位的显式一维区间；同一 space 内严格重叠才生成互斥边，贴边不算冲突。
/// Explicit 1D interval of a position; only a strict overlap inside one space creates a conflict edge, and
/// touching edges do not conflict.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingInterval {
    /// 舱位身份，必须与 positions 一一对应。 / Position identity, one-to-one with `positions`.
    pub id: String,
    /// 甲板或独立空间身份。 / Deck or independent-space identity.
    pub space_id: String,
    /// 起点，含单位。 / Start coordinate with a unit.
    pub start: PhysicalQuantity,
    /// 终点，含单位。 / End coordinate with a unit.
    pub end: PhysicalQuantity,
}

/// 舱位的显式二维矩形；同一 space 内按原几何内核精确判交生成互斥边。
/// Explicit 2D rectangle of a position; conflict edges come from the original geometry kernel's exact
/// intersection test within one space.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingRectangle {
    /// 舱位身份，必须与 positions 一一对应。 / Position identity, one-to-one with `positions`.
    pub id: String,
    /// 甲板或独立空间身份。 / Deck or independent-space identity.
    pub space_id: String,
    /// 左下角横坐标。 / Lower-left x coordinate.
    pub x: PhysicalQuantity,
    /// 左下角纵坐标。 / Lower-left y coordinate.
    pub y: PhysicalQuantity,
    /// 宽度。 / Width.
    pub width: PhysicalQuantity,
    /// 高度。 / Height.
    pub height: PhysicalQuantity,
}

/// 原共享 BPP 默认约束；不携带坐标、额外重量变量或任意业务回调。 / Original shared BPP default constraints, without coordinates, additional-weight variables or arbitrary business callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingProblem {
    /// 最多 512 件独立货物。 / At most 512 independent cargo items.
    pub items: Vec<BppLoadingItem>,
    /// 最多 128 个舱位。 / At most 128 positions.
    pub positions: Vec<BppLoadingPosition>,
    /// 最多 4096 条不重复合法分配。 / At most 4096 duplicate-free legal assignments.
    pub allowed_assignments: Vec<BppLoadingAssignment>,
    /// 最多 2048 条不重复互斥边。 / At most 2048 duplicate-free conflict edges.
    pub conflicts: Vec<BppLoadingConflict>,
    /// 最多 64 组、合计 512 布局及 4096 个布局舱位成员。 / At most 64 groups, 512 layouts total and 4096 layout-position memberships.
    pub layout_groups: Vec<BppLoadingLayoutGroup>,
    /// 模型重量口径，仅 kg 或 g。 / Explicit model weight unit, kg or g only.
    pub weight_unit: QuantityUnit,
    /// 可选的舱位二维矩形；给出时按原二维离散装载工厂补出精确矩形重叠的互斥边，且不得与 `intervals` 同时出现。
    /// Optional per-position 2D rectangles; when present the original 2D discrete-loading factory adds exact
    /// rectangle-overlap edges, and `intervals` must be absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rectangles: Option<Vec<BppLoadingRectangle>>,
    /// 可选的舱位一维区间；给出时由原几何守卫补出同空间严格重叠的互斥边。
    /// Optional per-position 1D intervals; when present the original geometry guard adds conflict edges for
    /// strict overlaps within one space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub intervals: Option<Vec<BppLoadingInterval>>,
}

impl BppLoadingProblem {
    pub(crate) fn domain(&self) -> Result<DiscreteLoadingProblem<f64, Unit>> {
        if self.items.len() > 512 || self.positions.len() > 128 || self.allowed_assignments.len() > 4096 || self.conflicts.len() > 2048
            || self.layout_groups.len() > 64 || self.layout_groups.iter().map(|group| group.layouts.len()).sum::<usize>() > 512
            || self.layout_groups.iter().flat_map(|group| &group.layouts).map(|layout| layout.position_ids.len()).sum::<usize>() > 4096 {
            return Err(BridgeError::new("PayloadTooLarge", "BPP loading source exceeds its item/position/assignment/conflict/layout budget"));
        }
        if !matches!(self.weight_unit, QuantityUnit::Kilogram | QuantityUnit::Gram) { return Err(invalid("BPP loading requires kg or g")); }
        let items = unique(self.items.iter().map(|item| item.id.as_str()))?;
        let positions = unique(self.positions.iter().map(|position| position.id.as_str()))?;
        unique(self.layout_groups.iter().map(|group| group.id.as_str()))?;
        let allowed_assignments = self.allowed_assignments.iter().map(|key| {
            identifier(&key.item_id)?; identifier(&key.position_id)?;
            if !items.contains(&key.item_id) || !positions.contains(&key.position_id) { return Err(invalid("unknown BPP loading assignment endpoint")); }
            Ok(AssignmentKey { item_id: key.item_id.clone(), position_id: key.position_id.clone() })
        }).collect::<Result<BTreeSet<_>>>()?;
        let mut conflicts = self.conflicts.iter().map(|edge| {
            identifier(&edge.first)?; identifier(&edge.second)?;
            // 在原冲突索引构造前约束顶点数，避免非法外部端点放大计算。 / Bound vertices before original conflict-index construction so invalid external endpoints cannot amplify work.
            if !positions.contains(&edge.first) || !positions.contains(&edge.second) { return Err(invalid("unknown BPP loading conflict endpoint")); }
            Ok(PositionConflict::new(&edge.first, &edge.second))
        }).collect::<Result<BTreeSet<_>>>()?;
        if allowed_assignments.len() != self.allowed_assignments.len() || conflicts.len() != self.conflicts.len() { return Err(invalid("duplicate BPP loading assignment or conflict")); }
        if let Some(intervals) = &self.intervals {
            if intervals.len() > 128 { return Err(BridgeError::new("PayloadTooLarge", "BPP loading intervals exceed the position budget")); }
            // 区间归一到米后由原几何守卫判交：同一 space 内严格重叠才生成边，贴边不算冲突。
            // Normalize intervals to meters, then let the original geometry guard decide: only a strict overlap
            // within one space creates an edge, and touching edges do not conflict.
            let meter = ospf_rust_quantities::unit::derived::Meter::INSTANT.clone();
            let length = |value: &PhysicalQuantity| -> Result<Quantity<f64, Unit>> {
                let quantity = value.quantity()?;
                quantity.to_unit(&meter).map_err(invalid)
            };
            let intervals = intervals.iter().map(|interval| {
                identifier(&interval.id)?; identifier(&interval.space_id)?;
                if !positions.contains(&interval.id) { return Err(invalid("unknown BPP loading interval position")); }
                Ok(LoadingInterval { id: interval.id.clone(), space_id: interval.space_id.clone(),
                    start: length(&interval.start)?, end: length(&interval.end)? })
            }).collect::<Result<Vec<_>>>()?;
            conflicts.extend(loading_conflicts(&intervals, &meter, &RuntimePackingValueAdapter, None).map_err(invalid)?);
        }
        if self.rectangles.is_some() && self.intervals.is_some() {
            return Err(invalid("BPP loading accepts either intervals or rectangles, not both"));
        }
        let problem = DiscreteLoadingProblem {
            items: self.items.iter().map(|item| {
                if let Some(position) = &item.fixed_position_id { identifier(position)?; }
                Ok(LoadingItem { id: item.id.clone(), weight: mass(&item.weight, self.weight_unit)?, required: item.required, fixed_position_id: item.fixed_position_id.clone() })
            }).collect::<Result<_>>()?,
            positions: self.positions.iter().map(|position| Ok(LoadingPosition { id: position.id.clone(), max_count: count(&position.max_count)?,
                max_weight: mass(&position.max_weight, self.weight_unit)?, fixed_occupied: position.fixed_occupied, enabled: position.enabled })).collect::<Result<_>>()?,
            allowed_assignments, conflicts,
            layout_groups: self.layout_groups.iter().map(|group| {
                if group.position_ids.len() > 128 { return Err(invalid("BPP layout scope exceeds position budget")); }
                unique(group.layouts.iter().map(|layout| layout.id.as_str()))?;
                Ok(LayoutSelectionGroup { id: group.id.clone(), position_ids: unique(group.position_ids.iter().map(String::as_str))?,
                    layouts: group.layouts.iter().map(|layout| Ok(LoadingLayout { id: layout.id.clone(), position_ids: unique(layout.position_ids.iter().map(String::as_str))? })).collect::<Result<_>>()? })
            }).collect::<Result<_>>()?, weight_unit: self.weight_unit.unit(),
        };
        let Some(rectangles) = &self.rectangles else { return Ok(problem); };
        if rectangles.len() > 128 { return Err(BridgeError::new("PayloadTooLarge", "BPP loading rectangles exceed the position budget")); }
        // 归一化到米后整段交给原二维离散装载工厂：它负责矩形范围校验、纵向扫描与精确矩形判交。
        // Normalize to meters and hand everything to the original 2D discrete-loading factory, which owns rectangle
        // extent validation, the sweep and the exact intersection test.
        let meter = ospf_rust_quantities::unit::derived::Meter::INSTANT.clone();
        let length = |value: &PhysicalQuantity| -> Result<Quantity<f64, Unit>> { value.quantity()?.to_unit(&meter).map_err(invalid) };
        let geometry = rectangles.iter().map(|rectangle| {
            Ok(ospf_rust_framework_bpp2d::domain::discrete_loading::LoadingRectangle { id: rectangle.id.clone(), space_id: rectangle.space_id.clone(),
                bounds: ospf_rust_framework_geometry2d::Box2Geometry { x: length(&rectangle.x)?, y: length(&rectangle.y)?,
                    shape: ospf_rust_framework_geometry2d::PackingShape2::Rectangle(ospf_rust_framework_geometry2d::RectanglePackingShape2 {
                        width: length(&rectangle.width)?, height: length(&rectangle.height)? }) } })
        }).collect::<Result<Vec<_>>>()?;
        let context = ospf_rust_framework_bpp2d::domain::discrete_loading::Bpp2dDiscreteLoading::context(problem, &geometry, &meter,
            &RuntimePackingValueAdapter, &RuntimePackingValueAdapter).map_err(invalid)?;
        Ok(context.problem().clone())
    }
}

/// 领域方案 DTO，空预留与布局选择均显式保留。 / Domain-plan DTO, explicitly retaining empty reservations and layout choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingSolution {
    /// 货物到舱位。 / Cargo-to-position assignments.
    pub assignments: BTreeMap<String, String>,
    /// 不重复的整体占用舱位，允许空预留。 / Duplicate-free whole occupied positions, allowing empty reservations.
    pub occupied_positions: Vec<String>,
    /// 每个显式选择组恰好一个布局。 / Exactly one layout per explicit selection group.
    pub selected_layouts: BTreeMap<String, String>,
}

impl BppLoadingSolution {
    pub(crate) fn domain(&self) -> Result<DiscreteLoadingSolution> {
        if self.assignments.len() > 512 || self.occupied_positions.len() > 128 || self.selected_layouts.len() > 64 { return Err(invalid("BPP loading solution exceeds its source budgets")); }
        for (item, position) in &self.assignments { identifier(item)?; identifier(position)?; }
        for (group, layout) in &self.selected_layouts { identifier(group)?; identifier(layout)?; }
        Ok(DiscreteLoadingSolution { assignments: self.assignments.clone(), occupied_positions: unique(self.occupied_positions.iter().map(String::as_str))?, selected_layouts: self.selected_layouts.clone() })
    }

    pub(crate) fn from_domain(value: &DiscreteLoadingSolution) -> Self {
        Self { assignments: value.assignments.clone(), occupied_positions: value.occupied_positions.iter().cloned().collect(), selected_layouts: value.selected_layouts.clone() }
    }
}

/// 独立复核结果，不将纯可行性验证提升为最优性或三维几何证明。 / Independent validation result, never promoting feasibility checking to optimality or 3D geometry proof.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingResult {
    /// 无损源身份。 / Lossless source identity.
    pub source_fingerprint: String,
    /// 已验证的完整领域方案。 / Validated complete domain plan.
    pub solution: BppLoadingSolution,
    /// 每舱位精确件数文本，包括空舱位的零值。 / Exact per-position count text, including zero for empty positions.
    pub position_counts: BTreeMap<String, String>,
    /// 每舱位装载重量，保留声明的模型单位。 / Loaded weight per position in the declared model unit.
    pub position_weights: BTreeMap<String, PhysicalQuantity>,
    /// 精确总件数文本。 / Exact total loaded-count text.
    pub loaded_count: String,
    /// 有限总装载重量。 / Finite total loaded weight.
    pub loaded_weight: PhysicalQuantity,
    /// 可选的已验证附加资源及含资源重量；无资源源保持旧结果编码。 / Optional validated resources and inclusive weights; resource-free sources retain the old result encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<BppLoadingResourceResult>,
}

#[cfg(test)]
mod tests;
