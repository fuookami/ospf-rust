//! 可移植 BPP3D 源校验，不在 WASM 生成列或求解。 / Portable BPP3D source validation without WASM column generation or solving.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{
    BridgeError, Bpp3dAssignment, Bpp3dExecutionReport, Bpp3dPacking, Bpp3dProblem, ModelArtifact, decode_bpp3d_execution_report,
    decode_bpp3d_problem, encode_bpp3d_problem, pack_bpp3d_layers,
};
use crate::{bridge_error, error};

fn parse<T: DeserializeOwned>(text: &str) -> std::result::Result<T, JsValue> {
    if text.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "BPP3D input exceeds 8 MiB"))); }
    serde_json::from_str(text).map_err(error)
}

/// 独立源与真实物理 trace 校验，不执行求解器。 / Independent source and real physical-trace validation without solver execution.
#[wasm_bindgen]
pub struct Bpp3dPackingContext { artifact: ModelArtifact }

#[wasm_bindgen]
impl Bpp3dPackingContext {
    /// 从 Rust 生成协议 DTO 构造。 / Construct from a Rust-generated protocol DTO.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> std::result::Result<Bpp3dPackingContext, JsValue> {
        Ok(Self { artifact: encode_bpp3d_problem(parse::<Bpp3dProblem>(text)?).map_err(bridge_error)? })
    }

    /// 复验原文、指纹及物理源。 / Revalidate original text, fingerprint and physical source.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(text: &str) -> std::result::Result<Bpp3dPackingContext, JsValue> {
        let artifact = parse(text)?;
        decode_bpp3d_problem(&artifact).map_err(bridge_error)?;
        Ok(Self { artifact })
    }

    /// 无损源封装。 / Lossless source artifact.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.artifact).map_err(error) }

    /// 已验证源 DTO；数量保持十进制文本。 / Validated source DTO with decimal-text counts.
    #[wasm_bindgen(js_name = problemJson)]
    pub fn problem_json(&self) -> std::result::Result<String, JsValue> {
        let (problem, _, _) = decode_bpp3d_problem(&self.artifact).map_err(bridge_error)?;
        serde_json::to_string(&problem).map_err(error)
    }

    /// 严格回放完整源层分配并返回米制物理方案，不执行求解。 / Strictly replay complete source-layer assignments into a meter-based physical plan without solving.
    #[wasm_bindgen(js_name = packLayersJson)]
    pub fn pack_layers_json(&self, text: &str) -> std::result::Result<String, JsValue> {
        let assignments = parse::<Vec<Bpp3dAssignment>>(text)?;
        let packing = pack_bpp3d_layers(&self.artifact, &assignments).map_err(bridge_error)?;
        serde_json::to_string(&packing).map_err(error)
    }

    /// 复验完整单轮执行报告，结果独立于 context 生命周期。 / Revalidate a complete one-round execution report, returning a result independent of the context lifetime.
    #[wasm_bindgen(js_name = readExecutionReport)]
    pub fn read_execution_report(&self, text: &str) -> std::result::Result<Bpp3dExecutionReportResult, JsValue> {
        Ok(Bpp3dExecutionReportResult { value: decode_bpp3d_execution_report(text, &self.artifact).map_err(bridge_error)?, text: text.into(), source: self.artifact.clone() })
    }
}

/// 已联合验证的完整单轮执行报告，不认证求解器／生成器来源或源全局最优。 / Jointly validated complete one-round execution report, without authenticating solver/generator provenance or source-wide optimality.
#[wasm_bindgen]
pub struct Bpp3dExecutionReportResult { value: Bpp3dExecutionReport, text: String, source: ModelArtifact }

#[wasm_bindgen]
impl Bpp3dExecutionReportResult {
    /// 按独立无损源封装复验。 / Revalidate against an independent lossless source artifact.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str, source: &str) -> std::result::Result<Bpp3dExecutionReportResult, JsValue> {
        let source = parse(source)?;
        Ok(Self { value: decode_bpp3d_execution_report(text, &source).map_err(bridge_error)?, text: text.into(), source })
    }

    /// 原完整文本，精确整数与子报告不经 JS number。 / Original complete text whose exact integers and child reports bypass JS number.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.text.clone() }

    /// Rust 派生的有限摘要。 / Rust-derived finite summary.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 原流程结果 info 中的连续半径条目；无半径源时为空对象。 / Continuous-radius entries from the original flow info; empty for sources without one.
    #[wasm_bindgen(js_name = continuousRadiusJson)]
    pub fn continuous_radius_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.artifact.continuous_radius).map_err(error) }

    /// Rust 派生的有序子阶段摘要。 / Rust-derived ordered child-stage summaries.
    #[wasm_bindgen(js_name = stagesJson)]
    pub fn stages_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.stages).map_err(error) }

    /// 保留实际列池及子报告的阶段封装。 / Stage artifact retaining the actual column pool and child report.
    #[wasm_bindgen(js_name = stageJson)]
    pub fn stage_json(&self, index: u32) -> std::result::Result<Option<String>, JsValue> {
        self.value.artifact.stages.get(index as usize).map(serde_json::to_string).transpose().map_err(error)
    }

    /// 该阶段实际求解所用的源绑定列池文本。 / Source-bound pool text actually solved by this stage.
    #[wasm_bindgen(js_name = stagePoolJson)]
    pub fn stage_pool_json(&self, index: u32) -> Option<String> { self.value.artifact.stages.get(index as usize).map(|stage| stage.pool_json.clone()) }

    /// 原始子报告文本；后端未接受报告时不存在。 / Raw child-report text, absent when the backend accepted none.
    #[wasm_bindgen(js_name = stageReportJson)]
    pub fn stage_report_json(&self, index: u32) -> Option<String> {
        self.value.artifact.stages.get(index as usize).and_then(|stage| stage.child.as_ref()).map(|child| child.solver_report_json.clone())
    }

    /// 独立拥有的源绑定装箱；仅严格物理回放成功时存在。 / Independently owned source-bound packing, present only after strict physical replay succeeds.
    pub fn packing(&self) -> Option<Bpp3dPackingResult> { self.value.packing.clone().map(|value| Bpp3dPackingResult { value }) }

    /// 该结果的源封装，供另一次复验使用。 / Source artifact of this result, reusable for an independent revalidation.
    #[wasm_bindgen(js_name = sourceJson)]
    pub fn source_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.source).map_err(error) }
}

/// 独立拥有的米制装箱副本，不声称求解器来源或最优性。 / Independently owned meter-based packing copy, without solver-provenance or optimality claims.
#[wasm_bindgen]
pub struct Bpp3dPackingResult { value: Bpp3dPacking }

#[wasm_bindgen]
impl Bpp3dPackingResult {
    /// Rust 生成的类型协议，长度为米。 / Rust-generated typed contract with lengths in meters.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}
