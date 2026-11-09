//! 广义流领域绑定：原 `network-scheduling` 的带产率与单位转换的最小费用流。
//! Generalized-flow domain binding: the original `network-scheduling` min-cost flow with production yield and unit
//! conversion.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework_network_scheduling::domain::generalized_flow::{
    GeneralizedFlowArc as DomainArc, GeneralizedFlowProblem as DomainProblem, GeneralizedFlowSolution as DomainSolution,
};
use ospf_rust_framework_network_scheduling::infrastructure::{CapacityBounds, NetworkArcId, NetworkCost};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::Unit;

use ospf_rust_core::solver::SolveReport;

use crate::network_flow::{summarize_domain_report, NetworkFlowQuantity};
use crate::quantities::{PhysicalQuantity, QuantityUnit};
use crate::{BridgeError, ReportSummary, Result};

/// 节点计量与硬净供给。 / Node measurement and hard net supply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralizedFlowNode {
    /// 稳定节点 ID。 / Stable node ID.
    pub id: String,
    /// 净供给；正数为供给，负数为需求。 / Net supply; positive is supply and negative is demand.
    pub net_supply: PhysicalQuantity,
}

/// 一条弧：有限投入范围、投入资费、无量纲产率与可选单位转换。
/// One arc: finite input bounds, input tariff, dimensionless yield and an optional unit conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralizedFlowArc {
    /// 稳定弧 ID。 / Stable arc ID.
    pub id: String,
    /// 投入节点。 / Input node.
    pub from: String,
    /// 产出到达节点。 / Arrival-output node.
    pub to: String,
    /// 按起点单位计的投入下界。 / Input lower bound in origin units.
    pub input_lower: PhysicalQuantity,
    /// 按起点单位计的投入上界。 / Input upper bound in origin units.
    pub input_upper: PhysicalQuantity,
    /// 每起点单位投入费用。 / Cost per origin-unit input.
    pub cost: PhysicalQuantity,
    /// 无量纲、非负产率。 / Dimensionless nonnegative yield.
    pub production_yield: PhysicalQuantity,
    /// 终点单位/起点单位转换；跨量纲时必须显式给出。 / Destination-per-origin conversion, required across dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub input_to_output_factor: Option<PhysicalQuantity>,
}

/// 广义流问题。 / A generalized-flow problem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralizedFlowProblem {
    /// 节点快照。 / Node snapshot.
    pub nodes: Vec<GeneralizedFlowNode>,
    /// 弧快照；至少一条。 / Arc snapshot; at least one.
    pub arcs: Vec<GeneralizedFlowArc>,
    /// 费用单位。 / Cost unit.
    pub cost_unit: QuantityUnit,
}

impl GeneralizedFlowProblem {
    /// 映射到原领域问题；身份引用、容量与产率都在这里校验。
    /// Map onto the original domain problem; identity references, capacities and yields are all checked here.
    pub fn to_domain(&self) -> Result<DomainProblem<f64>> {
        if self.arcs.is_empty() {
            return Err(BridgeError::new("InvalidInput", "generalized flow requires at least one arc"));
        }
        let mut node_ids = std::collections::HashSet::new();
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "generalized flow node ID must not be empty"));
            }
            if !node_ids.insert(node.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "generalized flow repeats a node ID"));
            }
            nodes.push(ospf_rust_framework_network_scheduling::domain::generalized_flow::GeneralizedFlowNode {
                id: node.id.as_str().into(), net_supply: node.net_supply.quantity()? });
        }
        let mut arc_ids = std::collections::HashSet::new();
        let mut arcs = Vec::with_capacity(self.arcs.len());
        for arc in &self.arcs {
            if arc.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "generalized flow arc ID must not be empty"));
            }
            if !arc_ids.insert(arc.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "generalized flow repeats an arc ID"));
            }
            for endpoint in [arc.from.as_str(), arc.to.as_str()] {
                if !node_ids.contains(endpoint) {
                    return Err(BridgeError::new("InvalidInput", "generalized flow arc references an unknown node"));
                }
            }
            let capacity = CapacityBounds::try_new(arc.input_lower.quantity()?, arc.input_upper.quantity()?)
                .map_err(|error| BridgeError::new("InvalidInput", &error))?;
            let yield_quantity = arc.production_yield.quantity()?;
            if yield_quantity.value < 0.0 || !yield_quantity.value.is_finite() {
                return Err(BridgeError::new("InvalidInput", "generalized flow production yield must be finite and nonnegative"));
            }
            arcs.push(DomainArc { id: arc.id.as_str().into(), from: arc.from.as_str().into(),
                to: arc.to.as_str().into(), input_capacity: capacity, cost: NetworkCost::new(arc.cost.quantity()?),
                production_yield: yield_quantity,
                input_to_output_factor: arc.input_to_output_factor.as_ref().map(PhysicalQuantity::quantity).transpose()? });
        }
        DomainProblem::new(nodes, arcs, self.cost_unit.unit())
            .map_err(|error| BridgeError::new("InvalidInput", &error))
    }
}

/// 一条弧的领域结果；投入、产出、转换差额与费用都分开保留。
/// Domain result of one arc; input, output, conversion difference and cost are kept separate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralizedFlowArcResult {
    /// 稳定弧 ID。 / Stable arc ID.
    pub id: String,
    /// 起点单位投入。 / Input in origin units.
    pub input: NetworkFlowQuantity,
    /// 终点单位到达产出。 / Arrival output in destination units.
    pub output: NetworkFlowQuantity,
    /// 转换后未计产率的产出减实际产出。 / Converted pre-yield output minus actual output.
    pub loss_or_gain: NetworkFlowQuantity,
    /// 弧费用。 / Arc cost.
    pub cost: NetworkFlowQuantity,
}

/// 一个节点的独立复核结果。 / Independently checked result of one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralizedFlowNodeResult {
    /// 稳定节点 ID。 / Stable node ID.
    pub id: String,
    /// 实际净供给。 / Actual net supply.
    pub actual_net_supply: NetworkFlowQuantity,
    /// 实际减声明净供给。 / Actual minus declared net supply.
    pub residual: NetworkFlowQuantity,
}

/// 广义流领域解。 / Generalized-flow domain solution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralizedFlowSolution {
    /// 按弧 ID 排序的弧结果。 / Arc results sorted by arc ID.
    pub arcs: Vec<GeneralizedFlowArcResult>,
    /// 按节点 ID 排序的节点结果。 / Node results sorted by node ID.
    pub nodes: Vec<GeneralizedFlowNodeResult>,
    /// 总费用。 / Total cost.
    pub total_cost: NetworkFlowQuantity,
}

/// 统一报告：通用摘要 + 广义流领域解。 / Unified report: shared summary plus the generalized-flow solution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralizedFlowReport {
    /// 通用求解摘要。 / Shared solve summary.
    pub summary: ReportSummary,
    /// 领域解；无 incumbent 时为空。 / The domain solution, absent without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<GeneralizedFlowSolution>,
}

fn quantity_of(quantity: &Quantity<f64, Unit>) -> Result<NetworkFlowQuantity> {
    if !quantity.value.is_finite() {
        return Err(BridgeError::new("InvalidReport", "generalized flow quantity is not finite"));
    }
    Ok(NetworkFlowQuantity { value: quantity.value, unit: quantity.unit.symbol().to_owned() })
}

fn arc_results(arcs: &BTreeMap<NetworkArcId, ospf_rust_framework_network_scheduling::domain::generalized_flow::GeneralizedFlowArcResult<f64>>)
    -> Result<Vec<GeneralizedFlowArcResult>> {
    arcs.iter().map(|(id, arc)| Ok(GeneralizedFlowArcResult { id: id.as_str().to_owned(), input: quantity_of(&arc.input)?,
        output: quantity_of(&arc.output)?, loss_or_gain: quantity_of(&arc.loss_or_gain)?,
        cost: quantity_of(&arc.cost)? })).collect()
}

/// 把原领域解展开成 DTO。 / Expand the domain solution into DTOs.
pub fn decode_generalized_flow_solution(solution: &DomainSolution<f64>) -> Result<GeneralizedFlowSolution> {
    Ok(GeneralizedFlowSolution {
        arcs: arc_results(&solution.arcs)?,
        nodes: solution.nodes.iter().map(|(id, node)| Ok(GeneralizedFlowNodeResult { id: id.as_str().to_owned(),
            actual_net_supply: quantity_of(&node.actual_net_supply)?, residual: quantity_of(&node.residual)? }))
            .collect::<Result<Vec<_>>>()?,
        total_cost: quantity_of(&solution.total_cost)?,
    })
}

/// 组装并复核报告；有最优性证明就必须有解。 / Assemble and re-check the report; a proof implies a solution.
pub fn summarize_generalized_flow_report(report: &SolveReport<DomainSolution<f64>>,
    solution: Option<GeneralizedFlowSolution>) -> Result<GeneralizedFlowReport> {
    let summary = summarize_domain_report(report, solution.is_some())?;
    Ok(GeneralizedFlowReport { summary, solution })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
    fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

    /// `a` 供 10、`b` 需 8；弧产率 0.8，因此 10 投入 → 8 产出且无损耗差额。
    /// `a` supplies 10 and `b` demands 8; the arc yields 0.8, so 10 in gives 8 out with no loss.
    fn problem() -> GeneralizedFlowProblem {
        GeneralizedFlowProblem {
            nodes: vec![GeneralizedFlowNode { id: "a".into(), net_supply: mass(10.0) },
                GeneralizedFlowNode { id: "b".into(), net_supply: mass(-8.0) }],
            arcs: vec![GeneralizedFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
                input_lower: mass(0.0), input_upper: mass(20.0), cost: pure(1.0),
                production_yield: pure(0.8), input_to_output_factor: Some(pure(1.0)) }],
            cost_unit: QuantityUnit::Kilogram,
        }
    }

    #[test]
    fn generalized_flow_problems_reject_bad_references_capacities_and_yields() {
        problem().to_domain().unwrap();
        let broken: Vec<(&str, GeneralizedFlowProblem)> = vec![
            ("no arcs", GeneralizedFlowProblem { arcs: Vec::new(), ..problem() }),
            ("empty node ID", GeneralizedFlowProblem { nodes: vec![GeneralizedFlowNode { id: String::new(), net_supply: mass(1.0) }], ..problem() }),
            ("unknown endpoint", GeneralizedFlowProblem { arcs: vec![GeneralizedFlowArc { to: "ghost".into(), ..problem().arcs[0].clone() }], ..problem() }),
            ("lower above upper", GeneralizedFlowProblem { arcs: vec![GeneralizedFlowArc { input_lower: mass(5.0), input_upper: mass(1.0), ..problem().arcs[0].clone() }], ..problem() }),
            ("negative yield", GeneralizedFlowProblem { arcs: vec![GeneralizedFlowArc { production_yield: pure(-0.5), ..problem().arcs[0].clone() }], ..problem() }),
            ("non-finite supply", GeneralizedFlowProblem { nodes: vec![GeneralizedFlowNode { id: "a".into(), net_supply: PhysicalQuantity { value: f64::NAN, unit: QuantityUnit::Kilogram } },
                GeneralizedFlowNode { id: "b".into(), net_supply: mass(-8.0) }], ..problem() }),
        ];
        for (label, case) in broken {
            assert!(case.to_domain().is_err(), "{label} must be rejected");
        }
        let mut repeated = problem();
        repeated.arcs.push(problem().arcs[0].clone());
        assert!(repeated.to_domain().is_err());
    }
}
