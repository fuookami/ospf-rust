//! 完整源 MILP 收尾的可移植证据门禁，不把加速证明重标记为源证明。
//! Portable evidence gate for complete-source MILP finishing, never relabeling accelerator proofs as source proofs.

use std::time::Duration;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::solver::{ProblemStatus, ProofStatus, SolveReport, TerminationReason};
use ospf_rust_quantities::unit::Unit;
use ospf_rust_framework_bpp::application::{DiscreteLoadingCertificationResult, retain_source_incumbent_report};
use crate::{BridgeError, ModelArtifact, Result, invalid};
use super::{BppLoadingApplicationEnvelope, BppLoadingApplicationReport, BppLoadingBendersEnvelope, BppLoadingContext, MAX_BYTES, TOLERANCE,
    decode_bpp_loading_application, decode_bpp_loading_benders, encode_bpp_loading_application};

/// 为完整源收尾显式预留同一总预算；不是额外追加的时限。 / Explicitly reserve the same total budget for full-source finishing, never an additional deadline.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingSourceFinishingOptions {
    /// 正毫秒数，必须严格小于任务总预算；默认任务预算为 60 秒。 / Positive milliseconds, strictly below the total job budget; the default total is 60 seconds.
    pub reserve_time_ms: u32,
}

impl BppLoadingSourceFinishingOptions {
    /// 校验预留量并返回加速阶段预算。 / Validate the reservation and return the accelerator-stage budget.
    pub fn accelerator_budget(&self, total: Duration) -> Result<Duration> {
        let reserve = Duration::from_millis(u64::from(self.reserve_time_ms));
        if reserve.is_zero() || reserve >= total { return Err(invalid("BPP source finishing reserve must be positive and strictly below the total budget")); }
        Ok(total - reserve)
    }
}

/// 原加速 application 与实际完整源收尾证据；原始文本独立保留。 / Original accelerator application and actual full-source finishing evidence, retaining independent raw texts.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BppLoadingCertificationEvidence {
    /// 收尾之前的完整源 application 文本，不改写分解／回退结果。 / Complete-source application text before finishing, without rewriting decomposition/fallback results.
    pub previous_application_report_json: String,
    /// 实际完整源求解原报告；没有调用后端时为空。 / Actual original full-source solve report, absent when no backend is invoked.
    pub report_json: Option<String>,
    /// 实际收尾的可恢复错误，不混为数学不可行。 / Actual recoverable finishing error, never conflated with mathematical infeasibility.
    pub error: Option<BridgeError>,
}

fn certified(report: &SolveReport<f64, u64>) -> bool {
    report.is_optimal() || (matches!(report.problem_status, ProblemStatus::Infeasible | ProblemStatus::Unbounded | ProblemStatus::InfeasibleOrUnbounded)
        && report.proof.as_ref().is_some_and(|proof| proof.status == ProofStatus::Verified))
}

fn close(left: f64, right: f64) -> bool { (left - right).abs() <= TOLERANCE * left.abs().max(right.abs()).max(1.0) }

pub(super) fn validate(
    evidence: &BppLoadingCertificationEvidence,
    previous: &BppLoadingApplicationReport,
    selected: &BppLoadingApplicationReport,
    source: &ModelArtifact,
) -> Result<()> {
    let old: SolveReport<f64, u64> = serde_json::from_str(&previous.envelope.report_json).map_err(invalid)?;
    let final_report: SolveReport<f64, u64> = serde_json::from_str(&selected.envelope.report_json).map_err(invalid)?;
    if certified(&old) {
        if evidence.report_json.is_some() || evidence.error.is_some() || selected.envelope.report_json != previous.envelope.report_json
            || selected.envelope.registration_present != previous.envelope.registration_present || selected.envelope.objective != previous.envelope.objective {
            return Err(invalid("an existing complete-source terminal proof must remain unchanged without another solve"));
        }
        return Ok(());
    }
    if evidence.report_json.is_some() && evidence.error.is_some() { return Err(invalid("source finishing cannot have both an actual report and a propagated backend error")); }
    if evidence.error.as_ref().is_some_and(|error| !matches!(error.code.as_str(), "UNSUPPORTED" | "ENVIRONMENT" | "LICENSE" | "BACKEND" | "NUMERICAL")) {
        return Err(invalid("source finishing only retains recoverable errors"));
    }
    if matches!(old.termination_reason, TerminationReason::Cancelled | TerminationReason::Interrupted) && (evidence.report_json.is_some() || evidence.error.is_some()) {
        return Err(invalid("source finishing cannot invoke a backend after prior cancellation/interruption"));
    }
    if let Some(text) = &evidence.report_json {
        if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "source finishing report exceeds 8 MiB")); }
        let actual: SolveReport<f64, u64> = serde_json::from_str(text).map_err(invalid)?;
        let context = BppLoadingContext::from_artifact(source)?;
        let model = crate::decode_model(&context.application_model()?)?;
        let objective = actual.solution.as_ref().map(|solution| {
            ospf_rust_core::model::LinearObjectiveEvaluationFlt64::new(model.c.iter().copied().zip(solution.values.iter().copied()), 0.0)?.total_value()
        }).transpose().map_err(invalid)?;
        let wrapper = BppLoadingApplicationEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(), registration_present: true,
            report_json: text.clone(), objective };
        let candidate = decode_bpp_loading_application(&serde_json::to_string(&wrapper).map_err(invalid)?, source)?;
        if previous.envelope.objective.is_some() && actual.problem_status == ProblemStatus::Infeasible {
            return Err(invalid("source finishing infeasibility contradicts a validated source incumbent"));
        }
        if actual.is_optimal() && previous.envelope.objective.zip(candidate.envelope.objective).is_some_and(|(old, new)| old < new && !close(old, new)) {
            return Err(invalid("source finishing optimality contradicts a better validated source incumbent"));
        }
        let retain_previous = !actual.is_optimal() && !matches!(actual.problem_status, ProblemStatus::Unbounded | ProblemStatus::InfeasibleOrUnbounded)
            && previous.envelope.objective.is_some_and(|old| candidate.envelope.objective.is_none_or(|new| old <= new));
        if retain_previous {
            let mut expected = actual;
            retain_source_incumbent_report(&model, &mut expected, old.solution.as_ref().ok_or_else(|| invalid("missing previous source incumbent"))?, TOLERANCE).map_err(invalid)?;
            if expected != final_report || !selected.envelope.registration_present || selected.envelope.objective != previous.envelope.objective {
                return Err(invalid("source finishing retained plan must match the original service's audit/gap/proof transformation"));
            }
        } else if selected.envelope.report_json != *text || !selected.envelope.registration_present
            || selected.envelope.objective.is_some() != candidate.envelope.objective.is_some()
            || selected.envelope.objective.zip(candidate.envelope.objective).is_some_and(|(left, right)| !close(left, right)) {
            return Err(invalid("selected full-source finishing report differs from the actual original solve"));
        }
    } else {
        let mut expected = old;
        let reason = final_report.termination_reason;
        let allowed = if let Some(error) = &evidence.error {
            reason == TerminationReason::Cancelled || reason == if error.code == "NUMERICAL" { TerminationReason::NumericalFailure } else { TerminationReason::BackendFailure }
        } else {
            reason == expected.termination_reason || reason == TerminationReason::Cancelled
                || (reason == TerminationReason::TimeLimit && !matches!(expected.termination_reason, TerminationReason::Cancelled | TerminationReason::Interrupted))
        };
        if !allowed || (evidence.error.is_some() && !previous.envelope.registration_present) { return Err(invalid("source finishing pre-solve/error terminal is inconsistent")); }
        expected.termination_reason = reason;
        if expected != final_report || selected.envelope.registration_present != previous.envelope.registration_present || selected.envelope.objective != previous.envelope.objective {
            return Err(invalid("source finishing without an actual report cannot change source identity, incumbent, proof or diagnostics"));
        }
    }
    Ok(())
}

/// 将原 certification 结果附加到真实加速报告，保留旧原子文本及新的完整源原报告。
/// Attach an original certification result to an actual accelerator report, retaining previous atomic texts and the new full-source original report.
pub fn encode_bpp_loading_certification(
    accelerator_json: &str,
    result: &DiscreteLoadingCertificationResult<Unit>,
    source: &ModelArtifact,
) -> Result<String> {
    let acceleration = decode_bpp_loading_benders(accelerator_json, source)?;
    let mut envelope: BppLoadingBendersEnvelope = acceleration.envelope;
    if envelope.certification.is_some() || envelope.options.source_milp_finishing.is_none() { return Err(invalid("source finishing requires one explicitly configured accelerator stage")); }
    envelope.certification = Some(BppLoadingCertificationEvidence { previous_application_report_json: envelope.application_report_json,
        report_json: result.certification_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?,
        error: result.certification_error.as_ref().map(|error| BridgeError::new(error.solver_error_class().code(), error)) });
    envelope.application_report_json = encode_bpp_loading_application(&result.result, source)?;
    let json = serde_json::to_string(&envelope).map_err(invalid)?;
    decode_bpp_loading_benders(&json, source)?; Ok(json)
}

#[cfg(test)]
mod tests;
