//! 可移植的 BPP2D 源、报告及有界装箱，不在 WASM 执行生成或求解。 / Portable BPP2D source/report validation and bounded packing, without generation or solving in WASM.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::*;
use crate::{bridge_error, error};

fn parse<T: DeserializeOwned>(text: &str) -> std::result::Result<T, JsValue> {
    if text.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "BPP2D input exceeds 8 MiB"))); }
    serde_json::from_str(text).map_err(error)
}

/// 独立源目录与建模校验，不启动原生执行器或时钟。 / Independent source catalog and modeling validation, without native executors or clocks.
#[wasm_bindgen]
pub struct Bpp2dPackingContext { artifact: ModelArtifact }

#[wasm_bindgen]
impl Bpp2dPackingContext {
    /// 从 Rust 生成协议的 DTO 构造。 / Construct from a DTO following the Rust-generated protocol.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> std::result::Result<Bpp2dPackingContext, JsValue> {
        Ok(Self { artifact: encode_bpp2d_problem(parse(text)?).map_err(bridge_error)? })
    }

    /// 重新验证 canonical 源文本及指纹。 / Revalidate canonical source text and fingerprint.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(text: &str) -> std::result::Result<Bpp2dPackingContext, JsValue> {
        let artifact = parse(text)?;
        decode_bpp2d_problem(&artifact).map_err(bridge_error)?;
        Ok(Self { artifact })
    }

    /// 无损源封装。 / Lossless source artifact.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.artifact).map_err(error) }

    /// 绑定源物品／板材校验形状、旋转、刀缝及全部排布。 / Validate shapes, rotation, kerf and complete layouts against source items/sheets.
    #[wasm_bindgen(js_name = validateLayers)]
    pub fn validate_layers(&self, text: &str) -> std::result::Result<(), JsValue> {
        let (_, input) = decode_bpp2d_problem(&self.artifact).map_err(bridge_error)?;
        decode_bpp2d_layers(&input.request, &input.demands, &parse::<Vec<Bpp2dLayer>>(text)?).map_err(bridge_error)?;
        Ok(())
    }

    /// 任意物理排布的有界装箱副本，不声称已满足源需求。 / Bounded packing copy of arbitrary physical layouts, without claiming source-demand satisfaction.
    #[wasm_bindgen(js_name = packLayers)]
    pub fn pack_layers(&self, text: &str) -> std::result::Result<Bpp2dPackingResult, JsValue> {
        let layers: Vec<Bpp2dLayer> = parse(text)?;
        let usages = (0..layers.len()).map(|index| Ok(Bpp2dUsage { column_index: u32::try_from(index).map_err(error)?, count: "1".into() }))
            .collect::<std::result::Result<Vec<_>, JsValue>>()?;
        Ok(Bpp2dPackingResult { value: pack_bpp2d_layers(&self.artifact, &layers, &usages).map_err(bridge_error)? })
    }

    /// 复验完整执行，结果独立于 context 生命周期。 / Revalidate complete execution, returning a result independent of the context lifetime.
    #[wasm_bindgen(js_name = readExecutionReport)]
    pub fn read_execution_report(&self, text: &str) -> std::result::Result<Bpp2dExecutionReportResult, JsValue> {
        Ok(Bpp2dExecutionReportResult { value: decode_bpp2d_execution_report(text, &self.artifact).map_err(bridge_error)?, text: text.into(), source: self.artifact.clone() })
    }
}

/// 已联合验证的完整原生报告，不认证来源或全局最优。 / Jointly validated complete native report, without authenticating provenance or global optimality.
#[wasm_bindgen]
pub struct Bpp2dExecutionReportResult { value: Bpp2dExecutionReport, text: String, source: ModelArtifact }

#[wasm_bindgen]
impl Bpp2dExecutionReportResult {
    /// 按独立无损源封装复验。 / Revalidate against an independent lossless source artifact.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str, source: &str) -> std::result::Result<Bpp2dExecutionReportResult, JsValue> {
        let source = parse(source)?;
        Ok(Self { value: decode_bpp2d_execution_report(text, &source).map_err(bridge_error)?, text: text.into(), source })
    }

    /// 原完整文本，精确元数据不经 JS number。 / Original complete text, whose exact metadata bypasses JS number.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.text.clone() }

    /// Rust 派生的有限摘要。 / Rust-derived finite summary.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// Rust 派生的有序阶段摘要。 / Rust-derived ordered stage summaries.
    #[wasm_bindgen(js_name = stagesJson)]
    pub fn stages_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.stages).map_err(error) }

    /// 完整物理池，不是 final 选中子集。 / Complete physical pool, not the final selected subset.
    #[wasm_bindgen(js_name = columnsJson)]
    pub fn columns_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.artifact.columns).map_err(error) }

    /// 已验证的定价批次，不代表未观察候选。 / Validated pricing batches, not unobserved candidates.
    #[wasm_bindgen(js_name = iterationsJson)]
    pub fn iterations_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.artifact.iterations).map_err(error) }

    /// 保留实际模型身份及完整原报告的阶段封装。 / Stage artifact retaining actual-model identity and the complete raw report.
    #[wasm_bindgen(js_name = stageJson)]
    pub fn stage_json(&self, index: u32) -> std::result::Result<Option<String>, JsValue> {
        self.value.artifact.stages.get(index as usize).map(serde_json::to_string).transpose().map_err(error)
    }

    /// 原始子报告文本，u64 不降精度。 / Raw child-report text without u64 precision loss.
    #[wasm_bindgen(js_name = stageReportJson)]
    pub fn stage_report_json(&self, index: u32) -> Option<String> { self.value.artifact.stages.get(index as usize).map(|stage| stage.solver_report_json.clone()) }

    /// 独立拥有的生产副本；无整数 incumbent 则不存在。 / Independently owned production copy, absent without an integer incumbent.
    pub fn production(&self) -> Option<Bpp2dProductionResult> { self.value.production.clone().map(|value| Bpp2dProductionResult { value }) }

    /// 仅实际 final 生产可构造压缩装箱，不展开份数；超出独立预算则报错。 / Only actual final production yields compressed packing, without expanding copies; independent budget exhaustion is an error.
    pub fn packing(&self) -> std::result::Result<Option<Bpp2dPackingResult>, JsValue> {
        self.value.production.as_ref().map(|production| pack_bpp2d_layers(&self.source, &self.value.artifact.columns, &production.usages)
            .map(|value| Bpp2dPackingResult { value }).map_err(bridge_error)).transpose()
    }
}

/// 有界压缩装箱的独立副本，份数与物料件数保留文本。 / Independent bounded compressed-packing copy, retaining copy/material counts as text.
#[wasm_bindgen]
pub struct Bpp2dPackingResult { value: Bpp2dPacking }

#[wasm_bindgen]
impl Bpp2dPackingResult {
    /// Rust 生成的类型协议，长度为米、面积为平方米。 / Rust-generated typed contract, with lengths in meters and areas in square meters.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

/// 精确件数保留文本的独立生产结果。 / Independent production result retaining exact counts as text.
#[wasm_bindgen]
pub struct Bpp2dProductionResult { value: Bpp2dProduction }

#[wasm_bindgen]
impl Bpp2dProductionResult {
    /// 件数文本可安全转换为 JS bigint，面积仍为有限 number。 / Count text may safely become JS bigint; areas remain finite numbers.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests;
