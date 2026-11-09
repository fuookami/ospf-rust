//! 网络结构（根树 + 设施归属）绑定：复用建设问题，不注册商品流。
//! Network-structure (rooted tree plus facility ownership) binding: it reuses the design problem and does not register
//! commodity flows.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework_network_scheduling::domain::structure::{
    FacilityAssignmentQuota, FacilityPartitionRequest, NetworkStructureProblem as DomainProblem,
    NetworkStructureSolution as DomainSolution,
};
use ospf_rust_core::solver::SolveReport;

use crate::network_design::{decode_construction, NetworkConstruction, NetworkDesignProblem};
use crate::network_flow::summarize_domain_report;
use crate::{BridgeError, ReportSummary, Result};

/// 分区覆盖请求；设施归属数量与物理流量无关。 / Partition coverage request; facility ownership is independent of physical flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkStructurePartition {
    /// 稳定且唯一的分区 ID。 / Stable, unique partition ID.
    pub id: String,
    /// 可负责的设施节点。 / Eligible facility nodes.
    pub eligible_facilities: Vec<String>,
    /// 是否必须恰好归属一个设施。 / Whether exactly one owner is required.
    #[serde(default)]
    pub required: bool,
}

/// 设施被分配的分区数量配额。 / Quota on how many partitions one facility may own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkStructureQuota {
    /// 设施节点 ID。 / Facility node ID.
    pub facility: String,
    /// 最少归属数量。 / Minimum owned count.
    #[serde(default)]
    pub minimum: u32,
    /// 可选最多归属数量。 / Optional maximum owned count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub maximum: Option<u32>,
}

/// 根树与设施归属问题；底层建设问题给出候选弧、等级与设施。
/// Rooted-tree and facility-ownership problem; the underlying design problem provides candidate arcs, levels and
/// facilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkStructureProblem {
    /// 底层建设问题。 / The underlying design problem.
    pub design: NetworkDesignProblem,
    /// 必选根节点。 / Required root node.
    pub root: String,
    /// 必须由根可达的终端。 / Terminals that must be reachable from the root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminals: Vec<String>,
    /// 可选中转候选；根与终端始终必选。 / Optional relay candidates; the root and terminals stay mandatory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steiner_nodes: Vec<String>,
    /// 分区覆盖请求。 / Partition coverage requests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<NetworkStructurePartition>,
    /// 设施归属配额。 / Facility ownership quotas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quotas: Vec<NetworkStructureQuota>,
}

impl NetworkStructureProblem {
    /// 映射到原领域问题；节点引用、分区与配额都在这里校验。
    /// Map onto the original domain problem; node references, partitions and quotas are checked here.
    pub fn to_domain(&self) -> Result<DomainProblem<f64>> {
        let design = self.design.to_domain()?;
        let nodes = self.design.flow.nodes.iter().map(|node| node.id.as_str()).collect::<BTreeSet<_>>();
        let known = |id: &str| nodes.contains(id);
        if self.root.trim().is_empty() || !known(self.root.as_str()) {
            return Err(BridgeError::new("InvalidInput", "network structure root is unknown"));
        }
        let mut terminals = BTreeSet::new();
        for terminal in &self.terminals {
            if !known(terminal.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network structure terminal is unknown"));
            }
            if !terminals.insert(terminal.as_str().into()) {
                return Err(BridgeError::new("InvalidInput", "network structure repeats a terminal"));
            }
        }
        let mut steiner = BTreeSet::new();
        for relay in &self.steiner_nodes {
            if !known(relay.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network structure relay is unknown"));
            }
            if !steiner.insert(relay.as_str().into()) {
                return Err(BridgeError::new("InvalidInput", "network structure repeats a relay"));
            }
        }
        let mut partition_ids = BTreeSet::new();
        let mut partitions = Vec::with_capacity(self.partitions.len());
        for partition in &self.partitions {
            if partition.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "network structure partition ID must not be empty"));
            }
            if !partition_ids.insert(partition.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network structure repeats a partition ID"));
            }
            let mut eligible = BTreeSet::new();
            for facility in &partition.eligible_facilities {
                if !known(facility.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "network structure partition references an unknown facility node"));
                }
                if !eligible.insert(facility.as_str().into()) {
                    return Err(BridgeError::new("InvalidInput", "network structure repeats an eligible facility"));
                }
            }
            partitions.push(FacilityPartitionRequest { id: partition.id.clone(), eligible_facilities: eligible,
                required: partition.required });
        }
        let mut quota_nodes = BTreeSet::new();
        let mut quotas = Vec::with_capacity(self.quotas.len());
        for quota in &self.quotas {
            if !known(quota.facility.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network structure quota references an unknown facility node"));
            }
            if !quota_nodes.insert(quota.facility.as_str()) {
                return Err(BridgeError::new("InvalidInput", "network structure repeats a facility quota"));
            }
            let minimum = usize::try_from(quota.minimum).map_err(|_| BridgeError::new("InvalidInput", "quota minimum overflow"))?;
            let maximum = quota.maximum.map(usize::try_from).transpose()
                .map_err(|_| BridgeError::new("InvalidInput", "quota maximum overflow"))?;
            if maximum.is_some_and(|maximum| maximum < minimum) {
                return Err(BridgeError::new("InvalidInput", "network structure quota maximum is below its minimum"));
            }
            quotas.push(FacilityAssignmentQuota { facility: quota.facility.as_str().into(), minimum, maximum });
        }
        DomainProblem::new(design, self.root.as_str().into(), terminals, steiner, partitions, quotas)
            .map_err(|error| BridgeError::new("InvalidInput", &error))
    }
}

/// 节点的唯一父弧。 / The unique parent arc of one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkParentArc {
    /// 非根活动节点。 / An active non-root node.
    pub node: String,
    /// 其唯一父弧。 / Its unique parent arc.
    pub arc: String,
}

/// 分区的合格归属设施。 / The eligible owner of one partition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkPartitionOwner {
    /// 分区 ID。 / Partition ID.
    pub partition: String,
    /// 归属设施节点。 / Owning facility node.
    pub owner: String,
}

/// 建设、树与分区归属的领域解。 / Domain solution of construction, tree and partition ownership.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkStructureSolution {
    /// 建设选择与建设费用。 / Construction choices and their cost.
    pub construction: NetworkConstruction,
    /// 选中的树弧，按稳定 ID 排序。 / Selected tree arcs sorted by stable ID.
    pub selected_tree_arcs: Vec<String>,
    /// 根树中的全部节点。 / Every node in the rooted tree.
    pub connected_nodes: Vec<String>,
    /// 每个非根活动节点的唯一父弧。 / The unique parent arc of each active non-root node.
    pub parent_arcs: Vec<NetworkParentArc>,
    /// 已归属分区的合格设施。 / The eligible owner of each assigned partition.
    pub partition_owners: Vec<NetworkPartitionOwner>,
}

/// 统一报告：通用摘要 + 结构领域解。 / Unified report: shared summary plus the structure solution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkStructureReport {
    /// 通用求解摘要。 / Shared solve summary.
    pub summary: ReportSummary,
    /// 领域解；无 incumbent 时为空。 / The domain solution, absent without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<NetworkStructureSolution>,
}

/// 把原领域解展开成 DTO；父弧与归属都保持确定性顺序。
/// Expand the domain solution into DTOs while keeping parent arcs and ownership in a deterministic order.
pub fn decode_network_structure_solution(solution: &DomainSolution<f64>) -> Result<NetworkStructureSolution> {
    Ok(NetworkStructureSolution {
        construction: decode_construction(&solution.construction)?,
        selected_tree_arcs: solution.selected_tree_arcs.iter().map(|id| id.as_str().to_owned()).collect(),
        connected_nodes: solution.connected_nodes.iter().map(|id| id.as_str().to_owned()).collect(),
        parent_arcs: solution.parent_arcs.iter()
            .map(|(node, arc)| NetworkParentArc { node: node.as_str().to_owned(), arc: arc.as_str().to_owned() })
            .collect(),
        partition_owners: solution.partition_owners.iter()
            .map(|(partition, owner)| NetworkPartitionOwner { partition: partition.clone(), owner: owner.as_str().to_owned() })
            .collect(),
    })
}

/// 组装并复核报告。 / Assemble and re-check the report.
pub fn summarize_network_structure_report(report: &SolveReport<DomainSolution<f64>>,
    solution: Option<NetworkStructureSolution>) -> Result<NetworkStructureReport> {
    let summary = summarize_domain_report(report, solution.is_some())?;
    Ok(NetworkStructureReport { summary, solution })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetworkDesignFacility, NetworkDesignLevel, NetworkDesignService, NetworkFlowArc, NetworkFlowCommodity,
        NetworkFlowNode, NetworkFlowNodeQuantity, PhysicalQuantity, QuantityUnit};

    fn mass(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
    fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

    fn problem() -> NetworkStructureProblem {
        NetworkStructureProblem {
            design: NetworkDesignProblem {
                flow: crate::NetworkFlowProblem {
                    nodes: vec![NetworkFlowNode { id: "a".into() }, NetworkFlowNode { id: "b".into() }],
                    arcs: vec![NetworkFlowArc { id: "a-b".into(), from: "a".into(), to: "b".into(),
                        capacity_lower: mass(0.0), capacity_upper: mass(10.0), cost: pure(1.0) }],
                    commodities: vec![NetworkFlowCommodity { id: "c".into(),
                        balances: vec![NetworkFlowNodeQuantity { node: "a".into(), quantity: mass(5.0) },
                            NetworkFlowNodeQuantity { node: "b".into(), quantity: mass(-5.0) }],
                        unit: None, allowed_arcs: None, costs: Vec::new(), capacity_consumption: Vec::new(), integer_step: None }],
                },
                services: vec![NetworkDesignService { arc: "a-b".into(), required: true, levels: vec![
                    NetworkDesignLevel { id: "small".into(), capacity: mass(8.0), cost: mass(3.0) }] }],
                facilities: vec![NetworkDesignFacility { node: "b".into(), cost: mass(2.0), required: false, capacity_rows: Vec::new() }],
                budget: None,
            },
            root: "a".into(),
            terminals: vec!["b".into()],
            steiner_nodes: Vec::new(),
            partitions: vec![NetworkStructurePartition { id: "p1".into(), eligible_facilities: vec!["b".into()], required: true }],
            quotas: Vec::new(),
        }
    }

    #[test]
    fn structure_problems_reject_unknown_nodes_partitions_and_quota_inversions() {
        problem().to_domain().unwrap();
        let broken: Vec<(&str, NetworkStructureProblem)> = vec![
            ("unknown root", NetworkStructureProblem { root: "ghost".into(), ..problem() }),
            ("unknown terminal", NetworkStructureProblem { terminals: vec!["ghost".into()], ..problem() }),
            ("unknown relay", NetworkStructureProblem { steiner_nodes: vec!["ghost".into()], ..problem() }),
            ("empty partition ID", NetworkStructureProblem { partitions: vec![NetworkStructurePartition {
                id: String::new(), eligible_facilities: vec!["b".into()], required: false }], ..problem() }),
            ("unknown eligible facility", NetworkStructureProblem { partitions: vec![NetworkStructurePartition {
                id: "p1".into(), eligible_facilities: vec!["ghost".into()], required: false }], ..problem() }),
            ("repeated partition", NetworkStructureProblem { partitions: vec![
                NetworkStructurePartition { id: "p1".into(), eligible_facilities: vec!["b".into()], required: false },
                NetworkStructurePartition { id: "p1".into(), eligible_facilities: vec!["b".into()], required: false }], ..problem() }),
            ("quota inversion", NetworkStructureProblem { quotas: vec![NetworkStructureQuota {
                facility: "b".into(), minimum: 3, maximum: Some(1) }], ..problem() }),
            ("unknown quota facility", NetworkStructureProblem { quotas: vec![NetworkStructureQuota {
                facility: "ghost".into(), minimum: 0, maximum: None }], ..problem() }),
        ];
        for (label, case) in broken {
            assert!(case.to_domain().is_err(), "{label} must be rejected");
        }
    }
}
