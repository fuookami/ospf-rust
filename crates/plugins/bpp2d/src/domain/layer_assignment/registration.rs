//! 共享的真实主问题注册；旧数据构件入口保持不变。 / Shared actual master registration; legacy data-artifact entry points remain unchanged.

use std::collections::{BTreeMap, BTreeSet};
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::{LinearObjectiveInput, MetaModel};
use ospf_rust_core::variable::{UContinuous, UInteger, VariableId, VariableRange};
use crate::domain::item::model::Layer;
use crate::domain::layer_assignment::context::LayerAssignmentContext;
use crate::domain::layer_assignment::model::{Bpp2dAssignmentDemandEntry, Bpp2dAssignmentDemandMode, Bpp2dAssignmentDemandValue, Bpp2dDemandShadowPriceKey, Bpp2dDemandShadowPriceMap, layer_demand_coefficient, sheet_area_terms};
use crate::domain::packing::service::geometry_guard::PackingGeometryGuard;

/// 主问题阶段；所有阶段共享同一需求行。 / Master stage; every stage shares the same demand rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerAssignmentStage {
    /// 连续份数，正常面积目标。 / Continuous copies with the ordinary area objective.
    Rmp,
    /// 连续份数，仅最小化归一化人工短缺。 / Continuous copies minimizing normalized artificial deficits only.
    FeasibilityLp,
    /// 整数份数，完整配置目标。 / Integer copies with the complete configured objective.
    Integer,
}

/// 已按米归一化的求解器边界输入，不代表外部物品目录认证。 / Solver-boundary input normalized to meters, not external item-catalog authentication.
pub struct LayerAssignmentModelInput<'a> {
    /// 保留输入顺序，不按旧字符串身份去重。 / Preserve input order without deduplicating by the legacy string identity.
    pub layers: &'a [Layer<f64, Meter>],
    /// 件数为精确整数，面积为平方米。 / Counts are exact integers; areas are square meters.
    pub demands: &'a [Bpp2dAssignmentDemandEntry],
    /// 明确选择 LP、可行性 LP 或整数模型。 / Explicitly select LP, feasibility LP or integer model.
    pub stage: LayerAssignmentStage,
}

/// 注册后的领域引用；允许下游追加变量、约束及目标，不在应用层拼领域矩阵。
/// Registered domain references; downstream code may append variables, constraints and objectives without assembling domain matrices in application code.
#[derive(Debug, Clone)]
pub struct LayerAssignmentModel {
    model_identity: u64,
    stage: LayerAssignmentStage,
    copies: Vec<usize>,
    copy_ids: Vec<VariableId>,
    deficits: Vec<usize>,
    deficit_ids: Vec<VariableId>,
    demands: Vec<Bpp2dAssignmentDemandEntry>,
    lower_rows: Vec<usize>,
    upper_rows: Vec<Option<usize>>,
    row_names: Vec<(usize, String)>,
}

fn invalid(message: impl Into<String>) -> ospf_rust_core::error::CoreError {
    ospf_rust_core::error::SolverError::ContractViolation(message.into()).into()
}

fn positive(value: f64) -> bool { value.is_finite() && value > 0.0 }

fn demand_value(mode: Bpp2dAssignmentDemandMode, value: &Bpp2dAssignmentDemandValue) -> ospf_rust_core::Result<f64> {
    match (mode, value) {
        (Bpp2dAssignmentDemandMode::ItemAmount, Bpp2dAssignmentDemandValue::Amount(value)) if *value <= 9_007_199_254_740_991 => Ok(*value as f64),
        (Bpp2dAssignmentDemandMode::ItemArea, Bpp2dAssignmentDemandValue::Area(value)) if value.is_finite() && *value >= 0.0 => Ok(*value),
        _ => Err(invalid("BPP2D demand kind/value mismatch, non-finite area or inexact solver integer")),
    }
}

fn validate_input(input: &LayerAssignmentModelInput<'_>) -> ospf_rust_core::Result<()> {
    if input.demands.is_empty() { return Err(invalid("BPP2D master requires demands")); }
    let mut keys = BTreeSet::new();
    for demand in input.demands {
        if demand.item_id.as_str().is_empty() || !keys.insert((demand.mode, demand.item_id.as_str())) {
            return Err(invalid("BPP2D demand identifier is empty or duplicated"));
        }
        let lower = demand_value(demand.mode, &demand.demand)?;
        if let Some(upper) = &demand.upper_bound {
            if demand_value(demand.mode, upper)? < lower { return Err(invalid("BPP2D demand upper bound is below its lower bound")); }
        }
        if input.stage == LayerAssignmentStage::FeasibilityLp && lower > 0.0 && !positive(1.0 / lower) {
            return Err(invalid("BPP2D deficit normalization overflows"));
        }
    }
    let mut sheets = BTreeMap::new();
    let mut shapes = BTreeMap::new();
    for layer in input.layers {
        let sheet = layer.sheet.as_ref().ok_or_else(|| invalid("BPP2D master column lacks a sheet type"))?;
        let sheet_values = (sheet.width.value, sheet.height.value, sheet.max_load.value, sheet.is_main);
        if sheet.type_code.as_str().is_empty() || !positive(sheet.width.value) || !positive(sheet.height.value)
            || !positive(sheet.area().value) || !sheet.max_load.value.is_finite() || sheet.max_load.value < 0.0
            || !positive(layer.shape.width.value) || !positive(layer.shape.height.value)
            || !positive(layer.shape.area().value) || layer.units.is_empty() {
            return Err(invalid("BPP2D column has invalid sheet, envelope or empty placement data"));
        }
        if let Some(previous) = sheets.insert(sheet.type_code.as_str(), sheet_values) {
            if previous != sheet_values { return Err(invalid("BPP2D sheet identifier has conflicting definitions")); }
        }
        for placement in &layer.units {
            let id = placement.item_id.as_deref().filter(|id| !id.is_empty()).ok_or_else(|| invalid("BPP2D placement lacks an item identifier"))?;
            let width = placement.width().value;
            let height = placement.height().value;
            let x = placement.position.x.value;
            let y = placement.position.y.value;
            if !positive(width) || !positive(height) || !positive(placement.area().value)
                || !x.is_finite() || !y.is_finite() || !(x + width).is_finite() || !(y + height).is_finite()
                || x + width <= x || y + height <= y {
                return Err(invalid("BPP2D placement has non-finite, non-positive or collapsed geometry"));
            }
            let (original_width, original_height) = if placement.rotated { (height, width) } else { (width, height) };
            let shape = (placement.shape.shape_type(), original_width, original_height);
            if let Some(previous) = shapes.insert(id, shape) {
                if previous != shape { return Err(invalid("BPP2D item identifier has conflicting shapes")); }
            }
        }
        PackingGeometryGuard::require_sheet_geometry(
            &sheet.geometry(),
            &layer.units,
            sheet.type_code.as_str(),
            "master registration",
            None,
        ).map_err(|error| invalid(error.to_string()))?;
        for demand in input.demands {
            let coefficient = layer_demand_coefficient(layer, demand);
            if !coefficient.is_finite() || coefficient < 0.0 { return Err(invalid("BPP2D coverage coefficient overflows")); }
        }
    }
    Ok(())
}

impl LayerAssignmentContext {
    /// 原子注册真实主问题。份数无人工总量上界；容量逐板校验，不限制所有板材的总面积。
    /// Atomically register an actual master. Copies have no artificial total bound; capacity is checked per sheet, not against the area of all sheets combined.
    ///
    /// 不继承旧 Load 的欠载默认值；只有 FeasibilityLp 显式允许人工短缺。刀缝、旋转权限、承重与目录认证仍由上游策略校验。
    /// Does not inherit legacy Load's default under-load flag; only FeasibilityLp explicitly permits artificial deficits. Kerf, rotation permission, load and catalog authentication remain upstream policy checks.
    pub fn register_model(
        &self,
        model: &mut MetaModel<f64>,
        input: LayerAssignmentModelInput<'_>,
    ) -> ospf_rust_core::Result<LayerAssignmentModel> {
        validate_input(&input)?;
        let config = &self.objective_config;
        if [config.sheet_area_weight, config.sheet_amount_weight, config.sheet_type_cost_weight, config.rmp_area_coefficient]
            .into_iter().chain(config.sheet_type_costs.values().copied()).any(|value| !value.is_finite() || value < 0.0) {
            return Err(invalid("BPP2D master requires finite nonnegative objective weights and costs"));
        }
        let terms = match input.stage {
            LayerAssignmentStage::Rmp => sheet_area_terms(input.layers, config.rmp_area_coefficient),
            LayerAssignmentStage::Integer => config.all_terms(input.layers),
            LayerAssignmentStage::FeasibilityLp => Vec::new(),
        };
        let mut coefficients = vec![0.0; input.layers.len()];
        for (index, coefficient) in terms { coefficients[index] += coefficient; }
        if coefficients.iter().any(|value| !value.is_finite()) { return Err(invalid("BPP2D objective coefficient overflows")); }
        model.transaction(|model| {
            let prefix = format!("bpp2d_{}", model.tokens().len());
            let copies = (0..input.layers.len()).map(|index| {
                let name = format!("{prefix}_copies_{index}");
                if input.stage == LayerAssignmentStage::Integer {
                    model.register_auto_variable_with_range::<UInteger>(&name, VariableRange::with_lower(0.0))
                } else {
                    model.register_auto_variable_with_range::<UContinuous>(&name, VariableRange::with_lower(0.0))
                }
            }).collect::<ospf_rust_core::Result<Vec<_>>>()?;
            let mut deficits = Vec::new();
            let mut lower_rows = Vec::new();
            let mut upper_rows = Vec::new();
            let mut objective = LinearObjectiveInput::minimize(format!("{prefix}_objective"));
            for (index, coefficient) in copies.iter().copied().zip(coefficients.iter().copied()) {
                objective = objective.term(index, coefficient);
            }
            for (index, demand) in input.demands.iter().enumerate() {
                let terms = input.layers.iter().zip(&copies).map(|(layer, variable)| (*variable, layer_demand_coefficient(layer, demand)))
                    .filter(|(_, coefficient)| *coefficient != 0.0).collect::<Vec<_>>();
                let lower = demand_value(demand.mode, &demand.demand)?;
                let mut lower_terms = terms.clone();
                if input.stage == LayerAssignmentStage::FeasibilityLp {
                    let deficit = model.register_auto_variable_with_range::<UContinuous>(
                        &format!("{prefix}_deficit_{index}"),
                        VariableRange::bounded(0.0, lower),
                    )?;
                    deficits.push(deficit);
                    lower_terms.push((deficit, 1.0));
                    objective = objective.term(deficit, if lower > 0.0 { 1.0 / lower } else { 0.0 });
                }
                lower_rows.push(model.constraints().len());
                model.add_ge_constraint(
                    &lower_terms,
                    lower,
                    &format!("{prefix}_{}", demand.constraint_name()),
                )?;
                upper_rows.push(if let Some(upper) = &demand.upper_bound {
                    let row = model.constraints().len();
                    model.add_le_constraint(
                        &terms,
                        demand_value(demand.mode, upper)?,
                        &format!("{prefix}_{}_upper", demand.constraint_name()),
                    )?;
                    Some(row)
                } else { None });
            }
            model.add_linear_objective_input(objective);
            let copy_ids = copies.iter().map(|index| model.tokens()[*index].id()).collect();
            let deficit_ids = deficits.iter().map(|index| model.tokens()[*index].id()).collect();
            let row_names = lower_rows.iter().copied().chain(upper_rows.iter().flatten().copied())
                .map(|row| (row, model.constraints()[row].name.clone())).collect();
            Ok(LayerAssignmentModel { model_identity: model.model_identity(), stage: input.stage, copies, copy_ids, deficits, deficit_ids, demands: input.demands.to_vec(), lower_rows, upper_rows, row_names })
        })
    }
}

impl LayerAssignmentModel {
    /// 已注册阶段。 / Registered stage.
    pub fn stage(&self) -> LayerAssignmentStage { self.stage }

    /// 与输入列严格同序的变量索引，可供额外 pipeline 使用。 / Variable indices in exact input-column order, available to extra pipelines.
    pub fn copy_variables(&self) -> &[usize] { &self.copies }

    /// 与需求同序，仅可行性阶段存在。 / In demand order, present only in the feasibility stage.
    pub fn deficit_variables(&self) -> &[usize] { &self.deficits }

    /// 原需求下界行，人工短缺也在此行。 / Original demand lower-bound rows, also containing artificial deficits.
    pub fn demand_rows(&self) -> &[usize] { &self.lower_rows }

    /// 独立业务上界行；短缺不会进入这些行。 / Separate business upper-bound rows; deficits never enter these rows.
    pub fn upper_bound_rows(&self) -> &[Option<usize>] { &self.upper_rows }

    pub(crate) fn require_model(&self, model: &MetaModel<f64>) -> ospf_rust_core::Result<()> {
        if model.model_identity() != self.model_identity { return Err(invalid("foreign BPP2D MetaModel")); }
        if self.copies.iter().zip(&self.copy_ids).any(|(index, id)| model.tokens().get(*index).map(|token| token.id()) != Some(*id))
            || self.deficits.iter().zip(&self.deficit_ids).any(|(index, id)| model.tokens().get(*index).map(|token| token.id()) != Some(*id))
            || self.row_names.iter().any(|(row, name)| model.constraints().get(*row).map(|constraint| &constraint.name) != Some(name)) {
            return Err(invalid("BPP2D registered variables or rows are no longer active"));
        }
        Ok(())
    }

    /// 从已复验的原始完整解提取份数，不 round、不截短、不展开大量板材。
    /// Extract copies from a revalidated complete original solution, without rounding, truncation or expanding large sheet collections.
    pub fn extract_copies(
        &self,
        model: &MetaModel<f64>,
        values: &BTreeMap<VariableId, f64>,
        tolerance: f64,
    ) -> ospf_rust_core::Result<Vec<f64>> {
        self.require_model(model)?;
        if !tolerance.is_finite() || !(0.0..0.5).contains(&tolerance) { return Err(invalid("invalid BPP2D extraction tolerance")); }
        model.linear_model_snapshot()?.validate_solution(values, &tolerance)?;
        self.copies.iter().map(|index| {
            model.tokens().get(*index).and_then(|token| values.get(&token.id())).copied().ok_or_else(|| invalid("missing BPP2D copy variable"))
        }).collect()
    }

    /// 按需求顺序提取原单位人工短缺；非可行性阶段返回空数组，仍复验完整解。
    /// Extract artificial deficits in demand order and original units; non-feasibility stages return an empty vector, still validating the complete solution.
    pub fn extract_deficits(
        &self,
        model: &MetaModel<f64>,
        values: &BTreeMap<VariableId, f64>,
        tolerance: f64,
    ) -> ospf_rust_core::Result<Vec<f64>> {
        self.require_model(model)?;
        if !tolerance.is_finite() || !(0.0..0.5).contains(&tolerance) { return Err(invalid("invalid BPP2D extraction tolerance")); }
        model.linear_model_snapshot()?.validate_solution(values, &tolerance)?;
        self.deficits.iter().map(|index| {
            model.tokens().get(*index).and_then(|token| values.get(&token.id())).copied().ok_or_else(|| invalid("missing BPP2D deficit variable"))
        }).collect()
    }

    /// 提取已按原始行还原的完整 LP 对偶；上下界对同一列系数的贡献相加。
    /// Extract complete LP duals already restored to original rows; lower/upper contributions for the same column coefficient are added.
    ///
    /// 调用前必须验证最优 LP 证书；此函数不认证对偶，也不包含额外 pipeline 的定价修正。
    /// Callers must validate an optimal LP certificate first; this function neither certifies duals nor includes extra-pipeline pricing corrections.
    pub fn extract_shadow_prices(
        &self,
        model: &MetaModel<f64>,
        duals: &[f64],
    ) -> ospf_rust_core::Result<Bpp2dDemandShadowPriceMap> {
        self.require_model(model)?;
        if self.stage == LayerAssignmentStage::Integer || duals.len() != model.constraints().len() || duals.iter().any(|value| !value.is_finite()) {
            return Err(invalid("BPP2D pricing requires complete finite LP duals in original row order"));
        }
        self.demands.iter().enumerate().map(|(index, demand)| {
            let value = duals[self.lower_rows[index]] + self.upper_rows[index].map(|row| duals[row]).unwrap_or(0.0);
            if !value.is_finite() { return Err(invalid("BPP2D combined shadow price overflows")); }
            Ok((Bpp2dDemandShadowPriceKey::new(demand.mode, demand.item_id.as_str()), value))
        }).collect()
    }
}

#[cfg(test)]
mod tests;
