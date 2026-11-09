//! 原生列生成的目录与物理排布门禁。 / Catalog and physical-layout gates for native column generation.

use std::collections::{BTreeMap, BTreeSet};
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_geometry2d::PackingShape2;
use crate::domain::item::model::{ActualItem, Bpp2dDemandEntry, Bpp2dDemandKey, Bpp2dDemandMode, Bpp2dDemandValue, DemandModeKey, Layer, SheetType};
use crate::domain::layer_assignment::{Bpp2dAssignmentDemandEntry, Bpp2dAssignmentDemandMode, Bpp2dAssignmentDemandValue, Bpp2dDemandShadowPriceMap};
use crate::domain::layer_generation::Bpp2dLayerGenerationRequest;
use crate::domain::packing::service::geometry_guard::{PackingGeometryGuard, PackingRotationRule};

pub(crate) fn invalid(message: impl Into<String>) -> ospf_rust_core::error::CoreError {
    ospf_rust_core::error::SolverError::ContractViolation(message.into()).into()
}

fn positive(value: f64) -> bool { value.is_finite() && value > 0.0 }
fn bits(value: f64) -> u64 { if value == 0.0 { 0 } else { value.to_bits() } }

fn shape_signature(shape: &PackingShape2<f64, Meter>) -> (u8, u64, u64) {
    (u8::from(shape.is_circle()), bits(shape.bounding_width().value), bits(shape.bounding_height().value))
}

fn sheet_signature(sheet: &SheetType<f64, Meter>) -> (u64, u64, u64, bool) {
    (bits(sheet.width.value), bits(sheet.height.value), bits(sheet.max_load.value), sheet.is_main)
}

/// 经目录校验的结构化物理排布身份，不包含生成器来源或迭代。 / Catalog-validated structured physical-layout identity, excluding generator provenance and iteration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LayerColumnKey {
    sheet: String,
    placements: Vec<(String, (u8, u64, u64), u64, u64, bool)>,
}

pub(crate) struct ColumnCatalog<'a> {
    items: BTreeMap<&'a str, &'a ActualItem<f64, Meter>>,
    sheets: BTreeMap<&'a str, &'a SheetType<f64, Meter>>,
    rotation: PackingRotationRule,
    request: &'a Bpp2dLayerGenerationRequest<f64, Meter>,
}

impl<'a> ColumnCatalog<'a> {
    pub(crate) fn new(request: &'a Bpp2dLayerGenerationRequest<f64, Meter>) -> ospf_rust_core::Result<Self> {
        if request.kerf.as_ref().is_some_and(|kerf| !kerf.value.is_finite() || kerf.value < 0.0) {
            return Err(invalid("BPP2D kerf must be finite and nonnegative"));
        }
        let mut items = BTreeMap::new();
        for (item, amount) in &request.items {
            if item.id.as_str().is_empty() || items.insert(item.id.as_str(), item).is_some()
                || !positive(item.shape.bounding_width().value) || !positive(item.shape.bounding_height().value)
                || !positive(item.shape.area().value) || (*amount as u128) > 9_007_199_254_740_991 {
                return Err(invalid("BPP2D source items require unique identifiers, positive finite shapes and exact generation counts"));
            }
            // 当前泛型把重量和长度共用 U，不能据此声称已校验质量。 / The current generic shares U for weight and length; it cannot certify mass.
            if item.weight.value != 0.0 { return Err(invalid("BPP2D native column generation does not support weight/load rules yet")); }
        }
        let mut sheets = BTreeMap::new();
        for sheet in request.sheets.iter().chain(request.sheet.iter()) {
            if sheet.type_code.as_str().is_empty() || !positive(sheet.width.value) || !positive(sheet.height.value)
                || !positive(sheet.area().value) || sheet.max_load.value != 0.0 {
                return Err(invalid("BPP2D source sheets require positive finite geometry; weight/load rules are not supported yet"));
            }
            if let Some(previous) = sheets.insert(sheet.type_code.as_str(), sheet) {
                if sheet_signature(previous) != sheet_signature(sheet) { return Err(invalid("BPP2D source sheet identifier has conflicting definitions")); }
            }
        }
        if items.is_empty() || sheets.is_empty() { return Err(invalid("BPP2D native column generation requires item and sheet catalogs")); }
        let rotation = PackingRotationRule::ByItemCatalog(items.iter().map(|(id, item)| (id.to_string(), item.allow_rotate)).collect());
        Ok(Self { items, sheets, rotation, request })
    }

    pub(crate) fn require_demands(&self, demands: &[Bpp2dAssignmentDemandEntry]) -> ospf_rust_core::Result<()> {
        if demands.iter().any(|demand| !self.items.contains_key(demand.item_id.as_str())) {
            return Err(invalid("BPP2D demand references an item outside the source catalog"));
        }
        let expected = generation_demands(demands);
        if !self.request.demand_entries.is_empty() && self.request.demand_entries != expected {
            return Err(invalid("BPP2D generation demands disagree with the assignment demands"));
        }
        Ok(())
    }

    pub(crate) fn validate(&self, layer: &Layer<f64, Meter>) -> ospf_rust_core::Result<LayerColumnKey> {
        let sheet = layer.sheet.as_ref().ok_or_else(|| invalid("BPP2D column lacks a sheet"))?;
        let source = self.sheets.get(sheet.type_code.as_str()).ok_or_else(|| invalid("BPP2D column sheet is outside the source catalog"))?;
        if sheet_signature(sheet) != sheet_signature(source) { return Err(invalid("BPP2D column forges its source sheet definition")); }
        if layer.units.is_empty() || !positive(layer.shape.width.value) || !positive(layer.shape.height.value)
            || layer.shape.width.value > sheet.width.value || layer.shape.height.value > sheet.height.value {
            return Err(invalid("BPP2D column has an empty layout or invalid envelope"));
        }
        let mut placements = Vec::with_capacity(layer.units.len());
        for placement in &layer.units {
            let id = placement.item_id.as_deref().ok_or_else(|| invalid("BPP2D column placement lacks an item identifier"))?;
            let item = self.items.get(id).ok_or_else(|| invalid("BPP2D column item is outside the source catalog"))?;
            let expected = if placement.rotated { item.shape.rotated() } else { item.shape.clone() };
            let x = placement.position.x.value;
            let y = placement.position.y.value;
            let width = placement.width().value;
            let height = placement.height().value;
            if shape_signature(&placement.shape) != shape_signature(&expected)
                || !x.is_finite() || !y.is_finite() || !(x + width).is_finite() || !(y + height).is_finite()
                || x + width <= x || y + height <= y {
                return Err(invalid("BPP2D column forges an item shape or has collapsed/non-finite coordinates"));
            }
            placements.push((id.to_string(), shape_signature(&placement.shape), bits(x), bits(y), placement.rotated));
        }
        for geometry in [sheet.geometry(), layer.shape.clone()] {
            PackingGeometryGuard::require_sheet_geometry_with_rule(
                &geometry, &layer.units, sheet.type_code.as_str(), "native column generation", self.request.kerf.as_ref(), &self.rotation,
            ).map_err(|error| invalid(error.to_string()))?;
        }
        placements.sort();
        Ok(LayerColumnKey { sheet: sheet.type_code.as_str().to_string(), placements })
    }

    pub(crate) fn keys(&self, layers: &[Layer<f64, Meter>]) -> ospf_rust_core::Result<BTreeSet<LayerColumnKey>> {
        layers.iter().map(|layer| self.validate(layer)).collect()
    }
}

pub(crate) fn generation_demands(demands: &[Bpp2dAssignmentDemandEntry]) -> Vec<Bpp2dDemandEntry> {
    demands.iter().map(|demand| Bpp2dDemandEntry::new(
        match demand.mode { Bpp2dAssignmentDemandMode::ItemAmount => Bpp2dDemandMode::ItemAmount, Bpp2dAssignmentDemandMode::ItemArea => Bpp2dDemandMode::ItemArea },
        Bpp2dDemandKey::item(demand.item_id.as_str()),
        match demand.demand { Bpp2dAssignmentDemandValue::Amount(value) => Bpp2dDemandValue::Amount(value), Bpp2dAssignmentDemandValue::Area(value) => Bpp2dDemandValue::Quantity(value) },
    )).collect()
}

pub(crate) fn generation_prices(prices: &Bpp2dDemandShadowPriceMap) -> BTreeMap<DemandModeKey, f64> {
    prices.iter().map(|(key, value)| (DemandModeKey::new(
        match key.mode { Bpp2dAssignmentDemandMode::ItemAmount => Bpp2dDemandMode::ItemAmount, Bpp2dAssignmentDemandMode::ItemArea => Bpp2dDemandMode::ItemArea },
        key.item_id.as_str(),
    ), *value)).collect()
}
