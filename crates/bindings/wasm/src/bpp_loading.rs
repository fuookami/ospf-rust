//! 共享 BPP 稀疏装载建模与报告读取，求解继续使用显式原生客户端。
//! Shared BPP sparse-loading modeling and report reading, with solving left to explicit native clients.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{BppLoadingApplicationReport, BppLoadingBendersReport, BppLoadingContext, BppLoadingReport, BppLoadingResult, BppLoadingSolution, BridgeError, decode_bpp_loading_application, decode_bpp_loading_benders};
use crate::{bridge_error, error};

mod partition;
pub use partition::{PartitionLoadingBatch, PartitionLoadingContext, PartitionLoadingMaster, PartitionLoadingPricing};
mod partition_execution;
pub use partition_execution::PartitionColumnGenerationReport;

const MAX_BYTES: usize = 8 * 1024 * 1024;

fn parse<T: DeserializeOwned>(text: &str, limit: usize) -> Result<T, JsValue> {
    if text.len() > limit { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "BPP loading input exceeds its text budget"))); }
    serde_json::from_str(text).map_err(error)
}

/// 独立注册的原共享离散装载 context，不导出任意扩展回调。 / Independently registered original shared discrete-loading context, without arbitrary extension callbacks.
#[wasm_bindgen]
pub struct DiscreteLoadingContext { value: BppLoadingContext }

#[wasm_bindgen]
impl DiscreteLoadingContext {
    /// 从生成契约构造；maxCount 使用 canonical 正 u64 文本。 / Construct from the generated contract, with maxCount encoded as canonical positive u64 text.
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<DiscreteLoadingContext, JsValue> {
        Ok(Self { value: BppLoadingContext::new(parse(json, MAX_BYTES)?).map_err(bridge_error)? })
    }

    /// 通过原 register_with 路径注册最小化成本，负成本为奖励。 / Register minimizing costs through the original register_with path, using negative costs as rewards.
    #[wasm_bindgen(js_name = withObjective)]
    pub fn with_objective(json: &str, objective: &str) -> Result<DiscreteLoadingContext, JsValue> {
        Ok(Self { value: BppLoadingContext::with_objective(parse(json, MAX_BYTES)?, parse(objective, MAX_BYTES)?).map_err(bridge_error)? })
    }

    /// 预注册有界连续附加重量；成本按模型重量单位计价。 / Preregister bounded continuous additional weight, with costs priced in the model weight unit.
    #[wasm_bindgen(js_name = withResources)]
    pub fn with_resources(
        json: &str,
        objective: &str,
        resources: &str,
    ) -> Result<DiscreteLoadingContext, JsValue> {
        Ok(Self { value: BppLoadingContext::with_resources(parse(json, MAX_BYTES)?, parse(objective, MAX_BYTES)?, parse(resources, MAX_BYTES)?).map_err(bridge_error)? })
    }

    /// 恢复独立源并复验 canonical 文本、版本与指纹。 / Restore an independent source after canonical-text, version and fingerprint validation.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(json: &str) -> Result<DiscreteLoadingContext, JsValue> {
        Ok(Self { value: BppLoadingContext::from_artifact(&parse(json, MAX_BYTES * 2 + 1024)?).map_err(bridge_error)? })
    }

    /// 独立无损源封装。 / Independent lossless source artifact.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.artifact()).map_err(error) }

    /// 原 MetaModel 注册的 MILP，不在 WASM 求解。 / MILP from original MetaModel registration, never solved in WASM.
    #[wasm_bindgen(js_name = toLinearTriadModelJson)]
    pub fn to_linear_triad_model_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.linear_model()).map_err(error) }

    /// 原 application 的实际 snapshot 编译模型，与 mechanism 导出身份不同。 / Actual snapshot-compiled application model, distinct from the mechanism-export identity.
    #[wasm_bindgen(js_name = toApplicationModelJson)]
    pub fn to_application_model_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.application_model().map_err(bridge_error)?).map_err(error) }

    /// 原非传递冲突边的完整团覆盖。 / Complete clique cover of the original non-transitive conflict edges.
    #[wasm_bindgen(js_name = conflictCliquesJson)]
    pub fn conflict_cliques_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.conflict_cliques()).map_err(error) }

    /// 复核领域方案并返回独立复制的完整向量。 / Validate a domain plan and return an independently copied full vector.
    #[wasm_bindgen(js_name = solutionValues)]
    pub fn solution_values(&self, json: &str) -> Result<Vec<f64>, JsValue> {
        self.value.solution_values(&parse::<BppLoadingSolution>(json, MAX_BYTES)?).map_err(bridge_error)
    }

    /// 方案显式携带全部资源重量，不猜测缺失资源值。 / Plans explicitly carry every resource weight, without guessing missing values.
    #[wasm_bindgen(js_name = solutionValuesWithResources)]
    pub fn solution_values_with_resources(&self, json: &str, resources: &str) -> Result<Vec<f64>, JsValue> {
        self.value.solution_values_with_resources(&parse(json, MAX_BYTES)?, &parse(resources, MAX_BYTES)?).map_err(bridge_error)
    }

    /// 复核方案并返回独立拥有的件数、重量与占位结果。 / Validate a plan and return independently owned count, weight and occupancy results.
    #[wasm_bindgen(js_name = verifySolution)]
    pub fn verify_solution(&self, json: &str) -> Result<DiscreteLoadingResult, JsValue> {
        Ok(DiscreteLoadingResult { value: self.value.verify_solution(&parse(json, MAX_BYTES)?).map_err(bridge_error)? })
    }

    /// 复核原容量、占位、比例与连续变量边界，并返回独立资源结果。 / Validate original capacity, occupancy, ratios and continuous bounds, returning independent resource results.
    #[wasm_bindgen(js_name = verifySolutionWithResources)]
    pub fn verify_solution_with_resources(&self, json: &str, resources: &str) -> Result<DiscreteLoadingResult, JsValue> {
        Ok(DiscreteLoadingResult { value: self.value.verify_solution_with_resources(&parse(json, MAX_BYTES)?, &parse(resources, MAX_BYTES)?).map_err(bridge_error)? })
    }

    /// 按实际原模型完整向量提取，不截断或忽略多余列。 / Extract from a complete actual-model vector, without truncation or ignored extra columns.
    #[wasm_bindgen(js_name = extractSolution)]
    pub fn extract_solution(&self, values: &[f64]) -> Result<DiscreteLoadingResult, JsValue> {
        Ok(DiscreteLoadingResult { value: self.value.extract_solution(values).map_err(bridge_error)? })
    }

    /// 原始报告必须绑定本源实际导出的模型，不修补身份或状态。 / Original reports must identify this source's actual exported model, without identity/status repair.
    #[wasm_bindgen(js_name = readReport)]
    pub fn read_report(&self, json: &str) -> Result<DiscreteLoadingReport, JsValue> {
        Ok(DiscreteLoadingReport { value: self.value.read_report(json).map_err(bridge_error)? })
    }
}

/// 原 application 独立报告，保留原 core 文本和前置终态，不修补来源身份。 / Independent original application report retaining core text and pre-solve terminals, without patching source identity.
#[wasm_bindgen]
pub struct DiscreteLoadingApplicationReport { value: BppLoadingApplicationReport }

/// 原 Benders 证据及源方案，分解合并报告保持无证明。 / Original Benders evidence and source plan, retaining proof-free merged decomposition reports.
#[wasm_bindgen]
pub struct DiscreteLoadingBendersReport { value: BppLoadingBendersReport }

#[wasm_bindgen]
impl DiscreteLoadingBendersReport {
    /// 重建完整源并复验，不独立重放切割或认证外部来源。 / Rebuild and revalidate the complete source, without independently replaying cuts or authenticating external provenance.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, source: &str) -> Result<DiscreteLoadingBendersReport, JsValue> {
        Ok(Self { value: decode_bpp_loading_benders(json, &parse(source, MAX_BYTES * 2 + 1024)?).map_err(bridge_error)? })
    }

    /// 完整无损原文。 / Complete lossless original text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.value.json.clone() }

    /// 原 application 封装文本。 / Original application envelope text.
    #[wasm_bindgen(js_name = applicationReportJson)]
    pub fn application_report_json(&self) -> String { self.value.envelope.application_report_json.clone() }

    /// 收尾前的加速 application 原文；未收尾时等于最终 application。 / Original pre-finishing accelerator application, equal to the final application when finishing is absent.
    #[wasm_bindgen(js_name = accelerationApplicationReportJson)]
    pub fn acceleration_application_report_json(&self) -> String {
        self.value.envelope.certification.as_ref().map(|evidence| evidence.previous_application_report_json.clone()).unwrap_or_else(|| self.application_report_json())
    }

    /// 实际完整源收尾原报告，不由加速报告推断。 / Actual original full-source finishing report, never inferred from accelerator reports.
    #[wasm_bindgen(js_name = certificationReportJson)]
    pub fn certification_report_json(&self) -> Option<String> { self.value.envelope.certification.as_ref().and_then(|evidence| evidence.report_json.clone()) }

    /// 保留的可恢复收尾错误。 / Retained recoverable finishing error.
    #[wasm_bindgen(js_name = certificationErrorJson)]
    pub fn certification_error_json(&self) -> Result<Option<String>, JsValue> {
        self.value.envelope.certification.as_ref().and_then(|evidence| evidence.error.as_ref()).map(serde_json::to_string).transpose().map_err(error)
    }

    /// 是否存在收尾阶段记录；不表示调用过后端。 / Whether a finishing-stage record exists, not whether a backend was invoked.
    #[wasm_bindgen(js_name = sourceFinishingPresent)]
    pub fn source_finishing_present(&self) -> bool { self.value.envelope.certification.is_some() }

    /// 实际后端返回报告或可恢复错误才表示已调用。 / Only an actual backend report or recoverable error indicates invocation.
    #[wasm_bindgen(js_name = sourceFinishingInvoked)]
    pub fn source_finishing_invoked(&self) -> bool {
        self.value.envelope.certification.as_ref().is_some_and(|evidence| evidence.report_json.is_some() || evidence.error.is_some())
    }

    /// 原 core 源报告文本，不是 master 报告。 / Original core source-report text, not the master report.
    #[wasm_bindgen(js_name = solverReportJson)]
    pub fn solver_report_json(&self) -> String { self.value.application.envelope.report_json.clone() }

    /// 原固定追索报告文本，缺省表示无实际分解。 / Original fixed-recourse report text, absent without an actual decomposition.
    #[wasm_bindgen(js_name = decompositionReportJson)]
    pub fn decomposition_report_json(&self) -> Option<String> { self.value.envelope.decomposition_report_json.clone() }

    /// 实际普通回退原报告，未被选择时仍保留。 / Actual ordinary fallback report, retained even when not selected.
    #[wasm_bindgen(js_name = fallbackReportJson)]
    pub fn fallback_report_json(&self) -> Option<String> { self.value.envelope.fallback_report_json.clone() }

    /// 原源报告摘要。 / Original source-report summary.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.application.summary).map_err(error) }

    /// 实际分解 stop、界和割数，不提升为源证明。 / Actual decomposition stop, bound and cut counts, never promoted to source proof.
    #[wasm_bindgen(js_name = decompositionSummaryJson)]
    pub fn decomposition_summary_json(&self) -> Result<Option<String>, JsValue> {
        self.value.decomposition.as_ref().map(|report| serde_json::to_string(&report.summary)).transpose().map_err(error)
    }

    /// 原可恢复分解／回退错误。 / Original recoverable decomposition/fallback error.
    #[wasm_bindgen(js_name = decompositionErrorJson)]
    pub fn decomposition_error_json(&self) -> Result<Option<String>, JsValue> {
        self.value.envelope.decomposition_error.as_ref().map(serde_json::to_string).transpose().map_err(error)
    }

    /// 实际回退是否启动。 / Whether fallback actually started.
    #[wasm_bindgen(js_name = fallbackUsed)]
    pub fn fallback_used(&self) -> bool { self.value.envelope.fallback_used }

    /// canonical 源身份。 / Canonical source identity.
    #[wasm_bindgen(js_name = sourceFingerprint)]
    pub fn source_fingerprint(&self) -> String { self.value.envelope.source_fingerprint.clone() }

    /// 每次返回独立完整源方案，无 incumbent 时为空。 / Return an independent complete-source plan each time, absent without an incumbent.
    pub fn solution(&self) -> Option<DiscreteLoadingResult> { self.value.application.solution.clone().map(|value| DiscreteLoadingResult { value }) }
}

#[wasm_bindgen]
impl DiscreteLoadingApplicationReport {
    /// 通过完整 canonical 源及实际注册模型复验。 / Revalidate against the complete canonical source and actual registered model.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, source: &str) -> Result<DiscreteLoadingApplicationReport, JsValue> {
        Ok(Self { value: decode_bpp_loading_application(json, &parse(source, MAX_BYTES * 2 + 1024)?).map_err(bridge_error)? })
    }

    /// application 无损原文。 / Lossless original application text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.value.json.clone() }

    /// core 无损原文，不能用 JS number 解析后重编码。 / Lossless core text, never to be parsed through JS number and re-encoded.
    #[wasm_bindgen(js_name = solverReportJson)]
    pub fn solver_report_json(&self) -> String { self.value.envelope.report_json.clone() }

    /// 原 core 摘要。 / Original core summary.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 源问题身份，含业务成本。 / Source-problem identity including business costs.
    #[wasm_bindgen(js_name = sourceFingerprint)]
    pub fn source_fingerprint(&self) -> String { self.value.envelope.source_fingerprint.clone() }

    /// 原 application 是否保留了实际完整注册。 / Whether the original application retained its actual complete registration.
    #[wasm_bindgen(js_name = registrationPresent)]
    pub fn registration_present(&self) -> bool { self.value.envelope.registration_present }

    /// 每次返回独立副本，无 incumbent 时不虚构空方案。 / Return an independent copy each time, without fabricating an empty plan when no incumbent exists.
    pub fn solution(&self) -> Option<DiscreteLoadingResult> { self.value.solution.clone().map(|value| DiscreteLoadingResult { value }) }
}

/// 与 context 及报告独立释放的已验证方案。 / Validated plan disposed independently of its context and report.
#[wasm_bindgen]
pub struct DiscreteLoadingResult { value: BppLoadingResult }

#[wasm_bindgen]
impl DiscreteLoadingResult {
    /// 件数保持十进制文本，重量保留显式单位。 / Counts remain decimal text and weights retain explicit units.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

/// 原始无损报告与独立方案，不将来源一致性当作外部求解器认证。 / Original lossless report and independent plan, without treating source consistency as external solver authentication.
#[wasm_bindgen]
pub struct DiscreteLoadingReport { value: BppLoadingReport }

#[wasm_bindgen]
impl DiscreteLoadingReport {
    /// 通过独立源重建原模型并复验报告。 / Rebuild the original model from an independent source and validate the report.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, source: &str) -> Result<DiscreteLoadingReport, JsValue> {
        Self::read(json, &BppLoadingContext::from_artifact(&parse(source, MAX_BYTES * 2 + 1024)?).map_err(bridge_error)?)
    }

    fn read(json: &str, source: &BppLoadingContext) -> Result<DiscreteLoadingReport, JsValue> {
        Ok(Self { value: source.read_report(json).map_err(bridge_error)? })
    }

    /// 原文保留精确 u64 计数器，不经 JS JSON 数值解析。 / Preserve original exact u64 counters without JS JSON numeric parsing.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.value.json.clone() }

    /// 原 core 报告摘要。 / Original core report summary.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 已复验源身份。 / Revalidated source identity.
    #[wasm_bindgen(js_name = sourceFingerprint)]
    pub fn source_fingerprint(&self) -> String { self.value.source_fingerprint.clone() }

    /// 仅实际 incumbent 产生独立方案，终态无解返回 None。 / Only an actual incumbent yields an independent plan; terminals without one return None.
    pub fn solution(&self) -> Option<DiscreteLoadingResult> { self.value.solution.clone().map(|value| DiscreteLoadingResult { value }) }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests;
