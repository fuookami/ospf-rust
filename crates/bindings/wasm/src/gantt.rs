//! 可移植 Gantt 活动调度源与方案复验，不在 WASM 求解。 / Portable Gantt activity-scheduling source and plan replay without WASM solving.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{
    BridgeError, GanttActivityExecutionReport, GanttActivityPlan, GanttActivityProblem, GanttActivityReplayReport,
    GanttActivitySourceSummary, ModelArtifact, decode_gantt_activity_execution_report, decode_gantt_activity_problem,
    encode_gantt_activity_problem, prepare_gantt_activity_source, replay_gantt_activity_plan,
};
use crate::{bridge_error, error};

fn parse<T: DeserializeOwned>(text: &str) -> std::result::Result<T, JsValue> {
    if text.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "Gantt activity input exceeds 8 MiB"))); }
    serde_json::from_str(text).map_err(error)
}

/// 独立活动调度源、候选展开与显式方案复验，不执行求解器。 / Independent activity-scheduling source, candidate expansion and explicit-plan replay without solver execution.
#[wasm_bindgen]
pub struct GanttActivityContext { artifact: ModelArtifact }

#[wasm_bindgen]
impl GanttActivityContext {
    /// 从 Rust 生成协议 DTO 构造。 / Construct from a Rust-generated protocol DTO.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> std::result::Result<GanttActivityContext, JsValue> {
        Ok(Self { artifact: encode_gantt_activity_problem(parse::<GanttActivityProblem>(text)?).map_err(bridge_error)? })
    }

    /// 复验原文、指纹及领域源。 / Revalidate original text, fingerprint and domain source.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(text: &str) -> std::result::Result<GanttActivityContext, JsValue> {
        let artifact = parse(text)?;
        decode_gantt_activity_problem(&artifact).map_err(bridge_error)?;
        Ok(Self { artifact })
    }

    /// 无损源封装。 / Lossless source artifact.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.artifact).map_err(error) }

    /// 已验证源 DTO，时间保持 RFC 3339 文本。 / Validated source DTO with RFC 3339 text times.
    #[wasm_bindgen(js_name = problemJson)]
    pub fn problem_json(&self) -> std::result::Result<String, JsValue> {
        let (problem, _) = decode_gantt_activity_problem(&self.artifact).map_err(bridge_error)?;
        serde_json::to_string(&problem).map_err(error)
    }

    /// 展开完整有限候选并返回摘要，不建模型也不求解。 / Expand complete finite candidates into a summary without building a model or solving.
    #[wasm_bindgen(js_name = sourceSummaryJson)]
    pub fn source_summary_json(&self) -> std::result::Result<String, JsValue> {
        serde_json::to_string(&prepare_gantt_activity_source(&self.artifact).map_err(bridge_error)?).map_err(error)
    }

    /// 复验显式方案的完整候选、原域规则与库存轨迹。 / Revalidate an explicit plan's complete choices, native rules and inventory trails.
    #[wasm_bindgen(js_name = replayJson)]
    pub fn replay_json(&self, text: &str) -> std::result::Result<String, JsValue> {
        let plan = parse::<GanttActivityPlan>(text)?;
        serde_json::to_string(&replay_gantt_activity_plan(&self.artifact, &plan).map_err(bridge_error)?).map_err(error)
    }

    /// 复验原生执行报告，结果独立于 context 生命周期。 / Revalidate a native execution report, returning a result independent of the context lifetime.
    #[wasm_bindgen(js_name = readExecutionReport)]
    pub fn read_execution_report(&self, text: &str) -> std::result::Result<GanttActivityExecutionReportResult, JsValue> {
        Ok(GanttActivityExecutionReportResult {
            value: decode_gantt_activity_execution_report(text, &self.artifact).map_err(bridge_error)?, text: text.into(), source: self.artifact.clone(),
        })
    }
}

/// 独立拥有的源摘要。 / Independently owned source summary.
#[wasm_bindgen]
pub struct GanttActivitySourceSummaryResult { value: GanttActivitySourceSummary, source: ModelArtifact }

#[wasm_bindgen]
impl GanttActivitySourceSummaryResult {
    /// 读取并校验摘要文本。 / Read and validate summary text.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str, source: &str) -> std::result::Result<GanttActivitySourceSummaryResult, JsValue> {
        let source: ModelArtifact = parse(source)?;
        let value = parse::<GanttActivitySourceSummary>(text)?;
        let actual = prepare_gantt_activity_source(&source).map_err(bridge_error)?;
        if actual != value { return Err(bridge_error(BridgeError::new("InvalidInput", "Gantt activity summary does not match its source"))); }
        Ok(Self { value, source })
    }

    /// 摘要文本。 / Summary text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }

    /// 已复验源指纹。 / Revalidated source fingerprint.
    #[wasm_bindgen(js_name = sourceFingerprint)]
    pub fn source_fingerprint(&self) -> String { self.value.source_fingerprint.clone() }

    /// 活动、模式、候选、资源与依赖计数。 / Activity, mode, choice, resource and dependency counts.
    #[wasm_bindgen(js_name = countsJson)]
    pub fn counts_json(&self) -> std::result::Result<String, JsValue> {
        serde_json::to_string(&serde_json::json!({ "activityCount": self.value.activity_count, "modeCount": self.value.mode_count,
            "choiceCount": self.value.choice_count, "resourceCount": self.value.resource_count, "dependencyCount": self.value.dependency_count })).map_err(error)
    }

    /// 源身份文本，便于重新打开 context。 / Source text, so the owning context can be reopened.
    #[wasm_bindgen(js_name = sourceJson)]
    pub fn source_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.source).map_err(error) }
}

/// 独立拥有的方案复验结果。 / Independently owned plan-replay result.
#[wasm_bindgen]
pub struct GanttActivityReplayResult { value: GanttActivityReplayReport }

#[wasm_bindgen]
impl GanttActivityReplayResult {
    /// 读取方案结果文本；不重新求解。 / Read plan-result text without re-solving.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str) -> std::result::Result<GanttActivityReplayResult, JsValue> {
        Ok(Self { value: parse::<GanttActivityReplayReport>(text)? })
    }

    /// 结果文本。 / Result text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }

    /// 原域费用文本。 / Native-domain cost text.
    #[wasm_bindgen(js_name = costJson)]
    pub fn cost_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.schedule.cost).map_err(error) }

    /// 分配、取消与候选计数。 / Placement, cancellation and candidate counts.
    #[wasm_bindgen(js_name = countsJson)]
    pub fn counts_json(&self) -> std::result::Result<String, JsValue> {
        serde_json::to_string(&serde_json::json!({ "placementCount": self.value.schedule.placements.len(),
            "cancelledCount": self.value.schedule.cancelled.len(), "candidateCount": self.value.schedule.candidate_count })).map_err(error)
    }
}

/// 独立拥有的执行报告。 / Independently owned execution report.
#[wasm_bindgen]
pub struct GanttActivityExecutionReportResult { value: GanttActivityExecutionReport, text: String, source: ModelArtifact }

#[wasm_bindgen]
impl GanttActivityExecutionReportResult {
    /// 读取并校验报告文本。 / Read and validate report text.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str, source: &str) -> std::result::Result<GanttActivityExecutionReportResult, JsValue> {
        let source: ModelArtifact = parse(source)?;
        let value = decode_gantt_activity_execution_report(text, &source).map_err(bridge_error)?;
        Ok(Self { value, text: text.into(), source })
    }

    /// 原始报告文本。 / Original report text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.text.clone() }

    /// 原问题状态与终止原因。 / Original problem status and termination reason.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> std::result::Result<String, JsValue> {
        serde_json::to_string(&serde_json::json!({ "problemStatus": self.value.problem_status, "terminationReason": self.value.termination_reason,
            "objectiveValue": self.value.objective_value, "solverModelOnly": self.value.solver_model_only,
            "hasSchedule": self.value.schedule.is_some(), "failure": self.value.failure })).map_err(error)
    }

    /// 分配快照文本，无 incumbent 时为空。 / Placement snapshot text, absent without an incumbent.
    #[wasm_bindgen(js_name = scheduleJson)]
    pub fn schedule_json(&self) -> std::result::Result<Option<String>, JsValue> {
        match &self.value.schedule { Some(schedule) => serde_json::to_string(schedule).map(Some).map_err(error), None => Ok(None) }
    }

    /// 已复验源封装，供重新打开 context。 / Revalidated source artifact for reopening a context.
    #[wasm_bindgen(js_name = sourceJson)]
    pub fn source_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.source).map_err(error) }
}
