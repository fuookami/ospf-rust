//! 仅从实际 final 整数 incumbent 派生精确件数和批次数，面积沿用原 region。 / Derive exact piece/batch counts only from the actual final integer incumbent, retaining original-region areas.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::LinearObjectiveEvaluationFlt64;
use ospf_rust_core::solver::evaluate_linear_solution;
use ospf_rust_framework_csp2d::{application::Csp2dColumnGenerationInput, domain::material::CuttingPlan2};
use crate::{Csp2dDemand, Csp2dProblem, PhysicalQuantity, QuantityUnit, Result, invalid};
use crate::csp2d::{CSP2D_TOLERANCE, exact_count_text};
use super::stage::DecodedStage;

/// 引用完整池的正整数用量，不逐次展开。 / Positive integer usages referencing the complete pool, without per-use expansion.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dUsage {
    /// 完整池位置，不是 final 选中子集的局部位置。 / Complete-pool position, not a local index within the final selected subset.
    pub column_index: u32,
    /// 精确 canonical 十进制文本。 / Exact canonical decimal text.
    pub count: String,
}

/// 已由源及真实整数向量复验的产出，不声明全局最优或外部来源认证。 / Production revalidated against the source/actual integer vector, without claiming global optimality or external provenance authentication.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dProduction {
    /// 正用量完整列引用。 / Complete-column references with positive usage.
    pub usages: Vec<Csp2dUsage>,
    /// 每产品的 checked u128 精确件数文本。 / Checked u128 exact piece-count text per product.
    pub produced_counts: BTreeMap<String, String>,
    /// 原实际放置面积，平方米。 / Original physical placement areas in square meters.
    pub produced_areas: BTreeMap<String, PhysicalQuantity>,
    /// 每物料的精确批次，卷材每次图案使用亦消耗一个批次。 / Exact batches per material; every coil-pattern use also consumes one batch.
    pub material_batches: BTreeMap<String, String>,
    /// 按原 region 累计的消耗面积，不扩大为整板。 / Consumed area accumulated from original regions, never expanded to whole sheets.
    pub consumed_area: PhysicalQuantity,
    /// 原领域净余料面积，含刀缝带，不声明最小余料阈值规则。 / Original-domain net waste area including kerf strips, without claiming remnant-threshold policies.
    pub waste_area: PhysicalQuantity,
    /// 各卷材段高用量之和，米；不是多段共享总卷长约束。 / Sum of used coil-segment heights per material in meters, not a shared total-roll-length constraint.
    pub coil_heights: BTreeMap<String, PhysicalQuantity>,
    /// 复验后的 final 目标，包含一次独立常数。 / Revalidated final objective including its separate constant once.
    pub objective_value: f64,
}

fn add_count(total: &mut u128, amount: u128) -> Result<()> {
    *total = total.checked_add(amount).ok_or_else(|| invalid("CSP2D exact production count overflows"))?;
    Ok(())
}

fn quantity(value: f64, unit: QuantityUnit) -> Result<PhysicalQuantity> {
    if !value.is_finite() || value < 0.0 { return Err(invalid("CSP2D production quantity is nonfinite or negative")); }
    Ok(PhysicalQuantity { value, unit })
}

pub(super) fn derive(
    problem: &Csp2dProblem,
    _input: &Csp2dColumnGenerationInput,
    columns: &[CuttingPlan2<f64, Meter>],
    stage: &DecodedStage,
) -> Result<Option<Csp2dProduction>> {
    let Some(solution) = &stage.report.solution else { return Ok(None); };
    let rounded = solution.values.iter().map(|&value| {
        let integer = value.round();
        if !(0.0..=9_007_199_254_740_991.0).contains(&integer) || (integer - value).abs() > CSP2D_TOLERANCE {
            return Err(invalid("CSP2D final usages are outside the exact supported integer range"));
        }
        Ok(integer)
    }).collect::<Result<Vec<_>>>()?;
    let diagnostics = evaluate_linear_solution(&stage.actual, &rounded, CSP2D_TOLERANCE).map_err(invalid)?;
    if diagnostics.constraint_evaluations.iter().any(|row| !row.satisfied) || diagnostics.variable_bound_evaluations.iter().any(|bound| !bound.satisfied) {
        return Err(invalid("rounded CSP2D vector violates its complete actual model"));
    }
    let objective = LinearObjectiveEvaluationFlt64::new(stage.actual.c.iter().copied().zip(rounded.iter().copied()), 0.0).map_err(invalid)?;
    objective.validate_reported(solution.objective_value.or(solution.objective), CSP2D_TOLERANCE).map_err(invalid)?;
    if solution.objective.is_some() { objective.validate_reported(solution.objective, CSP2D_TOLERANCE).map_err(invalid)?; }
    let snapshot = stage.source.linear_model_snapshot().map_err(invalid)?;
    let compiled = snapshot.compile_flt64().map_err(invalid)?;
    let values = compiled.model.variables.iter().enumerate().map(|(column, token)| (token.id(), rounded[stage.permutation[column]])).collect();
    let copies = stage.assignment.extract_usages(&stage.source, &values, CSP2D_TOLERANCE).map_err(invalid)?;
    let mut produced = problem.products.iter().map(|product| (product.id.clone(), 0u128)).collect::<BTreeMap<_, _>>();
    let mut areas = problem.products.iter().map(|product| (product.id.clone(), 0.0f64)).collect::<BTreeMap<_, _>>();
    let mut batches = problem.materials.iter().map(|material| (material.id.clone(), 0u128)).collect::<BTreeMap<_, _>>();
    let mut heights = problem.materials.iter().filter(|material| matches!(material.shape, crate::Csp2dMaterialShape::Coil { .. }))
        .map(|material| (material.id.clone(), 0.0f64)).collect::<BTreeMap<_, _>>();
    let mut usages = Vec::new();
    let mut consumed = 0.0;
    let mut waste = 0.0;
    for (index, (column, amount)) in columns.iter().zip(copies).enumerate() {
        if amount == 0.0 { continue; }
        let exact = amount as u128;
        usages.push(Csp2dUsage { column_index: index as u32, count: exact.to_string() });
        let material = column.pattern.material.id.as_str();
        add_count(batches.get_mut(material).ok_or_else(|| invalid("foreign CSP2D production material"))?, exact)?;
        consumed += column.consumed_area().value * amount;
        waste += column.waste_area().value * amount;
        if let Some(height) = column.consumed_height() {
            *heights.get_mut(material).ok_or_else(|| invalid("CSP2D production coil identity changed"))? += height.value * amount;
        }
        for (id, per_column) in column.product_amounts() {
            let contribution = exact.checked_mul(u128::from(per_column)).ok_or_else(|| invalid("CSP2D exact product count overflows"))?;
            add_count(produced.get_mut(id.as_str()).ok_or_else(|| invalid("foreign CSP2D production product"))?, contribution)?;
        }
        for placement in &column.pattern.placements {
            let id = placement.item_id.as_deref().ok_or_else(|| invalid("CSP2D production lost a product identity"))?;
            *areas.get_mut(id).ok_or_else(|| invalid("foreign CSP2D production area"))? += placement.area().value * amount;
        }
    }
    // 精确需求／库存独立复验，不能把 f64 相对误差当作额外批次或欠产许可。 / Independently recheck exact demands/stocks; f64 relative error never licenses extra batches or short production.
    for demand in &problem.demands {
        if let Csp2dDemand::Count { product_id, count: lower, upper_bound } = demand {
            let actual = *produced.get(product_id).ok_or_else(|| invalid("foreign CSP2D count demand"))?;
            if actual < u128::from(exact_count_text(lower)?) || upper_bound.as_ref().map(|upper| exact_count_text(upper).map(|upper| actual > u128::from(upper))).transpose()?.unwrap_or(false) {
                return Err(invalid("CSP2D production violates an exact source count bound"));
            }
        }
    }
    for material in &problem.materials {
        if batches[&material.id] > u128::from(exact_count_text(&material.available_batches)?) { return Err(invalid("CSP2D production exceeds exact material stock")); }
    }
    Ok(Some(Csp2dProduction { usages, produced_counts: produced.into_iter().map(|(id, value)| (id, value.to_string())).collect(),
        produced_areas: areas.into_iter().map(|(id, value)| Ok((id, quantity(value, QuantityUnit::SquareMeter)?))).collect::<Result<_>>()?,
        material_batches: batches.into_iter().map(|(id, value)| (id, value.to_string())).collect(),
        consumed_area: quantity(consumed, QuantityUnit::SquareMeter)?, waste_area: quantity(waste, QuantityUnit::SquareMeter)?,
        coil_heights: heights.into_iter().map(|(id, value)| Ok((id, quantity(value, QuantityUnit::Meter)?))).collect::<Result<_>>()?,
        objective_value: snapshot.evaluate_objective_flt64(&values).map_err(invalid)?.total_value().map_err(invalid)? }))
}
