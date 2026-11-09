//! 网络建设领域绑定：原 `network-scheduling` 的共享建设选择 + 名义流。
//! Network-design domain binding: the original `network-scheduling` shared construction choices plus nominal flow.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework_network_scheduling::domain::design::{
    CapacityLevel, NetworkDesignProblem as DomainProblem, NetworkDesignSolution as DomainSolution, NetworkFacility,
    NetworkService,
};
use ospf_rust_core::solver::SolveReport;

use crate::network_flow::{decode_network_flow_solution, summarize_domain_report, NetworkFlowProblem,
    NetworkFlowQuantity, NetworkFlowSolution};
use crate::quantities::{PhysicalQuantity, QuantityUnit};
use crate::{BridgeError, ReportSummary, Result};

/// 互斥容量等级：建成后的**总**容量与建设费用。 / Exclusive capacity level: **total** built capacity and its cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDesignLevel {
    /// 稳定等级 ID；同一通道内必须唯一。 / Stable level ID, unique within its service.
    pub id: String,
    /// 建成后的总容量，不是增量。 / Total built capacity, not an increment.
    pub capacity: PhysicalQuantity,
    /// 建设费用。 / Construction cost.
    pub cost: PhysicalQuantity,
}

/// 可选通道；没有任何等级被选中即视为关闭。 / Optional service, closed when no level is selected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDesignService {
    /// 已声明的稳定弧 ID。 / Declared stable arc ID.
    pub arc: String,
    /// 互斥容量等级；不得为空。 / Exclusive capacity levels; never empty.
    pub levels: Vec<NetworkDesignLevel>,
    /// 是否必须恰好选择一个等级。 / Whether exactly one level must be selected.
    #[serde(default)]
    pub required: bool,
}

/// 候选设施：开设后启用其相邻通道与声明资源行。 / Candidate facility enabling incident services and declared resource rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDesignFacility {
    /// 设施节点 ID。 / Facility node ID.
    pub node: String,
    /// 开设费用。 / Opening cost.
    pub cost: PhysicalQuantity,
    /// 是否必须开设。 / Whether opening is required.
    #[serde(default)]
    pub required: bool,
    /// 随设施启用的共享资源行。 / Shared resource rows gated by this facility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capacity_rows: Vec<String>,
}

/// 建设问题：流问题 + 通道等级 + 设施 + 可选预算。 / Design problem: flow problem plus service levels, facilities and an optional budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDesignProblem {
    /// 承载名义流的流问题。 / The flow problem carrying the nominal flow.
    pub flow: NetworkFlowProblem,
    /// 可选通道。 / Optional services.
    pub services: Vec<NetworkDesignService>,
    /// 候选设施。 / Candidate facilities.
    pub facilities: Vec<NetworkDesignFacility>,
    /// 建设预算；省略表示无预算上限。 / Construction budget; omission means no budget cap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub budget: Option<PhysicalQuantity>,
}

impl NetworkDesignProblem {
    /// 建设费用的量纲由原领域派生为「流单位 × 流成本单位」，调用方不能自行声明。
    /// The construction-cost dimension is derived by the original domain as `flow unit × flow cost unit`, so callers
    /// cannot declare it themselves.
    ///
    /// 桥接的单位是闭集，无法表达复合量纲，因此把**流成本单位设为无量纲 `"1"`** 时建设费用就是流单位
    /// （例如 kg），这也是本 DTO 能表达的唯一组合。
    /// The bridge units form a closed set that cannot express a composite dimension, so when the **flow cost unit is the
    /// dimensionless `"1"`** the construction cost is the flow unit (for example kg); that is the only combination this
    /// DTO can express.
    pub fn cost_dimension_hint() -> &'static str { "flow-unit × flow-cost-unit" }
}

impl NetworkDesignProblem {
    /// 映射到原领域问题；弧/节点引用与等级唯一性都在这里校验。
    /// Map onto the original domain problem; arc/node references and level uniqueness are checked here.
    pub fn to_domain(&self) -> Result<DomainProblem<f64>> {
        let flow = self.flow.to_domain()?;
        let arc_ids = self.flow.arcs.iter().map(|arc| arc.id.as_str()).collect::<BTreeSet<_>>();
        let node_ids = self.flow.nodes.iter().map(|node| node.id.as_str()).collect::<BTreeSet<_>>();
        let mut services = Vec::with_capacity(self.services.len());
        let mut declared = BTreeSet::new();
        for service in &self.services {
            if !arc_ids.contains(service.arc.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network design service references an unknown arc"));
            }
            if !declared.insert(service.arc.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network design repeats a service for one arc"));
            }
            if service.levels.is_empty() {
                return Err(BridgeError::new("InvalidInput", "network design service requires at least one level"));
            }
            let mut seen = BTreeSet::new();
            let mut levels = Vec::with_capacity(service.levels.len());
            for level in &service.levels {
                if level.id.trim().is_empty() {
                    return Err(BridgeError::new("InvalidInput", "network design level ID must not be empty"));
                }
                if !seen.insert(level.id.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "network design repeats a level ID within one service"));
                }
                levels.push(CapacityLevel { id: level.id.clone(), capacity: level.capacity.quantity()?,
                    cost: level.cost.quantity()? });
            }
            services.push(NetworkService { arc: service.arc.as_str().into(), levels, required: service.required });
        }
        let mut facilities = Vec::with_capacity(self.facilities.len());
        let mut facility_nodes = BTreeSet::new();
        for facility in &self.facilities {
            if !node_ids.contains(facility.node.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network design facility references an unknown node"));
            }
            if !facility_nodes.insert(facility.node.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network design repeats a facility for one node"));
            }
            let mut rows = BTreeSet::new();
            for row in &facility.capacity_rows {
                if row.trim().is_empty() {
                    return Err(BridgeError::new("InvalidInput", "network design capacity row must not be empty"));
                }
                if !rows.insert(row.clone()) {
                    return Err(BridgeError::new("InvalidInput", "network design repeats a capacity row within one facility"));
                }
            }
            facilities.push(NetworkFacility { node: facility.node.as_str().into(), cost: facility.cost.quantity()?,
                required: facility.required, capacity_rows: rows });
        }
        let budget = self.budget.as_ref().map(PhysicalQuantity::quantity).transpose()?;
        DomainProblem::new(flow, services, facilities, budget).map_err(|error| BridgeError::new("InvalidInput", &error))
    }
}

/// 建设选择与建设费用。 / Construction choices and their cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkConstruction {
    /// 已开设设施，按稳定 ID 排序。 / Open facilities sorted by stable ID.
    pub open_facilities: Vec<String>,
    /// 每条通道选中的等级。 / Selected level per service arc.
    pub service_levels: Vec<NetworkServiceLevel>,
    /// 建设费用。 / Construction cost.
    pub cost: NetworkFlowQuantity,
}

/// 一条通道选中的等级。 / The level selected for one service arc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkServiceLevel {
    /// 通道弧 ID。 / Service arc ID.
    pub arc: String,
    /// 选中的等级 ID。 / Selected level ID.
    pub level: String,
}

/// 建设 + 名义流的领域解。 / Domain solution of construction plus nominal flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDesignSolution {
    /// 共享建设选择。 / Shared construction choices.
    pub construction: NetworkConstruction,
    /// 名义流结果。 / Nominal flow result.
    pub flow: NetworkFlowSolution,
    /// 建设加运行费用。 / Construction plus operating cost.
    pub total_cost: NetworkFlowQuantity,
}

/// 统一报告：通用摘要 + 建设领域解。 / Unified report: shared summary plus the design solution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDesignReport {
    /// 通用求解摘要。 / Shared solve summary.
    pub summary: ReportSummary,
    /// 领域解；无 incumbent 时为空。 / The domain solution, absent without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<NetworkDesignSolution>,
}

fn quantity_of(quantity: &ospf_rust_quantities::quantity::Quantity<f64, ospf_rust_quantities::unit::Unit>)
    -> Result<NetworkFlowQuantity> {
    if !quantity.value.is_finite() {
        return Err(BridgeError::new("InvalidReport", "network design quantity is not finite"));
    }
    Ok(NetworkFlowQuantity { value: quantity.value, unit: quantity.unit.symbol().to_owned() })
}

/// 把原领域解展开成 DTO。 / Expand the domain solution into DTOs.
pub fn decode_network_design_solution(solution: &DomainSolution<f64>) -> Result<NetworkDesignSolution> {
    Ok(NetworkDesignSolution {
        construction: decode_construction(&solution.construction)?,
        flow: decode_network_flow_solution(&solution.flow)?,
        total_cost: quantity_of(&solution.total_cost)?,
    })
}

/// 只解码建设选择；`design` 与 `structure` 共用同一份结构，避免两处漂移。
/// Decode only the construction choices; `design` and `structure` share one implementation so the two cannot drift.
pub fn decode_construction(
    construction: &ospf_rust_framework_network_scheduling::domain::design::NetworkConstructionSolution<f64>,
) -> Result<NetworkConstruction> {
    Ok(NetworkConstruction {
        open_facilities: construction.open_facilities.iter().map(|id| id.as_str().to_owned()).collect(),
        service_levels: construction.service_levels.iter()
            .map(|(arc, level)| NetworkServiceLevel { arc: arc.as_str().to_owned(), level: level.clone() })
            .collect(),
        cost: quantity_of(&construction.cost)?,
    })
}

/// 组装并复核报告。 / Assemble and re-check the report.
pub fn summarize_network_design_report(report: &SolveReport<DomainSolution<f64>>, solution: Option<NetworkDesignSolution>)
    -> Result<NetworkDesignReport> {
    let summary = summarize_domain_report(report, solution.is_some())?;
    Ok(NetworkDesignReport { summary, solution })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetworkFlowArc, NetworkFlowCommodity, NetworkFlowNode, NetworkFlowNodeQuantity};

    fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }

    fn flow() -> NetworkFlowProblem {
        NetworkFlowProblem {
            nodes: vec![NetworkFlowNode { id: "a".into() }, NetworkFlowNode { id: "b".into() }],
            // 流成本单位取无量纲 `1`，于是原领域派生的建设费用量纲正好是流单位（kg）。
            // The flow cost unit is the dimensionless `1`, so the derived construction-cost dimension is exactly the
            // flow unit (kg).
            arcs: vec![NetworkFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
                capacity_lower: mass(0.0), capacity_upper: mass(10.0),
                cost: PhysicalQuantity { value: 1.0, unit: QuantityUnit::Dimensionless } }],
            commodities: vec![NetworkFlowCommodity { id: "c".into(),
                balances: vec![NetworkFlowNodeQuantity { node: "a".into(), quantity: mass(5.0) },
                    NetworkFlowNodeQuantity { node: "b".into(), quantity: mass(-5.0) }],
                unit: None, allowed_arcs: None, costs: Vec::new(), capacity_consumption: Vec::new(), integer_step: None }],
        }
    }

    fn problem() -> NetworkDesignProblem {
        NetworkDesignProblem {
            flow: flow(),
            services: vec![NetworkDesignService { arc: "a-b".into(), required: true, levels: vec![
                NetworkDesignLevel { id: "small".into(), capacity: mass(8.0), cost: mass(3.0) },
                // 等级容量是「建成后的总容量」，必须落在弧容量上界（10 kg）之内。
                // A level capacity is the total built capacity and must stay within the arc upper bound (10 kg).
                NetworkDesignLevel { id: "large".into(), capacity: mass(10.0), cost: mass(9.0) }] }],
            facilities: vec![NetworkDesignFacility { node: "a".into(), cost: mass(2.0), required: false, capacity_rows: Vec::new() }],
            budget: None,
        }
    }

    #[test]
    fn design_problems_reject_unknown_references_and_duplicate_levels() {
        problem().to_domain().unwrap();
        let broken: Vec<(&str, NetworkDesignProblem)> = vec![
            ("unknown service arc", NetworkDesignProblem { services: vec![NetworkDesignService { arc: "ghost".into(),
                levels: vec![NetworkDesignLevel { id: "x".into(), capacity: mass(1.0), cost: mass(1.0) }], required: false }], ..problem() }),
            ("empty levels", NetworkDesignProblem { services: vec![NetworkDesignService { arc: "a-b".into(), levels: Vec::new(), required: false }], ..problem() }),
            ("duplicate level", NetworkDesignProblem { services: vec![NetworkDesignService { arc: "a-b".into(), required: false,
                levels: vec![NetworkDesignLevel { id: "x".into(), capacity: mass(1.0), cost: mass(1.0) },
                    NetworkDesignLevel { id: "x".into(), capacity: mass(2.0), cost: mass(2.0) }] }], ..problem() }),
            ("unknown facility node", NetworkDesignProblem { facilities: vec![NetworkDesignFacility { node: "ghost".into(),
                cost: mass(1.0), required: false, capacity_rows: Vec::new() }], ..problem() }),
            ("duplicate service", NetworkDesignProblem { services: vec![
                NetworkDesignService { arc: "a-b".into(), required: false, levels: vec![NetworkDesignLevel { id: "x".into(), capacity: mass(1.0), cost: mass(1.0) }] },
                NetworkDesignService { arc: "a-b".into(), required: false, levels: vec![NetworkDesignLevel { id: "y".into(), capacity: mass(1.0), cost: mass(1.0) }] }], ..problem() }),
            ("non-finite budget", NetworkDesignProblem { budget: Some(PhysicalQuantity { value: f64::NAN, unit: QuantityUnit::Kilogram }), ..problem() }),
        ];
        for (label, case) in broken {
            assert!(case.to_domain().is_err(), "{label} must be rejected");
        }
    }
}
