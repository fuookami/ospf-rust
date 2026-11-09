//! 原共享 BPP Benders 的源绑定证据，不将子模型或合并报告提升为源最优证明。
//! Source-bound evidence for original shared BPP Benders, never promoting child or merged reports into source optimality proofs.

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::LinearModelSnapshot;
use ospf_rust_core::solver::{ProblemStatus, SolveReport, TerminationReason};
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_bpp::application::DiscreteLoadingBendersResult;
use crate::{BridgeError, FixedRecourseReport, FixedRecourseStop, ModelArtifact, Result, decode_fixed_recourse_report,
    encode_fixed_recourse_snapshot_result, invalid};
use super::{BppLoadingApplicationEnvelope, BppLoadingApplicationReport, BppLoadingContext, MAX_BYTES, TOLERANCE,
    decode_bpp_loading_application, encode_bpp_loading_application};

/// 按资源身份选择真实连续追索；仅接受预注册策略配置。 / Select real continuous recourse by resource identity, accepting only preregistered strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingBendersOptions {
    /// 非空、唯一、已声明的资源身份；不接受裸 token。 / Nonempty unique declared resource identities, never raw tokens.
    pub recourse_resources: Vec<String>,
    /// 最多 10000 轮，省略使用原默认 128。 / At most 10000 rounds, defaulting to the original 128.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub iteration_limit: Option<u32>,
    /// 省略沿用原缺失证书恢复策略；已有无效证书不替换。 / Omission retains original missing-certificate recovery, never replacing invalid existing evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recover_missing_certificates: Option<bool>,
    /// 必须显式允许全新等价工厂普通 MILP 回退，默认不回退。 / Fresh-equivalent-factory ordinary MILP fallback must be explicitly enabled; disabled by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fallback_to_milp: Option<bool>,
    /// 从总预算预留完整源 MILP 收尾时间，省略不收尾。 / Reserve full-source MILP finishing time from the total budget; omitted means disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_milp_finishing: Option<super::BppLoadingSourceFinishingOptions>,
}

impl BppLoadingBendersOptions {
    /// 校验有限工作预算及资源目录；不执行 solver。 / Validate finite work budgets and resource catalogs, without executing a solver.
    pub fn validate(&self, context: &BppLoadingContext) -> Result<()> {
        if self.recourse_resources.is_empty() || self.recourse_resources.len() > 128 || self.iteration_limit.is_some_and(|limit| limit == 0 || limit > 10_000)
            || self.recourse_resources.iter().collect::<BTreeSet<_>>().len() != self.recourse_resources.len() {
            return Err(invalid("invalid BPP loading Benders work budget or duplicate resources"));
        }
        context.benders_partition_options(&self.recourse_resources)?;
        if self.source_milp_finishing.as_ref().is_some_and(|options| options.reserve_time_ms == 0) {
            return Err(invalid("BPP source finishing reserve must be positive"));
        }
        Ok(())
    }
}

/// 保留原 application、分解及回退文本的独立封装。 / Independent envelope retaining original application, decomposition and fallback texts.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingBendersEnvelope {
    /// 独立报告 schema。 / Independent report schema.
    pub schema_version: String,
    /// 完整 canonical 源身份，包括资源。 / Complete canonical source identity, including resources.
    pub source_fingerprint: String,
    /// 显式预注册分解配置。 / Explicit preregistered decomposition configuration.
    pub options: BppLoadingBendersOptions,
    /// 原 application 的无损源报告。 / Lossless source report from the original application.
    pub application_report_json: String,
    /// 原固定连续追索 codec 文本；无实际分解时为空。 / Original fixed-continuous-recourse codec text, absent without an actual decomposition.
    pub decomposition_report_json: Option<String>,
    /// 原可恢复技术／能力错误，不与不可行混淆。 / Original recoverable technical/capability error, never confused with infeasibility.
    pub decomposition_error: Option<BridgeError>,
    /// 实际普通 MILP 回退报告，未被选择时仍保留。 / Actual ordinary MILP fallback report, retained even when not selected.
    pub fallback_report_json: Option<String>,
    /// 原 service 是否实际启动独立回退。 / Whether the original service actually started independent fallback.
    pub fallback_used: bool,
    /// 可选完整源收尾证据，旧报告保持原编码。 / Optional full-source finishing evidence, preserving old report encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub certification: Option<super::BppLoadingCertificationEvidence>,
}

/// 已复验源方案及实际分解证据；读取不独立重放全部切割或认证外部来源。
/// Revalidated source plan and actual decomposition evidence; reading neither independently replays every cut nor authenticates external provenance.
pub struct BppLoadingBendersReport {
    /// 无损原文。 / Lossless original text.
    pub json: String,
    /// 保留所有原子文本的封装。 / Envelope retaining every atomic text.
    pub envelope: BppLoadingBendersEnvelope,
    /// 完整模型和领域复验后的 application。 / Application after complete model and domain revalidation.
    pub application: BppLoadingApplicationReport,
    /// 通过既有固定追索门禁的分解报告。 / Decomposition report passing the existing fixed-recourse gate.
    pub decomposition: Option<FixedRecourseReport>,
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= TOLERANCE * left.abs().max(right.abs()).max(1.0)
}

fn stop_reason(report: &FixedRecourseReport) -> Result<TerminationReason> {
    Ok(match report.summary.stop {
        FixedRecourseStop::Converged => TerminationReason::Completed,
        FixedRecourseStop::Cancelled => TerminationReason::Cancelled,
        FixedRecourseStop::TimeLimit => TerminationReason::TimeLimit,
        FixedRecourseStop::IterationLimit => TerminationReason::IterationLimit,
        FixedRecourseStop::MasterTermination | FixedRecourseStop::SubproblemTermination | FixedRecourseStop::BackendFailure =>
            serde_json::from_value(serde_json::Value::String(report.summary.terminal_termination_reason.clone().ok_or_else(|| invalid("missing Benders terminal"))?)).map_err(invalid)?,
        _ => TerminationReason::Suboptimal,
    })
}

/// 验证完整源方案、保守证明边界与回退选择；不宣称独立重放分解证明。 / Validate complete source plans, conservative proof boundaries and fallback selection, without claiming independent decomposition-proof replay.
pub fn decode_bpp_loading_benders(json: &str, source: &ModelArtifact) -> Result<BppLoadingBendersReport> {
    if json.len() > MAX_BYTES * 16 + 8192 { return Err(BridgeError::new("PayloadTooLarge", "BPP loading finishing envelope exceeds 128 MiB + 8 KiB")); }
    let envelope: BppLoadingBendersEnvelope = serde_json::from_str(json).map_err(invalid)?;
    if envelope.certification.is_none() && json.len() > MAX_BYTES * 8 + 4096 { return Err(BridgeError::new("PayloadTooLarge", "BPP loading Benders envelope exceeds 64 MiB + 4 KiB")); }
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP loading Benders schema")); }
    if envelope.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP loading Benders source mismatch")); }
    let context = BppLoadingContext::from_artifact(source)?; envelope.options.validate(&context)?;
    let selected = decode_bpp_loading_application(&envelope.application_report_json, source)?;
    let previous = envelope.certification.as_ref().map(|evidence| decode_bpp_loading_application(&evidence.previous_application_report_json, source)).transpose()?;
    let application = previous.as_ref().unwrap_or(&selected);
    let core: SolveReport<f64, u64> = serde_json::from_str(&application.envelope.report_json).map_err(invalid)?;
    let artifact = context.application_model()?;
    let decomposition = envelope.decomposition_report_json.as_ref().map(|text| decode_fixed_recourse_report(text, &artifact)).transpose()?;
    if let Some(report) = &decomposition {
        if report.tolerance() != TOLERANCE || report.summary.iterations > envelope.options.iteration_limit.unwrap_or(128) { return Err(invalid("BPP Benders report differs from its actual tolerance or iteration budget")); }
    }
    if envelope.decomposition_error.as_ref().is_some_and(|error| !matches!(error.code.as_str(), "UNSUPPORTED" | "ENVIRONMENT" | "LICENSE" | "BACKEND" | "NUMERICAL")) {
        return Err(invalid("BPP Benders only retains recoverable technical/capability errors"));
    }
    let fallback = envelope.fallback_report_json.as_ref().map(|text| -> Result<SolveReport<f64, u64>> {
        if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP fallback report exceeds 8 MiB")); }
        let report: SolveReport<f64, u64> = serde_json::from_str(text).map_err(invalid)?;
        let bound = report.fingerprints.model.is_some();
        let wrapper = BppLoadingApplicationEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
            registration_present: bound, report_json: text.clone(), objective: report.solution.as_ref().and_then(|solution| solution.objective_value.or(solution.objective)) };
        decode_bpp_loading_application(&serde_json::to_string(&wrapper).map_err(invalid)?, source)?;
        if bound != envelope.fallback_used { return Err(invalid("BPP fallback registration and actual-use marker disagree")); }
        Ok(report)
    }).transpose()?;
    if (envelope.fallback_used || fallback.is_some() || envelope.decomposition_error.is_some()) && envelope.options.fallback_to_milp != Some(true) {
        return Err(invalid("BPP Benders fallback must be explicitly enabled"));
    }
    if (envelope.fallback_used || fallback.is_some()) && decomposition.as_ref().is_some_and(|report| {
        matches!(report.summary.stop, FixedRecourseStop::Converged | FixedRecourseStop::Cancelled | FixedRecourseStop::TimeLimit)
            || report.summary.terminal_termination_reason.as_deref().is_some_and(|reason| matches!(reason, "CANCELLED" | "INTERRUPTED" | "TIME_LIMIT"))
    }) { return Err(invalid("BPP Benders cannot start fallback after convergence, cancellation or time exhaustion")); }
    if decomposition.is_some() && envelope.decomposition_error.is_some() && !envelope.fallback_used {
        return Err(invalid("BPP retained decomposition errors require an actually invoked failing fallback"));
    }
    if (fallback.is_some() || envelope.fallback_used) && decomposition.is_none() && envelope.decomposition_error.is_none() {
        return Err(invalid("BPP fallback without decomposition must retain the original recoverable error"));
    }
    if envelope.decomposition_error.is_some() && fallback.is_none() && !envelope.fallback_used {
        return Err(invalid("BPP recoverable errors require an actual fallback outcome"));
    }
    if envelope.fallback_used && fallback.is_none() && (envelope.decomposition_error.is_none() || decomposition.as_ref().is_none_or(|report| report.summary.objective_value.is_none())) {
        return Err(invalid("BPP fallback without a report requires a retained incumbent and recoverable failure"));
    }
    if let Some(fallback) = &fallback {
        if let Some(previous) = decomposition.as_ref().and_then(|report| report.summary.objective_value) {
            if fallback.problem_status == ProblemStatus::Infeasible { return Err(invalid("fallback infeasibility contradicts the decomposition incumbent")); }
            if fallback.is_optimal() && fallback.solution.as_ref().and_then(|solution| solution.objective_value.or(solution.objective)).is_none_or(|objective| objective > previous && !close(objective, previous)) {
                return Err(invalid("fallback optimality contradicts the decomposition incumbent"));
            }
            if !matches!(fallback.problem_status, ProblemStatus::Unbounded | ProblemStatus::InfeasibleOrUnbounded)
                && application.envelope.objective.is_none_or(|objective| objective > previous && !close(objective, previous)) {
                return Err(invalid("BPP fallback discarded a better verified source incumbent"));
            }
        }
    }
    let selected_fallback = envelope.fallback_report_json.as_ref() == Some(&application.envelope.report_json);
    if !selected_fallback {
        if fallback.as_ref().and_then(|report| report.solution.as_ref()).and_then(|solution| solution.objective_value.or(solution.objective))
            .zip(application.envelope.objective).is_some_and(|(candidate, retained)| candidate < retained && !close(candidate, retained)) {
            return Err(invalid("BPP retained decomposition discarded a better fallback incumbent"));
        }
        if let Some(report) = &decomposition {
            let has_incumbent = report.summary.objective_value.is_some();
            if !application.envelope.registration_present || core.proof.is_some() || core.problem_status != if has_incumbent { ProblemStatus::Feasible } else { ProblemStatus::Unknown }
                || core.solution.as_ref().map(|solution| solution.values.as_slice()).unwrap_or_default() != report.summary.values
                || application.envelope.objective.is_some() != has_incumbent
                || application.envelope.objective.zip(report.summary.objective_value).is_some_and(|(left, right)| !close(left, right)) {
                return Err(invalid("BPP merged decomposition report must remain proof-free and match its complete source incumbent"));
            }
            let reason = if let Some(fallback) = &fallback { fallback.termination_reason }
                else if envelope.fallback_used { match envelope.decomposition_error.as_ref().map(|error| error.code.as_str()) {
                    _ if core.termination_reason == TerminationReason::Cancelled => TerminationReason::Cancelled,
                    Some("NUMERICAL") => TerminationReason::NumericalFailure,
                    _ => TerminationReason::BackendFailure,
                } } else { stop_reason(report)? };
            if core.termination_reason != reason { return Err(invalid("BPP merged termination differs from its actual execution stop")); }
        } else if application.envelope.registration_present || envelope.decomposition_error.is_some() || fallback.is_some() || envelope.fallback_used {
            return Err(invalid("BPP Benders requires actual decomposition or fallback evidence beyond pre-solve terminals"));
        }
    }
    if let Some(evidence) = &envelope.certification {
        if envelope.options.source_milp_finishing.is_none() { return Err(invalid("source finishing evidence requires explicit configuration")); }
        super::finishing::validate(evidence, application, &selected, source)?;
    }
    Ok(BppLoadingBendersReport { json: json.into(), envelope, application: selected, decomposition })
}

/// 编码真实原 service 结果，分解值始终按初始快照编码而非回退会话。 / Encode actual original-service results, always mapping decomposition values through the initial snapshot rather than fallback sessions.
pub fn encode_bpp_loading_benders(
    result: &DiscreteLoadingBendersResult<Unit>,
    source: &ModelArtifact,
    initial: Option<&LinearModelSnapshot<f64>>,
    options: &BppLoadingBendersOptions,
) -> Result<String> {
    let context = BppLoadingContext::from_artifact(source)?; options.validate(&context)?;
    let artifact = context.application_model()?;
    let decomposition_report_json = result.decomposition.as_ref().map(|result| -> Result<String> {
        let snapshot = initial.ok_or_else(|| invalid("missing initial BPP Benders snapshot"))?;
        let actual = crate::encode_model(&snapshot.compile_flt64().map_err(invalid)?.model)?;
        if actual.fingerprint != artifact.fingerprint || actual.model_json != artifact.model_json { return Err(BridgeError::new("FingerprintMismatch", "initial BPP Benders snapshot differs from source")); }
        encode_fixed_recourse_snapshot_result(result, snapshot, &artifact, TOLERANCE)
    }).transpose()?;
    let envelope = BppLoadingBendersEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), options: options.clone(),
        application_report_json: encode_bpp_loading_application(&result.result, source)?, decomposition_report_json,
        decomposition_error: result.decomposition_error.as_ref().map(|error| BridgeError::new(error.solver_error_class().code(), error)),
        fallback_report_json: result.fallback_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?, fallback_used: result.fallback_used, certification: None };
    let json = serde_json::to_string(&envelope).map_err(invalid)?; decode_bpp_loading_benders(&json, source)?; Ok(json)
}

#[cfg(test)]
mod tests;
