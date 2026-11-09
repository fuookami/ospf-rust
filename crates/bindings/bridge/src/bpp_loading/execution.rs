//! 普通 application 的源绑定报告；前置终态不伪造实际求解模型身份。 / Source-bound ordinary application reports, without fabricating actual solved-model identities for pre-solve terminals.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::solver::{SolveReport, TerminationReason};
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_bpp::application::DiscreteLoadingSolveResult;
use crate::{BridgeError, ModelArtifact, NativeBackend, ReportSummary, Result, encode_model, invalid, summarize_report};
use super::{BppLoadingContext, BppLoadingResult, BppLoadingSolution, MAX_BYTES, TOLERANCE};

/// HTTP／N-API 共用的普通 MILP 或显式连续资源 Benders 配置。 / Ordinary-MILP or explicit continuous-resource Benders configuration shared by HTTP/N-API.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingOptions {
    /// 必须已编译且运行时可用的原生后端。 / Native backend that must be compiled and runtime-available.
    pub backend: NativeBackend,
    /// 显式连续资源 Benders；省略沿用原普通 MILP。 / Explicit continuous-resource Benders; omission retains ordinary MILP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub benders: Option<super::BppLoadingBendersOptions>,
    /// 显式原分区列生成，与 Benders 互斥。 / Explicit original partition column generation, exclusive with Benders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub partition: Option<super::BppLoadingPartitionExecutionOptions>,
}

/// 原 application 报告协议；求解器报告保持原始文本。 / Original application report contract, retaining solver reports as lossless text.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingApplicationEnvelope {
    /// 独立报告 schema。 / Independent report schema.
    pub schema_version: String,
    /// canonical 源身份，含可选业务成本。 / Canonical source identity including optional business costs.
    pub source_fingerprint: String,
    /// 原 application 是否实际保留了完整注册；前置取消／时限时为 false。 / Whether the original application retained a complete registration; false for pre-solve cancellation/time limits.
    pub registration_present: bool,
    /// 无损原始 core 报告。 / Lossless original core report.
    pub report_json: String,
    /// 原最小化目标，只有完整 incumbent 时存在。 / Original minimizing objective, present only with a full incumbent.
    pub objective: Option<f64>,
}

/// 已复验的 application 原文、core 摘要与独立领域结果。 / Revalidated original application text, core summary and independent domain result.
pub struct BppLoadingApplicationReport {
    /// 无损 application 原文。 / Lossless original application text.
    pub json: String,
    /// 无损 core 原文及前置终态标记。 / Lossless core text and pre-solve-terminal marker.
    pub envelope: BppLoadingApplicationEnvelope,
    /// 原 core 状态和最小化目标摘要。 / Original core status and minimizing-objective summary.
    pub summary: ReportSummary,
    /// 仅来自复验过的完整 incumbent。 / Derived only from a revalidated complete incumbent.
    pub solution: Option<BppLoadingResult>,
}

/// 复验版本、源身份、完整原模型和业务方案，不认证外部报告来源。 / Revalidate schema, source identity, complete original model and business plan, without authenticating external report provenance.
pub fn decode_bpp_loading_application(json: &str, source: &ModelArtifact) -> Result<BppLoadingApplicationReport> {
    if json.len() > MAX_BYTES * 2 + 1024 { return Err(BridgeError::new("PayloadTooLarge", "BPP loading application report exceeds its text budget")); }
    let envelope: BppLoadingApplicationEnvelope = serde_json::from_str(json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP loading application schema")); }
    let context = BppLoadingContext::from_artifact(source)?;
    if envelope.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP loading application source mismatch")); }
    if envelope.report_json.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP loading core report exceeds 8 MiB")); }
    let (summary, solution) = if envelope.registration_present {
        let model = crate::decode_model(&context.application_model()?)?;
        let report = crate::linear_report::decode_linear_report(&model, &envelope.report_json, false)?;
        let solution = report.solution.as_ref().map(|solution| context.extract_solution(&solution.values)).transpose()?;
        (summarize_report(&envelope.report_json)?, solution)
    } else {
        let report: SolveReport<f64, u64> = serde_json::from_str(&envelope.report_json).map_err(invalid)?;
        report.validate().map_err(invalid)?;
        if report.solution.is_some() || report.proof.is_some() || report.model_mapping.is_some() || report.fingerprints.model.is_some()
            || report.problem_status != ospf_rust_core::solver::ProblemStatus::Unknown
            || !matches!(report.termination_reason, TerminationReason::Cancelled | TerminationReason::TimeLimit) {
            return Err(invalid("unregistered BPP loading report must remain an unbound proof-free pre-solve cancellation/time limit"));
        }
        (summarize_report(&envelope.report_json)?, None)
    };
    if solution.is_some() != envelope.objective.is_some() { return Err(invalid("BPP loading objective requires a validated incumbent")); }
    if let Some(objective) = envelope.objective {
        if !objective.is_finite() || summary.objective_value.is_none_or(|actual| (actual - objective).abs() > TOLERANCE * actual.abs().max(objective.abs()).max(1.0)) {
            return Err(invalid("BPP loading application objective differs from the original model report"));
        }
    }
    Ok(BppLoadingApplicationReport { json: json.into(), envelope, summary, solution })
}

/// 编码真实原 application 结果，并用同一可移植门禁复验；不补写模型身份。 / Encode the actual original application result and revalidate with the same portable gate, without patching model identity.
pub fn encode_bpp_loading_application(result: &DiscreteLoadingSolveResult<Unit>, source: &ModelArtifact) -> Result<String> {
    let context = BppLoadingContext::from_artifact(source)?;
    if let Some(registration) = &result.registration {
        let compiled = registration.model().linear_model_snapshot().map_err(invalid)?.compile_flt64().map_err(invalid)?;
        let actual = encode_model(&compiled.model)?;
        let expected = context.application_model()?;
        if actual.fingerprint != expected.fingerprint || actual.model_json != expected.model_json || compiled.objective_constant != 0.0 {
            return Err(BridgeError::new("FingerprintMismatch", "BPP loading application registration differs from its complete source"));
        }
    }
    let envelope = BppLoadingApplicationEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        registration_present: result.registration.is_some(), report_json: serde_json::to_string(&result.report).map_err(invalid)?, objective: result.objective };
    let json = serde_json::to_string(&envelope).map_err(invalid)?;
    let decoded = decode_bpp_loading_application(&json, source)?;
    if decoded.solution.as_ref().map(|solution| &solution.solution) != result.solution.as_ref().map(BppLoadingSolution::from_domain).as_ref() {
        return Err(invalid("BPP loading application solution differs from its actual original report"));
    }
    Ok(json)
}

#[cfg(test)]
mod tests;
