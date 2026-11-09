//! Gantt 活动调度有限候选源协议，只接受可序列化数据，不加载任意策略回调。
//! Gantt activity-scheduling finite-candidate source protocol accepting only serializable data, without arbitrary policy callbacks.

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_gantt_scheduling::{
    domain::activity::{
        Activity, ActivityCapacity, ActivityDependency, ActivityDependencyPresence, ActivityEndpoint, ActivityMode,
        ActivityChoiceKey, ActivityPreparation, ActivityResource, ActivityResourceUse, ActivitySchedule, ActivitySchedulingProblem,
    },
    domain::common::{ExecutorId, TaskId},
    domain::task::F64SolverValueAdapter,
    infrastructure::TimeRange,
};
use crate::{BridgeError, ModelArtifact, NativeBackend, PhysicalQuantity, QuantityUnit, Result, invalid};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ACTIVITIES: usize = 512;
const MAX_MODES: usize = 16;
const MAX_STARTS: usize = 4096;
const MAX_TOTAL_STARTS: usize = 20_000;
const MAX_RESOURCES: usize = 256;
const MAX_RESOURCE_USES: usize = 32;
const MAX_INTERVALS: usize = 256;
const MAX_DEPENDENCIES: usize = 4096;
const MAX_HORIZON_DAYS: i64 = 366;

/// RFC 3339 文本表示的时间区间，解析后归一为带时区的绝对时刻。 / RFC 3339 text time range, normalized to offset-bearing instants when parsed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttTimeRange {
    /// 起点，含。 / Inclusive start.
    pub start: String,
    /// 终点，不含。 / Exclusive end.
    pub end: String,
}

/// 依赖的时间端点。 / Temporal dependency endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum GanttActivityEndpoint {
    /// 开始。 / Start.
    Start,
    /// 完成。 / Finish.
    Finish,
}

/// 独立于时间的存在性语义。 / Presence semantics independent of timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum GanttActivityPresence {
    /// 仅共同执行时约束。 / Constrain only when both are present.
    WhenBothPresent,
    /// 后继存在要求前驱存在。 / Successor presence requires predecessor presence.
    RequirePredecessor,
    /// 共同执行或共同取消。 / Both present or both cancelled.
    RequireBoth,
}

/// 模式内的分阶段资源占用，不自动暂停。 / Phased resource use within a mode, without automatic preemption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityResourceUse {
    /// 已声明资源身份。 / Declared resource identity.
    pub resource: String,
    /// 非负占用量，单位须与资源容量同量纲。 / Non-negative usage with the resource capacity's dimension.
    pub quantity: PhysicalQuantity,
    /// 相对开始偏移秒数。 / Offset seconds from the activity start.
    pub from_seconds: i64,
    /// 相对结束偏移秒数，缺省表示模式结束。 / Offset seconds until the phase ends, defaulting to mode completion.
    #[ts(optional)]
    pub until_seconds: Option<i64>,
}

/// 不可抢占执行模式，带完整有限开始域而不是搜索预算。 / Non-preemptive mode with a complete finite start domain rather than a search budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityMode {
    /// 活动内唯一模式身份。 / Mode identity unique within its activity.
    pub id: String,
    /// 非负时长秒数。 / Non-negative duration in seconds.
    pub duration_seconds: i64,
    /// 完整允许开始时刻（RFC 3339），必须位于优化区间内。 / Complete allowed starts (RFC 3339) inside the horizon.
    pub starts: Vec<String>,
    /// 分阶段资源占用。 / Phased resource uses.
    pub resources: Vec<GanttActivityResourceUse>,
    /// 允许负值的完整模式费用。 / Complete mode cost, which may be negative.
    pub cost: PhysicalQuantity,
}

/// 待排活动，只有显式取消费才允许整单取消。 / Pending activity, cancellable only with an explicit cancellation cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivity {
    /// 有序唯一任务身份。 / Ordered unique task identity.
    pub id: String,
    /// 替代执行模式。 / Alternative execution modes.
    pub modes: Vec<GanttActivityMode>,
    /// 非负整单取消费，缺省表示必做。 / Non-negative cancellation cost, absent when mandatory.
    #[ts(optional)]
    pub cancellation_cost: Option<PhysicalQuantity>,
}

/// 时变资源容量覆盖，覆盖之间不允许重叠。 / Time-varying capacity override; overrides must not overlap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityCapacity {
    /// 半开区间。 / Half-open interval.
    pub time: GanttTimeRange,
    /// 区间容量。 / Capacity within the interval.
    pub capacity: PhysicalQuantity,
}

/// 多活动可同时占用的具名资源。 / Named resource shared concurrently by activities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityResource {
    /// 有序唯一资源身份。 / Ordered unique resource identity.
    pub id: String,
    /// 默认容量与单位。 / Default capacity and unit.
    pub capacity: PhysicalQuantity,
    /// 不可用区间，优先于容量覆盖。 / Unavailability, taking precedence over capacity overrides.
    pub unavailable_times: Vec<GanttTimeRange>,
    /// 时变容量覆盖。 / Time-varying capacity overrides.
    pub capacities: Vec<GanttActivityCapacity>,
}

/// FS/SS/FF/SF 端点依赖，允许负间隔和同步环。 / FS/SS/FF/SF endpoint dependency permitting negative lags and synchronization cycles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityDependency {
    /// 前驱任务。 / Predecessor task.
    pub predecessor: String,
    /// 后继任务。 / Successor task.
    pub successor: String,
    /// 前驱端点。 / Predecessor endpoint.
    pub from: GanttActivityEndpoint,
    /// 后继端点。 / Successor endpoint.
    pub to: GanttActivityEndpoint,
    /// 最小间隔秒数，允许负值。 / Minimum lag in seconds, which may be negative.
    pub minimum_seconds: i64,
    /// 可选最大间隔秒数，缺省表示无上界。 / Optional maximum lag in seconds, absent when unbounded.
    #[ts(optional)]
    pub maximum_seconds: Option<i64>,
    /// 存在性语义。 / Presence semantics.
    pub presence: GanttActivityPresence,
}

/// 活动调度源，不含基线、执行事实、附加业务行与物料流。 / Activity-scheduling source without baselines, execution facts, extra business rows or material flows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityProblem {
    /// 有限优化区间。 / Finite optimization horizon.
    pub horizon: GanttTimeRange,
    /// 统一费用单位。 / Canonical cost unit.
    pub cost_unit: QuantityUnit,
    /// 有序资源。 / Ordered resources.
    pub resources: Vec<GanttActivityResource>,
    /// 有序活动。 / Ordered activities.
    pub activities: Vec<GanttActivity>,
    /// 时间及存在性关系。 / Temporal and presence relations.
    pub dependencies: Vec<GanttActivityDependency>,
}

/// Gantt 活动调度原生执行配置；只含显式后端，不含策略回调或预算扩张。
/// Native Gantt activity execution configuration with an explicit backend only, without policy callbacks or budget expansion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityOptions {
    /// 显式原生后端。 / Explicit native backend.
    pub backend: NativeBackend,
}

/// 校验后的源摘要，不声称求解可行或最优。 / Validated source summary without claiming feasibility or optimality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GanttActivitySourceSummary {
    /// 已复验源指纹。 / Revalidated source fingerprint.
    pub source_fingerprint: String,
    /// 优化区间起点（规范 RFC 3339）。 / Canonical RFC 3339 horizon start.
    pub horizon_start: String,
    /// 优化区间终点（规范 RFC 3339）。 / Canonical RFC 3339 horizon end.
    pub horizon_end: String,
    /// 活动数。 / Activity count.
    pub activity_count: u32,
    /// 模式数。 / Mode count.
    pub mode_count: u32,
    /// 完整候选数，由有限开始域展开。 / Complete choices expanded from the finite start domains.
    pub choice_count: u32,
    /// 资源数。 / Resource count.
    pub resource_count: u32,
    /// 依赖数。 / Dependency count.
    pub dependency_count: u32,
}

/// 解析 RFC 3339 文本，保留原时区偏移。 / Parse RFC 3339 text while retaining the original offset.
fn instant(text: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|_| invalid("Gantt times must be RFC 3339 instants"))
}

/// 解析并复验半开区间。 / Parse and revalidate a half-open interval.
fn range(value: &GanttTimeRange) -> Result<TimeRange> {
    let (start, end) = (instant(&value.start)?, instant(&value.end)?);
    if start >= end { return Err(invalid("Gantt time ranges must be nonempty")); }
    Ok(TimeRange::new(start, end))
}

/// 规范文本，供摘要与指纹使用。 / Canonical text shared by summaries and fingerprints.
fn canonical(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_else(|_| String::new())
}

impl GanttActivityProblem {
    /// 复验声明并物化原领域输入，不加载任意回调。 / Revalidate declarations and materialize the original-domain input without loading callbacks.
    pub fn domain(&self) -> Result<ActivitySchedulingProblem<f64>> {
        let horizon = range(&self.horizon)?;
        if horizon.end - horizon.start > Duration::days(MAX_HORIZON_DAYS) {
            return Err(invalid("Gantt activity horizon must not exceed 366 days"));
        }
        if self.resources.len() > MAX_RESOURCES || self.dependencies.len() > MAX_DEPENDENCIES {
            return Err(BridgeError::new("PayloadTooLarge", "Gantt activity source exceeds its declaration limits"));
        }
        if self.activities.is_empty() || self.activities.len() > MAX_ACTIVITIES {
            return Err(invalid("Gantt activity source must declare 1..=512 activities"));
        }
        let mut resource_ids = BTreeSet::new();
        let mut resources = Vec::with_capacity(self.resources.len());
        for resource in &self.resources {
            if resource.id.trim().is_empty() || resource.id.len() > 256 || !resource_ids.insert(resource.id.clone()) {
                return Err(invalid("Gantt activity resources must have 1..=256 unique non-blank identities"));
            }
            if resource.unavailable_times.len() > MAX_INTERVALS || resource.capacities.len() > MAX_INTERVALS {
                return Err(BridgeError::new("PayloadTooLarge", "Gantt activity resource exceeds its calendar limits"));
            }
            let capacity = resource.capacity.quantity()?;
            if capacity.value < 0.0 { return Err(invalid("Gantt activity resource capacity must be non-negative")); }
            let mut unavailable = Vec::with_capacity(resource.unavailable_times.len());
            for time in &resource.unavailable_times { unavailable.push(range(time)?); }
            let mut capacities = Vec::with_capacity(resource.capacities.len());
            for override_entry in &resource.capacities {
                let capacity = override_entry.capacity.quantity()?;
                if capacity.value < 0.0 { return Err(invalid("Gantt activity capacity overrides must be non-negative")); }
                capacities.push(ActivityCapacity { time: range(&override_entry.time)?, capacity });
            }
            resources.push(ActivityResource { id: ExecutorId::from(resource.id.clone()), capacity, unavailable_times: unavailable, capacities });
        }
        let mut task_ids = BTreeSet::new();
        let mut total_starts = 0usize;
        let mut activities = Vec::with_capacity(self.activities.len());
        for activity in &self.activities {
            if activity.id.trim().is_empty() || activity.id.len() > 256 || !task_ids.insert(activity.id.clone()) {
                return Err(invalid("Gantt activities must have 1..=256 unique non-blank identities"));
            }
            if activity.modes.is_empty() || activity.modes.len() > MAX_MODES {
                return Err(invalid("Gantt activities must declare 1..=16 modes"));
            }
            let cancellation_cost = match &activity.cancellation_cost {
                Some(value) => {
                    let quantity = value.quantity()?;
                    if quantity.value < 0.0 { return Err(invalid("Gantt cancellation costs must be non-negative")); }
                    Some(quantity)
                }
                None => None,
            };
            let mut mode_ids = BTreeSet::new();
            let mut modes = Vec::with_capacity(activity.modes.len());
            for mode in &activity.modes {
                if mode.id.trim().is_empty() || mode.id.len() > 256 || !mode_ids.insert(mode.id.clone()) {
                    return Err(invalid("Gantt activity modes must have unique non-blank identities within their activity"));
                }
                if !(0..=MAX_HORIZON_DAYS * 86_400).contains(&mode.duration_seconds) {
                    return Err(invalid("Gantt activity mode durations must be 0..=366 days"));
                }
                if mode.starts.is_empty() || mode.starts.len() > MAX_STARTS {
                    return Err(invalid("Gantt activity modes must declare 1..=4096 candidate starts"));
                }
                total_starts += mode.starts.len();
                if total_starts > MAX_TOTAL_STARTS {
                    return Err(BridgeError::new("PayloadTooLarge", "Gantt activity source exceeds 20000 candidate starts"));
                }
                if mode.resources.len() > MAX_RESOURCE_USES {
                    return Err(invalid("Gantt activity modes must declare at most 32 resource uses"));
                }
                let mut starts = Vec::with_capacity(mode.starts.len());
                let duration = Duration::seconds(mode.duration_seconds);
                for start in &mode.starts {
                    let start = instant(start)?;
                    if start < horizon.start || start > horizon.end {
                        return Err(invalid("Gantt candidate starts must lie inside the optimization horizon"));
                    }
                    if starts.last().is_some_and(|previous| previous >= &start) {
                        return Err(invalid("Gantt candidate starts must be unique and strictly ascending"));
                    }
                    starts.push(start);
                }
                let mut uses = Vec::with_capacity(mode.resources.len());
                for use_entry in &mode.resources {
                    if !resource_ids.contains(&use_entry.resource) {
                        return Err(invalid("Gantt resource uses must reference declared resources"));
                    }
                    let quantity = use_entry.quantity.quantity()?;
                    if quantity.value < 0.0 { return Err(invalid("Gantt resource usage must be non-negative")); }
                    if use_entry.from_seconds < 0 || use_entry.from_seconds > mode.duration_seconds {
                        return Err(invalid("Gantt resource phases must start inside their mode"));
                    }
                    let until = match use_entry.until_seconds {
                        Some(value) if value < use_entry.from_seconds || value > mode.duration_seconds => {
                            return Err(invalid("Gantt resource phases must end inside their mode and after their start"));
                        }
                        Some(value) => Some(Duration::seconds(value)),
                        None => None,
                    };
                    uses.push(ActivityResourceUse { resource: ExecutorId::from(use_entry.resource.clone()), quantity,
                        from: Duration::seconds(use_entry.from_seconds), until });
                }
                modes.push(ActivityMode { id: mode.id.clone(), duration, starts, resources: uses, cost: mode.cost.quantity()?, material_flows: Vec::new() });
            }
            activities.push(Activity { id: TaskId::from(activity.id.clone()), modes, cancellation_cost });
        }
        let mut seen = BTreeSet::new();
        let mut dependencies = Vec::with_capacity(self.dependencies.len());
        for dependency in &self.dependencies {
            if !task_ids.contains(&dependency.predecessor) || !task_ids.contains(&dependency.successor) {
                return Err(invalid("Gantt dependencies must reference declared activities"));
            }
            for lag in [Some(dependency.minimum_seconds), dependency.maximum_seconds].into_iter().flatten() {
                if !(-MAX_HORIZON_DAYS * 86_400..=MAX_HORIZON_DAYS * 86_400).contains(&lag) {
                    return Err(invalid("Gantt dependency lags must stay within 366 days"));
                }
            }
            if dependency.maximum_seconds.is_some_and(|maximum| maximum < dependency.minimum_seconds) {
                return Err(invalid("Gantt dependency maximum lags must not be smaller than their minimums"));
            }
            let (predecessor, successor) = (TaskId::from(dependency.predecessor.clone()), TaskId::from(dependency.successor.clone()));
            if !seen.insert((predecessor.clone(), successor.clone(), dependency.from, dependency.to)) {
                return Err(invalid("Gantt dependencies must not be duplicated between the same endpoints"));
            }
            dependencies.push(ActivityDependency { predecessor, successor, from: endpoint(dependency.from), to: endpoint(dependency.to),
                minimum: Duration::seconds(dependency.minimum_seconds), maximum: dependency.maximum_seconds.map(Duration::seconds),
                presence: presence(dependency.presence) });
        }
        Ok(ActivitySchedulingProblem { horizon, activities, resources, dependencies, baselines: Vec::new(), executions: Vec::new(),
            rows: Vec::new(), cost_unit: self.cost_unit.unit(), materials: Vec::new(), material_events: Vec::new() })
    }
}

/// 映射到原领域端点枚举。 / Map into the original endpoint enum.
fn endpoint(value: GanttActivityEndpoint) -> ActivityEndpoint {
    match value { GanttActivityEndpoint::Start => ActivityEndpoint::Start, GanttActivityEndpoint::Finish => ActivityEndpoint::Finish }
}

/// 映射到原领域存在性枚举。 / Map into the original presence enum.
fn presence(value: GanttActivityPresence) -> ActivityDependencyPresence {
    match value {
        GanttActivityPresence::WhenBothPresent => ActivityDependencyPresence::WhenBothPresent,
        GanttActivityPresence::RequirePredecessor => ActivityDependencyPresence::RequirePredecessor,
        GanttActivityPresence::RequireBoth => ActivityDependencyPresence::RequireBoth,
    }
}

/// 显式完整候选：任务、模式与允许开始时刻。 / Explicit complete choice: task, mode and allowed start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivitySelection {
    /// 已声明任务身份。 / Declared task identity.
    pub task: String,
    /// 该任务下的模式身份。 / Mode identity of that task.
    pub mode: String,
    /// RFC 3339 开始时刻，必须在声明开始域内。 / RFC 3339 start inside the declared start domain.
    pub start: String,
}

/// 显式方案，不是求解结果。 / Explicit plan, not a solve result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GanttActivityPlan {
    /// 完整候选选择，任务与模式组合不得重复。 / Complete choice selections without duplicate task/mode pairs.
    pub selections: Vec<GanttActivitySelection>,
}

/// 绝对时间的资源占用副本。 / Resource reservation copy in absolute time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GanttActivityReservation {
    /// 资源身份。 / Resource identity.
    pub resource: String,
    /// 起点。 / Start.
    pub start: String,
    /// 终点。 / End.
    pub end: String,
    /// 占用量，单位取自源资源声明。 / Usage in the unit declared by the source resource.
    pub quantity: PhysicalQuantity,
}

/// 单活动复验后的绝对时间分配。 / Revalidated absolute-time placement of one activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GanttActivityPlacement {
    /// 任务身份。 / Task identity.
    pub task: String,
    /// 模式身份。 / Mode identity.
    pub mode: String,
    /// 声明开始时刻。 / Declared start instant.
    pub start: String,
    /// 实际或预计结束时刻。 / Actual or expected finish instant.
    pub end: String,
    /// 全部内部资源段。 / Every internal resource phase.
    pub reservations: Vec<GanttActivityReservation>,
}

/// 原领域分配快照，绝对时间与源声明单位。 / Native-domain placement snapshot in absolute time and source-declared units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GanttActivitySchedule {
    /// 按候选稳定顺序排列的分配。 / Placements in stable candidate order.
    pub placements: Vec<GanttActivityPlacement>,
    /// 未选择的可取消任务。 / Cancellable tasks left unselected.
    pub cancelled: Vec<String>,
    /// 原领域费用，单位为源费用单位。 / Native-domain cost in the source cost unit.
    pub cost: PhysicalQuantity,
    /// 完整声明候选数。 / Complete declared candidate count.
    pub candidate_count: u32,
}

/// 显式方案经原领域复核后的结果，不声称最优或认证来源。 / Result of an explicit plan revalidated by the original domain, without claiming optimality or authenticating provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GanttActivityReplayReport {
    /// 已复验源指纹。 / Revalidated source fingerprint.
    pub source_fingerprint: String,
    /// 复验后的分配快照。 / Revalidated placement snapshot.
    pub schedule: GanttActivitySchedule,
}

/// 原生单 MILP 执行报告，`solverModelOnly` 仍不构成任何证明。 / Native single-MILP execution report whose `solverModelOnly` remains no proof at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GanttActivityExecutionReport {
    /// 已复验源指纹。 / Revalidated source fingerprint.
    pub source_fingerprint: String,
    /// 原子问题状态。 / Original child problem status.
    pub problem_status: String,
    /// 原终止原因。 / Original termination reason.
    pub termination_reason: String,
    /// 派生目标值，缺省表示无解。 / Derived objective, absent without an incumbent.
    #[ts(optional)]
    pub objective_value: Option<f64>,
    /// 值域或适配器不保证原生 f64 时为真；不构成证明。 / True when values or the adapter do not guarantee native f64; never a proof.
    pub solver_model_only: bool,
    /// 仅在 incumbent 通过原域复核后存在。 / Present only after the incumbent passed native-domain revalidation.
    #[ts(optional)]
    pub schedule: Option<GanttActivitySchedule>,
    /// 原流程或复验失败原因，成功时缺省。 / Why the flow or revalidation failed, absent on success.
    #[ts(optional)]
    pub failure: Option<String>,
}

/// 协议封装，版本与问题文本一起参与指纹。 / Protocol envelope whose version and problem text participate in the fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProblemEnvelope {
    schema_version: String,
    problem: GanttActivityProblem,
}

/// 编码源并计算领域分离指纹，不声称求解结果。 / Encode the source and compute a domain-separated fingerprint without claiming a solve result.
pub fn encode_gantt_activity_problem(problem: GanttActivityProblem) -> Result<ModelArtifact> {
    problem.domain()?;
    let model_json = serde_json::to_string(&ProblemEnvelope { schema_version: "1.0".into(), problem }).map_err(invalid)?;
    if model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "Gantt activity source exceeds 8 MiB")); }
    let fingerprint = ospf_rust_core::solver::sha256_fingerprint("ospf.gantt.activity.problem.v1", model_json.as_bytes()).value;
    Ok(ModelArtifact { model_json, fingerprint })
}

/// 复验版本、canonical 文本、指纹与领域声明。 / Revalidate version, canonical text, fingerprint and domain declarations.
pub fn decode_gantt_activity_problem(artifact: &ModelArtifact) -> Result<(GanttActivityProblem, ActivitySchedulingProblem<f64>)> {
    if artifact.model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "Gantt activity source exceeds 8 MiB")); }
    let envelope: ProblemEnvelope = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported Gantt activity source schema")); }
    let encoded = encode_gantt_activity_problem(envelope.problem.clone())?;
    if encoded.model_json != artifact.model_json || encoded.fingerprint != artifact.fingerprint {
        return Err(BridgeError::new("FingerprintMismatch", "Gantt activity canonical source or fingerprint mismatch"));
    }
    let problem = envelope.problem.domain()?;
    Ok((envelope.problem, problem))
}

/// 展开有限候选并返回摘要，不建模型也不求解。 / Expand finite candidates and return a summary without building a model or solving.
pub fn prepare_gantt_activity_source(artifact: &ModelArtifact) -> Result<GanttActivitySourceSummary> {
    let (source, problem) = decode_gantt_activity_problem(artifact)?;
    let horizon = problem.horizon;
    let preparation = ActivityPreparation::new(problem, &F64SolverValueAdapter, None)
        .map_err(|error| invalid(format!("invalid Gantt activity source: {error}")))?;
    let mode_count = preparation.problem().activities.iter().map(|activity| activity.modes.len() as u32).sum();
    let choices = preparation.choices().len();
    Ok(GanttActivitySourceSummary {
        source_fingerprint: artifact.fingerprint.clone(),
        horizon_start: canonical(horizon.start),
        horizon_end: canonical(horizon.end),
        activity_count: u32::try_from(source.activities.len()).map_err(invalid)?,
        mode_count,
        choice_count: u32::try_from(choices).map_err(|_| BridgeError::new("PayloadTooLarge", "Gantt activity source expands too many choices"))?,
        resource_count: u32::try_from(preparation.problem().resources.len()).map_err(invalid)?,
        dependency_count: u32::try_from(preparation.problem().dependencies.len()).map_err(invalid)?,
    })
}

/// 复验显式方案的完整候选、原域规则与库存轨迹，返回绝对时间分配；不调用求解器。
/// Revalidate an explicit plan's complete choices, native rules and inventory trails, returning absolute-time placements without invoking a solver.
pub fn replay_gantt_activity_plan(artifact: &ModelArtifact, plan: &GanttActivityPlan) -> Result<GanttActivityReplayReport> {
    let (source, problem) = decode_gantt_activity_problem(artifact)?;
    if plan.selections.len() > MAX_ACTIVITIES {
        return Err(invalid("Gantt activity plans must not exceed the declared activity count"));
    }
    let preparation = ActivityPreparation::new(problem, &F64SolverValueAdapter, None)
        .map_err(|error| invalid(format!("invalid Gantt activity source: {error}")))?;
    let mut keys = Vec::with_capacity(plan.selections.len());
    let mut seen = BTreeSet::new();
    for selection in &plan.selections {
        let start = instant(&selection.start)?;
        let key = ActivityChoiceKey { task: TaskId::from(selection.task.clone()), mode: selection.mode.clone(), start };
        if !preparation.choices().contains_key(&key) {
            return Err(invalid("Gantt plan selections must be complete declared candidates"));
        }
        if !seen.insert((key.task.clone(), key.mode.clone())) {
            return Err(invalid("Gantt plans must not select the same task and mode twice"));
        }
        keys.push(key);
    }
    let selected = keys.iter().map(|key| key.task.clone()).collect::<BTreeSet<_>>();
    let cancelled = source.activities.iter().filter(|activity| activity.cancellation_cost.is_some()
        && !selected.contains(&TaskId::from(activity.id.clone()))).map(|activity| activity.id.clone()).collect::<Vec<_>>();
    let validation = preparation.validate(&keys).map_err(|error| invalid(format!("invalid Gantt activity plan: {error}")))?;
    if !validation.feasible() {
        return Err(invalid(format!("Gantt activity plan violates native rules: {:?}", validation.violations)));
    }
    let schedule = preparation.schedule(&keys, &cancelled.iter().map(|id| TaskId::from(id.clone())).collect())
        .map_err(|error| invalid(format!("invalid Gantt activity plan: {error}")))?;
    Ok(GanttActivityReplayReport { source_fingerprint: artifact.fingerprint.clone(), schedule: gantt_activity_schedule(&source, &schedule)? })
}

/// 把原领域分配转成协议快照，单位取自源声明，不重新求解。 / Convert native placements into a protocol snapshot using source-declared units, without re-solving.
pub fn gantt_activity_schedule(source: &GanttActivityProblem, schedule: &ActivitySchedule<f64>) -> Result<GanttActivitySchedule> {
    let units = source.resources.iter().map(|resource| (resource.id.as_str(), resource.capacity.unit)).collect::<std::collections::BTreeMap<_, _>>();
    let mut placements = Vec::with_capacity(schedule.placements.len());
    for placement in &schedule.placements {
        let mut reservations = Vec::with_capacity(placement.reservations.len());
        for reservation in &placement.reservations {
            let id = reservation.resource.as_str();
            let unit = *units.get(id).ok_or_else(|| invalid("Gantt placements must only reference declared resources"))?;
            reservations.push(GanttActivityReservation { resource: id.to_owned(), start: canonical(reservation.time.start),
                end: canonical(reservation.time.end), quantity: PhysicalQuantity { value: reservation.quantity.value, unit } });
        }
        placements.push(GanttActivityPlacement { task: placement.key.task.as_str().to_owned(), mode: placement.key.mode.clone(),
            start: canonical(placement.key.start), end: canonical(placement.time.end), reservations });
    }
    let cost = PhysicalQuantity { value: schedule.cost.value, unit: source.cost_unit };
    if !cost.value.is_finite() { return Err(invalid("Gantt activity schedules must have finite cost")); }
    Ok(GanttActivitySchedule { placements,
        cancelled: schedule.cancelled.iter().map(|task| task.as_str().to_owned()).collect(),
        cost, candidate_count: u32::try_from(schedule.candidate_scope.len())
            .map_err(|_| BridgeError::new("PayloadTooLarge", "Gantt activity source expands too many choices"))? })
}

/// 编码原生执行报告；报告本身不是最优性或来源证明。 / Encode a native execution report, which is neither an optimality nor a provenance proof.
pub fn encode_gantt_activity_execution_report(report: &GanttActivityExecutionReport) -> Result<String> {
    let text = serde_json::to_string(report).map_err(invalid)?;
    if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "Gantt activity execution report exceeds 8 MiB")); }
    Ok(text)
}

/// 解码执行报告并复验源绑定、字段范围与状态文本。 / Decode an execution report, revalidating source binding, field ranges and status text.
pub fn decode_gantt_activity_execution_report(text: &str, artifact: &ModelArtifact) -> Result<GanttActivityExecutionReport> {
    if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "Gantt activity execution report exceeds 8 MiB")); }
    let report: GanttActivityExecutionReport = serde_json::from_str(text).map_err(invalid)?;
    if report.source_fingerprint != artifact.fingerprint {
        return Err(BridgeError::new("FingerprintMismatch", "Gantt activity execution report belongs to a different source"));
    }
    if report.problem_status.trim().is_empty() || report.termination_reason.trim().is_empty() {
        return Err(invalid("Gantt activity execution report must declare its problem status and termination reason"));
    }
    if report.objective_value.is_some_and(|value| !value.is_finite()) {
        return Err(invalid("Gantt activity execution objective must be finite"));
    }
    if report.schedule.is_some() && (report.failure.is_some() || report.problem_status.eq_ignore_ascii_case("infeasible")) {
        return Err(invalid("Gantt activity execution report cannot carry placements alongside a failure or infeasibility"));
    }
    if let Some(schedule) = &report.schedule {
        if !schedule.cost.value.is_finite() || schedule.placements.len() > MAX_ACTIVITIES
            || schedule.cancelled.len() > MAX_ACTIVITIES || schedule.candidate_count as usize > MAX_TOTAL_STARTS {
            return Err(invalid("Gantt activity execution schedule exceeds the declared source limits"));
        }
        for placement in &schedule.placements {
            instant(&placement.start)?; instant(&placement.end)?;
            if placement.reservations.len() > MAX_RESOURCE_USES { return Err(invalid("Gantt activity placement declares too many resource phases")); }
            for reservation in &placement.reservations {
                instant(&reservation.start)?; instant(&reservation.end)?;
                reservation.quantity.quantity()?;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 单活动单模式的最小源，开始域由有限候选展开。 / Minimal single-activity single-mode source whose finite start domain expands into complete choices.
    fn problem() -> GanttActivityProblem {
        serde_json::from_str(r#"{
            "horizon": {"start": "2026-01-01T00:00:00Z", "end": "2026-01-02T00:00:00Z"},
            "costUnit": "kg",
            "resources": [{"id": "machine", "capacity": {"value": 1.0, "unit": "kg"}, "unavailableTimes": [], "capacities": []}],
            "activities": [{"id": "job", "modes": [{"id": "run", "durationSeconds": 3600,
                "starts": ["2026-01-01T01:00:00Z", "2026-01-01T02:00:00Z"],
                "resources": [{"resource": "machine", "quantity": {"value": 0.5, "unit": "kg"}, "fromSeconds": 0}],
                "cost": {"value": 2.0, "unit": "kg"}}]}],
            "dependencies": []
        }"#).unwrap()
    }

    #[test]
    fn source_roundtrips_with_canonical_text_and_domain_fingerprint() {
        let artifact = encode_gantt_activity_problem(problem()).unwrap();
        let (source, domain) = decode_gantt_activity_problem(&artifact).unwrap();
        assert_eq!(source, problem());
        assert_eq!(domain.activities.len(), 1);
        assert_eq!(domain.activities[0].modes[0].starts.len(), 2);
        let summary = prepare_gantt_activity_source(&artifact).unwrap();
        assert_eq!((summary.activity_count, summary.mode_count, summary.choice_count), (1, 1, 2));
        assert_eq!((summary.resource_count, summary.dependency_count), (1, 0));
        assert_eq!(summary.source_fingerprint, artifact.fingerprint);
        assert_eq!(summary.horizon_start, "2026-01-01T00:00:00Z");
        let mut foreign = artifact.clone();
        foreign.fingerprint = "foreign".into();
        assert_eq!(decode_gantt_activity_problem(&foreign).unwrap_err().code, "FingerprintMismatch");
    }

    #[test]
    fn source_rejects_undeclared_references_unsorted_starts_and_duplicate_dependencies() {
        let mut unknown_resource = problem();
        unknown_resource.activities[0].modes[0].resources[0].resource = "ghost".into();
        assert_eq!(encode_gantt_activity_problem(unknown_resource).unwrap_err().code, "InvalidInput");
        let mut unsorted = problem();
        unsorted.activities[0].modes[0].starts.reverse();
        assert_eq!(encode_gantt_activity_problem(unsorted).unwrap_err().code, "InvalidInput");
        let mut outside = problem();
        outside.activities[0].modes[0].starts = vec!["2026-01-05T00:00:00Z".into()];
        assert_eq!(encode_gantt_activity_problem(outside).unwrap_err().code, "InvalidInput");
        let mut negative = problem();
        negative.activities[0].modes[0].resources[0].quantity.value = -1.0;
        assert_eq!(encode_gantt_activity_problem(negative).unwrap_err().code, "InvalidInput");
        let mut phase = problem();
        phase.activities[0].modes[0].resources[0].until_seconds = Some(7200);
        assert_eq!(encode_gantt_activity_problem(phase).unwrap_err().code, "InvalidInput");
        let mut duplicate = problem();
        duplicate.activities[0].modes[0].duration_seconds = u32::MAX as i64;
        assert_eq!(encode_gantt_activity_problem(duplicate).unwrap_err().code, "InvalidInput");
    }

    /// 两台同能力机器争用同一时段，用于验证原域容量规则。 / Two equally capable machines contending for the same interval, used to exercise native capacity rules.
    fn two_jobs() -> GanttActivityProblem {
        let mut source = problem();
        source.activities[0].modes[0].resources[0].quantity.value = 1.0;
        source.activities.push(GanttActivity { id: "job-2".into(), modes: source.activities[0].modes.clone(), cancellation_cost: None });
        source
    }

    fn selection(task: &str, start: &str) -> GanttActivitySelection {
        GanttActivitySelection { task: task.into(), mode: "run".into(), start: start.into() }
    }

    #[test]
    fn plan_replay_revalidates_native_rules_and_returns_absolute_placements() {
        let artifact = encode_gantt_activity_problem(two_jobs()).unwrap();
        let report = replay_gantt_activity_plan(&artifact, &GanttActivityPlan { selections: vec![
            selection("job", "2026-01-01T01:00:00Z"), selection("job-2", "2026-01-01T02:00:00Z")] }).unwrap();
        assert_eq!(report.source_fingerprint, artifact.fingerprint);
        assert_eq!(report.schedule.placements.len(), 2);
        assert_eq!(report.schedule.cancelled.len(), 0);
        assert_eq!(report.schedule.candidate_count, 4);
        assert_eq!(report.schedule.cost, PhysicalQuantity { value: 4.0, unit: QuantityUnit::Kilogram });
        let first = &report.schedule.placements[0];
        assert_eq!((first.task.as_str(), first.mode.as_str()), ("job", "run"));
        assert_eq!(first.start, "2026-01-01T01:00:00Z"); assert_eq!(first.end, "2026-01-01T02:00:00Z");
        assert_eq!(first.reservations.len(), 1);
        assert_eq!(first.reservations[0].resource, "machine");
        assert_eq!(first.reservations[0].quantity, PhysicalQuantity { value: 1.0, unit: QuantityUnit::Kilogram });
        // 重叠时段争用同一容量只能作为原域违反被拒绝。 / Overlapping demand on the same capacity may only be rejected as a native rule violation.
        let overlapping = GanttActivityPlan { selections: vec![selection("job", "2026-01-01T01:00:00Z"), selection("job-2", "2026-01-01T01:30:00Z")] };
        assert_eq!(replay_gantt_activity_plan(&artifact, &overlapping).unwrap_err().code, "InvalidInput");
        // 必做活动缺项与未声明候选都不是可接受方案。 / Omitting a mandatory activity and selecting an undeclared candidate are both rejected.
        let missing = GanttActivityPlan { selections: vec![selection("job", "2026-01-01T01:00:00Z")] };
        assert_eq!(replay_gantt_activity_plan(&artifact, &missing).unwrap_err().code, "InvalidInput");
        let undeclared = GanttActivityPlan { selections: vec![selection("job-2", "2026-01-01T03:00:00Z")] };
        assert_eq!(replay_gantt_activity_plan(&artifact, &undeclared).unwrap_err().code, "InvalidInput");
        let duplicated = GanttActivityPlan { selections: vec![selection("job", "2026-01-01T01:00:00Z"), selection("job", "2026-01-01T02:00:00Z")] };
        assert_eq!(replay_gantt_activity_plan(&artifact, &duplicated).unwrap_err().code, "InvalidInput");
    }

    #[test]
    fn plan_replay_reports_explicit_cancellations_without_charging_them() {
        let mut source = problem();
        source.activities[0].cancellation_cost = Some(PhysicalQuantity { value: 1.0, unit: QuantityUnit::Kilogram });
        let artifact = encode_gantt_activity_problem(source).unwrap();
        let report = replay_gantt_activity_plan(&artifact, &GanttActivityPlan { selections: Vec::new() }).unwrap();
        assert_eq!(report.schedule.cancelled, vec!["job".to_string()]);
        assert!(report.schedule.placements.is_empty());
        assert_eq!(report.schedule.cost, PhysicalQuantity { value: 1.0, unit: QuantityUnit::Kilogram });
    }

    #[test]
    fn source_reports_duplicate_dependency_endpoints_and_blank_identities() {
        let mut duplicated = problem();
        duplicated.activities.push(GanttActivity { id: "next".into(), modes: duplicated.activities[0].modes.clone(), cancellation_cost: None });
        let dependency = GanttActivityDependency { predecessor: "job".into(), successor: "next".into(),
            from: GanttActivityEndpoint::Finish, to: GanttActivityEndpoint::Start, minimum_seconds: 0, maximum_seconds: None,
            presence: GanttActivityPresence::RequireBoth };
        duplicated.dependencies = vec![dependency.clone(), dependency];
        assert_eq!(encode_gantt_activity_problem(duplicated).unwrap_err().code, "InvalidInput");
        let mut blank = problem();
        blank.activities[0].id = "  ".into();
        assert_eq!(encode_gantt_activity_problem(blank).unwrap_err().code, "InvalidInput");
        let mut no_starts = problem();
        no_starts.activities[0].modes[0].starts.clear();
        assert_eq!(encode_gantt_activity_problem(no_starts).unwrap_err().code, "InvalidInput");
    }
}
