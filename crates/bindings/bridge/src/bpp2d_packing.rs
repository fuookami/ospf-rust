//! 有界压缩装箱与渲染，复用原装箱器且不展开生产份数。 / Bounded compressed packing/rendering, reusing the original packer without expanding production copies.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_framework_bpp2d::{domain::{item::model::{Sheet, SheetLayer}, packing::{Packer, PackingAggregation, PackingRendererAdapter}}, infrastructure::dto::RenderShape2Type};
use crate::{Bpp2dLayer, Bpp2dUsage, BridgeError, Geometry2d, Geometry2dBox, Geometry2dPlacement, Geometry2dPoint, Geometry2dShape, Geometry2dSheet,
    ModelArtifact, PhysicalQuantity, QuantityUnit, Result, decode_bpp2d_layers, decode_bpp2d_problem, encode_bpp2d_layer, invalid};
use crate::bpp2d::{BPP2D_MAX_BYTES, count};

/// 原渲染器的带单位放置，装载顺序来自原装箱器。 / Unit-bearing placement from the original renderer, with loading order from the original packer.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dRenderPlacement {
    /// 形状已体现实际朝向，不二次旋转。 / Shape already carries physical orientation, without a second rotation.
    pub placement: Geometry2dPlacement,
    /// 在本板内从零起的装载顺序。 / Zero-based loading order within this sheet.
    pub loading_order: u32,
}

/// 原渲染器单板输出的米制视图，不混用实际占用及包围面积。 / Meter-based view of original single-sheet rendering, without mixing actual occupied and bounding areas.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dRenderSheet {
    /// 源板材身份。 / Source sheet identity.
    pub sheet_id: String,
    /// 整板尺寸。 / Whole-sheet dimensions.
    pub geometry: Geometry2dSheet,
    /// 整板面积。 / Whole-sheet area.
    pub area: PhysicalQuantity,
    /// 实际形状面积除以整板面积，圆形不使用包围正方形面积。 / Actual-shape area divided by whole-sheet area, not bounding-square area for circles.
    pub loading_rate: f64,
    /// 保留层内原位置的渲染放置。 / Render placements retaining original layer positions.
    pub placements: Vec<Bpp2dRenderPlacement>,
}

/// 一种排布仅装箱一次；份数不展开，也不代表源需求已满足。 / Pack a layout only once; copies are not expanded and do not imply source-demand satisfaction.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dPackedPattern {
    /// 输入完整池位置及精确正份数。 / Input complete-pool index and exact positive copy count.
    pub usage: Bpp2dUsage,
    /// 原装箱器返回的物理层。 / Physical layer returned by the original packer.
    pub layer: Bpp2dLayer,
    /// 原渲染器输出，长度归一为米。 / Original-renderer output with lengths normalized to meters.
    pub render: Bpp2dRenderSheet,
    /// 单板余料矩形，不扣除刀缝耗损或圆内包围角隙。 / Single-sheet remnant rectangles, not deducting kerf loss or including circle bounding-corner gaps.
    pub remnants: Vec<Geometry2dBox>,
    /// 单板实际形状面积。 / Single-sheet actual-shape area.
    pub occupied_area: PhysicalQuantity,
    /// 单板保守可用余料面积，不等于含圆时的整板减实际占用。 / Single-sheet conservative free area, not whole-sheet minus actual occupancy for circles.
    pub remnant_area: PhysicalQuantity,
}

/// 独立装箱副本；不认证求解来源、不重新求解或扩展几何规则。 / Independent packing copy, without authenticating solver provenance, resolving or extending geometry rules.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dPacking {
    /// 源身份。 / Source identity.
    pub source_fingerprint: String,
    /// 压缩后的正份数排布，按输入列位置递增。 / Compressed positive-copy patterns in increasing input-column order.
    pub patterns: Vec<Bpp2dPackedPattern>,
    /// 原装箱器正产出成品的精确加权文本，默认键即物品 ID，省略零条目。 / Exact weighted original-packer finished-material counts, using item IDs and omitting zero entries.
    pub material_counts: BTreeMap<String, String>,
    /// 精确总板数文本。 / Exact total sheet-count text.
    pub sheet_count: String,
    /// 精确总件数文本，可超过 JS 安全整数。 / Exact total item-count text, possibly beyond JS safe integers.
    pub item_count: String,
    /// 加权消耗整板面积。 / Weighted consumed whole-sheet area.
    pub consumed_area: PhysicalQuantity,
    /// 加权实际形状面积。 / Weighted actual-shape area.
    pub occupied_area: PhysicalQuantity,
    /// 加权保守余料面积。 / Weighted conservative remnant area.
    pub remnant_area: PhysicalQuantity,
}

fn quantity(value: f64, unit: QuantityUnit) -> Result<PhysicalQuantity> {
    if !value.is_finite() || value < 0.0 { return Err(invalid("BPP2D packing quantity is nonfinite or negative")); }
    Ok(PhysicalQuantity { value, unit })
}

fn length(value: f64) -> Result<PhysicalQuantity> { quantity(value, QuantityUnit::Meter) }
fn area(value: f64) -> Result<PhysicalQuantity> { quantity(value, QuantityUnit::SquareMeter) }
fn add_count(total: &mut u128, value: u128) -> Result<()> {
    *total = total.checked_add(value).ok_or_else(|| invalid("BPP2D packing count overflow"))?;
    Ok(())
}

/// 校验源、全池和正份数后运行原装箱器及渲染适配；不把任意排布宣称为求解生产。
/// Validate source, full pool and positive copies before invoking the original packer/render adapter; arbitrary layouts are not claimed as solved production.
pub fn pack_bpp2d_layers(
    source: &ModelArtifact,
    columns: &[Bpp2dLayer],
    usages: &[Bpp2dUsage],
) -> Result<Bpp2dPacking> {
    let (_, input) = decode_bpp2d_problem(source)?;
    let layers = decode_bpp2d_layers(&input.request, &input.demands, columns)?;
    let mut previous = None;
    let mut budget = 0usize;
    for usage in usages {
        let index = usage.column_index as usize;
        if index >= layers.len() || previous.is_some_and(|previous| index <= previous) || count(&usage.count)? == 0 {
            return Err(invalid("BPP2D packing usages require increasing valid indices and canonical positive exact-range copies"));
        }
        previous = Some(index);
        let placements = layers[index].units.len();
        // 在调用会分配余料的原分析器前约束保守上界，不截断结果。 / Bound the conservative output before invoking the allocating original remnant analyzer, never truncating results.
        budget += (2 * placements + 1) * (placements + 1);
        if placements > 128 || budget > 65_536 { return Err(BridgeError::new("PayloadTooLarge", "BPP2D packing exceeds 128 placements per pattern or the 65536 remnant-work budget")); }
    }
    let packer = Packer::new().with_items(&input.request.items.iter().map(|(item, _)| item.clone()).collect::<Vec<_>>());
    let geometry = Geometry2d::new(QuantityUnit::Meter)?;
    let mut patterns = Vec::with_capacity(usages.len());
    let mut material_counts = BTreeMap::<String, u128>::new();
    let (mut sheet_count, mut item_count) = (0u128, 0u128);
    let (mut consumed, mut occupied, mut remnant) = (0.0, 0.0, 0.0);
    for usage in usages {
        let layer = &layers[usage.column_index as usize];
        let sheet_type = layer.sheet.clone().ok_or_else(|| invalid("BPP2D packing layer lacks a source sheet"))?;
        let sheet = Sheet::new(sheet_type.clone()).with_layers(vec![SheetLayer::new(sheet_type, layer.shape.clone(), vec![layer.clone()])]);
        let packed = packer.invoke(&[sheet], &PackingAggregation::new("bridge"), input.request.kerf.as_ref()).map_err(invalid)?;
        let rendered = PackingRendererAdapter::new().to_render(&packed);
        let packed_sheet = packed.sheets.first().ok_or_else(|| invalid("BPP2D packer omitted a sheet"))?;
        let rendered_sheet = rendered.sheets.first().ok_or_else(|| invalid("BPP2D renderer omitted a sheet"))?;
        let placements = rendered_sheet.layers.iter().flat_map(|layer| layer.placements.iter()).zip(&packed_sheet.items).map(|(placement, item)| {
            let shape = match placement.shape_type {
                RenderShape2Type::Rectangle => Geometry2dShape::Rectangle { width: length(placement.width)?, height: length(placement.height)? },
                RenderShape2Type::Circle => Geometry2dShape::Circle { radius: length(placement.radius.ok_or_else(|| invalid("BPP2D renderer lost a circle radius"))?)? },
            };
            Ok(Bpp2dRenderPlacement { placement: Geometry2dPlacement { shape, position: Geometry2dPoint { x: length(placement.x)?, y: length(placement.y)? },
                rotated: placement.rotated, item_id: placement.item_id.clone() }, loading_order: u32::try_from(item.loading_order).map_err(invalid)? })
        }).collect::<Result<Vec<_>>>()?;
        if placements.len() != layer.units.len() { return Err(invalid("BPP2D renderer changed the placement count")); }
        let copies = u128::from(count(&usage.count)?);
        add_count(&mut sheet_count, copies)?;
        add_count(&mut item_count, copies.checked_mul(u128::from(rendered.total_amount)).ok_or_else(|| invalid("BPP2D packed item count overflow"))?)?;
        for (key, amount) in &packed.material_summary.amounts {
            add_count(material_counts.entry(key.no.as_str().into()).or_default(), copies.checked_mul(u128::from(*amount)).ok_or_else(|| invalid("BPP2D material count overflow"))?)?;
        }
        consumed += rendered_sheet.area * copies as f64;
        occupied += rendered.total_area * copies as f64;
        remnant += rendered.total_waste_area * copies as f64;
        patterns.push(Bpp2dPackedPattern { usage: usage.clone(), layer: encode_bpp2d_layer(&packed_sheet.layers[0].layer)?,
            render: Bpp2dRenderSheet { sheet_id: rendered_sheet.sheet_type_code.clone(), geometry: Geometry2dSheet { width: length(rendered_sheet.width)?, height: length(rendered_sheet.height)? },
                area: area(rendered_sheet.area)?, loading_rate: rendered_sheet.loading_rate, placements },
            remnants: packed_sheet.remnants.iter().map(|remnant| geometry.write_box(remnant.rectangle.clone())).collect::<Result<_>>()?,
            occupied_area: area(rendered.total_area)?, remnant_area: area(rendered.total_waste_area)? });
    }
    let result = Bpp2dPacking { source_fingerprint: source.fingerprint.clone(), patterns,
        material_counts: material_counts.into_iter().map(|(id, count)| (id, count.to_string())).collect(), sheet_count: sheet_count.to_string(), item_count: item_count.to_string(),
        consumed_area: area(consumed)?, occupied_area: area(occupied)?, remnant_area: area(remnant)? };
    if serde_json::to_vec(&result).map_err(invalid)?.len() > BPP2D_MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP2D packing output exceeds 8 MiB")); }
    Ok(result)
}

#[cfg(test)]
mod tests;
