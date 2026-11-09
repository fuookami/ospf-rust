//! 独立的无损列生成报告读取器，不在 WASM 执行求解。 / Independent lossless column-generation report reader, without executing solvers in WASM.

use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{ColumnGenerationReport as Report, ModelArtifact, decode_column_generation_report};
use crate::{bridge_error, error};

/// 绑定源模型并复核完整赋值的原始结果。 / Original result bound to its source with complete-assignment verification.
#[wasm_bindgen]
pub struct ColumnGenerationReport { value: Report }

#[wasm_bindgen]
impl ColumnGenerationReport {
    /// 复验 schema、源指纹、赋值和子报告，不签发源证明。 / Recheck schema, source identity, assignment and child reports without issuing source proofs.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, artifact_json: &str) -> Result<ColumnGenerationReport, JsValue> {
        if json.len() > 8 * 1024 * 1024 { return Err(bridge_error(ospf_rust_bridge::BridgeError::new("PayloadTooLarge", "column-generation report exceeds 8 MiB"))); }
        let artifact: ModelArtifact = serde_json::from_str(artifact_json).map_err(error)?;
        Ok(Self { value: decode_column_generation_report(json, &artifact).map_err(bridge_error)? })
    }

    /// 无损原始封装。 / Lossless original envelope.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }

    /// Rust 生成契约定义的安全摘要。 / Safe summary defined by a Rust-generated contract.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 最后 RMP 原始文本。 / Last original RMP text.
    #[wasm_bindgen(js_name = masterReportJson)]
    pub fn master_report_json(&self) -> Option<String> { self.value.master_report_json.clone() }

    /// 最后定价原始文本。 / Last original pricing text.
    #[wasm_bindgen(js_name = pricingReportJson)]
    pub fn pricing_report_json(&self) -> Option<String> { self.value.pricing_report_json.clone() }

    /// 受限整数主问题原始文本，不是源模型证明。 / Original restricted-integer-master text, not a source-model proof.
    #[wasm_bindgen(js_name = finalReportJson)]
    pub fn final_report_json(&self) -> Option<String> { self.value.final_report_json.clone() }
}
