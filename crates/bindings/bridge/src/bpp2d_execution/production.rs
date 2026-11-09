//! 只从实际 final 整数向量导出生产，精确件数不经浮点累计。 / Derive production only from the actual final integer vector, accumulating exact counts without floating point.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::LinearObjectiveEvaluationFlt64;
use ospf_rust_core::solver::evaluate_linear_solution;
use ospf_rust_framework_bpp2d::{application::Bpp2dColumnGenerationInput, domain::item::model::Layer};
use crate::{Bpp2dDemand, Bpp2dProblem, PhysicalQuantity, QuantityUnit, Result, invalid};
use crate::bpp2d::{BPP2D_TOLERANCE, count};
use super::stage::DecodedStage;

/// 正整数份数，不展开大量板材。 / Positive integer copies without expanding large sheet collections.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dUsage {
    /// 完整池位置。 / Complete-pool position.
    pub column_index: u32,
    /// 精确十进制份数文本。 / Exact decimal copy-count text.
    pub count: String,
}

/// 经实际整数子模型和源需求共同复验的物理产出，不代表全局最优。 / Physical production revalidated against the actual integer child and source demands, without global optimality claims.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp2dProduction {
    /// 正份数列引用。 / Positive-copy column references.
    pub usages: Vec<Bpp2dUsage>,
    /// 每种源物品的精确产出文本，可超过 JS 安全整数。 / Exact production text per source item, possibly exceeding JS safe integers.
    pub produced_counts: BTreeMap<String, String>,
    /// 原领域实际形状面积，单位为平方米。 / Original-domain actual-shape areas in square meters.
    pub produced_areas: BTreeMap<String, PhysicalQuantity>,
    /// 每种源板材的精确张数文本。 / Exact sheet-count text per source sheet.
    pub sheet_counts: BTreeMap<String, String>,
    /// 消耗整板面积，不是占用面积。 / Consumed whole-sheet area, not occupied area.
    pub consumed_area: PhysicalQuantity,
    /// 由复验后的完整整数向量派生的 final 目标。 / Final objective derived from the revalidated complete integer vector.
    pub objective_value: f64,
}

fn add_count(total: &mut u128, amount: u128) -> Result<()> {
    *total = total.checked_add(amount).ok_or_else(|| invalid("BPP2D exact production count overflows"))?;
    Ok(())
}

fn square_area(value: f64) -> Result<PhysicalQuantity> {
    if !value.is_finite() || value < 0.0 { return Err(invalid("BPP2D production area is nonfinite or negative")); }
    Ok(PhysicalQuantity { value, unit: QuantityUnit::SquareMeter })
}

pub(super) fn derive(
    problem: &Bpp2dProblem,
    _input: &Bpp2dColumnGenerationInput,
    layers: &[Layer<f64, Meter>],
    stage: &DecodedStage,
) -> Result<Option<Bpp2dProduction>> {
    let Some(solution) = &stage.report.solution else { return Ok(None); };
    let rounded = solution.values.iter().map(|&value| {
        let integer = value.round();
        if !(0.0..=9_007_199_254_740_991.0).contains(&integer) || (integer - value).abs() > BPP2D_TOLERANCE {
            return Err(invalid("BPP2D final copies are outside the exact supported integer range"));
        }
        Ok(integer)
    }).collect::<Result<Vec<_>>>()?;
    let diagnostics = evaluate_linear_solution(&stage.actual, &rounded, BPP2D_TOLERANCE).map_err(invalid)?;
    if diagnostics.constraint_evaluations.iter().any(|row| !row.satisfied) || diagnostics.variable_bound_evaluations.iter().any(|bound| !bound.satisfied) {
        return Err(invalid("rounded BPP2D vector violates its complete actual model"));
    }
    let objective = LinearObjectiveEvaluationFlt64::new(stage.actual.c.iter().copied().zip(rounded.iter().copied()), 0.0).map_err(invalid)?;
    objective.validate_reported(solution.objective_value.or(solution.objective), BPP2D_TOLERANCE).map_err(invalid)?;
    if solution.objective.is_some() { objective.validate_reported(solution.objective, BPP2D_TOLERANCE).map_err(invalid)?; }
    let snapshot = stage.source.linear_model_snapshot().map_err(invalid)?;
    let compiled = snapshot.compile_flt64().map_err(invalid)?;
    let values = compiled.model.variables.iter().enumerate().map(|(column, token)| (token.id(), rounded[stage.permutation[column]])).collect();
    let copies = stage.assignment.extract_copies(&stage.source, &values, BPP2D_TOLERANCE).map_err(invalid)?;
    let mut produced = problem.items.iter().map(|item| (item.id.clone(), 0u128)).collect::<BTreeMap<_, _>>();
    let mut areas = problem.items.iter().map(|item| (item.id.clone(), 0.0f64)).collect::<BTreeMap<_, _>>();
    let mut sheets = problem.sheets.iter().map(|sheet| (sheet.id.clone(), 0u128)).collect::<BTreeMap<_, _>>();
    let mut usages = Vec::new();
    let mut consumed = 0.0;
    for (index, (layer, amount)) in layers.iter().zip(copies).enumerate() {
        if amount == 0.0 { continue; }
        let amount = amount as u128;
        usages.push(Bpp2dUsage { column_index: index as u32, count: amount.to_string() });
        let sheet = layer.sheet.as_ref().ok_or_else(|| invalid("BPP2D production column lacks a sheet"))?;
        add_count(sheets.get_mut(sheet.type_code.as_str()).ok_or_else(|| invalid("foreign BPP2D production sheet"))?, amount)?;
        consumed += sheet.area().value * amount as f64;
        for (id, per_layer) in layer.item_amounts() {
            let contribution = amount.checked_mul(u128::from(per_layer)).ok_or_else(|| invalid("BPP2D exact item production overflows"))?;
            add_count(produced.get_mut(id.as_str()).ok_or_else(|| invalid("foreign BPP2D production item"))?, contribution)?;
            let unit_area = layer.units.iter().find(|placement| placement.item_id.as_deref() == Some(id.as_str()))
                .ok_or_else(|| invalid("BPP2D production lost an item placement"))?.area().value;
            *areas.get_mut(id.as_str()).ok_or_else(|| invalid("foreign BPP2D production area"))? += unit_area * per_layer as f64 * amount as f64;
        }
    }
    // 件数需求重新用精确整数检查，不能仅依赖 f64 行容差。 / Recheck count demands with exact integers rather than only f64 row tolerances.
    for demand in &problem.demands {
        if let Bpp2dDemand::Count { item_id, count: lower, upper_bound } = demand {
            let actual = *produced.get(item_id).ok_or_else(|| invalid("foreign BPP2D count demand"))?;
            if actual < u128::from(count(lower)?) || upper_bound.as_ref().map(|upper| count(upper).map(|upper| actual > u128::from(upper))).transpose()?.unwrap_or(false) {
                return Err(invalid("BPP2D production violates an exact source count bound"));
            }
        }
    }
    Ok(Some(Bpp2dProduction { usages, produced_counts: produced.into_iter().map(|(id, value)| (id, value.to_string())).collect(),
        produced_areas: areas.into_iter().map(|(id, value)| Ok((id, square_area(value)?))).collect::<Result<_>>()?,
        sheet_counts: sheets.into_iter().map(|(id, value)| (id, value.to_string())).collect(), consumed_area: square_area(consumed)?,
        objective_value: snapshot.evaluate_objective_flt64(&values).map_err(invalid)?.total_value().map_err(invalid)? }))
}
