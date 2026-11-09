//! 原分区 context 的有界对象门面；不在语言桥复制列投影或定价算法。
//! Bounded object facade over the original partition context, without duplicating column projection or pricing algorithms.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::MetaModel;
use ospf_rust_framework::solver::{ColumnMasterRegistration, ColumnPricingRequest};
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_bpp::{LoadingPartition, PartitionDecision, PartitionLoadingContext, PartitionLoadingOptions};
use crate::{BridgeError, ModelArtifact, PhysicalQuantity, ReportSummary, Result, encode_compiled_model, invalid, summarize_report};
use super::{BppLoadingContext, BppLoadingResult, MAX_BYTES, TOLERANCE, identifier};

const COLUMN_LIMIT: usize = 4096;
const ENTRY_LIMIT: usize = 262_144;

/// 显式且完整覆盖源舱位的分区。 / Explicit partition exhaustively covering source positions.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartition {
    /// 不重复的分区身份。 / Unique partition identity.
    pub id: String,
    /// 非空且不重复的源舱位身份。 / Nonempty duplicate-free source position identities.
    pub position_ids: Vec<String>,
}

/// 预注册分区及连续资源补全白名单，不接收裸 token 或任意扩展。
/// Preregistered partitions and continuous-resource completion whitelist, without raw tokens or arbitrary extensions.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionOptions {
    /// 1..128 个显式分区，所有舱位恰归属一个分区。 / 1..128 explicit partitions, each position owned exactly once.
    pub partitions: Vec<BppLoadingPartition>,
    /// 允许补全的已声明资源；归属随其源舱位确定，省略为空。 / Declared resources allowed for completion; ownership follows their source positions, empty when omitted.
    #[serde(default)]
    pub refinable_resources: Vec<String>,
}

impl BppLoadingPartitionOptions {
    /// 在实际源注册内映射分区和资源，不传递进程内变量身份。 / Map partitions and resources inside the actual source registration, never transporting process-local identities.
    pub fn domain(&self, source: &BppLoadingContext) -> Result<PartitionLoadingOptions<f64>> {
        if self.partitions.is_empty() || self.partitions.len() > 128 || self.refinable_resources.len() > 128 { return Err(invalid("invalid BPP partition/resource count")); }
        let mut ids = BTreeSet::new(); let mut owners = BTreeMap::new();
        for partition in &self.partitions {
            identifier(&partition.id)?;
            if !ids.insert(&partition.id) || partition.position_ids.is_empty() || partition.position_ids.len() > 128 { return Err(invalid("duplicate or empty BPP partition")); }
            for position in &partition.position_ids {
                identifier(position)?;
                if owners.insert(position.clone(), partition.id.clone()).is_some() { return Err(invalid("duplicate BPP partition position ownership")); }
            }
        }
        if owners.keys().collect::<BTreeSet<_>>() != source.problem().positions.iter().map(|position| &position.id).collect() { return Err(invalid("BPP partitions must exactly cover all source positions")); }
        let resources = source.partition_resources()?; let mut refinable = BTreeSet::new();
        for id in &self.refinable_resources {
            let variable = resources.iter().find(|(resource, _, _)| resource == id).map(|(_, _, variable)| *variable).ok_or_else(|| invalid("unknown refinable BPP resource"))?;
            if !refinable.insert(variable) { return Err(invalid("duplicate refinable BPP resource")); }
        }
        Ok(PartitionLoadingOptions { partitions: self.partitions.iter().map(|partition| LoadingPartition { id: partition.id.clone(), position_ids: partition.position_ids.iter().cloned().collect() }).collect(),
            extra_decisions: resources.into_iter().map(|(id, position, variable)| PartitionDecision { id: format!("resource:{}:{id}", id.len()), partition_id: owners[&position].clone(), variable }).collect(),
            refinable_resources: refinable, tolerance: TOLERANCE })
    }
}

/// 局部决策的领域角色。 / Domain role of a local decision.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BppLoadingPartitionDecisionRole {
    /// 货物分配。 / Cargo assignment.
    Assignment,
    /// 整体占位。 / Whole-position occupancy.
    Occupied,
    /// 连续附加重量。 / Continuous additional weight.
    Resource,
}

/// 不含进程内 token 的局部决策目录。 / Local-decision catalog without process-local tokens.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionDecision {
    /// 原 Rust context 签发的决策身份。 / Decision identity issued by the original Rust context.
    pub id: String,
    /// 显式所属分区。 / Explicit owning partition.
    pub partition_id: String,
    /// 完整源 snapshot 编译向量中的索引。 / Index in the complete source snapshot-compiled vector.
    pub variable_index: u32,
    /// 领域角色。 / Domain role.
    pub role: BppLoadingPartitionDecisionRole,
    /// 源舱位身份。 / Source position identity.
    pub position_id: String,
    /// 仅分配决策具有货物身份。 / Cargo identity for assignment decisions only.
    pub item_id: Option<String>,
    /// 仅资源决策具有资源身份。 / Resource identity for resource decisions only.
    pub resource_id: Option<String>,
}

/// 提交的完整局部向量，未提供局部决策不会补零。 / Submitted complete local vector; omitted local decisions are never zero-filled.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionColumnInput {
    /// 目标分区。 / Target partition.
    pub partition_id: String,
    /// 该分区全部决策的有限数值。 / Finite values for every decision in this partition.
    pub decisions: BTreeMap<String, f64>,
}

/// 原 Rust 推导的列元数据，不独立充当完整源方案。 / Column metadata derived by original Rust, not independently a complete-source plan.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionColumn {
    /// 本 context 单调签发且不复用的列身份。 / Monotonic column identity issued by this context, never reused.
    pub id: String,
    /// 所属分区。 / Owning partition.
    pub partition_id: String,
    /// 完整局部向量，资源使用源模型重量口径。 / Complete local vector, with resources in the source model weight unit.
    pub decisions: BTreeMap<String, f64>,
    /// 局部货物分配。 / Local cargo assignments.
    pub assignments: BTreeMap<String, String>,
    /// 局部整体占位。 / Local whole-position occupancy.
    pub occupied_positions: Vec<String>,
    /// 货物使用次数 canonical u64 文本。 / Cargo usage as canonical u64 text.
    pub cargo_usage: BTreeMap<String, String>,
    /// 舱位件数 canonical u64 文本。 / Position counts as canonical u64 text.
    pub counts: BTreeMap<String, String>,
    /// 仅货物重量，不含附加资源。 / Cargo weight only, excluding additional resources.
    pub weights: BTreeMap<String, PhysicalQuantity>,
    /// 各选择组的局部兼容布局，不推断全局可行。 / Locally compatible layouts by group, without implying global feasibility.
    pub compatible_layouts: BTreeMap<String, Vec<String>>,
}

/// 主问题注册模式；资源补全必须为无人工整数主问题。 / Master registration mode; resource completion requires an artificial-free integer master.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionMasterOptions {
    /// Phase I 人工变量目标。 / Phase I artificial-variable objective.
    pub phase_one: bool,
    /// 恢复整数选列与全局变量类型。 / Restore integer column selection and global variable types.
    pub integer: bool,
    /// 只释放白名单资源，其他轮廓仍冻结。 / Release only whitelisted resources, leaving other profiles frozen.
    pub refine_resources: bool,
}

/// 主问题中保留的全局／资源变量到源向量的映射。 / Master global/resource variables mapped to the source vector.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionGlobalBinding {
    /// 完整源向量索引。 / Complete source-vector index.
    pub source_index: u32,
    /// 本次主问题向量索引。 / This master-vector index.
    pub master_index: u32,
}

/// 通过实际局部模型门禁的原报告与候选，不自动声明完成定价。 / Original report and candidate passing the actual local model gate, without automatically claiming pricing completion.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionPricingResult {
    /// 完整局部候选，无 incumbent 时为空。 / Complete local candidate, absent without an incumbent.
    pub decisions: Option<BTreeMap<String, f64>>,
    /// 包含凸性价格常数的检验数。 / Reduced cost including the convexity-price constant.
    pub reduced_cost: Option<f64>,
    /// 实际报告是否具有最优证明，不表示源最优。 / Whether the actual report has an optimality proof, not source optimality.
    pub optimal: bool,
    /// 无损原报告，不经 JS number 重编码。 / Lossless original report, never re-encoded through JS numbers.
    pub report_json: String,
}

/// 独立拥有领域注册、源模型及可变列池的分区会话。 / Partition session independently owning domain registration, source model and mutable column pool.
pub struct BppLoadingPartitionContext {
    context: PartitionLoadingContext<'static, f64, Unit>,
    source: MetaModel<f64>,
    artifact: ModelArtifact,
    options: BppLoadingPartitionOptions,
    decisions: Vec<BppLoadingPartitionDecision>,
    weight_unit: crate::QuantityUnit,
}

/// 原投影签发的不可伪造主问题对象；可独立释放。 / Unforgeable master object issued by the original projection, independently disposable.
pub struct BppLoadingPartitionMaster {
    value: ColumnMasterRegistration<f64>,
    artifact: ModelArtifact,
    constant: f64,
    columns: BTreeMap<String, u32>,
    globals: Vec<BppLoadingPartitionGlobalBinding>,
}

/// 原投影签发的只读定价快照，不接受裸对偶。 / Read-only pricing snapshot issued by the original projection, without bare duals.
pub struct BppLoadingPartitionPricing {
    request: ColumnPricingRequest<f64>,
    artifact: ModelArtifact,
    constant: f64,
    indices: BTreeMap<String, usize>,
}

/// 一轮定价候选的有界批次，整批消费同一对偶 epoch。 / Bounded batch of one pricing round, consuming one dual epoch atomically.
#[derive(Default)]
pub struct BppLoadingPartitionBatch { values: Vec<(ColumnPricingRequest<f64>, Vec<BTreeMap<String, f64>>)> }

fn artifact(model: &ospf_rust_core::model::intermediate::LinearTriadModel) -> Result<ModelArtifact> {
    let artifact = encode_compiled_model(model)?;
    if artifact.model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP partition model exceeds 8 MiB")); }
    crate::decode_compiled_model(&artifact)?; Ok(artifact)
}

impl BppLoadingPartitionContext {
    /// 独立重建并接管源注册，原离散 context 的释放不影响本会话。 / Independently rebuild and own the source registration, unaffected by disposal of the original discrete context.
    pub fn new(source: &ModelArtifact, options: BppLoadingPartitionOptions) -> Result<Self> {
        let bridge = BppLoadingContext::from_artifact(source)?; let domain = options.domain(&bridge)?;
        let weight_unit = bridge.problem().weight_unit;
        let resources = bridge.partition_resources()?.into_iter().map(|(id, position, variable)| (variable, (id, position))).collect::<BTreeMap<_, _>>();
        let (loading, source, artifact) = bridge.into_partition_parts();
        let context = PartitionLoadingContext::new_owned(loading, &source, domain).map_err(invalid)?;
        let decisions = context.decisions().iter().map(|decision| {
            let (role, position_id, item_id, resource_id) = if let Some((key, _)) = context.loading().aggregation().assignments.iter().find(|(_, variable)| **variable == decision.variable) {
                (BppLoadingPartitionDecisionRole::Assignment, key.position_id.clone(), Some(key.item_id.clone()), None)
            } else if let Some((position, _)) = context.loading().aggregation().occupied.iter().find(|(_, variable)| **variable == decision.variable) {
                (BppLoadingPartitionDecisionRole::Occupied, position.clone(), None, None)
            } else {
                let (id, position) = resources.get(&decision.variable).ok_or_else(|| invalid("unknown partition decision"))?;
                (BppLoadingPartitionDecisionRole::Resource, position.clone(), None, Some(id.clone()))
            };
            let index = source.tokens().iter().position(|token| token.id() == decision.variable).ok_or_else(|| invalid("missing partition source variable"))?;
            Ok(BppLoadingPartitionDecision { id: decision.id.clone(), partition_id: decision.block_id.clone(), variable_index: index as u32, role, position_id, item_id, resource_id })
        }).collect::<Result<_>>()?;
        Ok(Self { context, source, artifact, options, decisions, weight_unit })
    }

    /// 原始独立源封装。 / Original independent source artifact.
    pub fn artifact(&self) -> ModelArtifact { self.artifact.clone() }

    /// 显式配置的独立副本。 / Independent copy of the explicit configuration.
    pub fn options(&self) -> BppLoadingPartitionOptions { self.options.clone() }

    /// 原域签发的只读决策目录。 / Read-only decision catalog issued by the original domain.
    pub fn decisions(&self) -> &[BppLoadingPartitionDecision] { &self.decisions }

    /// 当前列元数据的独立副本。 / Independent copies of current column metadata.
    pub fn columns(&self) -> Vec<BppLoadingPartitionColumn> {
        self.context.columns().values().map(|column| BppLoadingPartitionColumn { id: column.id.clone(), partition_id: column.partition_id.clone(), decisions: column.decisions.clone(), assignments: column.assignments.clone(),
            occupied_positions: column.occupied_positions.iter().cloned().collect(), cargo_usage: column.cargo_usage.iter().map(|(id, value)| (id.clone(), value.to_string())).collect(),
            counts: column.counts.iter().map(|(id, value)| (id.clone(), value.to_string())).collect(),
            weights: column.weights.iter().map(|(id, weight)| (id.clone(), PhysicalQuantity { value: weight.value, unit: self.weight_unit })).collect(),
            compatible_layouts: column.compatible_layouts.iter().map(|(id, layouts)| (id.clone(), layouts.iter().cloned().collect())).collect() }).collect()
    }

    fn capacity(&self, columns: usize, entries: usize) -> Result<()> {
        if self.context.columns().len().saturating_add(columns) > COLUMN_LIMIT
            || self.context.columns().values().map(|column| column.decisions.len()).sum::<usize>().saturating_add(entries) > ENTRY_LIMIT {
            return Err(BridgeError::new("PayloadTooLarge", "BPP partition pool exceeds 4096 columns or 262144 decision entries"));
        }
        Ok(())
    }

    /// 全批验证后原子加列；失败不消费列身份或使旧注册失效。 / Atomically add fully validated columns; failure neither consumes identities nor invalidates registrations.
    pub fn add_columns(&mut self, columns: Vec<BppLoadingPartitionColumnInput>) -> Result<()> {
        if columns.len() > 512 { return Err(BridgeError::new("PayloadTooLarge", "BPP partition batch exceeds 512 columns")); }
        self.capacity(columns.len(), columns.iter().map(|column| column.decisions.len()).sum())?;
        self.context.add_columns(&self.source, columns.into_iter().map(|column| (column.partition_id, column.decisions)).collect()).map_err(invalid)
    }

    /// 原子删除后旧主问题与定价会话失效；列身份永不复用。 / Atomic removal invalidates old masters/pricing sessions; column identities are never reused.
    pub fn remove_columns(&mut self, ids: Vec<String>) -> Result<()> {
        if ids.len() > COLUMN_LIMIT || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() { return Err(invalid("invalid partition column removal set")); }
        self.context.remove_columns(&ids.into_iter().collect()).map_err(invalid)
    }

    /// 重建原主问题并冻结真实列映射，不拼装桥接数学模型。 / Rebuild the original master and freeze actual column mappings, without bridge-owned mathematics.
    pub fn master(&mut self, options: BppLoadingPartitionMasterOptions) -> Result<BppLoadingPartitionMaster> {
        let value = self.context.master(&self.source, options.phase_one, options.integer, options.refine_resources).map_err(invalid)?;
        let compilation = value.snapshot().compile_flt64().map_err(invalid)?; let artifact = artifact(&compilation.model)?;
        let indices = value.snapshot().variables.iter().enumerate().map(|(index, state)| (state.variable, index as u32)).collect::<BTreeMap<_, _>>();
        let columns = value.column_variables().iter().map(|(id, variable)| (id.clone(), indices[variable])).collect();
        let source_indices = self.source.tokens().iter().enumerate().map(|(index, token)| (token.id(), index as u32)).collect::<BTreeMap<_, _>>();
        let globals = value.global_variables().iter().map(|(source, master)| BppLoadingPartitionGlobalBinding { source_index: source_indices[source], master_index: indices[master] }).collect();
        Ok(BppLoadingPartitionMaster { constant: compilation.objective_constant, value, artifact, columns, globals })
    }

    /// 接收实际完整最优 LP 报告后绑定对偶；任何失败均使旧对偶失效。 / Bind duals only from an actual full optimal LP report; every failure invalidates prior duals.
    pub fn accept_lp_report(&mut self, master: &BppLoadingPartitionMaster, json: &str) -> Result<ReportSummary> {
        let mut error = None;
        let result = self.context.solve_master_lp_report::<u64, _>(&self.source, &master.value, |model| {
            crate::linear_report::decode_linear_report(model, json, true).map_err(|failure| {
                let message = failure.to_string(); error = Some(failure); ospf_rust_core::SolverError::ContractViolation(message).into()
            })
        });
        if let Some(error) = error { return Err(error); }
        result.map_err(invalid)?; summarize_report(json)
    }

    fn request(&self, master: Option<&BppLoadingPartitionMaster>, partition: &str, phase_one: bool) -> Result<BppLoadingPartitionPricing> {
        let master = master.map(|master| &master.value);
        let request = self.context.pricing_request(&self.source, master, partition, phase_one).map_err(invalid)?;
        let model = self.context.pricing_model(&self.source, master, &request).map_err(invalid)?;
        let snapshot = model.linear_model_snapshot().map_err(invalid)?; let compilation = snapshot.compile_flt64().map_err(invalid)?;
        let artifact = artifact(&compilation.model)?;
        let indices = self.context.decisions().iter().filter(|decision| decision.block_id == partition).map(|decision| {
            let index = snapshot.variables.iter().position(|state| state.variable == decision.variable).ok_or_else(|| invalid("missing pricing variable"))?;
            Ok((decision.id.clone(), index))
        }).collect::<Result<_>>()?;
        Ok(BppLoadingPartitionPricing { request, artifact, constant: compilation.objective_constant, indices })
    }

    /// 初始局部种子请求，不使用 LP 对偶。 / Initial local seed request, without LP duals.
    pub fn seed_pricing(&self, partition: &str, phase_one: bool) -> Result<BppLoadingPartitionPricing> { self.request(None, partition, phase_one) }

    /// 只从本会话当前 master 的已验证对偶签发请求。 / Issue a request only from verified duals of this session's current master.
    pub fn pricing(&self, master: &BppLoadingPartitionMaster, partition: &str, phase_one: bool) -> Result<BppLoadingPartitionPricing> { self.request(Some(master), partition, phase_one) }

    /// 一次提交同一轮所有分区候选，旧对偶只失效一次。 / Commit one round of partition candidates together, invalidating old duals exactly once.
    pub fn add_priced_batch(&mut self, master: &BppLoadingPartitionMaster, batch: &BppLoadingPartitionBatch) -> Result<()> {
        self.capacity(batch.values.iter().map(|(_, columns)| columns.len()).sum(), batch.values.iter().flat_map(|(_, columns)| columns).map(BTreeMap::len).sum())?;
        self.context.add_priced_batch(&self.source, &master.value, batch.values.clone()).map_err(invalid)
    }

    /// 按原整数 master 提取并复验完整源向量，拒绝 LP 与人工变量方案。 / Extract and validate a complete source vector from the original integer master, rejecting LP/artificial plans.
    pub fn source_values(&self, master: &BppLoadingPartitionMaster, values: &[f64]) -> Result<Vec<f64>> {
        let full = self.context.extract_solution(&self.source, &master.value, values).map_err(invalid)?;
        self.source.linear_model_snapshot().map_err(invalid)?.variables.iter().map(|state| full.get(&state.variable).copied().ok_or_else(|| invalid("incomplete partition source assignment"))).collect()
    }

    /// 原模型与完整领域双重复验后返回独立方案，不签发源最优证明。 / Return an independent plan after source-model/domain validation, without issuing source optimality proof.
    pub fn extract_solution(&self, master: &BppLoadingPartitionMaster, values: &[f64]) -> Result<BppLoadingResult> {
        BppLoadingContext::from_artifact(&self.artifact)?.extract_solution(&self.source_values(master, values)?)
    }

    /// 只物化白名单资源补全结果，整批成功后旧 master 与定价失效。 / Materialize only whitelisted resource completion, invalidating old masters/pricing after whole-batch success.
    pub fn materialize_completed_resources(&mut self, master: &BppLoadingPartitionMaster, values: &[f64]) -> Result<()> {
        self.capacity(self.options.partitions.len(), self.decisions.len())?;
        self.context.materialize_completed_resources(&self.source, &master.value, values).map_err(invalid)
    }
}

impl BppLoadingPartitionMaster {
    /// 实际编译模型，不含独立目标常数。 / Actual compiled model, excluding the separate objective constant.
    pub fn artifact(&self) -> ModelArtifact { self.artifact.clone() }
    /// 原快照独立目标常数。 / Separate objective constant of the original snapshot.
    pub fn objective_constant(&self) -> f64 { self.constant }
    /// 当前列到本次编译向量的索引。 / Current columns indexed into this compilation vector.
    pub fn column_indices(&self) -> &BTreeMap<String, u32> { &self.columns }
    /// 保留的全局／补全资源索引。 / Retained global/completion-resource indices.
    pub fn global_bindings(&self) -> &[BppLoadingPartitionGlobalBinding] { &self.globals }
}

impl BppLoadingPartitionPricing {
    /// 实际局部编译模型，原布局辅助变量均保留。 / Actual local compiled model, retaining original layout auxiliaries.
    pub fn artifact(&self) -> ModelArtifact { self.artifact.clone() }
    /// 凸性价格常数，不应再次加入 backend c*x。 / Convexity-price constant, never to be added again to backend c*x.
    pub fn objective_constant(&self) -> f64 { self.constant }
    /// 本请求的分区身份。 / Partition identity of this request.
    pub fn partition_id(&self) -> &str { self.request.block_id() }
    /// 完整局部向量检验数，仅针对本只读快照，不代表当前会话有效。 / Reduced cost of a complete local vector for this read-only snapshot, not current session validity.
    pub fn reduced_cost(&self, values: &BTreeMap<String, f64>) -> Result<f64> { self.request.reduced_cost(values).map_err(invalid) }
    /// 复验局部原报告并读取完整局部候选，不改写报告身份或精确元数据。 / Validate the original local report and read a complete candidate, without rewriting identity or exact metadata.
    pub fn read_report(&self, json: &str) -> Result<BppLoadingPartitionPricingResult> {
        let report = crate::linear_report::decode_linear_report(&crate::decode_compiled_model(&self.artifact)?, json, false)?;
        let decisions = report.solution.as_ref().map(|solution| self.indices.iter().map(|(id, index)| (id.clone(), solution.values[*index])).collect::<BTreeMap<_, _>>());
        let reduced_cost = decisions.as_ref().map(|values| self.reduced_cost(values)).transpose()?;
        Ok(BppLoadingPartitionPricingResult { decisions, reduced_cost, optimal: report.is_optimal(), report_json: json.into() })
    }
}

impl BppLoadingPartitionBatch {
    /// 拷贝只读请求及完整局部候选；不在此处修改源列池。 / Copy a read-only request and complete local candidates, without mutating the source pool here.
    pub fn push(&mut self, pricing: &BppLoadingPartitionPricing, columns: Vec<BTreeMap<String, f64>>) -> Result<()> {
        if self.values.len() >= 128 || self.values.iter().any(|(request, _)| request.block_id() == pricing.partition_id()) { return Err(invalid("duplicate or excessive partition pricing batch")); }
        if self.values.iter().map(|(_, columns)| columns.len()).sum::<usize>().saturating_add(columns.len()) > 512
            || self.values.iter().flat_map(|(_, columns)| columns).map(BTreeMap::len).sum::<usize>().saturating_add(columns.iter().map(BTreeMap::len).sum::<usize>()) > ENTRY_LIMIT {
            return Err(BridgeError::new("PayloadTooLarge", "BPP pricing batch exceeds its column/entry budget"));
        }
        for values in &columns { pricing.reduced_cost(values)?; }
        self.values.push((pricing.request.clone(), columns)); Ok(())
    }
}

#[cfg(test)]
mod tests;
