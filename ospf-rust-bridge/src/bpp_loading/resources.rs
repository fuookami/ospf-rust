//! 预注册连续附加重量，复用原 context 的真实容量及整体占位联动。
//! Preregistered continuous additional weight, reusing the original context's true-capacity and whole-occupancy links.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::flatten::{Linear, LinearMonomial};
use ospf_rust_core::model::{ConstraintRelation, LinearInequality, MetaModel, SubObjective};
use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_bpp::DiscreteLoadingContext;
use crate::{PhysicalQuantity, Result, invalid};
use super::{BppLoadingProblem, identifier, mass};

/// 每舱位最多一个有界连续附加重量；成本按声明的模型重量单位计价。
/// At most one bounded continuous additional weight per position; costs are priced in the declared model weight unit.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingResource {
    /// 稳定资源身份。 / Stable resource identity.
    pub id: String,
    /// 原源目录中的舱位。 / Position in the original source catalog.
    pub position_id: String,
    /// 带单位非负下界。 / Unit-bearing nonnegative lower bound.
    pub minimum_weight: PhysicalQuantity,
    /// 带单位有限上界。 / Unit-bearing finite upper bound.
    pub maximum_weight: PhysicalQuantity,
    /// 每单位货物重量所需的最少附加重量，无量纲且非负。 / Minimum additional weight per cargo weight, dimensionless and nonnegative.
    pub minimum_per_loaded_weight: f64,
    /// 每模型重量单位的有限最小化成本，负值表示奖励。 / Finite minimizing cost per model weight unit; negative values are rewards.
    pub cost_per_weight: f64,
}

/// 附加重量与含附加量的总重量；原 loadedWeight 始终只计货物。
/// Additional weights and inclusive totals; the original loadedWeight always counts cargo only.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingResourceResult {
    /// 每资源实际附加重量，包含零值。 / Actual additional weight per resource, including zero values.
    pub weights: BTreeMap<String, PhysicalQuantity>,
    /// 每舱位货物加资源重量，包括没有资源的舱位。 / Cargo plus resource weight per position, including positions without resources.
    pub position_total_weights: BTreeMap<String, PhysicalQuantity>,
    /// 所有舱位货物加资源重量。 / Cargo plus resource weight over all positions.
    pub total_weight: PhysicalQuantity,
}

pub(super) struct LoadingResources {
    specifications: Vec<BppLoadingResource>,
    columns: BTreeMap<String, usize>,
}

impl LoadingResources {
    pub(super) fn column(&self, id: &str) -> Option<usize> { self.columns.get(id).copied() }

    pub(super) fn specifications(&self) -> &[BppLoadingResource] { &self.specifications }

    pub(super) fn new(problem: &BppLoadingProblem, specifications: Option<Vec<BppLoadingResource>>) -> Result<Self> {
        if specifications.as_ref().is_some_and(|values| values.is_empty() || values.len() > 128) {
            return Err(invalid("BPP loading resources require 1..128 entries when provided"));
        }
        let specifications = specifications.unwrap_or_default();
        let mut ids = BTreeSet::new(); let mut positions = BTreeSet::new();
        for resource in &specifications {
            identifier(&resource.id)?; identifier(&resource.position_id)?;
            if !ids.insert(&resource.id) || !positions.insert(&resource.position_id)
                || !problem.positions.iter().any(|position| position.id == resource.position_id)
                || !resource.minimum_per_loaded_weight.is_finite() || resource.minimum_per_loaded_weight < 0.0 || !resource.cost_per_weight.is_finite() {
                return Err(invalid("BPP loading resources require unique known positions/identities and finite coefficients"));
            }
            let lower = Self::normalize(problem, &resource.minimum_weight)?;
            let upper = Self::normalize(problem, &resource.maximum_weight)?;
            if lower > upper { return Err(invalid("BPP loading resource lower weight exceeds its upper weight")); }
        }
        Ok(Self { specifications, columns: BTreeMap::new() })
    }

    fn normalize(problem: &BppLoadingProblem, value: &PhysicalQuantity) -> Result<f64> {
        Ok(mass(value, problem.weight_unit)?.to_unit(&problem.weight_unit.unit()).map_err(invalid)?.value)
    }

    pub(super) fn register(
        &mut self,
        problem: &BppLoadingProblem,
        context: &mut DiscreteLoadingContext<f64, Unit>,
        model: &mut MetaModel<f64>,
        namespace: &str,
    ) -> Result<()> {
        for (i, resource) in self.specifications.iter().enumerate() {
            let column = model.add_variable(ContinuousVariableItem::auto_with_range(&format!("{namespace}_resource_{i}"),
                VariableRange::bounded(Self::normalize(problem, &resource.minimum_weight)?, Self::normalize(problem, &resource.maximum_weight)?))).map_err(invalid)?;
            let mut terms = vec![LinearMonomial::new(1.0, column)];
            for term in context.aggregation().weights[&resource.position_id].monomials() {
                let coefficient = -resource.minimum_per_loaded_weight * term.coefficient();
                if !coefficient.is_finite() || (resource.minimum_per_loaded_weight != 0.0 && *term.coefficient() != 0.0 && coefficient == 0.0) {
                    return Err(invalid("BPP loading resource coupling coefficient overflow or underflow"));
                }
                terms.push(LinearMonomial::new(coefficient, term.var_index()));
            }
            model.register_linear_row(&format!("{namespace}_resource_minimum_{i}"), LinearInequality::new(Linear::new(terms, 0.0),
                ConstraintRelation::GreaterEqual, 0.0)).map_err(invalid)?;
            let additional = Linear::new(vec![LinearMonomial::new(1.0, column)], 0.0);
            context.link_additional_weight(model, &resource.position_id, additional.clone()).map_err(invalid)?;
            model.add_sub_objective(SubObjective::minimize(Linear::new(vec![LinearMonomial::new(resource.cost_per_weight, column)], 0.0),
                &format!("{namespace}_resource_cost_{i}")));
            self.columns.insert(resource.id.clone(), column);
        }
        Ok(())
    }

    pub(super) fn fill_values(
        &self,
        problem: &BppLoadingProblem,
        inputs: &BTreeMap<String, PhysicalQuantity>,
        values: &mut [f64],
    ) -> Result<BTreeMap<String, f64>> {
        if inputs.len() != self.specifications.len() || inputs.keys().any(|id| !self.columns.contains_key(id)) {
            return Err(invalid("BPP loading plans require exactly one explicit weight for every declared resource"));
        }
        let mut by_position = BTreeMap::new();
        for resource in &self.specifications {
            let weight = Self::normalize(problem, &inputs[&resource.id])?;
            values[self.columns[&resource.id]] = weight;
            by_position.insert(resource.position_id.clone(), weight);
        }
        Ok(by_position)
    }

    pub(super) fn result(
        &self,
        problem: &BppLoadingProblem,
        values: &[f64],
        cargo_weights: &BTreeMap<String, f64>,
    ) -> Result<Option<BppLoadingResourceResult>> {
        if self.specifications.is_empty() { return Ok(None); }
        let quantity = |value| PhysicalQuantity { value, unit: problem.weight_unit };
        let mut positions = cargo_weights.clone(); let mut weights = BTreeMap::new();
        for resource in &self.specifications {
            let weight = values[self.columns[&resource.id]];
            *positions.get_mut(&resource.position_id).ok_or_else(|| invalid("unknown resource result position"))? += weight;
            weights.insert(resource.id.clone(), quantity(weight));
        }
        let total = positions.values().sum::<f64>();
        if !total.is_finite() || positions.values().any(|weight| !weight.is_finite()) { return Err(invalid("BPP loading inclusive resource weight overflow")); }
        Ok(Some(BppLoadingResourceResult { weights, position_total_weights: positions.into_iter().map(|(id, weight)| (id, quantity(weight))).collect(), total_weight: quantity(total) }))
    }
}

#[cfg(test)]
mod tests;
