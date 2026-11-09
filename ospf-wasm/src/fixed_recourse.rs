//! 原生固定追索结果的可移植读取，不在 WASM 执行算法 / Portable native fixed-recourse result reader, without WASM algorithm execution.

use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{FixedRecourseReport as Report, ModelArtifact, decode_fixed_recourse_report};
use crate::{bridge_error, error};

/// 独立 Rust 报告；源模型释放后仍有效 / Independent Rust report, valid after source-model disposal.
#[wasm_bindgen]
pub struct FixedRecourseReport { value: Report }

#[wasm_bindgen]
impl FixedRecourseReport {
    /// 校验 schema、源模型身份、完整可行赋值和目标，不独立重放切割 / Validate schemas, source identity, complete feasible assignment and objective, without independently replaying cuts.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, artifact_json: &str) -> Result<FixedRecourseReport, JsValue> {
        if json.len() > 8 * 1024 * 1024 { return Err(bridge_error(ospf_rust_bridge::BridgeError::new("PayloadTooLarge", "fixed-recourse report exceeds 8 MiB"))); }
        let artifact: ModelArtifact = serde_json::from_str(artifact_json).map_err(error)?;
        Ok(Self { value: decode_fixed_recourse_report(json, &artifact).map_err(bridge_error)? })
    }

    /// 精确原始结果文本 / Exact original result text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }

    /// 由 Rust 生成契约定义的安全结果投影 / Safe result projection defined by a Rust-generated contract.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 最后实际 master 报告，不是源模型证明 / Last actual master report, not a source-model proof.
    #[wasm_bindgen(js_name = masterReportJson)]
    pub fn master_report_json(&self) -> Option<String> { self.value.master_report_json.clone() }

    /// 最后实际 recourse 报告，恢复证书模型的非控制终态仍见 stop / Last actual recourse report; a noncontrol certificate-recovery terminal remains represented by stop.
    #[wasm_bindgen(js_name = subproblemReportJson)]
    pub fn subproblem_report_json(&self) -> Option<String> { self.value.subproblem_report_json.clone() }
}
