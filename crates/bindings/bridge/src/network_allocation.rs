//! 分配领域绑定：原 `network-scheduling` 的运输分配 + 共享配额 + 枢纽建设。
//! Allocation domain binding: the original `network-scheduling` transportation allocation with shared quotas and hub
//! construction.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework_network_scheduling::domain::allocation::{
    AllocationFlowKey, AllocationHub, AllocationLocation, AllocationOptions, AllocationProblem as DomainProblem,
    AllocationQuota, AllocationRequest, AllocationService, AllocationSolution as DomainSolution,
};
use ospf_rust_framework_network_scheduling::infrastructure::CapacityBounds;
use ospf_rust_core::solver::SolveReport;

use crate::network_design::NetworkDesignLevel;
use crate::network_flow::{decode_network_flow_solution, summarize_domain_report, NetworkFlowQuantity,
    NetworkFlowSolution};
use crate::network_design::NetworkServiceLevel;
use crate::quantities::{PhysicalQuantity, QuantityUnit};
use crate::{BridgeError, ReportSummary, Result};

/// 具名物理量，用于地点与服务的保留属性。 / A named physical amount, used for preserved location/service attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationNamedQuantity {
    /// 属性名。 / Attribute name.
    pub name: String,
    /// 物理量。 / Physical amount.
    pub quantity: PhysicalQuantity,
}

/// 地点；层级用于「服务方向非递减」校验。 / A location; the layer drives the nondecreasing-service direction check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationLocationSpec {
    /// 稳定地点 ID。 / Stable location ID.
    pub id: String,
    /// 可选业务层级。 / Optional business layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub layer: Option<i64>,
    /// 保留的物理属性。 / Preserved physical attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AllocationNamedQuantity>,
}

/// 固定或带容量等级的服务。 / A fixed service or one with capacity levels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationServiceSpec {
    /// 稳定服务 ID，允许平行服务。 / Stable service ID; parallel services are allowed.
    pub id: String,
    /// 起点地点。 / Origin location.
    pub from: String,
    /// 终点地点。 / Destination location.
    pub to: String,
    /// 默认单位容量运行资费，请求可覆盖。 / Default per-capacity-unit tariff, overridable per request.
    pub cost: PhysicalQuantity,
    /// 容量下界。 / Capacity lower bound.
    pub capacity_lower: PhysicalQuantity,
    /// 容量上界。 / Capacity upper bound.
    pub capacity_upper: PhysicalQuantity,
    /// 总容量等级；空表示固定启用。 / Total-capacity levels; empty means fixed availability.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capacity_levels: Vec<NetworkDesignLevel>,
    /// 有等级时必须选择一个。 / Require one level when levels exist.
    #[serde(default)]
    pub required: bool,
    /// 保留的物理属性。 / Preserved physical attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AllocationNamedQuantity>,
}

/// 某请求在某服务上的一个物理量，用于配额用量、容量用量与资费覆盖。
/// A physical amount of one request on one service, used for quota usage, capacity usage and tariff overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationFlowQuantity {
    /// 请求 ID。 / Request ID.
    pub request: String,
    /// 服务 ID。 / Service ID.
    pub service: String,
    /// 物理量。 / Physical amount.
    pub quantity: PhysicalQuantity,
}

/// 单来源至单去向请求。 / A single-origin, single-destination request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationRequestSpec {
    /// 稳定请求 ID，同时是流商品 ID。 / Stable request ID; it is also the flow commodity ID.
    pub id: String,
    /// 起运地点。 / Shipment origin.
    pub origin: String,
    /// 目的地点。 / Shipment destination.
    pub destination: String,
    /// 有限正请求量。 / Finite positive amount.
    pub quantity: PhysicalQuantity,
    /// 请求自己的流量单位；省略时沿用 `quantity` 的单位。 / Request flow unit; omission reuses the `quantity` unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unit: Option<QuantityUnit>,
    /// 每个整数计数的物理量；不表示整单二元选择。 / Physical amount per integer count; not a whole-order binary choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub integer_step: Option<PhysicalQuantity>,
    /// 可用服务；省略表示全部。 / Compatible services; omission means all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allowed_services: Option<Vec<String>>,
    /// 按请求单位计量的服务资费覆盖。 / Per-request-unit tariff overrides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub costs: Vec<AllocationFlowQuantity>,
}

/// 显式共享配额；未声明的用量为零。 / An explicit shared quota; undeclared usage is zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationQuotaSpec {
    /// 唯一配额 ID。 / Unique quota ID.
    pub id: String,
    /// 配额下界。 / Quota lower bound.
    pub lower: PhysicalQuantity,
    /// 配额上界。 / Quota upper bound.
    pub upper: PhysicalQuantity,
    /// 每请求单位消耗的配额量。 / Quota consumed per request unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub usage: Vec<AllocationFlowQuantity>,
}

/// 枢纽建设及其控制的配额。 / Hub construction and the quotas it controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationHubSpec {
    /// 枢纽地点。 / Hub location.
    pub location: String,
    /// 开设费用。 / Opening cost.
    pub opening_cost: PhysicalQuantity,
    /// 是否必须开设。 / Whether opening is required.
    #[serde(default)]
    pub required: bool,
    /// 受其控制的配额 ID。 / Quota IDs it controls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quota_ids: Vec<String>,
}

/// 物理口径与可选共享资源配置。 / Physical units and optional shared-resource configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationOptionsSpec {
    /// 所有服务容量的规范单位。 / Canonical unit for service capacities.
    pub capacity_unit: QuantityUnit,
    /// 运行加建设的总费用单位。 / Total operating-and-construction cost unit.
    pub cost_unit: QuantityUnit,
    /// 共享配额。 / Shared quotas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quotas: Vec<AllocationQuotaSpec>,
    /// 可选枢纽。 / Optional hubs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hubs: Vec<AllocationHubSpec>,
    /// 每请求单位对服务容量的显式消耗。 / Explicit service-capacity usage per request unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_usage: Vec<AllocationFlowQuantity>,
    /// 可选建设预算。 / Optional construction budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub budget: Option<PhysicalQuantity>,
}

/// 分配问题。 / An allocation problem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationProblem {
    /// 地点快照。 / Location snapshot.
    pub locations: Vec<AllocationLocationSpec>,
    /// 服务快照。 / Service snapshot.
    pub services: Vec<AllocationServiceSpec>,
    /// 请求快照。 / Request snapshot.
    pub requests: Vec<AllocationRequestSpec>,
    /// 物理口径与共享资源。 / Physical units and shared resources.
    pub options: AllocationOptionsSpec,
}

impl AllocationProblem {
    /// 映射到原领域问题；引用、重复、层级方向与共享资源交叉引用都在这里先查一遍。
    /// Map onto the original domain problem; references, duplicates, layer direction and shared-resource cross
    /// references are all checked here first.
    pub fn to_domain(&self) -> Result<DomainProblem<f64>> {
        let mut location_ids = BTreeSet::new();
        let mut layers = BTreeMap::new();
        let mut locations = Vec::with_capacity(self.locations.len());
        for location in &self.locations {
            if location.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "allocation location ID must not be empty"));
            }
            if !location_ids.insert(location.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation repeats a location ID"));
            }
            layers.insert(location.id.as_str(), location.layer);
            let mut attributes = BTreeMap::new();
            for attribute in &location.attributes {
                if attributes.insert(attribute.name.clone(), attribute.quantity.quantity()?).is_some() {
                    return Err(BridgeError::new("InvalidInput", "allocation repeats a location attribute"));
                }
            }
            locations.push(AllocationLocation { id: location.id.as_str().into(), layer: location.layer, attributes });
        }
        let mut service_ids = BTreeSet::new();
        let mut services = Vec::with_capacity(self.services.len());
        for service in &self.services {
            if service.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "allocation service ID must not be empty"));
            }
            if !service_ids.insert(service.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation repeats a service ID"));
            }
            for endpoint in [service.from.as_str(), service.to.as_str()] {
                if !location_ids.contains(endpoint) {
                    return Err(BridgeError::new("InvalidInput", "allocation service references an unknown location"));
                }
            }
            let mut attributes = BTreeMap::new();
            for attribute in &service.attributes {
                if attributes.insert(attribute.name.clone(), attribute.quantity.quantity()?).is_some() {
                    return Err(BridgeError::new("InvalidInput", "allocation repeats a service attribute"));
                }
            }
            let mut level_ids = BTreeSet::new();
            let mut levels = Vec::with_capacity(service.capacity_levels.len());
            for level in &service.capacity_levels {
                if level.id.trim().is_empty() || !level_ids.insert(level.id.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "allocation level ID is empty or repeated within one service"));
                }
                levels.push(ospf_rust_framework_network_scheduling::domain::design::CapacityLevel { id: level.id.clone(),
                    capacity: level.capacity.quantity()?, cost: level.cost.quantity()? });
            }
            services.push(AllocationService { id: service.id.as_str().into(), from: service.from.as_str().into(),
                to: service.to.as_str().into(), cost: service.cost.quantity()?,
                capacity: CapacityBounds::try_new(service.capacity_lower.quantity()?, service.capacity_upper.quantity()?)
                    .map_err(|error| BridgeError::new("InvalidInput", &error))?,
                capacity_levels: levels, required: service.required, attributes });
        }
        let mut request_ids = BTreeSet::new();
        let mut requests = Vec::with_capacity(self.requests.len());
        for request in &self.requests {
            if request.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "allocation request ID must not be empty"));
            }
            if !request_ids.insert(request.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation repeats a request ID"));
            }
            for endpoint in [request.origin.as_str(), request.destination.as_str()] {
                if !location_ids.contains(endpoint) {
                    return Err(BridgeError::new("InvalidInput", "allocation request references an unknown location"));
                }
            }
            let quantity = request.quantity.quantity()?;
            let unit = request.unit.map(QuantityUnit::unit).unwrap_or_else(|| quantity.unit.clone());
            let mut allowed = None;
            if let Some(ids) = &request.allowed_services {
                let mut set = BTreeSet::new();
                for id in ids {
                    if !service_ids.contains(id.as_str()) {
                        return Err(BridgeError::new("InvalidInput", "allocation request references an unknown compatible service"));
                    }
                    if !set.insert(id.as_str().to_owned()) {
                        return Err(BridgeError::new("InvalidInput", "allocation repeats a compatible service"));
                    }
                }
                allowed = Some(set.into_iter().map(|id| id.as_str().into()).collect());
            }
            let mut costs = BTreeMap::new();
            for cost in &request.costs {
                if !service_ids.contains(cost.service.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "allocation tariff references an unknown service"));
                }
                if cost.request != request.id {
                    return Err(BridgeError::new("InvalidInput", "allocation tariff must belong to its own request"));
                }
                if costs.insert(cost.service.as_str().into(), cost.quantity.quantity()?).is_some() {
                    return Err(BridgeError::new("InvalidInput", "allocation repeats a tariff for one service"));
                }
            }
            requests.push(AllocationRequest { id: request.id.as_str().into(), origin: request.origin.as_str().into(),
                destination: request.destination.as_str().into(), quantity, unit,
                integer_step: request.integer_step.as_ref().map(PhysicalQuantity::quantity).transpose()?,
                allowed_services: allowed, costs });
        }
        // 共享配额的交叉引用必须在建模型之前查清：未知或不兼容的请求-服务对不能悄悄按零处理。
        // Shared-quota cross references must be resolved before the model is built: an unknown or incompatible
        // request-service pair must never be silently treated as zero.
        let key = |entry: &AllocationFlowQuantity| -> Result<AllocationFlowKey> {
            if !service_ids.contains(entry.service.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation usage references an unknown service"));
            }
            if !request_ids.contains(entry.request.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation usage references an unknown request"));
            }
            Ok(AllocationFlowKey { request: entry.request.as_str().into(), service: entry.service.as_str().into() })
        };
        let mut quota_ids = BTreeSet::new();
        let mut quotas = Vec::with_capacity(self.options.quotas.len());
        for quota in &self.options.quotas {
            if quota.id.trim().is_empty() || !quota_ids.insert(quota.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation quota ID is empty or repeated"));
            }
            let mut usage = BTreeMap::new();
            for entry in &quota.usage {
                if usage.insert(key(entry)?, entry.quantity.quantity()?).is_some() {
                    return Err(BridgeError::new("InvalidInput", "allocation repeats a quota usage pair"));
                }
            }
            quotas.push(AllocationQuota { id: quota.id.clone(),
                bounds: CapacityBounds::try_new(quota.lower.quantity()?, quota.upper.quantity()?)
                    .map_err(|error| BridgeError::new("InvalidInput", &error))?,
                usage });
        }
        let mut hub_nodes = BTreeSet::new();
        let mut hubs = Vec::with_capacity(self.options.hubs.len());
        for hub in &self.options.hubs {
            if !location_ids.contains(hub.location.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation hub references an unknown location"));
            }
            if !hub_nodes.insert(hub.location.as_str()) {
                return Err(BridgeError::new("InvalidInput", "allocation repeats a hub for one location"));
            }
            let mut ids = BTreeSet::new();
            for id in &hub.quota_ids {
                if !quota_ids.contains(id.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "allocation hub references an unknown quota"));
                }
                if !ids.insert(id.clone()) {
                    return Err(BridgeError::new("InvalidInput", "allocation repeats a quota within one hub"));
                }
            }
            hubs.push(AllocationHub { location: hub.location.as_str().into(), opening_cost: hub.opening_cost.quantity()?,
                required: hub.required, quota_ids: ids });
        }
        let mut service_usage = BTreeMap::new();
        for entry in &self.options.service_usage {
            if service_usage.insert(key(entry)?, entry.quantity.quantity()?).is_some() {
                return Err(BridgeError::new("InvalidInput", "allocation repeats a service-capacity usage pair"));
            }
        }
        let options = AllocationOptions { capacity_unit: self.options.capacity_unit.unit(),
            cost_unit: self.options.cost_unit.unit(), quotas, hubs, service_usage,
            budget: self.options.budget.as_ref().map(PhysicalQuantity::quantity).transpose()? };
        DomainProblem::new(locations, services, requests, options).map_err(|error| BridgeError::new("InvalidInput", &error))
    }
}

/// 一个请求在一个服务上的实际运输量。 / The actual shipped amount of one request on one service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationShipment {
    /// 请求 ID。 / Request ID.
    pub request: String,
    /// 服务 ID。 / Service ID.
    pub service: String,
    /// 实际物理量，不是整数计数。 / Actual physical amount, not integer counts.
    pub quantity: NetworkFlowQuantity,
}

/// 运输分配与建设决策的领域解。 / Domain solution of transportation allocation and construction decisions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationSolution {
    /// 各请求-服务的实际物理量。 / Actual amount per request-service pair.
    pub shipments: Vec<AllocationShipment>,
    /// 经生产复核的底层流结果。 / Production-validated underlying flow result.
    pub flow: NetworkFlowSolution,
    /// 已开设枢纽，按稳定 ID 排序。 / Open hubs sorted by stable ID.
    pub open_hubs: Vec<String>,
    /// 选中的服务容量等级。 / Selected service capacity levels.
    pub service_levels: Vec<NetworkServiceLevel>,
    /// 建设费用。 / Construction cost.
    pub construction_cost: NetworkFlowQuantity,
    /// 运行加建设费用。 / Operating-plus-construction cost.
    pub total_cost: NetworkFlowQuantity,
}

/// 统一报告：通用摘要 + 分配领域解。 / Unified report: shared summary plus the allocation solution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AllocationReport {
    /// 通用求解摘要。 / Shared solve summary.
    pub summary: ReportSummary,
    /// 领域解；无 incumbent 时为空。 / The domain solution, absent without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<AllocationSolution>,
}

fn quantity_of(quantity: &ospf_rust_quantities::quantity::Quantity<f64, ospf_rust_quantities::unit::Unit>)
    -> Result<NetworkFlowQuantity> {
    if !quantity.value.is_finite() {
        return Err(BridgeError::new("InvalidReport", "allocation quantity is not finite"));
    }
    Ok(NetworkFlowQuantity { value: quantity.value, unit: quantity.unit.symbol().to_owned() })
}

/// 把原领域解展开成 DTO。 / Expand the domain solution into DTOs.
pub fn decode_allocation_solution(solution: &DomainSolution<f64>) -> Result<AllocationSolution> {
    Ok(AllocationSolution {
        shipments: solution.shipments.iter()
            .map(|(key, quantity)| Ok(AllocationShipment { request: key.request.as_str().to_owned(),
                service: key.service.as_str().to_owned(), quantity: quantity_of(quantity)? }))
            .collect::<Result<Vec<_>>>()?,
        flow: decode_network_flow_solution(&solution.flow)?,
        open_hubs: solution.open_hubs.iter().map(|id| id.as_str().to_owned()).collect(),
        service_levels: solution.service_levels.iter()
            .map(|(service, level)| NetworkServiceLevel { arc: service.as_str().to_owned(), level: level.clone() })
            .collect(),
        construction_cost: quantity_of(&solution.construction_cost)?,
        total_cost: quantity_of(&solution.total_cost)?,
    })
}

/// 组装并复核报告。 / Assemble and re-check the report.
pub fn summarize_allocation_report(report: &SolveReport<DomainSolution<f64>>, solution: Option<AllocationSolution>)
    -> Result<AllocationReport> {
    let summary = summarize_domain_report(report, solution.is_some())?;
    Ok(AllocationReport { summary, solution })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kg(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
    fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }

    fn problem() -> AllocationProblem {
        AllocationProblem {
            locations: vec![AllocationLocationSpec { id: "a".into(), layer: Some(0), attributes: Vec::new() },
                AllocationLocationSpec { id: "b".into(), layer: Some(1), attributes: Vec::new() }],
            services: vec![AllocationServiceSpec { id: "a-b".into(), from: "a".into(), to: "b".into(), cost: pure(1.0),
                capacity_lower: kg(0.0), capacity_upper: kg(10.0), capacity_levels: Vec::new(), required: false,
                attributes: Vec::new() }],
            requests: vec![AllocationRequestSpec { id: "r".into(), origin: "a".into(), destination: "b".into(),
                quantity: kg(5.0), unit: None, integer_step: None, allowed_services: None, costs: Vec::new() }],
            // 服务成本是「每容量单位」的比值：capacityUnit 与 costUnit 同量纲时成本无量纲。
            // A service cost is per capacity unit, so with capacityUnit and costUnit sharing a dimension the cost itself
            // is dimensionless.
            options: AllocationOptionsSpec { capacity_unit: QuantityUnit::Kilogram, cost_unit: QuantityUnit::Kilogram,
                quotas: Vec::new(), hubs: Vec::new(), service_usage: Vec::new(), budget: None },
        }
    }

    #[test]
    fn allocation_problems_reject_bad_references_and_shared_resource_cross_links() {
        problem().to_domain().unwrap();
        let broken: Vec<(&str, AllocationProblem)> = vec![
            ("unknown service endpoint", AllocationProblem { services: vec![AllocationServiceSpec { to: "ghost".into(),
                ..problem().services[0].clone() }], ..problem() }),
            ("same origin and destination", AllocationProblem { requests: vec![AllocationRequestSpec {
                destination: "a".into(), ..problem().requests[0].clone() }], ..problem() }),
            ("unknown compatible service", AllocationProblem { requests: vec![AllocationRequestSpec {
                allowed_services: Some(vec!["ghost".into()]), ..problem().requests[0].clone() }], ..problem() }),
            ("tariff for another request", AllocationProblem { requests: vec![AllocationRequestSpec {
                costs: vec![AllocationFlowQuantity { request: "other".into(), service: "a-b".into(), quantity: pure(1.0) }],
                ..problem().requests[0].clone() }], ..problem() }),
            ("quota usage on unknown service", AllocationProblem { options: AllocationOptionsSpec {
                quotas: vec![AllocationQuotaSpec { id: "q".into(), lower: kg(0.0), upper: kg(1.0),
                    usage: vec![AllocationFlowQuantity { request: "r".into(), service: "ghost".into(), quantity: kg(1.0) }] }],
                ..problem().options }, ..problem() }),
            ("hub referencing unknown quota", AllocationProblem { options: AllocationOptionsSpec {
                hubs: vec![AllocationHubSpec { location: "a".into(), opening_cost: kg(1.0), required: false,
                    quota_ids: vec!["ghost".into()] }], ..problem().options }, ..problem() }),
            ("negative request amount", AllocationProblem { requests: vec![AllocationRequestSpec {
                quantity: kg(-1.0), ..problem().requests[0].clone() }], ..problem() }),
            ("layer direction reversed", AllocationProblem { locations: vec![
                AllocationLocationSpec { id: "a".into(), layer: Some(2), attributes: Vec::new() },
                AllocationLocationSpec { id: "b".into(), layer: Some(1), attributes: Vec::new() }], ..problem() }),
        ];
        for (label, case) in broken {
            assert!(case.to_domain().is_err(), "{label} must be rejected");
        }
    }
}
