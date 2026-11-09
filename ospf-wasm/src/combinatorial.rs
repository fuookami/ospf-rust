//! 原生组合报告的可移植读取；不在 WASM 执行编排 / Portable native aggregate report reading, without WASM orchestration.

use wasm_bindgen::prelude::*;
use ospf_rust_core::solver::CombinatorialSolveReport;
use ospf_rust_bridge::{BridgeError, ModelArtifact, decode_combinatorial_report, decode_model};
use crate::{bridge_error, error};

/// 独立聚合报告，保留全部原始 attempt 轨迹 / Independent aggregate report retaining every original attempt trace.
#[wasm_bindgen]
pub struct CombinatorialReport { value: CombinatorialSolveReport<f64, u64> }

#[wasm_bindgen]
impl CombinatorialReport {
    /// 校验报告、attempt 身份及源模型指纹 / Validate report, attempt identities and source model fingerprint.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, artifact_json: &str) -> Result<CombinatorialReport, JsValue> {
        if json.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "combinatorial report exceeds 8 MiB"))); }
        let artifact: ModelArtifact = serde_json::from_str(artifact_json).map_err(error)?;
        if artifact.model_json.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "linear model exceeds 8 MiB"))); }
        let model = decode_model(&artifact).map_err(bridge_error)?;
        Ok(Self { value: decode_combinatorial_report(json, &model).map_err(bridge_error)? })
    }

    /// 全部精确元数据作为文本保留 / Preserve all exact metadata as text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }

    /// 最终统一 SolveReport 文本 / Final unified SolveReport text.
    #[wasm_bindgen(js_name = reportJson)]
    pub fn report_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.report).map_err(error) }

    /// 按配置的 solver 顺序返回 attempt 身份，不代表并行完成顺序 / Attempt identities in configured solver order, not parallel completion order.
    #[wasm_bindgen(js_name = attemptIds)]
    pub fn attempt_ids(&self) -> Vec<String> { self.value.attempts.iter().map(|attempt| attempt.attempt_id.clone()).collect() }

    /// 原始 attempt 轨迹文本，不经 JS 数值解析 / Original attempt trace text, without JS numeric parsing.
    #[wasm_bindgen(js_name = attemptJson)]
    pub fn attempt_json(&self, id: &str) -> Result<String, JsValue> {
        let attempt = self.value.attempts.iter().find(|attempt| attempt.attempt_id == id).ok_or_else(|| error("unknown combinatorial attempt ID"))?;
        serde_json::to_string(attempt).map_err(error)
    }

    /// 被选择的 attempt，无可用结果时为空 / Selected attempt, absent when no usable result exists.
    #[wasm_bindgen(js_name = selectedAttemptId)]
    pub fn selected_attempt_id(&self) -> Option<String> { self.value.selected_attempt_id.clone() }

    /// Rust framework 的选择依据 / Selection rationale from Rust framework.
    #[wasm_bindgen(js_name = selectionReason)]
    pub fn selection_reason(&self) -> Option<String> { self.value.selection_reason.clone() }
}
