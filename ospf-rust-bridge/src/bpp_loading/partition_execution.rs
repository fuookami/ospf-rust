//! 原分区 application 的有界证据与无时钟重放，所有迭代仍由原 Rust service 执行。
//! Bounded evidence and clock-free replay of the original partition application; all iterations stay in the original Rust service.

use std::cell::Cell;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::solver::{SolveHandle, TerminationReason};
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_bpp::application::{PartitionColumnGenerationApplicationService, PartitionColumnGenerationConfig,
    PartitionColumnGenerationControl, PartitionColumnGenerationResult, DiscreteLoadingCertificationResult};
use crate::{BridgeError, ModelArtifact, Result, encode_compiled_model, invalid};
use super::{BppLoadingApplicationEnvelope, BppLoadingApplicationReport, BppLoadingContext, BppLoadingPartitionOptions,
    BppLoadingCertificationEvidence, BppLoadingSourceFinishingOptions, MAX_BYTES, TOLERANCE,
    decode_bpp_loading_application, encode_bpp_loading_application};

/// 原生分区根节点列生成的有界配置，与 Benders 互斥。 / Bounded native root-partition CG configuration, exclusive with Benders.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionExecutionOptions {
    /// 完整舱位分区与资源白名单。 / Complete position partition and resource whitelist.
    pub partition: BppLoadingPartitionOptions,
    /// 根 LP 轮数，1..64，默认 64。 / Root LP rounds, 1..64, default 64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub iteration_limit: Option<u32>,
    /// 根池列数，分区数..4096，默认 512。 / Root pool columns, partition count..4096, default 512.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub column_limit: Option<u32>,
    /// 单独补全列预算，0..128，默认 128。 / Separate completion-column cap, 0..128, default 128.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub completion_column_limit: Option<u32>,
    /// 实际子求解次数，1..1024，默认 256；耗尽按原 service 的错误恢复语义处理。
    /// Actual child-solve calls, 1..1024, default 256; exhaustion follows original service error recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub report_limit: Option<u32>,
    /// 从同一总时限预留完整源 MILP 收尾预算，省略不收尾。 / Reserve full-source MILP finishing within the same total deadline; omitted means disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_milp_finishing: Option<BppLoadingSourceFinishingOptions>,
}

impl BppLoadingPartitionExecutionOptions {
    /// 验证配置与实际源变量归属，不运行后端。 / Validate configuration and actual source ownership, without running a backend.
    pub fn validate(&self, context: &BppLoadingContext) -> Result<()> {
        self.partition.domain(context)?;
        let columns = self.column_limit.unwrap_or(512);
        let completion = self.completion_column_limit.unwrap_or(128);
        if !(1..=64).contains(&self.iteration_limit.unwrap_or(64)) || columns > 4096
            || columns < self.partition.partitions.len() as u32 || completion > 128
            || (!self.partition.refinable_resources.is_empty() && completion < self.partition.partitions.len() as u32)
            || !(1..=1024).contains(&self.report_limit.unwrap_or(256))
            || self.source_milp_finishing.as_ref().is_some_and(|options| options.reserve_time_ms == 0) {
            return Err(invalid("invalid BPP partition application work budgets"));
        }
        Ok(())
    }

    /// 加速阶段共享总预算扣除显式预留量。 / Accelerator budget after subtracting the explicit reserve from the shared total.
    pub fn accelerator_budget(&self, time_limit_ms: u32) -> Result<Duration> {
        let total = Duration::from_millis(u64::from(time_limit_ms));
        self.source_milp_finishing.as_ref().map(|options| options.accelerator_budget(total)).transpose().map(|value| value.unwrap_or(total))
    }

    /// 转换为原配置；总时间由共享执行任务提供。 / Convert to the original configuration; total time comes from the shared execution job.
    pub fn config(&self, time_limit_ms: u32) -> PartitionColumnGenerationConfig {
        PartitionColumnGenerationConfig { iteration_limit: self.iteration_limit.unwrap_or(64) as usize,
            column_limit: self.column_limit.unwrap_or(512) as usize, completion_column_limit: self.completion_column_limit.unwrap_or(128) as usize,
            time_limit: Duration::from_millis(u64::from(time_limit_ms)), tolerance: TOLERANCE }
    }
}

/// 一次原算法预算观察，纳秒使用无损文本。 / One original-algorithm budget observation, with lossless nanosecond text.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionObservation {
    /// 是否终态检查；false 表示读取剩余时限。 / Terminal check; false means reading the remaining duration.
    pub terminal_check: bool,
    /// 0..总预算的 canonical u64 纳秒文本，单调不增。 / Canonical u64 nanoseconds in 0..total budget, monotonically nonincreasing.
    pub remaining_nanos: String,
    /// 已观察取消后不能恢复为 false。 / Must not become false after cancellation has been observed.
    pub cancelled: bool,
}

/// 原流程一次子模型调用，保留实际模型及报告原文；错误不伪造报告。
/// One child call in the original flow, retaining its actual model and original report; errors never fabricate a report.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionCall {
    /// 实际 compiled-linear 子模型。 / Actual compiled-linear child model.
    pub model: ModelArtifact,
    /// 原始报告文本，不经过 JS number。 / Original report text, never routed through JS numbers.
    pub report_json: Option<String>,
    /// 原始可恢复错误或明确的预算保护错误。 / Original recoverable error or explicit work-budget guard error.
    pub error: Option<BridgeError>,
}

/// 原分区 application 的完整源绑定证据。 / Complete source-bound evidence for the original partition application.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingPartitionEnvelope {
    /// 独立 schema。 / Independent schema.
    pub schema_version: String,
    /// 完整 canonical 源身份。 / Complete canonical source identity.
    pub source_fingerprint: String,
    /// 原执行配置。 / Original execution configuration.
    pub options: BppLoadingPartitionExecutionOptions,
    /// 全流程原预算，含源注册与子模型构造。 / Original total budget, including source registration and child construction.
    pub time_limit_ms: u32,
    /// 按调用顺序记录的预算观察。 / Budget observations in call order.
    pub observations: Vec<BppLoadingPartitionObservation>,
    /// 按调用顺序记录的实际子模型与报告。 / Actual child models/reports in call order.
    pub calls: Vec<BppLoadingPartitionCall>,
    /// 原 service 派生的无证明源报告。 / Proof-free source report derived by the original service.
    pub application_report_json: String,
    /// 可选完整源收尾证据；根流程证据始终保持原文。 / Optional full-source finishing evidence; root-flow evidence always remains original text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub certification: Option<BppLoadingCertificationEvidence>,
}

/// 原根 LP 轨迹，不声明完整源最优。 / Original root LP trace, without claiming complete-source optimality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BppLoadingPartitionIteration {
    /// 是否 Phase I。 / Whether Phase I.
    pub phase_one: bool,
    /// 含独立常数一次的 LP 目标。 / LP objective including its separate constant once.
    pub objective: f64,
    /// 实际新增列数。 / Actual added columns.
    pub added_columns: u32,
    /// 本轮局部定价是否都有原最优证明。 / Whether every local pricing call has its original optimality proof.
    pub pricing_proven: bool,
}

/// 无时钟重放原 service 得出的执行摘要。 / Execution summary derived by clock-free replay of the original service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BppLoadingPartitionSummary {
    /// 原根 LP 定价是否收敛，不是源 MILP 最优证明。 / Original root LP pricing convergence, not source-MILP optimality proof.
    pub lp_converged: bool,
    /// 根列池大小。 / Root pool size.
    pub root_column_count: u32,
    /// 补全新增列数。 / Additional completion columns.
    pub completion_column_count: u32,
    /// 原迭代轨迹。 / Original iteration trace.
    pub trace: Vec<BppLoadingPartitionIteration>,
    /// 实际资源补全报告在 calls 中的位置。 / Actual resource-completion report position in calls.
    pub refinement_call_index: Option<u32>,
    /// 实际受限 final 报告在 calls 中的位置。 / Actual restricted-final report position in calls.
    pub final_call_index: Option<u32>,
}

/// 独立复验的原文、源方案与原生流程摘要。 / Independently revalidated text, source plan and native-flow summary.
pub struct BppLoadingPartitionReport {
    /// 无损原文。 / Lossless original text.
    pub json: String,
    /// 全部原始证据。 / All original evidence.
    pub envelope: BppLoadingPartitionEnvelope,
    /// 已复验源报告与物理方案。 / Revalidated source report and physical plan.
    pub application: BppLoadingApplicationReport,
    /// 原 service 重放结果。 / Original service replay result.
    pub summary: BppLoadingPartitionSummary,
}

fn summary<C: Serialize>(result: &PartitionColumnGenerationResult<C>, calls: &[BppLoadingPartitionCall]) -> Result<BppLoadingPartitionSummary> {
    let position = |report: &Option<ospf_rust_core::solver::SolveReport<f64, C>>, before: usize| -> Result<Option<u32>> {
        let Some(report) = report else { return Ok(None); }; let expected = serde_json::to_value(report).map_err(invalid)?;
        for (index, call) in calls.iter().take(before).enumerate().rev() {
            if let Some(text) = &call.report_json {
                if serde_json::from_str::<serde_json::Value>(text).map_err(invalid)? == expected { return Ok(Some(index as u32)); }
            }
        }
        Err(invalid("original partition terminal report is missing from its child evidence"))
    };
    let final_call_index = position(&result.final_report, calls.len())?;
    let refinement_call_index = position(&result.refinement_report, final_call_index.map_or(calls.len(), |index| index as usize))?;
    Ok(BppLoadingPartitionSummary { lp_converged: result.lp_converged, root_column_count: result.root_column_count as u32,
        completion_column_count: result.completion_column_count as u32, trace: result.trace.iter().map(|step| BppLoadingPartitionIteration {
            phase_one: step.phase_one, objective: step.objective, added_columns: step.added_columns as u32, pricing_proven: step.pricing_proven }).collect(),
        refinement_call_index, final_call_index })
}

struct Replay<'a> { observations: &'a [BppLoadingPartitionObservation], position: Cell<usize> }

fn core(error: impl ToString) -> ospf_rust_core::error::CoreError { ospf_rust_core::SolverError::ContractViolation(error.to_string()).into() }

impl Replay<'_> {
    fn observe(&self, terminal: bool) -> ospf_rust_core::Result<(Duration, bool)> {
        let value = self.observations.get(self.position.get()).ok_or_else(|| core("missing partition budget observation"))?;
        if value.terminal_check != terminal { return Err(core("partition budget call order mismatch")); }
        let nanos = value.remaining_nanos.parse::<u64>().map_err(core)?;
        self.position.set(self.position.get() + 1); Ok((Duration::from_nanos(nanos), value.cancelled))
    }
}

impl PartitionColumnGenerationControl for Replay<'_> {
    fn remaining(&self) -> ospf_rust_core::Result<Duration> { Ok(self.observe(false)?.0) }
    fn terminal(&self) -> ospf_rust_core::Result<Option<TerminationReason>> {
        let (remaining, cancelled) = self.observe(true)?;
        Ok(if cancelled { Some(TerminationReason::Cancelled) } else if remaining.is_zero() { Some(TerminationReason::TimeLimit) } else { None })
    }
}

fn replay_error(error: &BridgeError) -> ospf_rust_core::Result<ospf_rust_core::error::CoreError> {
    use ospf_rust_core::SolverError;
    Ok(match error.code.as_str() {
        "UNSUPPORTED" => ospf_rust_core::error::CoreError::NotImplemented("recorded partition work guard or backend limitation".into()),
        "NUMERICAL" => SolverError::NumericalError("recorded partition numerical failure".into()).into(),
        "BACKEND" | "ENVIRONMENT" | "LICENSE" => SolverError::SolveFailed("recorded partition backend failure".into()).into(),
        _ => return Err(core("unsupported partition failure evidence")),
    })
}

/// 严格重放原 service 的模型、报告、预算观察与最终选择，不执行 solver 或时钟。
/// Strictly replay the original service's models, reports, budget observations and final selection, without a solver or clock.
pub fn decode_bpp_loading_partition(json: &str, source: &ModelArtifact) -> Result<BppLoadingPartitionReport> {
    if json.len() > MAX_BYTES * 16 + 8192 { return Err(BridgeError::new("PayloadTooLarge", "partition application envelope exceeds 128 MiB + 8 KiB")); }
    let envelope: BppLoadingPartitionEnvelope = serde_json::from_str(json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP partition application schema")); }
    if envelope.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP partition application source mismatch")); }
    let context = BppLoadingContext::from_artifact(source)?; envelope.options.validate(&context)?;
    if envelope.time_limit_ms == 0 || envelope.calls.len() > envelope.options.report_limit.unwrap_or(256) as usize + 1 || envelope.observations.len() > 16384 {
        return Err(invalid("partition evidence exceeds its work budget"));
    }
    let accelerator_budget = envelope.options.accelerator_budget(envelope.time_limit_ms)?;
    let mut remaining = accelerator_budget.as_nanos() as u64; let mut cancelled = false;
    for observation in &envelope.observations {
        let value = observation.remaining_nanos.parse::<u64>().map_err(invalid)?;
        if value.to_string() != observation.remaining_nanos || value > remaining || (cancelled && !observation.cancelled) {
            return Err(invalid("noncanonical or nonmonotonic partition budget evidence"));
        }
        remaining = value; cancelled = observation.cancelled;
    }
    let application = decode_bpp_loading_application(&envelope.application_report_json, source)?;
    let previous = envelope.certification.as_ref().map(|evidence| decode_bpp_loading_application(&evidence.previous_application_report_json, source)).transpose()?;
    let acceleration = previous.as_ref().unwrap_or(&application);
    let domain = envelope.options.partition.domain(&context)?;
    let registration = context.into_registration().map_err(invalid)?;
    let budget = Replay { observations: &envelope.observations, position: Cell::new(0) }; let mut position = 0; let mut bytes = 0usize;
    let result = PartitionColumnGenerationApplicationService::solve_with_control::<_, u64, _, _>(&registration, domain,
        envelope.options.config(accelerator_budget.as_millis() as u32), &SolveHandle::new(), &budget, |model, _, _| {
            let call = envelope.calls.get(position).ok_or_else(|| core("missing partition child call"))?; position += 1;
            let expected = encode_compiled_model(model).map_err(core)?;
            if expected.fingerprint != call.model.fingerprint || expected.model_json != call.model.model_json { return Err(core("partition child differs from its actual reconstructed model")); }
            let guarded = position > envelope.options.report_limit.unwrap_or(256) as usize || bytes.saturating_add(call.model.model_json.len()) > 32 * 1024 * 1024;
            if guarded && (call.report_json.is_some() || call.error.as_ref().is_none_or(|error| error.code != "UNSUPPORTED")) { return Err(core("partition child bypasses its report work guard")); }
            bytes = bytes.saturating_add(call.model.model_json.len()).saturating_add(call.report_json.as_ref().map_or(0, String::len));
            if call.report_json.is_some() && bytes > 32 * 1024 * 1024 { return Err(core("partition child exceeds its accumulated text budget")); }
            match (&call.report_json, &call.error) {
                (Some(text), None) => crate::linear_report::decode_linear_report(model, text, false).map_err(core),
                (None, Some(error)) => Err(replay_error(error)?),
                _ => Err(core("partition call requires exactly one report or error")),
            }
        }).map_err(invalid)?;
    if position != envelope.calls.len() || budget.position.get() != envelope.observations.len()
        || serde_json::to_value(&result.report).map_err(invalid)? != serde_json::from_str::<serde_json::Value>(&acceleration.envelope.report_json).map_err(invalid)?
        || result.objective != acceleration.envelope.objective {
        return Err(invalid("partition evidence or selected source report differs from the original service replay"));
    }
    if let Some(evidence) = &envelope.certification {
        if envelope.options.source_milp_finishing.is_none() { return Err(invalid("partition source finishing requires explicit configuration")); }
        super::finishing::validate(evidence, acceleration, &application, source)?;
    }
    Ok(BppLoadingPartitionReport { json: json.into(), summary: summary(&result, &envelope.calls)?, envelope, application })
}

#[cfg(test)]
mod tests;

/// 编码实际执行结果，并通过同一无时钟重放复验。 / Encode actual execution results and revalidate through the same clock-free replay.
pub fn encode_bpp_loading_partition(
    source: &ModelArtifact,
    mut envelope: BppLoadingPartitionEnvelope,
    result: &PartitionColumnGenerationResult,
) -> Result<String> {
    let application = BppLoadingApplicationEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        registration_present: result.report.fingerprints.model.is_some(), report_json: serde_json::to_string(&result.report).map_err(invalid)?, objective: result.objective };
    envelope.application_report_json = serde_json::to_string(&application).map_err(invalid)?;
    let text = serde_json::to_string(&envelope).map_err(invalid)?;
    let decoded = decode_bpp_loading_partition(&text, source)?;
    if decoded.summary != summary(result, &decoded.envelope.calls)? { return Err(invalid("partition summary differs from the original execution")); }
    Ok(text)
}

/// 附加真实完整源收尾结果，不覆盖原根流程或提升受限证明。
/// Attach actual full-source finishing without overwriting the original root flow or promoting restricted proofs.
pub fn encode_bpp_loading_partition_certification(
    accelerator_json: &str,
    result: &DiscreteLoadingCertificationResult<Unit>,
    source: &ModelArtifact,
) -> Result<String> {
    let mut envelope = decode_bpp_loading_partition(accelerator_json, source)?.envelope;
    if envelope.certification.is_some() || envelope.options.source_milp_finishing.is_none() {
        return Err(invalid("partition finishing requires one explicitly configured accelerator stage"));
    }
    envelope.certification = Some(BppLoadingCertificationEvidence { previous_application_report_json: envelope.application_report_json,
        report_json: result.certification_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?,
        error: result.certification_error.as_ref().map(|error| BridgeError::new(error.solver_error_class().code(), error)) });
    envelope.application_report_json = encode_bpp_loading_application(&result.result, source)?;
    let text = serde_json::to_string(&envelope).map_err(invalid)?;
    decode_bpp_loading_partition(&text, source)?; Ok(text)
}
