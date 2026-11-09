//! 网络流领域绑定：把原 `network-scheduling` 的多商品最小费用流映射成可移植 DTO。
//! Network-flow domain binding: maps the original `network-scheduling` multi-commodity min-cost flow onto portable DTOs.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework_network_scheduling::domain::flow::{
    CommodityId, FlowArc, FlowCommodity, FlowNode, FlowProblem, FlowSolution,
};
use ospf_rust_framework_network_scheduling::infrastructure::{CapacityBounds, NetworkArcId, NetworkCost, NetworkNodeId};
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::Unit;

use crate::quantities::{PhysicalQuantity, QuantityUnit};
use ospf_rust_core::solver::report::{ProofKind, ProofStatus};
use ospf_rust_core::solver::SolveReport;

use crate::{BridgeError, ReportSummary, Result};

/// flow 节点。 / A flow node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowNode {
    /// 稳定节点 ID。 / Stable node ID.
    pub id: String,
}

/// flow 弧：起点、终点、容量上下界与单位流成本。 / A flow arc: endpoints, capacity bounds and unit-flow cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowArc {
    /// 稳定弧 ID。 / Stable arc ID.
    pub id: String,
    /// 起点节点 ID。 / Origin node ID.
    pub from: String,
    /// 终点节点 ID。 / Destination node ID.
    pub to: String,
    /// 容量下界。 / Capacity lower bound.
    pub capacity_lower: PhysicalQuantity,
    /// 容量上界；必须不小于下界。 / Capacity upper bound, never below the lower bound.
    pub capacity_upper: PhysicalQuantity,
    /// 单位流成本，允许为负。 / Unit-flow cost; negative values are allowed.
    pub cost: PhysicalQuantity,
}

/// 某节点上的一个物理量，用于供需、弃供与缺货。 / One physical amount at a node, used for balances, spills and shortages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowNodeQuantity {
    /// 节点 ID。 / Node ID.
    pub node: String,
    /// 物理量；正值是供给，负值是需求。 / Physical amount; positive is supply and negative is demand.
    pub quantity: PhysicalQuantity,
}

/// 某条弧上的一个物理量，用于每商品资费与容量消耗。 / One physical amount on an arc, used for per-commodity tariffs and capacity consumption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowArcQuantity {
    /// 弧 ID。 / Arc ID.
    pub arc: String,
    /// 物理量。 / Physical amount.
    pub quantity: PhysicalQuantity,
}

/// 一个商品的供需与可选约束。 / Supply-demand and optional restrictions of one commodity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowCommodity {
    /// 稳定商品 ID。 / Stable commodity ID.
    pub id: String,
    /// 节点平衡表。 / Node balance map.
    pub balances: Vec<NetworkFlowNodeQuantity>,
    /// 独立商品单位；省略时使用问题默认流单位。 / Independent commodity unit; omission uses the problem default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unit: Option<QuantityUnit>,
    /// 可用弧；省略表示所有弧。 / Allowed arcs; omission means every arc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allowed_arcs: Option<Vec<String>>,
    /// 每商品弧资费，覆盖默认弧成本。 / Per-commodity arc tariffs overriding the default arc cost.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub costs: Vec<NetworkFlowArcQuantity>,
    /// 每商品单位消耗的弧容量。 / Arc capacity consumed per commodity unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capacity_consumption: Vec<NetworkFlowArcQuantity>,
    /// 整数计数变量对应的物理量；省略表示连续流。 / Physical amount per integer count; omission means continuous flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub integer_step: Option<PhysicalQuantity>,
}

/// 多商品最小费用流问题。 / A multi-commodity min-cost flow problem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowProblem {
    /// 节点快照。 / Node snapshot.
    pub nodes: Vec<NetworkFlowNode>,
    /// 弧快照；至少一条，流的单位口径由第一条弧推导。 / Arc snapshot; at least one, since the first arc defines the flow unit contract.
    pub arcs: Vec<NetworkFlowArc>,
    /// 商品与供需。 / Commodities with their balances.
    pub commodities: Vec<NetworkFlowCommodity>,
}

impl NetworkFlowProblem {
    /// 映射到原领域问题；所有身份引用与数值都在这里校验，不把坏输入交给下游。
    /// Map onto the original domain problem; every identity reference and value is checked here so bad input never
    /// reaches the domain.
    pub fn to_domain(&self) -> Result<FlowProblem<f64>> {
        if self.arcs.is_empty() {
            return Err(BridgeError::new("InvalidInput", "network flow requires at least one arc to derive its unit"));
        }
        let mut node_ids: HashSet<&str> = HashSet::new();
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "network flow node ID must not be empty"));
            }
            if !node_ids.insert(node.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network flow repeats a node ID"));
            }
        }
        let mut arc_ids: HashSet<&str> = HashSet::new();
        let mut arcs = Vec::with_capacity(self.arcs.len());
        for arc in &self.arcs {
            if arc.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "network flow arc ID must not be empty"));
            }
            if !arc_ids.insert(arc.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network flow repeats an arc ID"));
            }
            for endpoint in [arc.from.as_str(), arc.to.as_str()] {
                if !node_ids.contains(endpoint) {
                    return Err(BridgeError::new("InvalidInput", "network flow arc references an unknown node"));
                }
            }
            let capacity = CapacityBounds::try_new(arc.capacity_lower.quantity()?, arc.capacity_upper.quantity()?)
                .map_err(|error| BridgeError::new("InvalidInput", &error))?;
            arcs.push(FlowArc::new(arc.id.as_str(), arc.from.as_str(), arc.to.as_str(), capacity,
                NetworkCost::new(arc.cost.quantity()?)));
        }
        let nodes = self.nodes.iter().map(|node| FlowNode::new(node.id.as_str())).collect::<Vec<_>>();
        let mut commodity_ids = HashSet::new();
        let mut commodities = Vec::with_capacity(self.commodities.len());
        for commodity in &self.commodities {
            if commodity.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "network flow commodity ID must not be empty"));
            }
            if !commodity_ids.insert(commodity.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network flow repeats a commodity ID"));
            }
            let mut built = FlowCommodity::<f64>::new(commodity.id.as_str());
            for balance in &commodity.balances {
                if !node_ids.contains(balance.node.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "network flow balance references an unknown node"));
                }
                if built.balances.insert(balance.node.as_str().into(), balance.quantity.quantity()?).is_some() {
                    return Err(BridgeError::new("InvalidInput", "network flow repeats a balance for one node"));
                }
            }
            if let Some(unit) = commodity.unit {
                built.unit = Some(unit.unit());
            }
            if let Some(allowed) = &commodity.allowed_arcs {
                let mut set = HashSet::with_capacity(allowed.len());
                for arc in allowed {
                    if !arc_ids.contains(arc.as_str()) {
                        return Err(BridgeError::new("InvalidInput", "network flow allowed arc is unknown"));
                    }
                    if !set.insert(arc.as_str().to_owned()) {
                        return Err(BridgeError::new("InvalidInput", "network flow repeats an allowed arc"));
                    }
                }
                built.allowed_arcs = Some(set.into_iter().map(|arc| arc.as_str().into()).collect());
            }
            for entry in &commodity.costs {
                if !arc_ids.contains(entry.arc.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "network flow tariff references an unknown arc"));
                }
                if built.costs.insert(entry.arc.as_str().into(), NetworkCost::new(entry.quantity.quantity()?)).is_some() {
                    return Err(BridgeError::new("InvalidInput", "network flow repeats a tariff for one arc"));
                }
            }
            for entry in &commodity.capacity_consumption {
                if !arc_ids.contains(entry.arc.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "network flow capacity consumption references an unknown arc"));
                }
                if built.capacity_consumption.insert(entry.arc.as_str().into(), entry.quantity.quantity()?).is_some() {
                    return Err(BridgeError::new("InvalidInput", "network flow repeats capacity consumption for one arc"));
                }
            }
            if let Some(step) = &commodity.integer_step {
                built.integer_step = Some(step.quantity()?);
            }
            commodities.push(built);
        }
        FlowProblem::new(nodes, arcs, commodities).map_err(|error| BridgeError::new("InvalidInput", &error))
    }
}

/// 报告侧物理量：单位保留原领域给出的符号，包含复合与无量纲，不用闭集枚举硬套。
/// Report-side physical amount: the unit keeps the symbol the domain produced, including composite and dimensionless
/// ones, instead of being forced into a closed enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowQuantity {
    /// 有限数值。 / Finite numeric value.
    pub value: f64,
    /// 原领域单位符号。 / The domain unit symbol.
    pub unit: String,
}

/// 一条（商品，弧）上的实际流量。 / Actual flow on one (commodity, arc) pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowArcFlow {
    /// 商品 ID。 / Commodity ID.
    pub commodity: String,
    /// 弧 ID。 / Arc ID.
    pub arc: String,
    /// 物理流量，不是整数计数。 / Physical flow, not integer counts.
    pub quantity: NetworkFlowQuantity,
}

/// 具名物理量，用于共享资源使用。 / A named physical amount, used for shared-resource usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowNamedQuantity {
    /// 资源名。 / Resource name.
    pub name: String,
    /// 物理量。 / Physical amount.
    pub quantity: NetworkFlowQuantity,
}

/// 原领域解；费用分项与弃供/缺货都保留，不合并成一个总费用。 
/// The original domain solution; cost components, spills and shortages are all preserved rather than collapsed into a
/// single total.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowSolution {
    /// 每条（商品，弧）的实际流量。 / Actual flow per (commodity, arc).
    pub flows: Vec<NetworkFlowArcFlow>,
    /// 弃供量。 / Spilled supply.
    pub spills: Vec<NetworkFlowArcFlow>,
    /// 缺货量。 / Unmet demand.
    pub shortages: Vec<NetworkFlowArcFlow>,
    /// 共享资源使用。 / Shared-resource usage.
    pub resources: Vec<NetworkFlowNamedQuantity>,
    /// 运输运行费用。 / Transport operating cost.
    pub operating_cost: NetworkFlowQuantity,
    /// 弃供费用。 / Spill cost.
    pub spill_cost: NetworkFlowQuantity,
    /// 缺货费用。 / Shortage cost.
    pub shortage_cost: NetworkFlowQuantity,
    /// 总费用。 / Total cost.
    pub total_cost: NetworkFlowQuantity,
}

/// 统一报告：既有通用摘要，也有按原领域结构展开的解。
/// Unified report: the shared summary plus a solution expanded in the original domain structure.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFlowReport {
    /// 通用求解摘要。 / Shared solve summary.
    pub summary: ReportSummary,
    /// 原领域解；无 incumbent 时为空。 / The domain solution, absent without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<NetworkFlowSolution>,
}

/// 领域报告的统一摘要：目标值取 solver 的浮点快照，`values` 留空——领域解已按 domain DTO 给出，
/// 这里不把领域对象伪装成求解器列向量。
/// Shared summary of a domain report: the objective comes from the solver's float snapshot and `values` stays empty
/// because the domain solution is delivered as a domain DTO instead of pretending to be a solver column vector.
///
/// 任何值类型都适用：`SolveSolution<V>` 的浮点目标快照本身就是 `Option<f64>`。
/// This works for any value type because `SolveSolution<V>` keeps the float objective snapshot as `Option<f64>`.
pub fn summarize_domain_report<V>(report: &SolveReport<V>, has_solution: bool) -> Result<ReportSummary> {
    report.validate().map_err(|error| BridgeError::new("InvalidReport", &error))?;
    let has_optimality_proof = report.proof.as_ref().is_some_and(|proof| {
        matches!(proof.kind, ProofKind::Optimality) && matches!(proof.status, ProofStatus::Verified)
    });
    // 有最优性证明却没有解，是关于报告本身的假声明。
    // A verified optimality proof with no solution is a false claim about the report itself.
    if has_optimality_proof && !has_solution {
        return Err(BridgeError::new("InvalidReport", "domain report claims optimality without a solution"));
    }
    Ok(ReportSummary {
        problem_status: serde_json::to_value(report.problem_status).map_err(|error| BridgeError::new("CodecFailure", error))?
            .as_str().unwrap_or_default().to_owned(),
        termination_reason: serde_json::to_value(report.termination_reason).map_err(|error| BridgeError::new("CodecFailure", error))?
            .as_str().unwrap_or_default().to_owned(),
        objective_value: report.solution.as_ref().and_then(|solution| solution.objective_value),
        values: Vec::new(),
        has_optimality_proof,
        relative_gap: report.statistics.relative_gap,
        absolute_gap: report.statistics.absolute_gap,
        nodes: report.statistics.nodes.and_then(|nodes| u64::try_from(nodes).ok()),
        solve_time_ms: report.statistics.solve_time.as_millis().min(u128::from(u64::MAX)) as u64,
    })
}

/// 组装网络流报告。 / Assemble a network-flow report.
pub fn summarize_flow_report(report: &SolveReport<FlowSolution<f64>>, solution: Option<NetworkFlowSolution>) -> Result<NetworkFlowReport> {
    let summary = summarize_domain_report(report, solution.is_some())?;
    Ok(NetworkFlowReport { summary, solution })
}

fn quantity_of(quantity: &Quantity<f64, Unit>) -> Result<NetworkFlowQuantity> {
    if !quantity.value.is_finite() {
        return Err(BridgeError::new("InvalidReport", "network flow quantity is not finite"));
    }
    Ok(NetworkFlowQuantity { value: quantity.value, unit: quantity.unit.symbol().to_owned() })
}

/// 把原领域解展开成 DTO，保持按商品/弧排序的确定性顺序。
/// Expand the domain solution into DTOs while keeping a deterministic commodity/arc order.
pub fn decode_network_flow_solution(solution: &FlowSolution<f64>) -> Result<NetworkFlowSolution> {
    // 流量、弃供与缺货都以 `(商品, 弧/节点)` 为键；三元结构统一用 `arc` 位承载那个身份。
    // Flows, spills and shortages are all keyed by `(commodity, arc-or-node)`; the triple carries that identity in its
    // `arc` slot.
    let flatten = |entries: &BTreeMap<(CommodityId, NetworkNodeId), Quantity<f64, Unit>>| entries.iter()
        .map(|((commodity, node), quantity)| Ok(NetworkFlowArcFlow { commodity: commodity.as_str().to_owned(),
            arc: node.as_str().to_owned(), quantity: quantity_of(quantity)? }))
        .collect::<Result<Vec<_>>>();
    let flows = solution.flows.iter()
        .map(|((commodity, arc), quantity)| Ok(NetworkFlowArcFlow { commodity: commodity.as_str().to_owned(),
            arc: arc.as_str().to_owned(), quantity: quantity_of(quantity)? }))
        .collect::<Result<Vec<_>>>()?;
    Ok(NetworkFlowSolution {
        flows,
        spills: flatten(&solution.spills)?,
        shortages: flatten(&solution.shortages)?,
        resources: solution.resources.iter()
            .map(|(name, quantity)| Ok(NetworkFlowNamedQuantity { name: name.clone(), quantity: quantity_of(quantity)? }))
            .collect::<Result<Vec<_>>>()?,
        operating_cost: quantity_of(&solution.operating_cost)?,
        spill_cost: quantity_of(&solution.spill_cost)?,
        shortage_cost: quantity_of(&solution.shortage_cost)?,
        total_cost: quantity_of(&solution.total_cost)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }

    fn problem() -> NetworkFlowProblem {
        NetworkFlowProblem {
            nodes: vec![NetworkFlowNode { id: "a".into() }, NetworkFlowNode { id: "b".into() }],
            arcs: vec![NetworkFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
                capacity_lower: mass(0.0), capacity_upper: mass(10.0), cost: mass(1.0) }],
            commodities: vec![NetworkFlowCommodity { id: "c".into(),
                balances: vec![NetworkFlowNodeQuantity { node: "a".into(), quantity: mass(5.0) },
                    NetworkFlowNodeQuantity { node: "b".into(), quantity: mass(-5.0) }],
                unit: None, allowed_arcs: None, costs: Vec::new(), capacity_consumption: Vec::new(), integer_step: None }],
        }
    }

    #[test]
    fn flow_problems_map_onto_the_domain_with_every_identity_checked() {
        let domain = problem().to_domain().unwrap();
        let _ = domain;
        // 每个坏引用都必须在这里被拒绝，而不是交给下游产生含糊错误。
        // Every bad reference must be rejected here instead of surfacing as an opaque downstream error.
        let broken: Vec<(&str, NetworkFlowProblem)> = vec![
            ("empty node ID", NetworkFlowProblem { nodes: vec![NetworkFlowNode { id: String::new() }, NetworkFlowNode { id: "b".into() }], ..problem() }),
            ("unknown arc node", NetworkFlowProblem { nodes: vec![NetworkFlowNode { id: "a".into() }], ..problem() }),
            ("no arcs", NetworkFlowProblem { arcs: Vec::new(), ..problem() }),
            ("unknown arc endpoint", NetworkFlowProblem { arcs: vec![NetworkFlowArc { to: "ghost".into(), ..problem().arcs[0].clone() }], ..problem() }),
            ("unknown balance node", NetworkFlowProblem { commodities: vec![NetworkFlowCommodity {
                balances: vec![NetworkFlowNodeQuantity { node: "ghost".into(), quantity: mass(1.0) }], ..problem().commodities[0].clone() }], ..problem() }),
            ("unknown allowed arc", NetworkFlowProblem { commodities: vec![NetworkFlowCommodity {
                allowed_arcs: Some(vec!["ghost".into()]), ..problem().commodities[0].clone() }], ..problem() }),
            ("unknown tariff arc", NetworkFlowProblem { commodities: vec![NetworkFlowCommodity {
                costs: vec![NetworkFlowArcQuantity { arc: "ghost".into(), quantity: mass(1.0) }], ..problem().commodities[0].clone() }], ..problem() }),
            ("lower above upper", NetworkFlowProblem { arcs: vec![NetworkFlowArc { capacity_lower: mass(5.0), capacity_upper: mass(1.0), ..problem().arcs[0].clone() }], ..problem() }),
            ("non-finite cost", NetworkFlowProblem { arcs: vec![NetworkFlowArc { cost: PhysicalQuantity { value: f64::NAN, unit: QuantityUnit::Kilogram }, ..problem().arcs[0].clone() }], ..problem() }),
        ];
        for (label, case) in broken {
            assert!(case.to_domain().is_err(), "{label} must be rejected");
        }
        // 重复身份同样拒绝。 / Repeated identities are rejected too.
        let mut repeated = problem();
        repeated.nodes.push(NetworkFlowNode { id: "a".into() });
        assert!(repeated.to_domain().is_err());
        let mut repeated_commodity = problem();
        repeated_commodity.commodities.push(problem().commodities[0].clone());
        assert!(repeated_commodity.to_domain().is_err());
    }
}
