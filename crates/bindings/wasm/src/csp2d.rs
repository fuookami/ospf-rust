//! CSP2D 源问题的可移植校验，不启动生成器或原生求解器。 / Portable CSP2D source validation, without generators or native solvers.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{BridgeError, Csp2dColumn, ModelArtifact, decode_csp2d_columns, decode_csp2d_problem, encode_csp2d_problem};
use crate::{bridge_error, error};

mod report;
pub use report::{Csp2dExecutionReportResult, Csp2dProductionResult};

const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: usize = MAX_TEXT_BYTES * 2 + 1024;

fn parse<T: DeserializeOwned>(text: &str, limit: usize) -> Result<T, JsValue> {
    if text.len() > limit { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "CSP2D input exceeds its text budget"))); }
    serde_json::from_str(text).map_err(error)
}

/// 独立拥有的源封装，全部校验复用共享 Rust codec 和原领域注册。 / Independently owned source artifact, with all validation reusing the shared Rust codec/original-domain registration.
#[wasm_bindgen]
pub struct Csp2dCuttingContext { artifact: ModelArtifact }

#[wasm_bindgen]
impl Csp2dCuttingContext {
    /// 从生成协议 DTO 文本构造，精确件数必须使用十进制字符串。 / Construct from generated-protocol DTO text, requiring decimal strings for exact counts.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> Result<Csp2dCuttingContext, JsValue> {
        Ok(Self { artifact: encode_csp2d_problem(parse(text, MAX_TEXT_BYTES)?).map_err(bridge_error)? })
    }

    /// 按原文复验 schema、canonical 编码及源指纹。 / Revalidate schema, canonical encoding and source fingerprint against the original text.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(text: &str) -> Result<Csp2dCuttingContext, JsValue> {
        // 外层引用会再次转义 canonical 源文本，必须能恢复所有已接受的源。 / The outer string escapes canonical source text again and must restore every accepted source.
        let artifact = parse(text, MAX_ARTIFACT_BYTES)?;
        decode_csp2d_problem(&artifact).map_err(bridge_error)?;
        Ok(Self { artifact })
    }

    /// 独立无损封装副本，内部文本不应经 JS 数值解析后改写。 / Independent lossless artifact copy; inner text should not be rewritten through JS numeric parsing.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.artifact).map_err(error) }

    /// 绑定源目录复验完整列池的尺寸、旋转、刀缝及切分树，不声称任何用量可行。 / Revalidate complete pool dimensions/rotation/kerf/cut trees against sources, without claiming any usage feasible.
    #[wasm_bindgen(js_name = validateColumns)]
    pub fn validate_columns(&self, text: &str) -> Result<(), JsValue> {
        let (_, input) = decode_csp2d_problem(&self.artifact).map_err(bridge_error)?;
        decode_csp2d_columns(&input, &parse::<Vec<Csp2dColumn>>(text, MAX_TEXT_BYTES)?).map_err(bridge_error)?;
        Ok(())
    }

    /// 按源、全池、实际模型／报告和定价生命周期复验，报告独立拥有。 / Revalidate against source, complete pool, actual models/reports and pricing lifecycle, returning an independently owned report.
    #[wasm_bindgen(js_name = readExecutionReport)]
    pub fn read_execution_report(&self, text: &str) -> Result<Csp2dExecutionReportResult, JsValue> {
        Csp2dExecutionReportResult::read(text, &self.artifact)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;
    use super::*;

    #[wasm_bindgen_test]
    fn portable_source_validation_uses_original_registration_without_native_clocks_or_lossy_counts() {
        let source = r#"{"products":[{"id":"a","name":"A","geometry":{"width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"allowRotate":false}],"materials":[{"id":"s","name":"S","shape":{"kind":"sheet","geometry":{"width":{"value":4,"unit":"m"},"height":{"value":2,"unit":"m"}}},"availableBatches":"10","process":"freeform","maxSegmentHeight":null,"products":[{"productId":"a","count":"4"}]}],"demands":[{"kind":"count","productId":"a","count":"9007199254740991","upperBound":null}],"initialColumns":[],"kerf":{"value":0,"unit":"m"},"objective":{"areaWeight":1,"wasteWeight":0,"heightWeight":0,"usageWeight":0}}"#;
        let context = Csp2dCuttingContext::new(source).unwrap();
        let artifact = context.to_artifact_json().unwrap(); assert!(artifact.contains("9007199254740991"));
        let restored = Csp2dCuttingContext::from_artifact(&artifact).unwrap(); drop(context);
        restored.validate_columns("[]").unwrap(); assert_eq!(restored.to_artifact_json().unwrap(), artifact);
        assert!(Csp2dCuttingContext::new(&source.replace("9007199254740991", "9007199254740992")).is_err());
        let mut forged: ModelArtifact = serde_json::from_str(&artifact).unwrap(); forged.fingerprint = "foreign".into();
        assert!(Csp2dCuttingContext::from_artifact(&serde_json::to_string(&forged).unwrap()).is_err());
    }

    #[wasm_bindgen_test]
    fn artifact_wrapper_budget_allows_json_escaping_without_relaxing_inner_source_budget() {
        let artifact = ModelArtifact { model_json: "\"".repeat(MAX_TEXT_BYTES), fingerprint: "a".repeat(64) };
        let wrapped = serde_json::to_string(&artifact).unwrap(); assert!(wrapped.len() > MAX_TEXT_BYTES);
        let restored: ModelArtifact = parse(&wrapped, MAX_ARTIFACT_BYTES).unwrap();
        assert_eq!(restored.model_json, artifact.model_json);
        let oversized = ModelArtifact { model_json: " ".repeat(MAX_TEXT_BYTES + 1), fingerprint: "a".repeat(64) };
        assert_eq!(decode_csp2d_problem(&oversized).unwrap_err().code, "PayloadTooLarge");
        assert!(parse::<ModelArtifact>(&" ".repeat(MAX_ARTIFACT_BYTES + 1), MAX_ARTIFACT_BYTES).is_err());
    }
}
