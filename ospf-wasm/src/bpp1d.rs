//! 一维装箱纯计算与独立报告读取，不在 WASM 执行原生求解。 / Portable BPP1D computation and independent reports, without native solving in WASM.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::*;
use crate::{bridge_error, error};

fn parse<T: DeserializeOwned>(json: &str) -> std::result::Result<T, JsValue> {
    if json.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "BPP1D input exceeds 8 MiB"))); }
    serde_json::from_str(json).map_err(error)
}

/// 复用 Rust 单位归一、候选生成和物理校验。 / Reuse Rust normalization, candidate generation and physical validation.
#[wasm_bindgen]
pub struct Bpp1dPackingContext { problem: Bpp1dProblem, context: Bpp1dContext, artifact: ModelArtifact }

#[wasm_bindgen]
impl Bpp1dPackingContext {
    /// 从生成契约的 DTO 构造。 / Construct from the generated-contract DTO.
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> std::result::Result<Bpp1dPackingContext, JsValue> {
        let artifact = encode_bpp1d_problem(parse(json)?).map_err(bridge_error)?;
        Self::from_artifact(&serde_json::to_string(&artifact).map_err(error)?)
    }

    /// 验证 canonical 文本与源身份。 / Verify canonical text and source identity.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(json: &str) -> std::result::Result<Bpp1dPackingContext, JsValue> {
        let artifact = parse(json)?;
        let (problem, context) = decode_bpp1d_problem(&artifact).map_err(bridge_error)?;
        Ok(Self { problem, context, artifact })
    }

    /// 完整无损源封装。 / Complete lossless source artifact.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.artifact).map_err(error) }

    /// 有界默认规则搜索，结果独立持有数据。 / Bounded default-policy search with independently owned results.
    #[wasm_bindgen(js_name = generatePatterns)]
    pub fn generate_patterns(&self, json: &str) -> std::result::Result<Bpp1dPatternGenerationResult, JsValue> {
        Ok(Bpp1dPatternGenerationResult { value: generate_bpp1d_patterns(&self.context, &parse(json)?).map_err(bridge_error)? })
    }

    /// 校验单列的容量、质量及需求。 / Validate one pattern's capacity, mass and demand.
    #[wasm_bindgen(js_name = validatePattern)]
    pub fn validate_pattern(&self, json: &str) -> std::result::Result<(), JsValue> {
        let pattern: Bpp1dPattern = parse(json)?;
        self.context.validate_pattern(&pattern.pattern().map_err(bridge_error)?).map_err(error)
    }

    /// 不展开物理件数，使用受检整数校验总需求和库存。 / Check total demand and stock with checked integers, without physical expansion.
    #[wasm_bindgen(js_name = verifyUsages)]
    pub fn verify_usages(&self, json: &str) -> std::result::Result<(), JsValue> {
        self.context.verify_usages(&bpp1d_usages(&parse::<Vec<Bpp1dUsage>>(json)?).map_err(bridge_error)?).map_err(error)
    }

    /// 有界展开并返回独立物理结果。 / Bounded expansion returning independently owned physical results.
    pub fn pack(&self, json: &str, expansion_limit: u32) -> std::result::Result<Bpp1dPackingResult, JsValue> {
        if expansion_limit == 0 || expansion_limit > 100_000 { return Err(error("expansion limit must be in [1, 100000]")); }
        let result = self.context.pack(&bpp1d_usages(&parse::<Vec<Bpp1dUsage>>(json)?).map_err(bridge_error)?, expansion_limit.into()).map_err(error)?;
        Ok(Bpp1dPackingResult { value: encode_bpp1d_packing(&self.context, &result, self.problem.length_unit).map_err(bridge_error)? })
    }

    /// 归一并复核外部坐标，不信任导入的 KPI。 / Normalize and verify external coordinates without trusting imported KPIs.
    #[wasm_bindgen(js_name = verifyPlans)]
    pub fn verify_plans(&self, json: &str) -> std::result::Result<Bpp1dPackingResult, JsValue> {
        let result = verify_bpp1d_plans(&self.context, &parse::<Vec<Bpp1dPlan>>(json)?).map_err(bridge_error)?;
        Ok(Bpp1dPackingResult { value: encode_bpp1d_packing(&self.context, &result, self.problem.length_unit).map_err(bridge_error)? })
    }

    /// 从两张严格 CSV 构造：表头必须闭合，空库存表示无限；产物与 JSON 构造等价。
    /// Build from two strict CSV tables: the header must be closed and an empty inventory means unlimited stock;
    /// the result is equivalent to JSON construction.
    #[wasm_bindgen(js_name = fromCsv)]
    pub fn from_csv(items_text: &str, bins_text: &str, options_json: &str) -> std::result::Result<Bpp1dPackingContext, JsValue> {
        let options: Bpp1dCsvOptions = parse(options_json)?;
        let problem = load_bpp1d_csv(items_text, bins_text, &options).map_err(bridge_error)?;
        let artifact = encode_bpp1d_problem(problem).map_err(bridge_error)?;
        Self::from_artifact(&serde_json::to_string(&artifact).map_err(error)?)
    }

    /// 用原域渲染适配器生成展示方案；展示单位下的数值不是精度或最优性证据。
    /// Render through the original domain adapter; display-unit numbers are never precision or optimality evidence.
    pub fn render(&self, json: &str, unit: &str) -> std::result::Result<String, JsValue> {
        let unit: ospf_rust_bridge::QuantityUnit = serde_json::from_value(serde_json::Value::String(unit.into())).map_err(error)?;
        let schema = render_bpp1d_packing(&self.context, &parse::<Vec<Bpp1dPlan>>(json)?, unit).map_err(bridge_error)?;
        serde_json::to_string(&schema).map_err(error)
    }
}

/// 与上下文独立释放的候选结果。 / Candidate result disposed independently of its context.
#[wasm_bindgen]
pub struct Bpp1dPatternGenerationResult { value: Bpp1dPatterns }

#[wasm_bindgen]
impl Bpp1dPatternGenerationResult {
    /// 生成契约定义的独立数据副本。 / Independent data copy defined by the generated contract.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

/// 已物理验证、独立拥有的结果。 / Physically validated and independently owned result.
#[wasm_bindgen]
pub struct Bpp1dPackingResult { value: Bpp1dPacking }

#[wasm_bindgen]
impl Bpp1dPackingResult {
    /// 精确物料计数仍为文本。 / Exact material counts remain text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

/// 复验物理方案的无损原生执行报告。 / Lossless native execution report with physical-plan revalidation.
#[wasm_bindgen]
pub struct Bpp1dReport { value: ospf_rust_bridge::Bpp1dReport }

#[wasm_bindgen]
impl Bpp1dReport {
    /// 验证问题指纹、core 契约和完整物理结果。 / Verify problem identity, core contract and complete physical results.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, artifact: &str) -> std::result::Result<Bpp1dReport, JsValue> {
        Ok(Self { value: decode_bpp1d_report(json, &parse(artifact)?).map_err(bridge_error)? })
    }

    /// 保留原始子报告的整数文本。 / Preserve original child-report integer text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }

    /// 生成契约定义的安全摘要。 / Safe summary defined by the generated contract.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 候选模型原始求解报告，不伪装成源问题报告。 / Original candidate-model report, not relabeled as a source-problem report.
    #[wasm_bindgen(js_name = solverReportJson)]
    pub fn solver_report_json(&self) -> String { self.value.solver_report_json.clone() }

    /// 每次返回新的独立结果。 / Return a fresh independent result each time.
    pub fn packing(&self) -> Option<Bpp1dPackingResult> { self.value.packing.clone().map(|value| Bpp1dPackingResult { value }) }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn portable_domain_reuses_units_exact_counts_and_independent_results() {
        let context = Bpp1dPackingContext::new(r#"{"items":[{"id":"a","length":{"value":40,"unit":"mm"},"weight":{"value":100,"unit":"g"},"amount":"2","materials":{"steel":"9007199254740993"}}],"binTypes":[{"id":"bin","length":{"value":10,"unit":"cm"},"maxWeight":{"value":1,"unit":"kg"},"cost":1,"inventory":null}],"spacing":{"value":1,"unit":"cm"},"lengthUnit":"cm","weightUnit":"kg","objective":"binCount"}"#).unwrap();
        let result = context.pack(r#"[{"pattern":{"binTypeId":"bin","amounts":{"a":"2"}},"count":"1"}]"#, 2).unwrap();
        let generated = context.generate_patterns("{}").unwrap();
        assert!(context.pack("[]", 2).is_err());
        assert!(context.generate_patterns(r#"{"maxNodes":0}"#).is_err());
        let text = result.to_json().unwrap();
        assert!(text.contains("18014398509481986"));
        assert_eq!(context.verify_plans(&serde_json::to_string(&result.value.plans).unwrap()).unwrap().to_json().unwrap(), text);
        assert_eq!(result.value.kpi.remnant_length.value, 1.0);
        drop(context);
        assert_eq!(result.to_json().unwrap(), text);
        assert!(generated.to_json().unwrap().contains("\"complete\":true"));
    }
}
