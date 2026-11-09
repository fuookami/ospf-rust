//! 预注册的最小化业务成本，不接受跨进程回调或裸变量索引。 / Preregistered minimizing business costs, without cross-process callbacks or raw variable indices.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::flatten::{Linear, LinearMonomial};
use ospf_rust_core::model::{MetaModel, SubObjective};
use ospf_rust_framework_bpp::LoadingAggregation;
use crate::{Result, invalid};
use super::BppLoadingProblem;

/// 显式布局选择成本；同一组／布局不得重复。 / Explicit layout-selection cost, with no duplicate group/layout pairs.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingLayoutCost {
    /// 源选择组。 / Source selection group.
    pub group_id: String,
    /// 源布局。 / Source layout.
    pub layout_id: String,
    /// 有限最小化系数。 / Finite minimizing coefficient.
    pub cost: f64,
}

/// 货物／占位／布局的线性最小化成本；负成本表示奖励，不自动带入物理单位。
/// Linear minimizing cargo/occupancy/layout costs; negative costs are rewards, without implicit physical units.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingObjective {
    /// 已装载货物成本，未知身份拒绝。 / Loaded-cargo costs, rejecting unknown identities.
    pub item_costs: BTreeMap<String, f64>,
    /// 整体占位成本，空预留亦计费。 / Whole-position occupancy costs, including empty reservations.
    pub position_costs: BTreeMap<String, f64>,
    /// 显式选择布局成本。 / Explicit selected-layout costs.
    pub layout_costs: Vec<BppLoadingLayoutCost>,
}

impl BppLoadingObjective {
    pub(super) fn validate(&self, problem: &BppLoadingProblem) -> Result<()> {
        if self.item_costs.len() > 512 || self.position_costs.len() > 128 || self.layout_costs.len() > 512 {
            return Err(invalid("BPP loading objective exceeds source budgets"));
        }
        if self.item_costs.iter().any(|(id, cost)| !cost.is_finite() || !problem.items.iter().any(|item| &item.id == id))
            || self.position_costs.iter().any(|(id, cost)| !cost.is_finite() || !problem.positions.iter().any(|position| &position.id == id)) {
            return Err(invalid("BPP loading objective requires known identities and finite costs"));
        }
        let mut keys = std::collections::BTreeSet::new();
        for layout in &self.layout_costs {
            if !layout.cost.is_finite() || !keys.insert((&layout.group_id, &layout.layout_id)) || !problem.layout_groups.iter()
                .any(|group| group.id == layout.group_id && group.layouts.iter().any(|candidate| candidate.id == layout.layout_id)) {
                return Err(invalid("BPP loading objective requires unique known layouts and finite costs"));
            }
        }
        Ok(())
    }

    pub(super) fn register(&self, model: &mut MetaModel<f64>, aggregation: &LoadingAggregation<f64>) -> ospf_rust_core::Result<()> {
        let index = |variable| model.find_token(variable).map(|token| token.solver_index)
            .ok_or_else(|| ospf_rust_core::ModelError::InvalidConstraint("missing loading objective token".into()));
        let mut terms = Vec::new();
        for (key, variable) in &aggregation.assignments {
            if let Some(cost) = self.item_costs.get(&key.item_id) { terms.push(LinearMonomial::new(*cost, index(*variable)?)); }
        }
        for (id, cost) in &self.position_costs { terms.push(LinearMonomial::new(*cost, index(aggregation.occupied[id])?)); }
        for layout in &self.layout_costs {
            terms.push(LinearMonomial::new(layout.cost, index(aggregation.layouts[&(layout.group_id.clone(), layout.layout_id.clone())])?));
        }
        model.add_sub_objective(SubObjective::minimize(Linear::new(terms, 0.0), "loading-business-cost"));
        Ok(())
    }
}
