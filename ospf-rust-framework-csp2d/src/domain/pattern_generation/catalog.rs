//! 报告驱动列生成的源目录与物理候选门禁。 / Source-catalog and physical-candidate gates for report-driven column generation.

use std::collections::{BTreeMap, BTreeSet};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_geometry2d::PackingShape2;
use crate::domain::material::{CuttingPlan2, DemandMode2, Material2, Product2};
use crate::domain::pattern_generation::PatternGenerationRequest2;
use crate::domain::produce::{Csp2dMasterDemand, Csp2dMasterProblem, Csp2dMasterShadowPrices, Csp2dPatternAssignment, validate_physical_plans};

pub(crate) fn invalid(message: impl Into<String>) -> ospf_rust_core::error::CoreError {
    ospf_rust_core::error::SolverError::ContractViolation(message.into()).into()
}

fn positive(value: f64) -> bool { value.is_finite() && value > 0.0 }
fn bits(value: f64) -> u64 { if value == 0.0 { 0 } else { value.to_bits() } }

fn material_signature(material: &Material2<f64, Meter>) -> (u64, Option<u64>, Option<u64>, u64) {
    (bits(material.width.value), material.height.as_ref().map(|value| bits(value.value)), material.length.as_ref().map(|value| bits(value.value)), material.available_batches)
}

/// 默认目标下等价的物理列身份，不包含方案名、图案名或生成器来源。 / Physical column identity under default objectives, excluding plan/pattern names and generator provenance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PatternColumnKey {
    material: String,
    region: [u64; 4],
    placements: Vec<(String, [u64; 4], bool)>,
}

pub(crate) struct ColumnCatalog<'a> {
    requests: BTreeMap<&'a str, &'a PatternGenerationRequest2<f64, Meter>>,
    products: BTreeMap<&'a str, &'a Product2<f64, Meter>>,
    availability: BTreeMap<String, u64>,
    kerf: &'a Quantity<f64, Meter>,
}

impl<'a> ColumnCatalog<'a> {
    pub(crate) fn new(requests: &'a [PatternGenerationRequest2<f64, Meter>]) -> ospf_rust_core::Result<Self> {
        let kerf = &requests.first().ok_or_else(|| invalid("CSP2D requires explicit material generation requests"))?.kerf;
        if !kerf.value.is_finite() || kerf.value < 0.0 { return Err(invalid("CSP2D kerf must be finite and nonnegative")); }
        let mut sources = BTreeMap::new();
        let mut products: BTreeMap<&str, &Product2<f64, Meter>> = BTreeMap::new();
        let mut availability = BTreeMap::new();
        for request in requests {
            request.validate().map_err(|error| invalid(error.to_string()))?;
            let material = &request.material;
            if request.kerf.value != kerf.value || material.id.as_str().trim().is_empty() || sources.insert(material.id.as_str(), request).is_some()
                || !positive(material.width.value) || material.height.as_ref().is_some_and(|value| !positive(value.value))
                || material.length.as_ref().is_some_and(|value| !positive(value.value))
                || (material.height.is_some() && material.length.is_some()) || material.available_batches > 9_007_199_254_740_991
                || material.height.as_ref().or(material.length.as_ref()).is_some_and(|height| !positive(material.width.value * height.value)) {
                return Err(invalid("CSP2D source requires unique materials, a common kerf and finite positive exact-range material definitions"));
            }
            if !request.costars.is_empty() || material.unit_weight.is_some() || material.machine_id.is_some() || material.min_remnant_area.is_some() {
                return Err(invalid("CSP2D report-driven generation does not yet bind by-product, weight, machine or remnant-threshold policies"));
            }
            if request.max_segment_height.as_ref().is_some_and(|height| !positive(height.value) || material.height.is_some()) {
                return Err(invalid("CSP2D segment-height cap requires a positive finite coil bound"));
            }
            let mut local = BTreeSet::new();
            for (product, count) in &request.products {
                if product.id.as_str().trim().is_empty() || !local.insert(product.id.as_str())
                    || !positive(product.width.value) || !positive(product.height.value) || !positive(product.area().value)
                    || *count > 9_007_199_254_740_991 || product.unit_weight.is_some() {
                    return Err(invalid("CSP2D source requires unique finite product shapes, exact generation counts and no unbound weight rules"));
                }
                if let Some(previous) = products.insert(product.id.as_str(), product) {
                    if previous.width.value != product.width.value || previous.height.value != product.height.value || previous.allow_rotate != product.allow_rotate {
                        return Err(invalid("CSP2D source product definitions disagree across materials"));
                    }
                }
            }
            availability.insert(material.id.as_str().to_string(), material.available_batches);
            let bounded = normalized_request(request);
            if !positive(bounded.region_height_bound().value) || !positive(material.width.value * bounded.region_height_bound().value) {
                return Err(invalid("CSP2D source generation envelope overflows or collapses"));
            }
        }
        Ok(Self { requests: sources, products, availability, kerf })
    }

    pub(crate) fn require_demands(&self, demands: &[Csp2dMasterDemand]) -> ospf_rust_core::Result<()> {
        if demands.iter().any(|demand| !self.products.contains_key(demand.product_id.as_str())) {
            return Err(invalid("CSP2D demand references a product outside the source catalog"));
        }
        Ok(())
    }

    pub(crate) fn kerf(&self) -> &Quantity<f64, Meter> { self.kerf }

    pub(crate) fn master(&self, plans: &[CuttingPlan2<f64, Meter>], demands: &[Csp2dMasterDemand]) -> Csp2dMasterProblem {
        Csp2dMasterProblem { assignment: Csp2dPatternAssignment { plans: plans.to_vec(), usage_upper_bounds: vec![0; plans.len()] },
            demands: demands.to_vec(), material_availability: self.availability.clone() }
    }

    pub(crate) fn validate(&self, plan: &CuttingPlan2<f64, Meter>) -> ospf_rust_core::Result<PatternColumnKey> {
        let pattern = &plan.pattern;
        let request = self.requests.get(pattern.material.id.as_str()).ok_or_else(|| invalid("CSP2D column material is outside the source catalog"))?;
        if material_signature(&pattern.material) != material_signature(&request.material) || pattern.process != request.process {
            return Err(invalid("CSP2D column forges its source material or process"));
        }
        if request.max_segment_height.as_ref().is_some_and(|height| pattern.region.max_y().value > height.value) {
            return Err(invalid("CSP2D column exceeds the configured segment-height cap"));
        }
        let mut placements = Vec::with_capacity(pattern.placements.len());
        for placement in &pattern.placements {
            let id = placement.item_id.as_deref().ok_or_else(|| invalid("CSP2D column placement lacks a product identity"))?;
            let product = request.products.iter().map(|(product, _)| product).find(|product| product.id.as_str() == id)
                .ok_or_else(|| invalid("CSP2D column product is not registered for its material"))?;
            let (width, height) = product.size_of(placement.rotated);
            if !matches!(placement.shape, PackingShape2::Rectangle(_)) || placement.width().value != width.value || placement.height().value != height.value
                || (placement.rotated && !product.allow_rotate) {
                return Err(invalid("CSP2D column forges source dimensions or rotation permission"));
            }
            placements.push((id.to_string(), [bits(placement.x().value), bits(placement.y().value), bits(width.value), bits(height.value)], placement.rotated));
        }
        validate_physical_plans(std::slice::from_ref(plan), &self.availability, self.kerf)?;
        placements.sort();
        Ok(PatternColumnKey { material: pattern.material.id.as_str().to_string(),
            region: [bits(pattern.region.x.value), bits(pattern.region.y.value), bits(pattern.region.width.value), bits(pattern.region.height.value)], placements })
    }

    pub(crate) fn keys(&self, plans: &[CuttingPlan2<f64, Meter>]) -> ospf_rust_core::Result<BTreeSet<PatternColumnKey>> {
        plans.iter().map(|plan| self.validate(plan)).collect()
    }

    pub(crate) fn generation_request(
        &self,
        source: &PatternGenerationRequest2<f64, Meter>,
        pool: &[CuttingPlan2<f64, Meter>],
        prices: &Csp2dMasterShadowPrices,
        iteration: usize,
        limit: usize,
    ) -> ospf_rust_core::Result<PatternGenerationRequest2<f64, Meter>> {
        let mut request = normalized_request(source);
        request.iteration = iteration as i64;
        request.max_candidates = limit;
        request.existing_patterns = pool.iter().filter(|plan| plan.pattern.material.id == source.material.id).map(|plan| plan.pattern.clone()).collect();
        request.shadow_prices.clear();
        // 旧生成器只接受按面积的提示；真实检验数另行保留各需求口径及库存行重算。 / Legacy generators accept area-priced hints only; actual reduced costs are recomputed separately with distinct demand bases and stock rows.
        for (key, price) in &prices.demands {
            if let Some(product) = self.products.get(key.product_id.as_str()) {
                let contribution = if key.mode == DemandMode2::Amount { price / product.area().value } else { *price };
                let combined = request.shadow_prices.entry(key.product_id.clone()).or_insert(0.0);
                *combined += contribution;
                if !combined.is_finite() { return Err(invalid("CSP2D generator price hint overflows")); }
            }
        }
        Ok(request)
    }
}

fn normalized_request(source: &PatternGenerationRequest2<f64, Meter>) -> PatternGenerationRequest2<f64, Meter> {
    let mut request = PatternGenerationRequest2::new(source.material.clone(), source.products.clone(), source.kerf.clone()).with_process(source.process);
    request.max_segment_height = source.max_segment_height.clone();
    if let Some(length) = &source.material.length {
        if request.max_segment_height.as_ref().is_none_or(|height| height.value > length.value) { request.max_segment_height = Some(length.clone()); }
    }
    request
}
