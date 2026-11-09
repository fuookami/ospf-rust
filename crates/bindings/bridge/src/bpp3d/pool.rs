//! 源绑定的有界单轮列池，不将物理 trace 或求解器来源视为已认证。
//! Source-bound bounded one-round column pools, without treating physical traces or solver provenance as certified.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_bpp3d::{
    application::ColumnGenerationApplicationState,
    domain::{
        item::{ActualItem, BinLayer, BinType, Bpp3dDemandKey, Bpp3dDemandMode, Bpp3dLayerDemandCoverage, PackageShapeSpec},
        layer_generation::{LayerBlockTrace, LayerPlacementTrace},
    },
    infrastructure::geometry::{MetricPoint3, MetricSize3},
};
use super::{Bpp3dOrientation, MAX_BYTES, decode_bpp3d_problem};
use crate::{BridgeError, ModelArtifact, Result, invalid};

const MAX_COLUMNS: usize = 512;
const MAX_TRACES: usize = 16_384;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pool {
    schema_version: String,
    source_fingerprint: String,
    iteration: String,
    generated: Vec<Column>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Column {
    iteration: String,
    from: String,
    bin_type: Option<String>,
    depth: f64,
    coverage: Vec<Coverage>,
    blocks: Option<Vec<Block>>,
    placements: Option<Vec<Placement>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Coverage { item_id: String, coefficient: f64 }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Block {
    block_index: String,
    item_index: String,
    item_id: String,
    orientation: Bpp3dOrientation,
    grid: [String; 3],
    item_count: String,
    size: [f64; 3],
    origin: [f64; 3],
}

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    item_index: String,
    item_id: String,
    orientation: Bpp3dOrientation,
    position: [f64; 3],
    amount: String,
}

fn decimal(value: &str) -> Result<u64> {
    let number = value.parse::<u64>().map_err(invalid)?;
    if number.to_string() != value { return Err(invalid("BPP3D pool integer is not canonical decimal")); }
    Ok(number)
}

fn index(value: &str) -> Result<usize> { usize::try_from(decimal(value)?).map_err(invalid) }

fn finite(values: &[f64]) -> Result<()> {
    if values.iter().any(|value| !value.is_finite()) { return Err(invalid("BPP3D pool contains a non-finite number")); }
    Ok(())
}

fn point(value: &MetricPoint3<f64, Meter>) -> [f64; 3] { [value.x.value, value.y.value, value.z.value] }

fn size(value: &MetricSize3<f64, Meter>) -> [f64; 3] { [value.width.value, value.height.value, value.depth.value] }

fn domain_point(value: [f64; 3]) -> MetricPoint3<f64, Meter> {
    MetricPoint3 { x: Quantity::new_ct(value[0]), y: Quantity::new_ct(value[1]), z: Quantity::new_ct(value[2]) }
}

fn same_bin(left: &BinType<f64, Meter>, right: &BinType<f64, Meter>) -> bool {
    left.type_code == right.type_code && left.is_main == right.is_main
        && [left.width.value, left.height.value, left.depth.value, left.capacity.value]
            .iter().zip([right.width.value, right.height.value, right.depth.value, right.capacity.value])
            .all(|(a, b)| a.to_bits() == b.to_bits())
}

/// 形状按原领域几何身份比较；圆柱不会因为不是长方体而被判为不同源。
/// Compare shapes by their original-domain geometry, so a cylinder is not treated as a foreign source merely for not being a cuboid.
fn same_shape(left: &Option<PackageShapeSpec<f64, Meter>>, right: &Option<PackageShapeSpec<f64, Meter>>) -> bool {
    match (left, right) {
        (Some(PackageShapeSpec::Cuboid), Some(PackageShapeSpec::Cuboid)) => true,
        (Some(PackageShapeSpec::Cylinder { axis: a, radius: r, radius_candidates: c, radius_lower_bound: l, radius_upper_bound: u }),
         Some(PackageShapeSpec::Cylinder { axis: b, radius: r2, radius_candidates: c2, radius_lower_bound: l2, radius_upper_bound: u2 })) => {
            let (same_candidates, same_bounds) = match (c, c2) {
                (None, None) => (true, true),
                (Some(values), Some(values2)) => (values.len() == values2.len()
                    && values.iter().zip(values2).all(|(x, y)| x.value.to_bits() == y.value.to_bits()), true),
                _ => (false, false),
            };
            let bounds = |value: &Option<Quantity<f64, Meter>>, other: &Option<Quantity<f64, Meter>>| match (value, other) {
                (None, None) => true,
                (Some(x), Some(y)) => x.value.to_bits() == y.value.to_bits(),
                _ => false,
            };
            a == b && r.value.to_bits() == r2.value.to_bits() && same_candidates && same_bounds
                && bounds(l, l2) && bounds(u, u2)
        }
        _ => false,
    }
}

fn same_item(left: &ActualItem<f64, Meter>, right: &ActualItem<f64, Meter>) -> bool {
    left.id == right.id && left.name == right.name && left.package_code.is_none() && left.pack.is_none()
        && same_shape(&left.shape_spec_override, &right.shape_spec_override)
        && left.enabled_orientations == right.enabled_orientations
        && [left.width.value, left.height.value, left.depth.value, left.weight.value]
            .iter().zip([right.width.value, right.height.value, right.depth.value, right.weight.value])
            .all(|(a, b)| a.to_bits() == b.to_bits())
}

fn same_layer(left: &BinLayer<f64, Meter>, right: &BinLayer<f64, Meter>) -> bool {
    left.iteration == right.iteration && left.from == right.from && left.depth.value.to_bits() == right.depth.value.to_bits()
        && match (&left.bin, &right.bin) { (Some(a), Some(b)) => same_bin(a, b), (None, None) => true, _ => false }
        && left.demand_coverage.len() == right.demand_coverage.len()
        && left.demand_coverage.iter().zip(&right.demand_coverage).all(|(a, b)|
            a.mode == b.mode && a.key == b.key && a.coefficient.to_bits() == b.coefficient.to_bits())
}

fn encode_placement(value: &LayerPlacementTrace<f64, Meter>) -> Result<Placement> {
    let position = point(&value.position);
    finite(&position)?;
    if value.item_id.as_str().len() > 256 { return Err(invalid("BPP3D trace item ID is too long")); }
    Ok(Placement { item_index: value.item_index.to_string(), item_id: value.item_id.as_str().into(),
        orientation: value.orientation.into(), position, amount: value.amount.to_string() })
}

fn same_placement(left: &LayerPlacementTrace<f64, Meter>, right: &LayerPlacementTrace<f64, Meter>) -> bool {
    left.item_index == right.item_index && left.item_id == right.item_id && left.orientation == right.orientation
        && left.amount == right.amount && point(&left.position).iter().zip(point(&right.position))
            .all(|(a, b)| a.to_bits() == b.to_bits())
}

fn decode_placement(value: Placement) -> Result<LayerPlacementTrace<f64, Meter>> {
    finite(&value.position)?;
    if value.item_id.len() > 256 { return Err(invalid("BPP3D trace item ID is too long")); }
    Ok(LayerPlacementTrace { item_index: index(&value.item_index)?, item_id: value.item_id.into(),
        orientation: value.orientation.into(), position: domain_point(value.position), amount: decimal(&value.amount)? })
}

fn encode_block(value: &LayerBlockTrace<f64, Meter>) -> Result<Block> {
    let origin = point(&value.origin);
    let size = size(&value.size);
    finite(&origin)?;
    finite(&size)?;
    if value.item_id.as_str().len() > 256 { return Err(invalid("BPP3D trace item ID is too long")); }
    Ok(Block { block_index: value.block_index.to_string(), item_index: value.item_index.to_string(), item_id: value.item_id.as_str().into(),
        orientation: value.orientation.into(), grid: [value.nx.to_string(), value.ny.to_string(), value.nz.to_string()],
        item_count: value.item_count.to_string(), size, origin })
}

fn decode_block(value: Block) -> Result<LayerBlockTrace<f64, Meter>> {
    finite(&value.origin)?;
    finite(&value.size)?;
    if value.item_id.len() > 256 { return Err(invalid("BPP3D trace item ID is too long")); }
    Ok(LayerBlockTrace { block_index: index(&value.block_index)?, item_index: index(&value.item_index)?, item_id: value.item_id.into(),
        orientation: value.orientation.into(), nx: decimal(&value.grid[0])?, ny: decimal(&value.grid[1])?, nz: decimal(&value.grid[2])?,
        item_count: decimal(&value.item_count)?, size: MetricSize3 { width: Quantity::new_ct(value.size[0]),
            height: Quantity::new_ct(value.size[1]), depth: Quantity::new_ct(value.size[2]) }, origin: domain_point(value.origin) })
}

/// 将实际单轮列池编码为源绑定文本；原始 trace 原样保存，尚不声明物理可行或求解器来源。
/// Encode an actual one-round pool as source-bound text, preserving original traces without claiming physical feasibility or solver provenance.
pub fn encode_bpp3d_column_pool(source: &ModelArtifact, state: &ColumnGenerationApplicationState) -> Result<String> {
    let (_, request, traces) = decode_bpp3d_problem(source)?;
    // 连续半径组件完全由源派生，因此池只要求它与源一致，而不是一概拒绝。
    // The continuous-radius component is derived from the source, so the pool requires it to match the source
    // instead of rejecting it outright.
    if state.continuous_radius_component != request.continuous_radius_component
        || !state.additional_shadow_prices.is_empty() || !state.info.is_empty()
        || state.items.len() != request.items.len() || state.bins.len() != request.bins.len()
        || !state.items.iter().zip(&request.items).all(|(a, b)| same_item(a, b))
        || !state.bins.iter().zip(&request.bins).all(|(a, b)| same_bin(a, b))
        || state.initial_layers.len() != request.initial_layers.len() || state.layers.len() < request.initial_layers.len()
        || !state.initial_layers.iter().zip(&request.initial_layers).all(|(a, b)| same_layer(a, b))
        || !state.layers.iter().zip(&request.initial_layers).all(|(a, b)| same_layer(a, b))
    { return Err(invalid("BPP3D pool state differs from the canonical source")); }
    if state.layers.len() > MAX_COLUMNS || state.iteration > 1
        || state.layer_block_traces.keys().chain(state.layer_placement_traces.keys()).any(|index| *index >= state.layers.len())
    { return Err(invalid("BPP3D pool exceeds one-round column or index bounds")); }
    if state.layer_block_traces.values().map(Vec::len).chain(state.layer_placement_traces.values().map(Vec::len))
        .try_fold(0_usize, usize::checked_add).is_none_or(|count| count > MAX_TRACES)
    { return Err(invalid("BPP3D pool exceeds trace budget")); }
    for (index, (_, expected)) in traces.iter().enumerate() {
        if state.layer_block_traces.contains_key(&index)
            || state.layer_placement_traces.get(&index).is_none_or(|values| values.len() != expected.len()
                || !values.iter().zip(expected).all(|(a, b)| same_placement(a, b)))
        { return Err(invalid("BPP3D pool initial traces differ from the canonical source")); }
    }
    let mut generated = Vec::new();
    for (index, layer) in state.layers.iter().enumerate().skip(request.initial_layers.len()) {
        if layer.from.len() > 256 || layer.demand_coverage.len() > request.items.len() {
            return Err(invalid("BPP3D generated column exceeds source or coverage bounds"));
        }
        finite(&[layer.depth.value])?;
        let bin_type = match &layer.bin {
            Some(bin) if request.bins.iter().any(|expected| same_bin(bin, expected)) => Some(bin.type_code.as_str().to_owned()),
            Some(_) => return Err(invalid("BPP3D pool column has a foreign bin type")),
            None => None,
        };
        let coverage = layer.demand_coverage.iter().map(|entry| {
            let Bpp3dDemandKey::Item { id } = &entry.key else { return Err(invalid("BPP3D pool requires item coverage")); };
            if entry.mode != Bpp3dDemandMode::Item || !request.items.iter().any(|item| item.id == *id) {
                return Err(invalid("BPP3D pool coverage references a foreign item or mode"));
            }
            finite(&[entry.coefficient])?;
            Ok(Coverage { item_id: id.as_str().into(), coefficient: entry.coefficient })
        }).collect::<Result<_>>()?;
        let blocks = state.layer_block_traces.get(&index).map(|values| values.iter().map(encode_block).collect::<Result<Vec<_>>>()).transpose()?;
        let placements = state.layer_placement_traces.get(&index).map(|values| values.iter().map(encode_placement).collect::<Result<Vec<_>>>()).transpose()?;
        generated.push(Column { iteration: layer.iteration.to_string(), from: layer.from.clone(), bin_type, depth: layer.depth.value, coverage, blocks, placements });
    }
    let text = serde_json::to_string(&Pool { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        iteration: state.iteration.to_string(), generated }).map_err(invalid)?;
    decode_bpp3d_column_pool(&text, source)?;
    Ok(text)
}

/// 从原源重建初始层并还原生成列；须对实际选择另行执行严格物理回放。
/// Rebuild initial layers from the source and restore generated columns; actual selections still require strict physical replay.
pub fn decode_bpp3d_column_pool(text: &str, source: &ModelArtifact) -> Result<ColumnGenerationApplicationState> {
    if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP3D column pool exceeds 8 MiB")); }
    let pool: Pool = serde_json::from_str(text).map_err(invalid)?;
    if pool.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP3D pool schema")); }
    let (_, request, traces) = decode_bpp3d_problem(source)?;
    if pool.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP3D pool belongs to a different source")); }
    let iteration = index(&pool.iteration)?;
    if iteration > 1 || request.initial_layers.len().saturating_add(pool.generated.len()) > MAX_COLUMNS {
        return Err(invalid("BPP3D pool exceeds one-round column bounds"));
    }
    let mut layers = request.initial_layers.clone();
    let mut block_traces = HashMap::new();
    let mut placement_traces = traces.into_iter().enumerate().map(|(index, (_, placements))| (index, placements)).collect::<HashMap<_, _>>();
    let mut trace_count: usize = placement_traces.values().map(Vec::len).sum();
    let mut identities = layers.iter().map(BinLayer::identity_key).collect::<HashSet<_>>();
    for column in pool.generated {
        if column.from.len() > 256 || column.coverage.len() > request.items.len() {
            return Err(invalid("BPP3D generated column exceeds source or coverage bounds"));
        }
        finite(&[column.depth])?;
        let layer_iteration = column.iteration.parse::<i64>().map_err(invalid)?;
        if layer_iteration.to_string() != column.iteration { return Err(invalid("BPP3D layer iteration is not canonical decimal")); }
        let bin = column.bin_type.map(|code| request.bins.iter().find(|bin| bin.type_code.as_str() == code)
            .cloned().ok_or_else(|| invalid("BPP3D pool column has a foreign bin type"))).transpose()?;
        let mut covered = HashSet::new();
        let coverage = column.coverage.into_iter().map(|entry| {
            finite(&[entry.coefficient])?;
            if !covered.insert(entry.item_id.clone()) || !request.items.iter().any(|item| item.id.as_str() == entry.item_id) {
                return Err(invalid("BPP3D pool coverage has duplicate or foreign items"));
            }
            Ok(Bpp3dLayerDemandCoverage::new(Bpp3dDemandMode::Item, Bpp3dDemandKey::Item { id: entry.item_id.into() }, entry.coefficient))
        }).collect::<Result<Vec<_>>>()?;
        let layer = BinLayer { iteration: layer_iteration, from: column.from, bin, depth: Quantity::new_ct(column.depth), demand_coverage: coverage };
        if !identities.insert(layer.identity_key()) { return Err(invalid("duplicate BPP3D pool column identity")); }
        let layer_index = layers.len();
        if let Some(blocks) = column.blocks {
            trace_count = trace_count.checked_add(blocks.len()).ok_or_else(|| invalid("BPP3D pool trace count overflows"))?;
            if trace_count > MAX_TRACES { return Err(invalid("BPP3D pool exceeds trace budget")); }
            block_traces.insert(layer_index, blocks.into_iter().map(decode_block).collect::<Result<Vec<_>>>()?);
        }
        if let Some(placements) = column.placements {
            trace_count = trace_count.checked_add(placements.len()).ok_or_else(|| invalid("BPP3D pool trace count overflows"))?;
            if trace_count > MAX_TRACES { return Err(invalid("BPP3D pool exceeds trace budget")); }
            placement_traces.insert(layer_index, placements.into_iter().map(decode_placement).collect::<Result<Vec<_>>>()?);
        }
        layers.push(layer);
    }
    Ok(ColumnGenerationApplicationState { items: request.items, bins: request.bins,
        continuous_radius_component: request.continuous_radius_component.clone(), initial_layers: request.initial_layers,
        layers, iteration, layer_block_traces: block_traces, layer_placement_traces: placement_traces,
        additional_shadow_prices: HashMap::new(), info: HashMap::new() })
}

#[cfg(test)]
pub(super) mod tests;
