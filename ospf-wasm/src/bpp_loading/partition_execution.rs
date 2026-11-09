//! 原分区 application 的无时钟报告门面，不在 WASM 执行求解器。
//! Clock-free report facade for the original partition application, without executing solvers in WASM.

use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{BppLoadingPartitionReport, decode_bpp_loading_partition};
use super::{DiscreteLoadingResult, MAX_BYTES, parse};
use crate::{bridge_error, error};

/// 独立重放验证的分区报告和源方案。 / Independently replay-validated partition report and source plan.
#[wasm_bindgen]
pub struct PartitionColumnGenerationReport { value: BppLoadingPartitionReport }

#[wasm_bindgen]
impl PartitionColumnGenerationReport {
    /// 重建原 service 的实际子模型及控制流，不调用时钟或后端。 / Reconstruct the original service's actual children and control flow, without clocks or backends.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str, source: &str) -> Result<PartitionColumnGenerationReport, JsValue> {
        Ok(Self { value: decode_bpp_loading_partition(text, &parse(source, MAX_BYTES * 2 + 1024)?).map_err(bridge_error)? })
    }

    /// 保留完整原文。 / Retain complete original text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.value.json.clone() }

    /// 选定源报告摘要，只有实际完整源报告能提供源证明。 / Selected source-report summary; only an actual complete-source report can supply source proofs.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.application.summary).map_err(error) }

    /// 原根 LP／资源补全轨迹。 / Original root LP/resource-completion trace.
    #[wasm_bindgen(js_name = executionSummaryJson)]
    pub fn execution_summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 按原顺序保存实际子模型与无损报告，不经 JS 重编码。 / Preserve actual child models and lossless reports in original order, without JS re-encoding.
    #[wasm_bindgen(js_name = callsJson)]
    pub fn calls_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.envelope.calls).map_err(error) }

    /// 最终选定的 core 源报告，不重标记子报告。 / Final selected core source report, without relabeling child reports.
    #[wasm_bindgen(js_name = solverReportJson)]
    pub fn solver_report_json(&self) -> String { self.value.application.envelope.report_json.clone() }

    /// 最终选定的完整源 application 原文。 / Final selected complete-source application text.
    #[wasm_bindgen(js_name = applicationReportJson)]
    pub fn application_report_json(&self) -> String { self.value.envelope.application_report_json.clone() }

    /// 保留收尾前的根流程源报告，不被认证结果覆盖。 / Retain the pre-finishing root-flow source report without certification overwrites.
    #[wasm_bindgen(js_name = accelerationApplicationReportJson)]
    pub fn acceleration_application_report_json(&self) -> String {
        self.value.envelope.certification.as_ref().map(|evidence| evidence.previous_application_report_json.clone()).unwrap_or_else(|| self.application_report_json())
    }

    /// 实际完整源后端报告，未调用则为空。 / Actual complete-source backend report, absent without a call.
    #[wasm_bindgen(js_name = certificationReportJson)]
    pub fn certification_report_json(&self) -> Option<String> { self.value.envelope.certification.as_ref().and_then(|evidence| evidence.report_json.clone()) }

    /// 原收尾可恢复错误，不与根阶段错误混淆。 / Original recoverable finishing error, separate from root-stage failures.
    #[wasm_bindgen(js_name = certificationErrorJson)]
    pub fn certification_error_json(&self) -> Result<Option<String>, JsValue> {
        self.value.envelope.certification.as_ref().and_then(|evidence| evidence.error.as_ref()).map(serde_json::to_string).transpose().map_err(error)
    }

    /// 是否实际经过显式收尾阶段，包括前置停止。 / Whether an explicit finishing stage was processed, including pre-solve stops.
    #[wasm_bindgen(js_name = sourceFinishingPresent)]
    pub fn source_finishing_present(&self) -> bool { self.value.envelope.certification.is_some() }

    /// 是否调用完整源后端并得到报告或可恢复错误。 / Whether the complete-source backend produced a report or recoverable error.
    #[wasm_bindgen(js_name = sourceFinishingInvoked)]
    pub fn source_finishing_invoked(&self) -> bool {
        self.value.envelope.certification.as_ref().is_some_and(|evidence| evidence.report_json.is_some() || evidence.error.is_some())
    }

    /// 实际资源补全原报告；未调用或无报告时为空。 / Actual original resource-completion report, absent without a reported call.
    #[wasm_bindgen(js_name = refinementReportJson)]
    pub fn refinement_report_json(&self) -> Option<String> { self.value.summary.refinement_call_index.and_then(|index| self.value.envelope.calls[index as usize].report_json.clone()) }

    /// 实际受限整数 final 原报告，不是完整源证明。 / Actual restricted-integer final report, never a complete-source proof.
    #[wasm_bindgen(js_name = finalReportJson)]
    pub fn final_report_json(&self) -> Option<String> { self.value.summary.final_call_index.and_then(|index| self.value.envelope.calls[index as usize].report_json.clone()) }

    /// 已有完整源方案之后保留的可恢复错误。 / Recoverable error retained after obtaining a complete-source plan.
    #[wasm_bindgen(js_name = accelerationErrorJson)]
    pub fn acceleration_error_json(&self) -> Result<Option<String>, JsValue> {
        self.value.envelope.calls.last().and_then(|call| call.error.as_ref()).map(serde_json::to_string).transpose().map_err(error)
    }

    /// 完整 canonical 源身份。 / Complete canonical source identity.
    #[wasm_bindgen(js_name = sourceFingerprint)]
    pub fn source_fingerprint(&self) -> String { self.value.envelope.source_fingerprint.clone() }

    /// 每份方案独立拥有，报告释放不使其失效。 / Each plan is independently owned and survives report disposal.
    pub fn solution(&self) -> Option<DiscreteLoadingResult> { self.value.application.solution.clone().map(|value| DiscreteLoadingResult { value }) }
}
