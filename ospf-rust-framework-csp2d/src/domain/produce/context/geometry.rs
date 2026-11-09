//! 原几何原语上的主问题物理门禁，不认证外部产品目录。 / Master physical gate using original geometry primitives, without authenticating an external product catalog.

use std::collections::BTreeMap;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_geometry2d::{Box2Geometry, PackingShape2, inflated_by_shape};
use crate::domain::material::CuttingPlan2;
use crate::infrastructure::{CutTree2, Region2};
use super::invalid;

fn positive(value: f64) -> bool { value.is_finite() && value > 0.0 }

fn region_valid(region: &Region2<f64, Meter>) -> bool {
    let (x, y, width, height) = (region.x.value, region.y.value, region.width.value, region.height.value);
    x.is_finite() && y.is_finite() && positive(width) && positive(height) && positive(region.area().value)
        && (x + width).is_finite() && (y + height).is_finite() && x + width > x && y + height > y
}

fn same_physical_region(left: &Region2<f64, Meter>, right: &Region2<f64, Meter>) -> bool {
    // 仅容纳量级相关的末位差异，不把旧 1e-9 绝对容差当作微小件的几何证明。 / Admit only scale-relative rounding differences, not the legacy 1e-9 absolute tolerance as a proof for tiny pieces.
    [left.x.value, left.y.value, left.width.value, left.height.value].into_iter()
        .zip([right.x.value, right.y.value, right.width.value, right.height.value])
        .all(|(left, right)| (left - right).abs() <= 8.0 * f64::EPSILON * left.abs().max(right.abs()))
}

pub(crate) fn validate(
    plans: &[CuttingPlan2<f64, Meter>],
    availability: &BTreeMap<String, u64>,
    kerf: &Quantity<f64, Meter>,
) -> ospf_rust_core::Result<()> {
    if !kerf.value.is_finite() || kerf.value < 0.0 { return Err(invalid("CSP2D kerf must be finite and nonnegative")); }
    let mut materials = BTreeMap::new();
    let mut products = BTreeMap::new();
    for plan in plans {
        let pattern = &plan.pattern;
        let material = &pattern.material;
        if plan.id.as_str().trim().is_empty() || pattern.id.as_str().trim().is_empty() || material.id.as_str().trim().is_empty()
            || !positive(material.width.value) || material.height.as_ref().is_some_and(|value| !positive(value.value))
            || material.length.as_ref().is_some_and(|value| !positive(value.value)) || material.available_batches > 9_007_199_254_740_991
            || (material.height.is_some() && material.length.is_some())
            || material.height.as_ref().or(material.length.as_ref()).is_some_and(|height| !positive(material.width.value * height.value)) {
            return Err(invalid("CSP2D plan/material identity, size or exact stock range is invalid"));
        }
        if material.unit_weight.is_some() || material.machine_id.is_some() || material.min_remnant_area.is_some() || !pattern.costar_placements.is_empty() {
            return Err(invalid("CSP2D report-driven master does not yet bind weight, machine, remnant-threshold or by-product policies"));
        }
        material.validate().map_err(|error| invalid(error.to_string()))?;
        let definition = (material.width.value, material.height.as_ref().map(|value| value.value), material.length.as_ref().map(|value| value.value), material.available_batches);
        if materials.insert(material.id.as_str(), definition).is_some_and(|previous| previous != definition)
            || availability.get(material.id.as_str()) != Some(&material.available_batches) {
            return Err(invalid("CSP2D material definitions or explicit stock rows disagree"));
        }
        let region = &pattern.region;
        if !region_valid(region) || region.x.value < 0.0 || region.y.value < 0.0 || region.max_x().value > material.width.value
            || material.height.as_ref().or(material.length.as_ref()).is_some_and(|height| region.max_y().value > height.value)
            || pattern.placements.is_empty() {
            return Err(invalid("CSP2D plan region exceeds its material, collapses or has no finished placement"));
        }
        let mut boxes = Vec::with_capacity(pattern.placements.len());
        for placement in &pattern.placements {
            let id = placement.item_id.as_deref().filter(|id| !id.trim().is_empty()).ok_or_else(|| invalid("CSP2D finished placement lacks a product identity"))?;
            if !matches!(placement.shape, PackingShape2::Rectangle(_)) { return Err(invalid("CSP2D only supports rectangular finished products")); }
            let (width, height) = (placement.width().value, placement.height().value);
            let actual = Region2::new(placement.x(), placement.y(), placement.width(), placement.height());
            if !region_valid(&actual) || !region.contains(&actual) { return Err(invalid("CSP2D placement is nonfinite, collapsed or outside its region")); }
            let original = if placement.rotated { (height, width) } else { (width, height) };
            if products.insert(id, original).is_some_and(|previous| previous != original) { return Err(invalid("CSP2D product identity has conflicting source dimensions")); }
            let bounds = actual.to_box();
            let inflated_shape = inflated_by_shape(&placement.shape, kerf);
            let inflated = Region2::new(placement.x(), placement.y(), inflated_shape.bounding_width(), inflated_shape.bounding_height());
            if !region_valid(&inflated) { return Err(invalid("CSP2D kerf-inflated bounds overflow or collapse")); }
            let inflated = Box2Geometry::new(placement.x(), placement.y(), inflated_shape);
            if boxes.iter().any(|(other, other_inflated): &(Box2Geometry<f64, Meter>, Box2Geometry<f64, Meter>)| bounds.overlapped(other_inflated) || inflated.overlapped(other)) {
                return Err(invalid("CSP2D placements overlap or violate double-sided kerf spacing"));
            }
            boxes.push((bounds, inflated));
        }
        if let Some(tree) = &pattern.cut_tree {
            if !same_physical_region(tree.region(), region) { return Err(invalid("CSP2D cut-tree root does not match its physical pattern")); }
            let mut stack = vec![(tree, 0usize)];
            let mut nodes = 0usize;
            let mut checked = Vec::new();
            let mut matched = vec![false; pattern.placements.len()];
            while let Some((node, depth)) = stack.pop() {
                nodes += 1;
                if depth > 128 || nodes > 4096 || !region_valid(node.region()) { return Err(invalid("CSP2D cut tree exceeds depth/node limits or contains invalid regions")); }
                checked.push(node);
                if let CutTree2::Cut(node) = node { stack.push((&node.lower, depth + 1)); stack.push((&node.upper, depth + 1)); }
            }
            for node in checked {
                match node {
                    CutTree2::Cut(node) => {
                        if node.kerf.value != kerf.value || !node.line.position.value.is_finite() { return Err(invalid("CSP2D cut-tree kerf or cut coordinate is inconsistent")); }
                        let (lower, upper) = node.region.cut(node.line.orientation, &node.line.position, &node.kerf).map_err(|error| invalid(error.to_string()))?;
                        if !same_physical_region(&lower, node.lower.region()) || !same_physical_region(&upper, node.upper.region())
                            || node.split.as_ref().is_some_and(|(cached_lower, cached_upper)| !same_physical_region(&lower, cached_lower) || !same_physical_region(&upper, cached_upper)) {
                            return Err(invalid("CSP2D cut-tree child/cached split differs from the actual cut"));
                        }
                    },
                    CutTree2::Leaf(leaf) => {
                        if let Some(id) = &leaf.part_id {
                            let index = pattern.placements.iter().enumerate().position(|(index, placement)| !matched[index] && placement.item_id.as_ref() == Some(id)
                                && same_physical_region(&leaf.region, &Region2::new(placement.x(), placement.y(), placement.width(), placement.height())))
                                .ok_or_else(|| invalid("CSP2D cut-tree part leaf does not match an unconsumed physical placement"))?;
                            matched[index] = true;
                        }
                    },
                }
            }
            if matched.iter().any(|matched| !matched) { return Err(invalid("CSP2D cut tree omits a finished physical placement")); }
        }
        pattern.validate(kerf).map_err(|error| invalid(error.to_string()))?;
    }
    Ok(())
}
