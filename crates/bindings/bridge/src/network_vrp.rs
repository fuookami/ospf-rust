//! VRPTW 领域绑定：原 `network-scheduling` 的带时间窗车辆路径（分支定价）。
//! VRPTW domain binding: the original `network-scheduling` vehicle routing with time windows (branch and price).

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use ts_rs::TS;

use ospf_rust_framework_network_scheduling::domain::vrp::{
    Coordinate, Customer, CustomerId, Depot, ServiceTimeWindow, TravelTime, VehicleType, VehicleTypeId, VrptwArc,
    VrptwInstance, VrptwSolution as DomainSolution, VrptwTolerances, VrptwUnits,
};
use ospf_rust_framework_gantt_scheduling::infrastructure::{TimeRange, TimeWindow};
use ospf_rust_core::solver::SolveReport;

use crate::network_flow::{summarize_domain_report, NetworkFlowQuantity};
use crate::quantities::{PhysicalQuantity, QuantityUnit};
use crate::{BridgeError, ReportSummary, Result};

/// 一条带单位的坐标轴。 / One unit-bearing coordinate axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpAxis {
    /// 轴名。 / Axis name.
    pub name: String,
    /// 该轴上的坐标值。 / Coordinate value on that axis.
    pub value: PhysicalQuantity,
}

/// 网络节点：稳定 ID + 坐标。 / A network node: stable ID plus coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpNodeSpec {
    /// 稳定节点 ID。 / Stable node ID.
    pub id: String,
    /// 坐标轴；所有节点必须使用同一组轴名。 / Coordinate axes; every node must share the same axis names.
    pub axes: Vec<VrpAxis>,
}

/// 闭区间服务时间窗（RFC 3339 绝对时刻）。 / A closed service-time window of RFC 3339 instants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpTimeWindow {
    /// 最早服务开始时刻。 / Earliest service-start instant.
    pub ready: String,
    /// 最晚服务开始时刻；等号可行。 / Latest service-start instant; equality is feasible.
    pub due: String,
}

/// 起止仓库。 / A start or end depot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpDepotSpec {
    /// 仓库节点。 / Depot node.
    pub node: VrpNodeSpec,
    /// 仓库开放时间窗。 / Depot opening window.
    pub window: VrpTimeWindow,
}

/// 客户。 / A customer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpCustomerSpec {
    /// 稳定客户 ID。 / Stable customer ID.
    pub id: String,
    /// 客户节点。 / Customer node.
    pub node: VrpNodeSpec,
    /// 非负需求。 / Non-negative demand.
    pub demand: PhysicalQuantity,
    /// 服务时间窗。 / Service-time window.
    pub window: VrpTimeWindow,
    /// 服务持续时间。 / Service duration.
    pub service_time: PhysicalQuantity,
}

/// 有限车队中的车辆类型。 / A vehicle type in a finite fleet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpVehicleTypeSpec {
    /// 车辆类型 ID。 / Vehicle-type ID.
    pub id: String,
    /// 单车容量；必须为正。 / Per-vehicle capacity; must be positive.
    pub capacity: PhysicalQuantity,
    /// 固定使用成本。 / Fixed usage cost.
    pub fixed_cost: PhysicalQuantity,
    /// 可用车辆数；必须为正。 / Available vehicle count; must be positive.
    pub amount: u32,
}

/// 可选基础网络弧；省略时使用策略生成的隐式完全图。 / An optional base arc; omission uses the policy-generated complete graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpArcSpec {
    /// 稳定弧 ID。 / Stable arc ID.
    pub id: String,
    /// 起点节点。 / Origin node.
    pub from: String,
    /// 终点节点。 / Destination node.
    pub to: String,
    /// 弧距离。 / Arc distance.
    pub distance: PhysicalQuantity,
    /// 弧行驶时间。 / Arc travel time.
    pub travel_time: PhysicalQuantity,
    /// 弧成本。 / Arc cost.
    pub cost: PhysicalQuantity,
    /// 是否允许该弧；省略表示允许。 / Whether the arc is allowed; omission means allowed.
    #[serde(default = "allowed_by_default")]
    pub feasible: bool,
}

fn allowed_by_default() -> bool { true }

/// 单位口径。 / The unit contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpUnitsSpec {
    /// 距离单位；必须是线性长度单位。 / Distance unit; must be a linear length unit.
    pub distance: QuantityUnit,
    /// 负载单位；必须是线性单位。 / Load unit; must be linear.
    pub load: QuantityUnit,
    /// 成本单位；必须是线性单位。 / Cost unit; must be linear.
    pub cost: QuantityUnit,
}

/// 算法容差；省略时使用领域默认值。 / Algorithm tolerances; omission uses the domain defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpTolerancesSpec {
    /// 可行性容差。 / Feasibility tolerance.
    pub feasibility: f64,
    /// 定价容差。 / Pricing tolerance.
    pub pricing: f64,
    /// 整数性容差。 / Integrality tolerance.
    pub integrality: f64,
    /// 成本复核容差。 / Cost-validation tolerance.
    pub cost_validation: f64,
    /// 相对 gap 容差。 / Relative-gap tolerance.
    pub relative_gap: f64,
}

impl Default for VrpTolerancesSpec {
    fn default() -> Self {
        let defaults = VrptwTolerances::default();
        Self { feasibility: defaults.feasibility, pricing: defaults.pricing, integrality: defaults.integrality,
            cost_validation: defaults.cost_validation, relative_gap: defaults.relative_gap }
    }
}

/// 分支定价预算；时间上限来自请求的 `timeLimitMs`。 / Branch-and-price budget; the time limit comes from the request's `timeLimitMs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpSearchOptions {
    /// 分支节点上限；省略表示不限。 / Branch-node limit; omission means unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node_limit: Option<u32>,
    /// 相对 gap 容差；省略表示沿用 `tolerances.relativeGap`。 / Relative-gap tolerance; omission reuses `tolerances.relativeGap`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relative_gap_tolerance: Option<f64>,
    /// 每节点列生成迭代上限；省略表示沿用领域默认值。 / Per-node column-generation iteration limit; omission reuses the domain default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_cg_iterations_per_node: Option<u32>,
}

/// VRPTW 实例。 / A VRPTW instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpProblem {
    /// 实例名称。 / Instance name.
    pub name: String,
    /// 起始仓库。 / Start depot.
    pub start_depot: VrpDepotSpec,
    /// 结束仓库；必须使用与起始仓库不同的节点 ID。 / End depot; its node ID must differ from the start depot.
    pub end_depot: VrpDepotSpec,
    /// 客户快照。 / Customer snapshot.
    pub customers: Vec<VrpCustomerSpec>,
    /// 车辆类型快照。 / Vehicle-type snapshot.
    pub vehicle_types: Vec<VrpVehicleTypeSpec>,
    /// 可选基础网络弧目录。 / Optional base-arc catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arcs: Vec<VrpArcSpec>,
    /// 业务绝对时间轴。 / The business absolute time axis.
    pub scheduling_window: VrpTimeWindow,
    /// 单位口径。 / The unit contract.
    pub units: VrpUnitsSpec,
    /// 算法容差。 / Algorithm tolerances.
    #[serde(default)]
    pub tolerances: VrpTolerancesSpec,
    /// 分支定价预算。 / Branch-and-price budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub search: Option<VrpSearchOptions>,
}

/// 解析 RFC 3339 时刻。 / Parse an RFC 3339 instant.
fn instant(text: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|_| BridgeError::new("InvalidInput", "VRP times must be RFC 3339 instants"))
}

/// 规范文本。 / Canonical text.
fn canonical(value: OffsetDateTime) -> String { value.format(&Rfc3339).unwrap_or_default() }

fn window_of(window: &VrpTimeWindow) -> Result<ServiceTimeWindow> {
    ServiceTimeWindow::new(instant(&window.ready)?, instant(&window.due)?)
        .map_err(|error| BridgeError::new("InvalidInput", &error))
}

/// 把物理量当作时长；必须是时间量纲。 / Treat a physical amount as a duration; it must be a time dimension.
fn duration_of(quantity: &PhysicalQuantity) -> Result<Duration> {
    use ospf_rust_quantities::unit::CTUnit;
    let value = quantity.quantity()?;
    let seconds = value.to_unit(&ospf_rust_quantities::unit::Second::INSTANT)
        .map_err(|error| BridgeError::new("InvalidInput", &format!("VRP durations must be time quantities: {error}")))?;
    Ok(Duration::seconds_f64(seconds.value))
}

fn node_of(node: &VrpNodeSpec) -> Result<ospf_rust_framework_network_scheduling::domain::vrp::VrpNode<f64>> {
    if node.id.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput", "VRP node ID must not be empty"));
    }
    if node.axes.is_empty() {
        return Err(BridgeError::new("InvalidInput", "VRP nodes must carry non-empty coordinate axes"));
    }
    let mut axes = Vec::with_capacity(node.axes.len());
    for axis in &node.axes {
        if axis.name.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "VRP axis names must not be empty"));
        }
        axes.push((axis.name.clone(), axis.value.quantity()?));
    }
    let coordinate = Coordinate::new(axes).map_err(|error| BridgeError::new("InvalidInput", &error))?;
    Ok(ospf_rust_framework_network_scheduling::infrastructure::NetworkNode::new(node.id.as_str(), coordinate))
}

impl VrpProblem {
    /// 映射到原领域实例；引用、重复、单位与时间窗都在建模前校验。
    /// Map onto the original domain instance; references, duplicates, units and time windows are all checked before
    /// modelling.
    pub fn to_domain(&self) -> Result<Arc<VrptwInstance<f64>>> {
        let units = VrptwUnits::new(self.units.distance.unit(), self.units.load.unit(), self.units.cost.unit())
            .map_err(|error| BridgeError::new("InvalidInput", &error))?;
        let tolerances = VrptwTolerances::new(self.tolerances.feasibility, self.tolerances.pricing,
            self.tolerances.integrality, self.tolerances.cost_validation, self.tolerances.relative_gap)
            .map_err(|error| BridgeError::new("InvalidInput", &error))?;
        let start_depot = Depot { node: node_of(&self.start_depot.node)?,
            time_window: window_of(&self.start_depot.window)? };
        let end_depot = Depot { node: node_of(&self.end_depot.node)?, time_window: window_of(&self.end_depot.window)? };
        if start_depot.node.id == end_depot.node.id {
            return Err(BridgeError::new("InvalidInput", "VRP start and end depots must use different node IDs"));
        }
        let mut customer_ids = HashSet::new();
        let mut customers = Vec::with_capacity(self.customers.len());
        for customer in &self.customers {
            if customer.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "VRP customer ID must not be empty"));
            }
            if !customer_ids.insert(customer.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "VRP repeats a customer ID"));
            }
            customers.push(Customer { id: CustomerId::new(customer.id.as_str()), node: node_of(&customer.node)?,
                demand: customer.demand.quantity()?, time_window: window_of(&customer.window)?,
                service_time: duration_of(&customer.service_time)? });
        }
        let mut vehicle_ids = HashSet::new();
        let mut vehicle_types = Vec::with_capacity(self.vehicle_types.len());
        for vehicle in &self.vehicle_types {
            if vehicle.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "VRP vehicle-type ID must not be empty"));
            }
            if !vehicle_ids.insert(vehicle.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "VRP repeats a vehicle-type ID"));
            }
            let amount = usize::try_from(vehicle.amount)
                .map_err(|_| BridgeError::new("InvalidInput", "VRP vehicle count overflow"))?;
            vehicle_types.push(VehicleType::new(VehicleTypeId::new(vehicle.id.as_str()), vehicle.capacity.quantity()?,
                vehicle.fixed_cost.quantity()?, amount).map_err(|error| BridgeError::new("InvalidInput", &error))?);
        }
        let mut arc_ids = HashSet::new();
        let mut arcs = Vec::with_capacity(self.arcs.len());
        for arc in &self.arcs {
            if arc.id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "VRP base-arc ID must not be empty"));
            }
            if !arc_ids.insert(arc.id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "VRP repeats a base-arc ID"));
            }
            let travel = duration_of(&arc.travel_time)?;
            arcs.push(VrptwArc::new(arc.id.as_str(), arc.from.as_str(), arc.to.as_str(), arc.distance.quantity()?,
                TravelTime::new(travel).map_err(|error| BridgeError::new("InvalidInput", &error))?, arc.cost.quantity()?)
                .map_err(|error| BridgeError::new("InvalidInput", &error))?.with_feasibility(arc.feasible));
        }
        let (window_start, window_end) = (instant(&self.scheduling_window.ready)?, instant(&self.scheduling_window.due)?);
        if window_start >= window_end {
            return Err(BridgeError::new("InvalidInput", "VRP scheduling window must be nonempty"));
        }
        let scheduling_window = TimeWindow::seconds(TimeRange::new(window_start, window_end), 0.0, false, 1.0);
        let instance = if arcs.is_empty() {
            VrptwInstance::new(self.name.clone(), start_depot, end_depot, customers, vehicle_types, scheduling_window,
                units, tolerances)
        } else {
            VrptwInstance::new_with_arcs(self.name.clone(), start_depot, end_depot, customers, vehicle_types,
                scheduling_window, units, tolerances, arcs)
        };
        instance.map(Arc::new).map_err(|error| BridgeError::new("InvalidInput", &error))
    }
}

/// 路线停靠点的完整资源状态。 / The complete resource state at one route stop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpStop {
    /// 节点 ID。 / Node ID.
    pub node: String,
    /// 客户 ID；仓库处为空。 / Customer ID; absent at depots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub customer: Option<String>,
    /// 到达时刻。 / Arrival instant.
    pub arrival: String,
    /// 服务开始时刻。 / Service-start instant.
    pub service_start: String,
    /// 离开时刻。 / Departure instant.
    pub departure: String,
    /// 累计负载。 / Accumulated load.
    pub accumulated_load: NetworkFlowQuantity,
}

/// 单车 elementary 路线。 / An elementary single-vehicle route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpRoute {
    /// 车辆类型 ID。 / Vehicle-type ID.
    pub vehicle_type: String,
    /// 有序停靠点。 / Ordered stops.
    pub stops: Vec<VrpStop>,
    /// 有序稳定弧 ID。 / Ordered stable arc IDs.
    pub arc_ids: Vec<String>,
    /// 总距离。 / Total distance.
    pub distance: NetworkFlowQuantity,
    /// 总成本。 / Total cost.
    pub cost: NetworkFlowQuantity,
}

/// 已校验路线集合。 / A validated route set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpSolution {
    /// 路线快照。 / Route snapshot.
    pub routes: Vec<VrpRoute>,
    /// 总距离。 / Total distance.
    pub total_distance: NetworkFlowQuantity,
    /// 总成本。 / Total cost.
    pub total_cost: NetworkFlowQuantity,
}

/// 统一报告：通用摘要 + VRP 领域解。 / Unified report: shared summary plus the VRP solution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VrpReport {
    /// 通用求解摘要。 / Shared solve summary.
    pub summary: ReportSummary,
    /// 领域解；无 incumbent 时为空。 / The domain solution, absent without an incumbent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution: Option<VrpSolution>,
}

fn quantity_of(quantity: &ospf_rust_quantities::quantity::Quantity<f64, ospf_rust_quantities::unit::Unit>)
    -> Result<NetworkFlowQuantity> {
    if !quantity.value.is_finite() {
        return Err(BridgeError::new("InvalidReport", "VRP quantity is not finite"));
    }
    Ok(NetworkFlowQuantity { value: quantity.value, unit: quantity.unit.symbol().to_owned() })
}

/// 把原领域解展开成 DTO。 / Expand the domain solution into DTOs.
pub fn decode_vrp_solution(solution: &DomainSolution<f64>) -> Result<VrpSolution> {
    Ok(VrpSolution {
        routes: solution.routes.iter().map(|route| Ok(VrpRoute {
            vehicle_type: route.vehicle_type_id.as_str().to_owned(),
            stops: route.stops.iter().map(|stop| Ok(VrpStop {
                node: stop.node_id.as_str().to_owned(),
                customer: stop.customer_id.as_ref().map(|id| id.as_str().to_owned()),
                arrival: canonical(stop.arrival), service_start: canonical(stop.service_start),
                departure: canonical(stop.departure), accumulated_load: quantity_of(&stop.accumulated_load)?,
            })).collect::<Result<Vec<_>>>()?,
            arc_ids: route.effective_arc_ids().iter().map(|id| id.as_str().to_owned()).collect(),
            distance: quantity_of(&route.distance)?, cost: quantity_of(&route.cost)?,
        })).collect::<Result<Vec<_>>>()?,
        total_distance: quantity_of(&solution.total_distance)?,
        total_cost: quantity_of(&solution.total_cost)?,
    })
}

/// 组装并复核报告。 / Assemble and re-check the report.
pub fn summarize_vrp_report(report: &SolveReport<DomainSolution<f64>>, solution: Option<VrpSolution>) -> Result<VrpReport> {
    let summary = summarize_domain_report(report, solution.is_some())?;
    Ok(VrpReport { summary, solution })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meter(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Meter } }
    fn kg(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Kilogram } }
    fn pure(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Dimensionless } }
    fn seconds(value: f64) -> PhysicalQuantity { PhysicalQuantity { value, unit: QuantityUnit::Second } }

    fn node(id: &str, x: f64, y: f64) -> VrpNodeSpec {
        VrpNodeSpec { id: id.into(), axes: vec![
            VrpAxis { name: "x".into(), value: meter(x) }, VrpAxis { name: "y".into(), value: meter(y) }] }
    }

    fn problem() -> VrpProblem {
        VrpProblem {
            name: "tiny".into(),
            start_depot: VrpDepotSpec { node: node("start", 0.0, 0.0),
                window: VrpTimeWindow { ready: "2024-01-01T00:00:00Z".into(), due: "2024-01-01T10:00:00Z".into() } },
            end_depot: VrpDepotSpec { node: node("end", 0.0, 0.0),
                window: VrpTimeWindow { ready: "2024-01-01T00:00:00Z".into(), due: "2024-01-01T10:00:00Z".into() } },
            customers: vec![VrpCustomerSpec { id: "c1".into(), node: node("c1-node", 1000.0, 0.0), demand: kg(1.0),
                window: VrpTimeWindow { ready: "2024-01-01T00:00:00Z".into(), due: "2024-01-01T10:00:00Z".into() },
                service_time: seconds(0.0) }],
            vehicle_types: vec![VrpVehicleTypeSpec { id: "v".into(), capacity: kg(10.0), fixed_cost: pure(0.0), amount: 2 }],
            arcs: Vec::new(),
            scheduling_window: VrpTimeWindow { ready: "2024-01-01T00:00:00Z".into(), due: "2024-01-01T10:00:00Z".into() },
            units: VrpUnitsSpec { distance: QuantityUnit::Meter, load: QuantityUnit::Kilogram,
                cost: QuantityUnit::Dimensionless },
            tolerances: VrpTolerancesSpec::default(),
            search: None,
        }
    }

    #[test]
    fn vrp_problems_reject_bad_references_units_and_windows() {
        problem().to_domain().unwrap();
        let broken: Vec<(&str, VrpProblem)> = vec![
            ("same depot node", VrpProblem { end_depot: VrpDepotSpec { node: node("start", 0.0, 0.0),
                window: problem().end_depot.window }, ..problem() }),
            ("blank customer ID", VrpProblem { customers: vec![VrpCustomerSpec { id: "  ".into(),
                ..problem().customers[0].clone() }], ..problem() }),
            ("repeated customer ID", VrpProblem { customers: vec![problem().customers[0].clone(),
                VrpCustomerSpec { node: node("other", 500.0, 0.0), ..problem().customers[0].clone() }], ..problem() }),
            ("window due before ready", VrpProblem { customers: vec![VrpCustomerSpec {
                window: VrpTimeWindow { ready: "2024-01-01T10:00:00Z".into(), due: "2024-01-01T00:00:00Z".into() },
                ..problem().customers[0].clone() }], ..problem() }),
            ("customer window outside the axis", VrpProblem { customers: vec![VrpCustomerSpec {
                window: VrpTimeWindow { ready: "2024-01-01T00:00:00Z".into(), due: "2024-01-02T00:00:00Z".into() },
                ..problem().customers[0].clone() }], ..problem() }),
            ("zero vehicle count", VrpProblem { vehicle_types: vec![VrpVehicleTypeSpec { amount: 0,
                ..problem().vehicle_types[0].clone() }], ..problem() }),
            ("demand above every capacity", VrpProblem { customers: vec![VrpCustomerSpec { demand: kg(11.0),
                ..problem().customers[0].clone() }], ..problem() }),
            ("non-length distance unit", VrpProblem { units: VrpUnitsSpec { distance: QuantityUnit::Kilogram,
                ..problem().units }, ..problem() }),
            ("service duration in mass", VrpProblem { customers: vec![VrpCustomerSpec { service_time: kg(1.0),
                ..problem().customers[0].clone() }], ..problem() }),
            ("mismatched axes", VrpProblem { customers: vec![VrpCustomerSpec { node: VrpNodeSpec { id: "c1-node".into(),
                axes: vec![VrpAxis { name: "x".into(), value: meter(0.0) }] }, ..problem().customers[0].clone() }],
                ..problem() }),
            ("empty scheduling window", VrpProblem { scheduling_window: VrpTimeWindow {
                ready: "2024-01-01T00:00:00Z".into(), due: "2024-01-01T00:00:00Z".into() }, ..problem() }),
            ("self-loop base arc", VrpProblem { arcs: vec![VrpArcSpec { id: "loop".into(), from: "start".into(),
                to: "start".into(), distance: meter(0.0), travel_time: seconds(0.0), cost: pure(0.0), feasible: true }],
                ..problem() }),
        ];
        for (label, case) in broken {
            assert!(case.to_domain().is_err(), "{label} must be rejected");
        }
    }
}
