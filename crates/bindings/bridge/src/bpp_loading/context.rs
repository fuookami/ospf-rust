//! 独立装载源、真实模型与报告复验；不包含启发式求解或跨进程回调。
//! Independent loading source, actual model and report revalidation, without heuristic solving or cross-process callbacks.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::sha256_fingerprint;
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_bpp::{DiscreteLoadingContext, DiscreteLoadingSolution, RuntimePackingValueAdapter};
use crate::{BridgeError, ModelArtifact, PhysicalQuantity, ReportSummary, Result, decode_model, encode_model, invalid, summarize_report};
use super::{BppLoadingObjective, BppLoadingProblem, BppLoadingResource, BppLoadingResult, BppLoadingSolution, MAX_BYTES, TOLERANCE};
use super::resources::LoadingResources;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    schema_version: String,
    problem: BppLoadingProblem,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    objective: Option<BppLoadingObjective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resources: Option<Vec<BppLoadingResource>>,
}

/// 原 BPP context 与单一 MetaModel 的独立桥接，不在 TypeScript 重建装载约束。
/// Independent bridge owning the original BPP context and one MetaModel, without rebuilding loading constraints in TypeScript.
pub struct BppLoadingContext {
    artifact: ModelArtifact,
    problem: BppLoadingProblem,
    context: DiscreteLoadingContext<f64, Unit>,
    model: MetaModel<f64>,
    linear: ModelArtifact,
    resources: LoadingResources,
}

impl BppLoadingContext {
    /// 注册默认稀疏分配／容量／占位／冲突／布局约束，目标保持原纯可行性零目标。
    /// Register default sparse assignment/capacity/occupancy/conflict/layout constraints, retaining the original zero feasibility objective.
    pub fn new(problem: BppLoadingProblem) -> Result<Self> {
        Self::build(problem, None, None)
    }

    /// 注册预置货物／占位／布局成本，不接受任意模型回调。 / Register predefined cargo/occupancy/layout costs, without arbitrary modeling callbacks.
    pub fn with_objective(problem: BppLoadingProblem, objective: BppLoadingObjective) -> Result<Self> {
        Self::build(problem, Some(objective), None)
    }

    /// 预注册有界连续附加重量及最小装载比例，不接收任意回调。 / Preregister bounded continuous additional weight and minimum cargo ratios, without arbitrary callbacks.
    pub fn with_resources(
        problem: BppLoadingProblem,
        objective: Option<BppLoadingObjective>,
        resources: Vec<BppLoadingResource>,
    ) -> Result<Self> {
        Self::build(problem, objective, Some(resources))
    }

    fn build(
        problem: BppLoadingProblem,
        objective: Option<BppLoadingObjective>,
        resources: Option<Vec<BppLoadingResource>>,
    ) -> Result<Self> {
        if let Some(objective) = &objective { objective.validate(&problem)?; }
        let mut registered_resources = LoadingResources::new(&problem, resources.clone())?;
        let model_json = serde_json::to_string(&Envelope { schema_version: "1.0".into(), problem: problem.clone(), objective: objective.clone(), resources }).map_err(invalid)?;
        if model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP loading source exceeds 8 MiB")); }
        let fingerprint = sha256_fingerprint("ospf.bpp.loading.problem.v1", model_json.as_bytes()).value;
        // 源摘要进入实际名称；数学相同但货物身份不同的源不能互换原报告。 / Include the source digest in actual names so mathematically equal sources with different cargo identities cannot exchange original reports.
        let mut context = DiscreteLoadingContext::new(problem.domain()?, &RuntimePackingValueAdapter).map_err(invalid)?
            .with_namespace(format!("bpp_loading_{fingerprint}")).map_err(invalid)?;
        let mut model = MetaModel::new("bpp-loading");
        context.register_with(&mut model, |model, aggregation| {
            if let Some(objective) = &objective { objective.register(model, aggregation)?; }
            Ok(())
        }).map_err(invalid)?;
        registered_resources.register(&problem, &mut context, &mut model, &format!("bpp_loading_{fingerprint}"))?;
        let linear = encode_model(&model.try_to_linear_triad_model().map_err(invalid)?)?;
        if linear.model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP loading execution model exceeds 8 MiB")); }
        // 确认实际导出模型可由执行端既有 codec 还原；绝不绕过交换门禁。 / Confirm the actual exported model passes the executor's existing codec rather than bypassing exchange gates.
        decode_model(&linear)?;
        Ok(Self { artifact: ModelArtifact { model_json, fingerprint }, problem, context, model, linear, resources: registered_resources })
    }

    /// 校验版本、canonical 源文本及领域分离摘要，随后独立注册默认模型。 / Verify version, canonical source text and domain-separated digest before independent default-model registration.
    pub fn from_artifact(artifact: &ModelArtifact) -> Result<Self> {
        if artifact.model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP loading source exceeds 8 MiB")); }
        let envelope: Envelope = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
        if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP loading source schema")); }
        let canonical = serde_json::to_string(&envelope).map_err(invalid)?;
        if canonical != artifact.model_json || sha256_fingerprint("ospf.bpp.loading.problem.v1", canonical.as_bytes()).value != artifact.fingerprint {
            return Err(BridgeError::new("FingerprintMismatch", "BPP loading source fingerprint or canonical text mismatch"));
        }
        Self::build(envelope.problem, envelope.objective, envelope.resources)
    }

    /// 独立源封装，不等于执行模型的指纹。 / Independent source artifact, distinct from the execution-model fingerprint.
    pub fn artifact(&self) -> ModelArtifact { self.artifact.clone() }

    pub(super) fn problem(&self) -> &BppLoadingProblem { &self.problem }

    pub(super) fn partition_resources(&self) -> Result<Vec<(String, String, ospf_rust_core::variable::VariableId)>> {
        self.resources.specifications().iter().map(|resource| {
            let column = self.resources.column(&resource.id).ok_or_else(|| invalid("missing registered partition resource"))?;
            let token = self.model.tokens().get(column).ok_or_else(|| invalid("missing partition resource token"))?;
            Ok((resource.id.clone(), resource.position_id.clone(), token.id()))
        }).collect()
    }

    pub(super) fn into_partition_parts(self) -> (DiscreteLoadingContext<f64, Unit>, MetaModel<f64>, ModelArtifact) {
        (self.context, self.model, self.artifact)
    }

    /// 原 MetaModel 编译的 MILP，省略业务成本时为零目标。 / MILP compiled from the original MetaModel, with a zero objective when business costs are omitted.
    pub fn linear_model(&self) -> ModelArtifact { self.linear.clone() }

    /// 原 application 实际使用 snapshot 编译，不将其报告重标记为 mechanism 导出模型。 / Actual snapshot compilation used by the original application, never relabeling its reports as the mechanism-export model.
    pub fn application_model(&self) -> Result<ModelArtifact> {
        let compiled = self.model.linear_model_snapshot().map_err(invalid)?.compile_flt64().map_err(invalid)?;
        if compiled.objective_constant != 0.0 { return Err(invalid("BPP loading application does not support objective constants")); }
        let artifact = encode_model(&compiled.model)?;
        if artifact.model_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP loading application model exceeds 8 MiB")); }
        decode_model(&artifact)?;
        Ok(artifact)
    }

    /// 保留本次初始注册身份供分解编码，不与回退会话混用。 / Retain this initial registration's identities for decomposition encoding, never mixing fallback sessions.
    pub fn application_snapshot(&self) -> Result<ospf_rust_core::model::LinearModelSnapshot<f64>> {
        self.model.linear_model_snapshot().map_err(invalid)
    }

    /// 仅按已声明的连续资源身份划分追索，拒绝空集合、重复及未知资源。 / Stage recourse only by declared continuous resource identities, rejecting empty, duplicate or unknown selections.
    pub fn benders_partition_options(&self, resources: &[String]) -> Result<ospf_rust_framework::solver::LinearBendersPartitionOptions> {
        use ospf_rust_framework::solver::{LinearBendersPartitionOptions, LinearBendersStage};
        if resources.is_empty() || resources.len() > 128 { return Err(invalid("BPP loading Benders requires 1..128 explicit recourse resources")); }
        let mut stages = BTreeMap::new();
        for id in resources {
            let column = self.resources.column(id).ok_or_else(|| invalid("unknown BPP loading recourse resource"))?;
            let variable = self.model.tokens().get(column).ok_or_else(|| invalid("missing BPP loading resource token"))?.id();
            if stages.insert(variable, LinearBendersStage::ContinuousRecourse).is_some() { return Err(invalid("duplicate BPP loading recourse resource")); }
        }
        Ok(LinearBendersPartitionOptions { stages })
    }

    /// 将完整注册移交原 application；所有权转移后不共享模型会话。 / Transfer the complete registration to the original application, without sharing model sessions after ownership transfer.
    pub fn into_registration(self) -> ospf_rust_core::Result<ospf_rust_framework_bpp::application::RegisteredDiscreteLoading<f64, Unit>> {
        ospf_rust_framework_bpp::application::RegisteredDiscreteLoading::new(self.context, self.model)
    }

    /// 保全原非传递边的团覆盖，不把连通分量当作互斥团。 / Original edge-preserving clique cover, never treating connected components as conflict cliques.
    pub fn conflict_cliques(&self) -> Vec<Vec<String>> {
        self.context.conflict_index().cliques().iter().map(|clique| clique.iter().cloned().collect()).collect()
    }

    /// 从完整领域方案生成独立二进制向量，并通过原模型及领域双重验证。 / Build an independent binary vector from a complete domain plan and validate both the original model and domain.
    pub fn solution_values(&self, solution: &BppLoadingSolution) -> Result<Vec<f64>> {
        self.solution_values_with_resources(solution, &BTreeMap::new())
    }

    /// 完整方案必须显式提供全部资源重量，不猜测零值或最优值。 / A complete plan must explicitly provide every resource weight, never guessing zero or optimal values.
    pub fn solution_values_with_resources(
        &self,
        solution: &BppLoadingSolution,
        resources: &BTreeMap<String, PhysicalQuantity>,
    ) -> Result<Vec<f64>> {
        let solution = solution.domain()?;
        let mut values = vec![0.0; self.model.num_tokens()];
        let additional = self.resources.fill_values(&self.problem, resources, &mut values)?;
        self.context.verify(&solution, &additional, &TOLERANCE).map_err(invalid)?;
        let mut select = |variable| -> Result<()> {
            let index = self.model.find_token(variable).ok_or_else(|| invalid("missing BPP loading token"))?.solver_index;
            values[index] = 1.0;
            Ok(())
        };
        let aggregation = self.context.aggregation();
        for (key, variable) in &aggregation.assignments {
            if solution.assignments.get(&key.item_id) == Some(&key.position_id) { select(*variable)?; }
        }
        for position in &solution.occupied_positions { select(aggregation.occupied[position])?; }
        for (group, layout) in &solution.selected_layouts { select(aggregation.layouts[&(group.clone(), layout.clone())])?; }
        let extracted = self.context.extract_solution(&self.model, &values, &TOLERANCE).map_err(invalid)?;
        if extracted != solution { return Err(invalid("BPP loading plan is not representable by its actual registered sparse model")); }
        Ok(values)
    }

    /// 独立复核外部领域方案，不声明求解器证明或几何可行性。 / Independently validate an external domain plan without claiming solver proof or geometric feasibility.
    pub fn verify_solution(&self, solution: &BppLoadingSolution) -> Result<BppLoadingResult> {
        self.extract_solution(&self.solution_values(solution)?)
    }

    /// 复核含显式资源重量的完整方案，并独立返回含资源总量。 / Verify a complete plan with explicit resource weights and independently return inclusive totals.
    pub fn verify_solution_with_resources(
        &self,
        solution: &BppLoadingSolution,
        resources: &BTreeMap<String, PhysicalQuantity>,
    ) -> Result<BppLoadingResult> {
        self.extract_solution(&self.solution_values_with_resources(solution, resources)?)
    }

    /// 原注册模型完整向量的行／边界／整数性与领域复验。 / Full-vector row/bound/integrality and domain validation against the original registration.
    pub fn extract_solution(&self, values: &[f64]) -> Result<BppLoadingResult> {
        let solution = self.context.extract_solution(&self.model, values, &TOLERANCE).map_err(invalid)?;
        self.result(&solution, values)
    }

    fn result(&self, solution: &DiscreteLoadingSolution, values: &[f64]) -> Result<BppLoadingResult> {
        let mut counts = self.problem.positions.iter().map(|position| (position.id.clone(), 0u64)).collect::<BTreeMap<_, _>>();
        let mut weights = self.problem.positions.iter().map(|position| (position.id.clone(), 0.0)).collect::<BTreeMap<_, _>>();
        let mut total = 0.0;
        for item in &self.problem.items {
            if let Some(position) = solution.assignments.get(&item.id) {
                let weight = item.weight.quantity()?.to_unit(&self.problem.weight_unit.unit()).map_err(invalid)?.value;
                *counts.get_mut(position).ok_or_else(|| invalid("unknown BPP loading result position"))? += 1;
                *weights.get_mut(position).ok_or_else(|| invalid("unknown BPP loading result position"))? += weight;
                total += weight;
                if !total.is_finite() || !weights[position].is_finite() { return Err(invalid("BPP loading total weight overflow")); }
            }
        }
        let resources = self.resources.result(&self.problem, values, &weights)?;
        Ok(BppLoadingResult { source_fingerprint: self.artifact.fingerprint.clone(), solution: BppLoadingSolution::from_domain(solution), resources,
            position_counts: counts.into_iter().map(|(id, value)| (id, value.to_string())).collect(),
            position_weights: weights.into_iter().map(|(id, value)| (id, PhysicalQuantity { value, unit: self.problem.weight_unit })).collect(),
            loaded_count: solution.assignments.len().to_string(), loaded_weight: PhysicalQuantity { value: total, unit: self.problem.weight_unit } })
    }

    /// 只接受本 context 实际导出模型的原始报告；有限 incumbent 经完整模型与领域复验。
    /// Accept only original reports for this context's actual exported model; every incumbent passes full model/domain validation.
    pub fn read_report(&self, json: &str) -> Result<BppLoadingReport> {
        let model = decode_model(&self.linear)?;
        let report = crate::linear_report::decode_linear_report(&model, json, false)?;
        let solution = report.solution.as_ref().map(|solution| self.extract_solution(&solution.values)).transpose()?;
        Ok(BppLoadingReport { source_fingerprint: self.artifact.fingerprint.clone(), json: json.into(), summary: summarize_report(json)?, solution })
    }
}

/// 原报告与独立领域结果；无 incumbent 时不虚构空方案。 / Original report and independent domain result, without fabricating an empty plan when no incumbent exists.
pub struct BppLoadingReport {
    /// 已验证的源问题身份。 / Verified source-problem identity.
    pub source_fingerprint: String,
    /// 无损原始报告文本，精确 u64 元数据不经 JS number。 / Original lossless report text, keeping exact u64 metadata out of JS number.
    pub json: String,
    /// 原 core 报告摘要，不改写终态。 / Original core report summary, without terminal rewriting.
    pub summary: ReportSummary,
    /// 仅在实际 incumbent 复验后存在。 / Present only after revalidation of an actual incumbent.
    pub solution: Option<BppLoadingResult>,
}
