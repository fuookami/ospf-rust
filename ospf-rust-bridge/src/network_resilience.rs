//! 韧性领域绑定：原 `network-scheduling` 的显式故障场景 + 名义图连通要求。
//! Resilience domain binding: the original `network-scheduling` explicit failure scenarios plus nominal-graph
//! connectivity requirements.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework_network_scheduling::domain::flow::FlowBalanceSlack;
use ospf_rust_framework_network_scheduling::domain::resilience::{
    ConnectivityKind, ConnectivityRequirement, FailureScenario, NetworkResilienceProblem as DomainProblem,
    NetworkResilienceSolution as DomainSolution,
};
use ospf_rust_framework_network_scheduling::infrastructure::NetworkCost;
use ospf_rust_core::solver::SolveReport;

use crate::network_design::{decode_construction, NetworkConstruction, NetworkDesignProblem};
use crate::network_flow::{decode_network_flow_solution, summarize_domain_report, NetworkFlowNodeQuantity,
    NetworkFlowQuantity, NetworkFlowSolution};
use crate::quantities::{PhysicalQuantity, QuantityUnit};
use crate::{BridgeError, ReportSummary, Result};

/// 不相交合同。 / The disjointness contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ResilienceDisjointness {
    /// 弧不相交。 / Edge-disjoint.
    EdgeDisjoint,
    /// 内部节点不相交。 / Internally node-disjoint.
    NodeDisjoint,
}

/// 名义建设图上的业务连通要求。 / A business connectivity requirement on the nominal construction graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceConnectivity {
    /// 起点。 / Source.
    pub source: String,
    /// 终点。 / Target.
    pub target: String,
    /// 路径数量，必须为正。 / Positive path count.
    pub paths: u32,
    /// 不相交合同。 / Disjointness contract.
    pub kind: ResilienceDisjointness,
}

/// 某条弧上的降级上界。 / A degraded upper bound on one arc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceArcQuantity {
    /// 弧 ID。 / Arc ID.
    pub arc: String,
    /// 物理量；不能超过名义值。 / Physical amount, never above the nominal value.
    pub quantity: PhysicalQuantity,
}

/// 一个商品的完整节点供需替换。 / Complete node-balance replacement for one commodity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceCommodityBalances {
    /// 商品 ID。 / Commodity ID.
    pub commodity: String,
    /// 该商品的完整供需表。 / The complete supply-demand map for this commodity.
    pub balances: Vec<NetworkFlowNodeQuantity>,
}

/// 一个节点上的一组松弛。 / One set of slacks at a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceNodeSlack {
    /// 节点 ID。 / Node ID.
    pub node: String,
    /// 最大弃供量。 / Maximum spilled supply.
    pub spill_limit: PhysicalQuantity,
    /// 最大缺货量。 / Maximum unmet demand.
    pub shortage_limit: PhysicalQuantity,
    /// 每单位弃供费用。 / Penalty per unit of spill.
    pub spill_cost: PhysicalQuantity,
    /// 每单位缺货费用。 / Penalty per unit of shortage.
    pub shortage_cost: PhysicalQuantity,
}

/// 一个商品的完整松弛替换。 / Complete slack replacement for one commodity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceCommoditySlacks {
    /// 商品 ID。 / Commodity ID.
    pub commodity: String,
    /// 该商品的完整松弛表。 / The complete slack map for this commodity.
    pub slacks: Vec<ResilienceNodeSlack>,
}

/// 显式故障场景；未出现的覆盖继承名义状态。 / An explicit failure scenario; absent overrides inherit nominal state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceScenario {
    /// 唯一场景 ID；`nominal` 是保留字。 / Unique scenario ID; `nominal` is reserved.
    pub id: String,
    /// 关闭的弧。 / Closed arcs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closed_arcs: Vec<String>,
    /// 失效节点及其相邻弧。 / Failed nodes and their incident arcs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_nodes: Vec<String>,
    /// 弧降级上界。 / Degraded arc upper bounds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upper_capacities: Vec<ResilienceArcQuantity>,
    /// 按商品完整替换的节点供需表。 / Complete per-commodity node-balance replacements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub balances: Vec<ResilienceCommodityBalances>,
    /// 按商品完整替换的松弛。 / Complete per-commodity slack replacements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slacks: Vec<ResilienceCommoditySlacks>,
}

/// 韧性规划问题：共享建设 + 显式场景 + 连通要求。 / Resilience problem: shared design, explicit scenarios and connectivity requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkResilienceProblem {
    /// 共享建设问题。 / The shared design problem.
    pub design: NetworkDesignProblem,
    /// 显式故障场景。 / Explicit failure scenarios.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenarios: Vec<ResilienceScenario>,
    /// 名义图连通要求。 / Nominal-graph connectivity requirements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connectivity: Vec<ResilienceConnectivity>,
}

impl NetworkResilienceProblem {
    /// 映射到原领域问题；场景与连通引用在建模前全部校验。
    /// Map onto the original domain problem; scenario and connectivity references are all checked before modelling.
    pub fn to_domain(&self) -> Result<DomainProblem<f64>> {
        let design = self.design.to_domain()?;
        let arc_ids = self.design.flow.arcs.iter().map(|arc| arc.id.as_str()).collect::<BTreeSet<_>>();
        let node_ids = self.design.flow.nodes.iter().map(|node| node.id.as_str()).collect::<BTreeSet<_>>();
        let commodities = self.design.flow.commodities.iter().map(|c| c.id.as_str()).collect::<BTreeSet<_>>();
        let mut scenario_ids = BTreeSet::new();
        let mut scenarios = Vec::with_capacity(self.scenarios.len());
        for scenario in &self.scenarios {
            if scenario.id.trim().is_empty() || scenario.id == "nominal" {
                return Err(BridgeError::new("InvalidInput", "resilience scenario ID is empty or uses the reserved `nominal`"));
            }
            if !scenario_ids.insert(scenario.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "resilience repeats a scenario ID"));
            }
            let mut closed = BTreeSet::new();
            for arc in &scenario.closed_arcs {
                if !arc_ids.contains(arc.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "resilience scenario closes an unknown arc"));
                }
                if !closed.insert(arc.as_str().into()) {
                    return Err(BridgeError::new("InvalidInput", "resilience repeats a closed arc"));
                }
            }
            let mut failed = BTreeSet::new();
            for node in &scenario.failed_nodes {
                if !node_ids.contains(node.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "resilience scenario fails an unknown node"));
                }
                if !failed.insert(node.as_str().into()) {
                    return Err(BridgeError::new("InvalidInput", "resilience repeats a failed node"));
                }
            }
            let mut upper = BTreeMap::new();
            for entry in &scenario.upper_capacities {
                if !arc_ids.contains(entry.arc.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "resilience capacity override references an unknown arc"));
                }
                if upper.insert(entry.arc.as_str().into(), entry.quantity.quantity()?).is_some() {
                    return Err(BridgeError::new("InvalidInput", "resilience repeats a capacity override for one arc"));
                }
            }
            let mut balances = BTreeMap::new();
            for entry in &scenario.balances {
                if !commodities.contains(entry.commodity.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "resilience balance override references an unknown commodity"));
                }
                let mut table = HashMap::new();
                for balance in &entry.balances {
                    if !node_ids.contains(balance.node.as_str()) {
                        return Err(BridgeError::new("InvalidInput", "resilience balance override references an unknown node"));
                    }
                    if table.insert(balance.node.as_str().into(), balance.quantity.quantity()?).is_some() {
                        return Err(BridgeError::new("InvalidInput", "resilience repeats a balance override for one node"));
                    }
                }
                if balances.insert(entry.commodity.as_str().into(), table).is_some() {
                    return Err(BridgeError::new("InvalidInput", "resilience repeats a balance override for one commodity"));
                }
            }
            let mut slacks = BTreeMap::new();
            for entry in &scenario.slacks {
                if !commodities.contains(entry.commodity.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "resilience slack override references an unknown commodity"));
                }
                let mut table = HashMap::new();
                for slack in &entry.slacks {
                    if !node_ids.contains(slack.node.as_str()) {
                        return Err(BridgeError::new("InvalidInput", "resilience slack override references an unknown node"));
                    }
                    let built = FlowBalanceSlack { spill_limit: slack.spill_limit.quantity()?,
                        shortage_limit: slack.shortage_limit.quantity()?,
                        spill_cost: NetworkCost::new(slack.spill_cost.quantity()?),
                        shortage_cost: NetworkCost::new(slack.shortage_cost.quantity()?) };
                    if table.insert(slack.node.as_str().into(), built).is_some() {
                        return Err(BridgeError::new("InvalidInput", "resilience repeats a slack override for one node"));
                    }
                }
                if slacks.insert(entry.commodity.as_str().into(), table).is_some() {
                    return Err(BridgeError::new("InvalidInput", "resilience repeats a slack override for one commodity"));
                }
            }
            scenarios.push(FailureScenario { id: scenario.id.clone(), closed_arcs: closed, failed_nodes: failed,
                upper_capacities: upper, balances, slacks });
        }
        let mut connectivity = Vec::with_capacity(self.connectivity.len());
        for requirement in &self.connectivity {
            if requirement.paths == 0 {
                return Err(BridgeError::new("InvalidInput", "resilience connectivity requires a positive path count"));
            }
            for endpoint in [requirement.source.as_str(), requirement.target.as_str()] {
                if !node_ids.contains(endpoint) {
                    return Err(BridgeError::new("InvalidInput", "resilience connectivity references an unknown node"));
                }
            }
            if requirement.source == requirement.target {
                return Err(BridgeError::new("InvalidInput", "resilience connectivity source and target must differ"));
            }
            connectivity.push(ConnectivityRequirement { source: requirement.source.as_str().into(),
                target: requirement.target.as_str().into(),
                paths: usize::try_from(requirement.paths).map_err(|_| BridgeError::new("InvalidInput", "path count overflow"))?,
                kind: match requirement.kind {
                    ResilienceDisjointness::EdgeDisjoint => ConnectivityKind::EdgeDisjoint,
                    ResilienceDisjointness::NodeDisjoint => ConnectivityKind::NodeDisjoint,
                } });
        }
        DomainProblem::new(design, scenarios, connectivity).map_err(|error| BridgeError::new("InvalidInput", &error))
    }
}

/// 一个场景的流结果。 / The flow result of one scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceScenarioResult {
    /// 场景 ID；`nominal` 表示名义状态。 / Scenario ID; `nominal` is the nominal state.
    pub id: String,
    /// 该场景的流与费用。 / The scenario's flow and costs.
    pub flow: NetworkFlowSolution,
}

/// 一个场景下的建成有效上界。 / Built effective upper bounds under one scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceEffectiveCapacities {
    /// 场景 ID。 / Scenario ID.
    pub scenario: String,
    /// 该场景的逐弧有效上界。 / Per-arc effective upper bounds for that scenario.
    pub capacities: Vec<FlowRadiusValue>,
}

/// 弧上的一个浮点值，用于有效上界与连通辅助流。 / A float value on an arc, used for effective bounds and connectivity flows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowRadiusValue {
    /// 弧 ID。 / Arc ID.
    pub arc: String,
    /// 数值。 / The value.
    pub value: f64,
}

/// 连通要求的辅助流，按连通要求顺序给出。 / Auxiliary flow of one connectivity requirement, in requirement order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResilienceConnectivityFlow {
    /// 该要求上的逐弧辅助流。 / Per-arc auxiliary flow for that requirement.
    pub flows: Vec<FlowRadiusValue>,
}

/// 韧性领域解。 / Resilience domain solution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkResilienceSolution {
    /// 建设选择。 / Construction choices.
    pub construction: NetworkConstruction,
    /// 含 `nominal` 的独立场景结果。 / Independent scenario results including `nominal`.
    pub scenarios: Vec<ResilienceScenarioResult>,
    /// 实际最大运行费用。 / Actual maximum operating cost.
    pub worst_operating_cost: NetworkFlowQuantity,
    /// 求解模型的费用上包络；可行但非最优时可能有余量。 / Solver epigraph, possibly slack in feasible nonoptimal results.
    pub operating_cost_epigraph: NetworkFlowQuantity,
    /// 运行费用最大的场景。 / The scenario with the maximum realized operating cost.
    pub worst_scenario: String,
    /// 各显式场景的建成有效上界。 / Built effective upper bounds for each explicit scenario.
    pub effective_capacities: Vec<ResilienceEffectiveCapacities>,
    /// 与连通要求同序的辅助流。 / Auxiliary flows in connectivity-requirement order.
    pub connectivity_flows: Vec<ResilienceConnectivityFlow>,
    /// 建设加实际最坏运行费用。 / Construction plus the actual worst operating cost.
    pub total_cost: NetworkFlowQuantity,
}

/// 统一报告：通用摘要 + 韧性领域解。 / Unified report: shared summary plus the resilience solution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkResilienceReport {
    /// 通用求解摘要。 / Shared solve summary.
    pub summary: ReportSummary,
    /// 领域解；无 incumbent 时为空。 / The domain solution, absent without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<NetworkResilienceSolution>,
}

fn quantity_of(quantity: &ospf_rust_quantities::quantity::Quantity<f64, ospf_rust_quantities::unit::Unit>)
    -> Result<NetworkFlowQuantity> {
    if !quantity.value.is_finite() {
        return Err(BridgeError::new("InvalidReport", "resilience quantity is not finite"));
    }
    Ok(NetworkFlowQuantity { value: quantity.value, unit: quantity.unit.symbol().to_owned() })
}

/// 把原领域解展开成 DTO。 / Expand the domain solution into DTOs.
pub fn decode_network_resilience_solution(solution: &DomainSolution<f64>) -> Result<NetworkResilienceSolution> {
    Ok(NetworkResilienceSolution {
        construction: decode_construction(&solution.construction)?,
        scenarios: solution.scenarios.iter()
            .map(|(id, flow)| Ok(ResilienceScenarioResult { id: id.clone(),
                flow: decode_network_flow_solution(flow)? }))
            .collect::<Result<Vec<_>>>()?,
        worst_operating_cost: quantity_of(&solution.worst_operating_cost)?,
        operating_cost_epigraph: quantity_of(&solution.operating_cost_epigraph)?,
        worst_scenario: solution.worst_scenario.clone(),
        effective_capacities: solution.effective_capacities.iter()
            .map(|(scenario, capacities)| Ok(ResilienceEffectiveCapacities { scenario: scenario.clone(),
                capacities: capacities.iter().map(|(arc, quantity)| Ok(FlowRadiusValue { arc: arc.as_str().to_owned(),
                    value: quantity.value })).collect::<Result<Vec<_>>>()? }))
            .collect::<Result<Vec<_>>>()?,
        connectivity_flows: solution.connectivity_flows.iter()
            .map(|flows| ResilienceConnectivityFlow { flows: flows.iter()
                .map(|(arc, value)| FlowRadiusValue { arc: arc.as_str().to_owned(), value: *value }).collect() })
            .collect(),
        total_cost: quantity_of(&solution.total_cost)?,
    })
}

/// 组装并复核报告。 / Assemble and re-check the report.
pub fn summarize_network_resilience_report(report: &SolveReport<DomainSolution<f64>>,
    solution: Option<NetworkResilienceSolution>) -> Result<NetworkResilienceReport> {
    let summary = summarize_domain_report(report, solution.is_some())?;
    Ok(NetworkResilienceReport { summary, solution })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetworkDesignLevel, NetworkDesignService, NetworkFlowArc, NetworkFlowCommodity, NetworkFlowNode,
        NetworkFlowNodeQuantity};

    fn kg(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
    fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

    fn problem() -> NetworkResilienceProblem {
        NetworkResilienceProblem {
            design: NetworkDesignProblem {
                flow: crate::NetworkFlowProblem {
                    nodes: vec![NetworkFlowNode { id: "a".into() }, NetworkFlowNode { id: "b".into() }],
                    arcs: vec![NetworkFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
                        capacity_lower: kg(0.0), capacity_upper: kg(10.0), cost: pure(1.0) }],
                    commodities: vec![NetworkFlowCommodity { id: "c".into(),
                        balances: vec![NetworkFlowNodeQuantity { node: "a".into(), quantity: kg(5.0) },
                            NetworkFlowNodeQuantity { node: "b".into(), quantity: kg(-5.0) }],
                        unit: None, allowed_arcs: None, costs: Vec::new(), capacity_consumption: Vec::new(), integer_step: None }],
                },
                services: vec![NetworkDesignService { arc: "a-b".into(), required: true, levels: vec![
                    NetworkDesignLevel { id: "small".into(), capacity: kg(8.0), cost: kg(3.0) }] }],
                facilities: Vec::new(),
                budget: None,
            },
            scenarios: vec![ResilienceScenario { id: "arc-down".into(), closed_arcs: vec!["a-b".into()],
                failed_nodes: Vec::new(), upper_capacities: Vec::new(), balances: Vec::new(), slacks: Vec::new() }],
            connectivity: vec![ResilienceConnectivity { source: "a".into(), target: "b".into(), paths: 1,
                kind: ResilienceDisjointness::EdgeDisjoint }],
        }
    }

    #[test]
    fn resilience_problems_reject_reserved_ids_bad_references_and_zero_paths() {
        problem().to_domain().unwrap();
        let broken: Vec<(&str, NetworkResilienceProblem)> = vec![
            ("reserved scenario id", NetworkResilienceProblem { scenarios: vec![ResilienceScenario { id: "nominal".into(),
                closed_arcs: Vec::new(), failed_nodes: Vec::new(), upper_capacities: Vec::new(), balances: Vec::new(),
                slacks: Vec::new() }], ..problem() }),
            ("unknown closed arc", NetworkResilienceProblem { scenarios: vec![ResilienceScenario { id: "x".into(),
                closed_arcs: vec!["ghost".into()], failed_nodes: Vec::new(), upper_capacities: Vec::new(),
                balances: Vec::new(), slacks: Vec::new() }], ..problem() }),
            ("unknown failed node", NetworkResilienceProblem { scenarios: vec![ResilienceScenario { id: "x".into(),
                closed_arcs: Vec::new(), failed_nodes: vec!["ghost".into()], upper_capacities: Vec::new(),
                balances: Vec::new(), slacks: Vec::new() }], ..problem() }),
            ("unknown balance commodity", NetworkResilienceProblem { scenarios: vec![ResilienceScenario { id: "x".into(),
                closed_arcs: Vec::new(), failed_nodes: Vec::new(), upper_capacities: Vec::new(),
                balances: vec![ResilienceCommodityBalances { commodity: "ghost".into(), balances: Vec::new() }],
                slacks: Vec::new() }], ..problem() }),
            ("zero paths", NetworkResilienceProblem { connectivity: vec![ResilienceConnectivity { source: "a".into(),
                target: "b".into(), paths: 0, kind: ResilienceDisjointness::NodeDisjoint }], ..problem() }),
            ("same source and target", NetworkResilienceProblem { connectivity: vec![ResilienceConnectivity {
                source: "a".into(), target: "a".into(), paths: 1, kind: ResilienceDisjointness::EdgeDisjoint }], ..problem() }),
            ("unknown connectivity node", NetworkResilienceProblem { connectivity: vec![ResilienceConnectivity {
                source: "a".into(), target: "ghost".into(), paths: 1, kind: ResilienceDisjointness::EdgeDisjoint }], ..problem() }),
        ];
        for (label, case) in broken {
            assert!(case.to_domain().is_err(), "{label} must be rejected");
        }
        let mut repeated = problem();
        repeated.scenarios.push(problem().scenarios[0].clone());
        assert!(repeated.to_domain().is_err());
    }
}
