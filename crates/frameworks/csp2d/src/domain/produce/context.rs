//! 报告驱动路径共用的事务性主问题注册，不替换旧启发式。 / Transactional master registration shared by report-driven paths, without replacing legacy heuristics.

use std::collections::{BTreeMap, BTreeSet};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::{LinearObjectiveInput, MetaModel};
use ospf_rust_core::variable::{UContinuous, UInteger, VariableId, VariableRange};
use crate::domain::material::{CuttingPlan2, DemandMode2, DemandValue2};
use crate::domain::produce::{Csp2dMasterDemand, Csp2dMasterProblem};

pub(crate) mod geometry;

pub(super) fn invalid(message: impl Into<String>) -> ospf_rust_core::error::CoreError {
    ospf_rust_core::error::SolverError::ContractViolation(message.into()).into()
}

/// 显式原生阶段，三阶段共享需求上下界及物料库存行。 / Explicit native stage, sharing demand bounds and material-stock rows across all stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProduceStage {
    /// 连续受限主问题。 / Continuous restricted master.
    Rmp,
    /// 连续归一化人工短缺，仅放松需求下界。 / Continuous normalized artificial deficits, relaxing only demand lower bounds.
    FeasibilityLp,
    /// 整数使用次数。 / Integer usage counts.
    Integer,
}

/// 归一为米／平方米后的边界权重，不改变旧 service 默认值。 / Boundary weights after meter/square-meter normalization, without changing legacy service defaults.
#[derive(Debug, Clone)]
pub struct ProduceObjective {
    /// 料域消耗面积权重，默认 1。 / Consumed-region area weight, default 1.
    pub area_weight: f64,
    /// 料域内净余料面积权重，默认 0。 / Net waste area within the region, default 0.
    pub waste_weight: f64,
    /// 仅卷材段高权重，定尺列不贡献，默认 0。 / Coil segment-height weight only, with no contribution from sheet columns, default 0.
    pub height_weight: f64,
    /// 每次方案使用的权重，默认 0。 / Weight per pattern use, default 0.
    pub usage_weight: f64,
}

impl Default for ProduceObjective {
    fn default() -> Self { Self { area_weight: 1.0, waste_weight: 0.0, height_weight: 0.0, usage_weight: 0.0 } }
}

impl ProduceObjective {
    /// 用原方案面积／余料／段高函数计算一次使用成本，Phase I 另用零真实列成本。
    /// Compute per-use cost with original plan area/waste/height functions; Phase I instead uses zero real-column cost.
    pub fn column_cost(&self, plan: &CuttingPlan2<f64, Meter>) -> ospf_rust_core::Result<f64> {
        if [self.area_weight, self.waste_weight, self.height_weight, self.usage_weight].into_iter().any(|value| !value.is_finite() || value < 0.0) {
            return Err(invalid("CSP2D objective weights must be finite and nonnegative"));
        }
        let cost = plan.consumed_area().value * self.area_weight + plan.waste_area().value * self.waste_weight
            + plan.consumed_height().map(|height| height.value).unwrap_or(0.0) * self.height_weight + self.usage_weight;
        if !cost.is_finite() || cost < 0.0 { return Err(invalid("CSP2D objective coefficient overflows")); }
        Ok(cost)
    }
}

/// 原生建模边界；源产品尺寸及旋转权限目录必须另由调用方认证。 / Native modeling boundary; source product dimensions/rotation permissions still require caller-side catalog authentication.
pub struct ProduceModelInput<'a> {
    /// 完整物理候选及需求、库存，保留全部列位置。 / Complete physical candidates, demands and stocks, preserving every column position.
    pub master: &'a Csp2dMasterProblem,
    /// 明确选择 LP、Phase I 或整数阶段。 / Explicit LP, Phase I or integer stage.
    pub stage: ProduceStage,
    /// 原几何口径的非负米制刀缝。 / Nonnegative meter-based kerf with original geometry semantics.
    pub kerf: &'a Quantity<f64, Meter>,
}

/// 需求身份包含口径，不把同产品的件数与面积对偶覆盖。 / Demand identity includes the basis, never overwriting count/area duals of the same product.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Csp2dDemandShadowPriceKey {
    /// 产品身份。 / Product identity.
    pub product_id: String,
    /// 件数或面积口径。 / Count or area basis.
    pub mode: DemandMode2,
}

/// 已还原原始行的价格，不是对偶证书或额外 pipeline 定价修正。 / Prices restored to original rows, not a dual certificate or extra-pipeline pricing correction.
#[derive(Debug, Clone)]
pub struct Csp2dMasterShadowPrices {
    /// 需求上下界贡献之和。 / Combined demand lower/upper-bound contributions.
    pub demands: BTreeMap<Csp2dDemandShadowPriceKey, f64>,
    /// 每次列使用消耗一个原领域批次，库存行贡献不能丢弃。 / Each column use consumes one original-domain batch; stock-row contributions must not be discarded.
    pub materials: BTreeMap<String, f64>,
}

/// 同一主问题口径的定价输入，不接受旧覆盖缺口估计价格代替真实 LP 对偶。 / Pricing input using the same master basis, not substituting legacy deficit estimates for actual LP duals.
pub struct ProducePricingInput<'a> {
    /// 已由源目录及几何策略验证的候选，方法本身不认证源。 / Candidate already validated by source-catalog/geometry policies; this method itself does not authenticate the source.
    pub candidate: &'a CuttingPlan2<f64, Meter>,
    /// 原主问题全部需求，含独立件数／面积口径。 / All original master demands with distinct count/area bases.
    pub demands: &'a [Csp2dMasterDemand],
    /// 经证书验证的原始 LP 行价格，含库存贡献。 / Certificate-validated original-LP-row prices including stock contributions.
    pub prices: &'a Csp2dMasterShadowPrices,
    /// Phase I 使用零真实列成本，RMP 使用配置成本，不允许整数阶段。 / Phase I uses zero real-column cost, RMP uses configured cost, and integer stages are rejected.
    pub stage: ProduceStage,
}

/// 共享主问题 context，附加业务 pipeline 可在注册后继续追加。 / Shared master context, allowing additional business pipelines after registration.
#[derive(Debug, Clone, Default)]
pub struct ProduceContext {
    /// RMP 与整数模型共用目标；Phase I 仅最小化人工短缺。 / Shared RMP/integer objective; Phase I minimizes only artificial deficits.
    pub objective: ProduceObjective,
}

/// 已注册模型身份与原始变量／行引用，不拥有或替换 MetaModel。 / Registered model identity and original variable/row references, without owning or replacing MetaModel.
#[derive(Debug, Clone)]
pub struct ProduceModel {
    model_identity: u64,
    stage: ProduceStage,
    usages: Vec<usize>,
    deficits: Vec<usize>,
    variable_ids: Vec<(usize, VariableId)>,
    lower_rows: Vec<usize>,
    upper_rows: Vec<Option<usize>>,
    material_rows: BTreeMap<String, usize>,
    row_names: Vec<(usize, String)>,
    demands: Vec<Csp2dMasterDemand>,
}

fn exact_count(value: u64) -> ospf_rust_core::Result<f64> {
    if value > 9_007_199_254_740_991 { return Err(invalid("CSP2D solver integer is outside 0..=2^53-1")); }
    Ok(value as f64)
}

fn demand_value(mode: DemandMode2, value: &DemandValue2) -> ospf_rust_core::Result<f64> {
    match (mode, value) {
        (DemandMode2::Amount, DemandValue2::Amount(value)) => exact_count(*value),
        (DemandMode2::Area, DemandValue2::Area(value)) if value.is_finite() && *value >= 0.0 => Ok(*value),
        _ => Err(invalid("CSP2D demand mode/value mismatch or invalid area")),
    }
}

impl ProduceContext {
    /// 计算完整默认行的检验数；附加 pipeline 必须另提供对应修正，不能宣称本方法覆盖。
    /// Compute reduced cost for all default rows; extra pipelines need separate matching corrections and are not covered by this method.
    pub fn reduced_cost(&self, input: ProducePricingInput<'_>) -> ospf_rust_core::Result<f64> {
        if input.stage == ProduceStage::Integer || input.demands.is_empty() { return Err(invalid("CSP2D pricing requires LP stage and original demands")); }
        let projection = super::master_problem::PlanCoverage::new(input.candidate, input.demands.iter().any(|demand| demand.mode == DemandMode2::Area));
        let mut reduced = if input.stage == ProduceStage::FeasibilityLp { 0.0 } else { self.objective.column_cost(input.candidate)? };
        let mut keys = BTreeSet::new();
        for demand in input.demands {
            demand_value(demand.mode, &demand.demand)?;
            let key = Csp2dDemandShadowPriceKey { product_id: demand.product_id.as_str().into(), mode: demand.mode };
            if !keys.insert(key.clone()) { return Err(invalid("CSP2D pricing contains duplicate demand bases")); }
            let price = input.prices.demands.get(&key).filter(|value| value.is_finite()).ok_or_else(|| invalid("CSP2D pricing lacks a finite demand-row price"))?;
            let coefficient = projection.coefficient(demand);
            if !coefficient.is_finite() || coefficient < 0.0 { return Err(invalid("CSP2D pricing coefficient is nonfinite or negative")); }
            reduced -= coefficient * price;
        }
        let stock = input.prices.materials.get(input.candidate.pattern.material.id.as_str()).filter(|value| value.is_finite())
            .ok_or_else(|| invalid("CSP2D pricing lacks the candidate material's stock-row price"))?;
        reduced -= stock;
        if !reduced.is_finite() { return Err(invalid("CSP2D reduced cost overflows")); }
        Ok(reduced)
    }

    /// 事务注册真实主问题，库存始终是硬约束；不继承启发式的总件数裁剪上界。
    /// Transactionally register the actual master with hard stock constraints, without heuristic total-piece clipping bounds.
    pub fn register_model(
        &self,
        model: &mut MetaModel<f64>,
        input: ProduceModelInput<'_>,
    ) -> ospf_rust_core::Result<ProduceModel> {
        let master = input.master;
        if master.demands.is_empty() || master.assignment.usage_upper_bounds.len() != master.assignment.plans.len() {
            return Err(invalid("CSP2D master requires demands and a consistent plan vector"));
        }
        geometry::validate(&master.assignment.plans, &master.material_availability, input.kerf)?;
        let mut keys = BTreeSet::new();
        for demand in &master.demands {
            if demand.product_id.as_str().trim().is_empty() || !keys.insert((demand.product_id.as_str(), demand.mode)) {
                return Err(invalid("CSP2D demand identifier is empty or duplicated for its basis"));
            }
            let lower = demand_value(demand.mode, &demand.demand)?;
            if input.stage == ProduceStage::FeasibilityLp && lower > 0.0 && !(1.0 / lower).is_finite() { return Err(invalid("CSP2D deficit normalization overflows")); }
            if let Some(upper) = &demand.upper_bound {
                if demand_value(demand.mode, upper)? < lower { return Err(invalid("CSP2D demand upper bound is below its lower bound")); }
            }
        }
        for (material, count) in &master.material_availability {
            if material.trim().is_empty() { return Err(invalid("CSP2D material identifier is empty")); }
            exact_count(*count)?;
        }
        if [self.objective.area_weight, self.objective.waste_weight, self.objective.height_weight, self.objective.usage_weight]
            .into_iter().any(|value| !value.is_finite() || value < 0.0) { return Err(invalid("invalid CSP2D objective weights")); }
        let costs = master.assignment.plans.iter().map(|plan| self.objective.column_cost(plan)).collect::<ospf_rust_core::Result<Vec<_>>>()?;
        let coefficients = master.demands.iter().map(|demand| (0..master.assignment.len()).map(|index| {
            let value = master.assignment.coverage_coefficient(index, demand);
            if !value.is_finite() || value < 0.0 { return Err(invalid("CSP2D coverage coefficient overflows")); }
            Ok(value)
        }).collect::<ospf_rust_core::Result<Vec<_>>>()).collect::<ospf_rust_core::Result<Vec<_>>>()?;
        model.transaction(|model| {
            let prefix = format!("csp2d_{}", model.tokens().len());
            let usages = (0..master.assignment.len()).map(|index| {
                let name = format!("{prefix}_usage_{index}");
                if input.stage == ProduceStage::Integer { model.register_auto_variable_with_range::<UInteger>(&name, VariableRange::with_lower(0.0)) }
                else { model.register_auto_variable_with_range::<UContinuous>(&name, VariableRange::with_lower(0.0)) }
            }).collect::<ospf_rust_core::Result<Vec<_>>>()?;
            let mut objective = LinearObjectiveInput::minimize(format!("{prefix}_objective"));
            if input.stage != ProduceStage::FeasibilityLp {
                for (&index, &cost) in usages.iter().zip(&costs) { objective = objective.term(index, cost); }
            }
            let mut deficits = Vec::new();
            let mut lower_rows = Vec::new();
            let mut upper_rows = Vec::new();
            for (index, demand) in master.demands.iter().enumerate() {
                let terms = usages.iter().copied().zip(coefficients[index].iter().copied()).filter(|(_, value)| *value != 0.0).collect::<Vec<_>>();
                let lower = demand_value(demand.mode, &demand.demand)?;
                let mut lower_terms = terms.clone();
                if input.stage == ProduceStage::FeasibilityLp {
                    let deficit = model.register_auto_variable_with_range::<UContinuous>(&format!("{prefix}_deficit_{index}"), VariableRange::bounded(0.0, lower))?;
                    deficits.push(deficit);
                    lower_terms.push((deficit, 1.0));
                    objective = objective.term(deficit, if lower > 0.0 { 1.0 / lower } else { 0.0 });
                }
                lower_rows.push(model.constraints().len());
                model.add_ge_constraint(&lower_terms, lower, &format!("{prefix}_demand_{index}"))?;
                upper_rows.push(if let Some(upper) = &demand.upper_bound {
                    let row = model.constraints().len();
                    model.add_le_constraint(&terms, demand_value(demand.mode, upper)?, &format!("{prefix}_demand_{index}_upper"))?;
                    Some(row)
                } else { None });
            }
            let mut material_rows = BTreeMap::new();
            for (index, (id, count)) in master.material_availability.iter().enumerate() {
                let terms = master.assignment.plans.iter().zip(&usages).filter(|(plan, _)| plan.pattern.material.id.as_str() == id)
                    .map(|(_, &variable)| (variable, 1.0)).collect::<Vec<_>>();
                material_rows.insert(id.clone(), model.constraints().len());
                model.add_le_constraint(&terms, exact_count(*count)?, &format!("{prefix}_stock_{index}"))?;
            }
            model.add_linear_objective_input(objective);
            let variable_ids = usages.iter().chain(&deficits).map(|&index| (index, model.tokens()[index].id())).collect();
            let row_names = lower_rows.iter().copied().chain(upper_rows.iter().flatten().copied()).chain(material_rows.values().copied())
                .map(|row| (row, model.constraints()[row].name.clone())).collect();
            Ok(ProduceModel { model_identity: model.model_identity(), stage: input.stage, usages, deficits, variable_ids,
                lower_rows, upper_rows, material_rows, row_names, demands: master.demands.clone() })
        })
    }
}

impl ProduceModel {
    /// 注册阶段。 / Registered stage.
    pub fn stage(&self) -> ProduceStage { self.stage }

    /// 与完整池同序的使用次数变量。 / Usage variables in complete-pool order.
    pub fn usage_variables(&self) -> &[usize] { &self.usages }

    /// 仅 Phase I 存在的原需求单位短缺变量。 / Original-demand-unit deficits present only in Phase I.
    pub fn deficit_variables(&self) -> &[usize] { &self.deficits }

    /// 与需求同序的原始下界行。 / Original lower-bound rows in demand order.
    pub fn demand_rows(&self) -> &[usize] { &self.lower_rows }

    /// 不受短缺放松的业务上界行。 / Business upper-bound rows never relaxed by deficits.
    pub fn upper_bound_rows(&self) -> &[Option<usize>] { &self.upper_rows }

    /// 每次列使用消耗一个批次的库存行。 / Stock rows consuming one batch per column use.
    pub fn material_rows(&self) -> &BTreeMap<String, usize> { &self.material_rows }

    pub(crate) fn require_model(&self, model: &MetaModel<f64>) -> ospf_rust_core::Result<()> {
        if model.model_identity() != self.model_identity || self.variable_ids.iter().any(|(index, id)| model.tokens().get(*index).map(|token| token.id()) != Some(*id))
            || self.row_names.iter().any(|(row, name)| model.constraints().get(*row).map(|constraint| &constraint.name) != Some(name)) {
            return Err(invalid("CSP2D registration belongs to a foreign or invalidated MetaModel"));
        }
        Ok(())
    }

    fn extract(
        &self,
        model: &MetaModel<f64>,
        values: &BTreeMap<VariableId, f64>,
        tolerance: f64,
        indices: &[usize],
    ) -> ospf_rust_core::Result<Vec<f64>> {
        self.require_model(model)?;
        if !tolerance.is_finite() || !(0.0..0.5).contains(&tolerance) { return Err(invalid("invalid CSP2D extraction tolerance")); }
        model.linear_model_snapshot()?.validate_solution(values, &tolerance)?;
        indices.iter().map(|&index| values.get(&model.tokens()[index].id()).copied().ok_or_else(|| invalid("missing CSP2D registered variable"))).collect()
    }

    /// 复验完整原快照后提取使用次数，不 round、不截短。 / Extract usages after validating the complete original snapshot, without rounding or truncation.
    pub fn extract_usages(
        &self,
        model: &MetaModel<f64>,
        values: &BTreeMap<VariableId, f64>,
        tolerance: f64,
    ) -> ospf_rust_core::Result<Vec<f64>> { self.extract(model, values, tolerance, &self.usages) }

    /// 复验完整解后提取原单位短缺；其他阶段为空。 / Extract original-unit deficits after complete validation; empty in other stages.
    pub fn extract_deficits(
        &self,
        model: &MetaModel<f64>,
        values: &BTreeMap<VariableId, f64>,
        tolerance: f64,
    ) -> ospf_rust_core::Result<Vec<f64>> { self.extract(model, values, tolerance, &self.deficits) }

    /// 提取完整原始 LP 行价格；调用前必须验证最优 LP 证书，不包含附加 pipeline 的修正。
    /// Extract complete original-LP-row prices; requires a previously validated optimal LP certificate and excludes extra-pipeline corrections.
    pub fn extract_shadow_prices(&self, model: &MetaModel<f64>, duals: &[f64]) -> ospf_rust_core::Result<Csp2dMasterShadowPrices> {
        self.require_model(model)?;
        if self.stage == ProduceStage::Integer || duals.len() != model.constraints().len() || duals.iter().any(|value| !value.is_finite()) {
            return Err(invalid("CSP2D pricing requires complete finite original-row LP duals"));
        }
        let demands = self.demands.iter().enumerate().map(|(index, demand)| {
            let price = duals[self.lower_rows[index]] + self.upper_rows[index].map(|row| duals[row]).unwrap_or(0.0);
            if !price.is_finite() { return Err(invalid("CSP2D demand shadow price overflows")); }
            Ok((Csp2dDemandShadowPriceKey { product_id: demand.product_id.as_str().into(), mode: demand.mode }, price))
        }).collect::<ospf_rust_core::Result<_>>()?;
        Ok(Csp2dMasterShadowPrices { demands, materials: self.material_rows.iter().map(|(id, &row)| (id.clone(), duals[row])).collect() })
    }
}

#[cfg(test)]
mod tests;
