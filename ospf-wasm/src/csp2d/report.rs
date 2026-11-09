//! 仅复验原生报告及提取独立生产副本，不在 WASM 生成或求解。 / Only revalidate native reports and extract independent production copies, without WASM generation/solving.

use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{Csp2dExecutionReport, Csp2dProduction, ModelArtifact, decode_csp2d_execution_report};
use crate::{bridge_error, error};
use super::{MAX_ARTIFACT_BYTES, parse};

/// 绑定源与全部实际子阶段的独立报告，原文不经 JS 数值重编码。 / Independent report bound to its source/every actual child stage, without JS numeric re-encoding of original text.
#[wasm_bindgen]
pub struct Csp2dExecutionReportResult { value: Csp2dExecutionReport, text: String }

impl Csp2dExecutionReportResult {
    pub(super) fn read(text: &str, source: &ModelArtifact) -> Result<Self, JsValue> {
        Ok(Self { value: decode_csp2d_execution_report(text, source).map_err(bridge_error)?, text: text.into() })
    }
}

#[wasm_bindgen]
impl Csp2dExecutionReportResult {
    /// 用独立源封装复验完整报告，拒绝源／实际模型身份冲突。 / Revalidate a complete report against an independent source artifact, rejecting source/actual-model identity conflicts.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str, source: &str) -> Result<Csp2dExecutionReportResult, JsValue> {
        Self::read(text, &parse(source, MAX_ARTIFACT_BYTES)?)
    }

    /// 完整原文，精确 u64 元数据保持字节。 / Complete original text, retaining exact u64 metadata bytes.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.text.clone() }

    /// Rust 复验派生的有界摘要，受限最优不升级为源问题最优。 / Bounded Rust-revalidated summary, without promoting restricted optimality to source optimality.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 与实际执行同序的阶段摘要副本。 / Stage-summary copies in actual execution order.
    #[wasm_bindgen(js_name = stagesJson)]
    pub fn stages_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.stages).map_err(error) }

    /// 完整物理池，而非 final 选中子集。 / Complete physical pool, not just the final selected subset.
    #[wasm_bindgen(js_name = columnsJson)]
    pub fn columns_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.artifact.columns).map_err(error) }

    /// 以认证 LP 和原源行重算的定价批次，不代表完整搜索。 / Pricing batches recomputed from certified LPs/original source rows, not exhaustive search.
    #[wasm_bindgen(js_name = iterationsJson)]
    pub fn iterations_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.artifact.iterations).map_err(error) }

    /// 实际模型封装和原始子报告文本；越界返回空。 / Actual model artifact and original child-report text; out-of-range access returns none.
    #[wasm_bindgen(js_name = stageJson)]
    pub fn stage_json(&self, index: u32) -> Result<Option<String>, JsValue> {
        self.value.artifact.stages.get(index as usize).map(serde_json::to_string).transpose().map_err(error)
    }

    /// 原始统一子报告，不修复身份或舍入元数据。 / Original unified child report, without repairing identity or rounding metadata.
    #[wasm_bindgen(js_name = stageReportJson)]
    pub fn stage_report_json(&self, index: u32) -> Option<String> { self.value.artifact.stages.get(index as usize).map(|stage| stage.solver_report_json.clone()) }

    /// 仅来自实际 final 整数 incumbent 的独立副本，无解时为空。 / Independent copy derived only from an actual final integer incumbent, absent without one.
    pub fn production(&self) -> Option<Csp2dProductionResult> { self.value.production.clone().map(|value| Csp2dProductionResult { value }) }
}

/// 件数／批次精确文本、原 region 面积和卷段高的独立结果。 / Independent result with exact piece/batch text, original-region areas and coil-segment heights.
#[wasm_bindgen]
pub struct Csp2dProductionResult { value: Csp2dProduction }

#[wasm_bindgen]
impl Csp2dProductionResult {
    /// Rust 生成契约；件数安全转换 bigint，物理量仍是有限 f64。 / Rust-generated contract; counts safely convert to bigint while physical quantities remain finite f64.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests;
