//! 宿主列生成配置及无损受限主问题报告。 / Host column-generation configuration and lossless restricted-master reports.

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::{MetaModel, ObjectiveCategory};
use ospf_rust_core::solver::{CURRENT_SOLVE_REPORT_SCHEMA_VERSION, SolveReport};
use ospf_rust_framework::solver::{ColumnDecision, ProjectedColumnGenerationResult};
use crate::{BridgeError, ModelArtifact, NativeBackend, Result, decode_meta_model, invalid};

/// 源交换列索引定义的局部配置块。 / Local configuration block defined by source exchange column indices.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnGenerationBlock {
    /// 独立块身份。 / Distinct block identity.
    pub id: String,
    /// 非空源变量索引，不能跨块重复。 / Nonempty source-variable indices, never repeated across blocks.
    pub variable_indices: Vec<u32>,
}

/// 内置同源列投影策略，不接受调用方提供的裸对偶或 TS 回调。 / Built-in source-projection strategy, accepting neither caller-supplied bare duals nor TS callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnGenerationOptions {
    /// 显式原生后端。 / Explicit native backend.
    pub backend: NativeBackend,
    /// 最多 256 个局部块；未映射变量保留为全局变量。 / Up to 256 local blocks; unmapped variables remain global.
    pub blocks: Vec<ColumnGenerationBlock>,
    /// 包括 Phase I 的 RMP 轮数，默认 128，最大 10000。 / RMP rounds including Phase I; default 128, maximum 10000.
    #[ts(optional)]
    pub iteration_limit: Option<u32>,
    /// 活动列上限，默认和最大均为 10000。 / Active-column limit; default and maximum are 10000.
    #[ts(optional)]
    pub column_limit: Option<u32>,
    /// 有限且位于 (0, 0.5)，默认 1e-7。 / Finite and in (0, 0.5), default 1e-7.
    #[ts(optional)]
    pub tolerance: Option<f64>,
}

impl ColumnGenerationOptions {
    /// 在解码/创建原生模型前限制协议工作量。 / Bound protocol work before decoding/constructing native models.
    pub fn validate(&self) -> Result<()> {
        if self.blocks.is_empty() || self.blocks.len() > 256
            || self.iteration_limit.is_some_and(|value| value == 0 || value > 10_000)
            || self.column_limit.is_some_and(|value| value == 0 || value > 10_000)
            || self.tolerance.is_some_and(|value| !value.is_finite() || value <= 0.0 || value >= 0.5) {
            return Err(invalid("invalid column-generation blocks, budget or tolerance"));
        }
        let mut names = BTreeSet::new();
        let mut indices = BTreeSet::new();
        for block in &self.blocks {
            if block.id.trim().is_empty() || block.id.len() > 256 || !names.insert(&block.id) || block.variable_indices.is_empty() {
                return Err(invalid("invalid or duplicate column-generation block"));
            }
            for index in &block.variable_indices {
                if !indices.insert(*index) { return Err(invalid("duplicate source variable in column-generation blocks")); }
                if indices.len() > 100_000 { return Err(BridgeError::new("PayloadTooLarge", "too many mapped column-generation variables")); }
            }
        }
        Ok(())
    }

    /// 在 worker 所有的 MetaModel 内映射真实身份，不传进程内 ID。 / Map real identities inside the worker-owned MetaModel, never transporting process-local IDs.
    pub fn decisions(&self, source: &MetaModel<f64>) -> Result<Vec<ColumnDecision>> {
        self.validate()?;
        self.blocks.iter().flat_map(|block| block.variable_indices.iter().map(move |index| (block, index))).map(|(block, index)| {
            let token = source.tokens().get(*index as usize).ok_or_else(|| invalid("unknown column-generation source variable"))?;
            Ok(ColumnDecision { id: format!("variable-{index}"), block_id: block.id.clone(), variable: token.id() })
        }).collect()
    }
}

/// 列生成执行终态；不表示源模型全局最优性。 / Column-generation execution terminal, not source-model global optimality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ColumnGenerationStop {
    /// 定价及 final 完成。 / Pricing and final completed.
    Completed,
    /// RMP 轮数耗尽。 / RMP round limit.
    IterationLimit,
    /// 列数耗尽。 / Column limit.
    ColumnLimit,
    /// Phase I 停滞。 / Phase I stall.
    PhaseOneStall,
    /// 重复列停滞。 / Duplicate-column pricing stall.
    PricingStall,
    /// RMP 原始终态。 / Original RMP terminal.
    MasterTermination,
    /// 定价原始终态。 / Original pricing terminal.
    PricingTermination,
    /// final 原始终态。 / Original final terminal.
    FinalTermination,
    /// 缺失或无效 LP 证书。 / Missing or invalid LP certificate.
    CertificateInvalid,
    /// 取消或中断。 / Cancelled or interrupted.
    Cancelled,
    /// 求解总预算或后端时限耗尽。 / Total solve budget or backend time limit exhausted.
    TimeLimit,
}

/// 安全数值摘要，不能据此把受限整数最优性提升为源模型最优性。 / Safe numeric summary, never promoting restricted-integer optimality to source optimality.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnGenerationSummary {
    /// 实际停止原因。 / Actual stop reason.
    pub stop: ColumnGenerationStop,
    /// 人工变量已消除。 / Artificials eliminated.
    pub phase_one_complete: bool,
    /// 全部块均完成 Phase II 的无改进定价。 / Every block completed nonimproving Phase II pricing.
    pub pricing_converged: bool,
    /// RMP 轮数。 / RMP rounds.
    pub iterations: u32,
    /// 定价求解次数。 / Pricing solve count.
    pub pricing_calls: u32,
    /// 已提交列数。 / Committed column count.
    pub column_count: u32,
    /// 最后实际定价块，顺序为 Rust 块 ID 字典序。 / Last actual pricing block, in Rust block-ID lexicographic order.
    pub pricing_block: Option<String>,
    /// 源交换列顺序的完整 incumbent，无解时为空。 / Complete incumbent in source exchange order; empty without an incumbent.
    pub values: Vec<f64>,
    /// 源方向的保守 incumbent 目标，不是全局最优界。 / Conservative source-direction incumbent objective, not a global best bound.
    pub objective_value: Option<f64>,
}

/// 三份最后真实子报告独立保持无损文本。 / Independently retain the last three actual child reports as lossless text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnGenerationReport {
    schema_version: String,
    fingerprint: String,
    tolerance: f64,
    /// Rust 生成的安全摘要。 / Rust-generated safe summary.
    pub summary: ColumnGenerationSummary,
    /// 最后 RMP 报告，不是源模型报告。 / Last RMP report, not a source-model report.
    pub master_report_json: Option<String>,
    /// 最后局部定价报告。 / Last local pricing report.
    pub pricing_report_json: Option<String>,
    /// 最后受限整数主问题报告。 / Last restricted-integer-master report.
    pub final_report_json: Option<String>,
}

/// 编码实际 Rust 结果，再经共享导入门禁复核。 / Encode actual Rust results and recheck through the shared import gate.
pub fn encode_column_generation_result(
    result: &ProjectedColumnGenerationResult,
    artifact: &ModelArtifact,
    source: &MetaModel<f64>,
    tolerance: f64,
) -> Result<String> {
    use ospf_rust_framework::solver::ProjectedColumnGenerationStop as Stop;
    let stop = match result.stop {
        Stop::Completed => ColumnGenerationStop::Completed, Stop::IterationLimit => ColumnGenerationStop::IterationLimit,
        Stop::ColumnLimit => ColumnGenerationStop::ColumnLimit, Stop::PhaseOneStall => ColumnGenerationStop::PhaseOneStall,
        Stop::PricingStall => ColumnGenerationStop::PricingStall, Stop::MasterTermination => ColumnGenerationStop::MasterTermination,
        Stop::PricingTermination => ColumnGenerationStop::PricingTermination, Stop::FinalTermination => ColumnGenerationStop::FinalTermination,
        Stop::CertificateInvalid => ColumnGenerationStop::CertificateInvalid, Stop::Cancelled => ColumnGenerationStop::Cancelled,
        Stop::TimeLimit => ColumnGenerationStop::TimeLimit,
    };
    let sign = if source.linear_model_snapshot().map_err(invalid)?.objective_category == ObjectiveCategory::Maximum { -1.0 } else { 1.0 };
    let report = ColumnGenerationReport { schema_version: "1.0".into(), fingerprint: artifact.fingerprint.clone(), tolerance,
        summary: ColumnGenerationSummary { stop, phase_one_complete: result.phase_one_complete, pricing_converged: result.pricing_converged,
            iterations: u32::try_from(result.iterations).map_err(invalid)?, pricing_calls: u32::try_from(result.pricing_calls).map_err(invalid)?,
            column_count: u32::try_from(result.column_count).map_err(invalid)?, pricing_block: result.pricing_block.clone(),
            values: if result.objective.is_some() { source.tokens().iter().map(|token| result.values.get(&token.id()).copied().ok_or_else(|| invalid("incomplete source incumbent"))).collect::<Result<_>>()? } else { vec![] },
            objective_value: result.objective.map(|value| sign * value) },
        master_report_json: result.master_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?,
        pricing_report_json: result.pricing_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?,
        final_report_json: result.final_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?,
    };
    let json = serde_json::to_string(&report).map_err(invalid)?;
    decode_column_generation_report(&json, artifact)?;
    Ok(json)
}

/// 验证源赋值、目标、schema、指纹及子报告不变量；不独立重放定价或签发源证明。
/// Validate source assignments/objective, schema, fingerprint and child invariants, without independently replaying pricing or issuing source proofs.
pub fn decode_column_generation_report(json: &str, artifact: &ModelArtifact) -> Result<ColumnGenerationReport> {
    if json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "column-generation report exceeds 8 MiB")); }
    let report: ColumnGenerationReport = serde_json::from_str(json).map_err(invalid)?;
    if report.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported column-generation report schema")); }
    let source = decode_meta_model(artifact)?;
    if report.fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "column-generation report belongs to another source")); }
    let summary = &report.summary;
    if !report.tolerance.is_finite() || report.tolerance <= 0.0 || report.tolerance >= 0.5
        || summary.iterations > 10_000 || summary.column_count > 10_000 || summary.pricing_calls > summary.iterations * 256 || summary.column_count > summary.pricing_calls
        || (summary.iterations > 0) != report.master_report_json.is_some()
        || (summary.pricing_calls > 0) != report.pricing_report_json.is_some()
        || report.pricing_report_json.is_some() != summary.pricing_block.is_some()
        || summary.pricing_block.as_ref().is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        || (summary.phase_one_complete && (summary.iterations == 0 || summary.column_count == 0))
        || (summary.pricing_converged && !summary.phase_one_complete)
        || (report.final_report_json.is_some() && !summary.phase_one_complete)
        || (summary.stop == ColumnGenerationStop::Completed && (!summary.pricing_converged || report.final_report_json.is_none() || summary.objective_value.is_none())) {
        return Err(invalid("inconsistent column-generation report metadata"));
    }
    if let Some(objective) = summary.objective_value {
        if !objective.is_finite() || summary.values.len() != source.tokens().len() || report.final_report_json.is_none() { return Err(invalid("invalid source incumbent shape")); }
        let values = source.tokens().iter().zip(&summary.values).map(|(token, value)| (token.id(), *value)).collect();
        let snapshot = source.linear_model_snapshot().map_err(invalid)?;
        snapshot.validate_solution(&values, &report.tolerance).map_err(invalid)?;
        let sign = if snapshot.objective_category == ObjectiveCategory::Maximum { -1.0 } else { 1.0 };
        if sign * snapshot.evaluate_objective_flt64(&values).map_err(invalid)?.total_value().map_err(invalid)? != objective { return Err(invalid("source incumbent objective mismatch")); }
    } else if !summary.values.is_empty() { return Err(invalid("source values without an objective")); }
    for json in [&report.master_report_json, &report.pricing_report_json, &report.final_report_json].into_iter().flatten() {
        let child: SolveReport<f64, u64> = serde_json::from_str(json).map_err(invalid)?;
        if child.schema_version != CURRENT_SOLVE_REPORT_SCHEMA_VERSION { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported child report schema")); }
        child.validate().map_err(invalid)?;
    }
    if let Some(json) = &report.final_report_json {
        let child: SolveReport<f64, u64> = serde_json::from_str(json).map_err(invalid)?;
        if child.has_incumbent() != summary.objective_value.is_some() || (summary.stop == ColumnGenerationStop::Completed && !child.is_optimal()) {
            return Err(invalid("source incumbent and final report disagree"));
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::solver::{ProblemStatus, SolveProof, SolveSolution, TerminationReason};

    #[test]
    fn configuration_is_bounded_distinct_and_uses_source_columns() {
        for json in [r#"{"backend":"scip","blocks":[]}"#, r#"{"backend":"scip","blocks":[{"id":"a","variableIndices":[0,0]}]}"#,
            r#"{"backend":"scip","blocks":[{"id":"a","variableIndices":[0]}],"callback":"code"}"#] {
            assert!(serde_json::from_str::<ColumnGenerationOptions>(json).and_then(|config| config.validate().map_err(serde::de::Error::custom)).is_err());
        }
        let configuration: ColumnGenerationOptions = serde_json::from_str(r#"{"backend":"scip","blocks":[{"id":"a","variableIndices":[0]}]}"#).unwrap();
        assert!(configuration.decisions(&MetaModel::new("empty")).is_err());
    }

    #[test]
    fn codec_keeps_exact_child_counters_and_rejects_false_source_assignments() {
        let source = MetaModel::<f64>::new("report-source");
        let artifact = crate::encode_model(&source.try_to_linear_triad_model().unwrap()).unwrap();
        let mut solution = SolveSolution::<f64>::vector(vec![]); solution.objective_value = Some(0.0);
        let child = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality()).build().unwrap();
        let child = serde_json::to_string(&child).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
        let mut report = ColumnGenerationReport { schema_version: "1.0".into(), fingerprint: artifact.fingerprint.clone(), tolerance: 1e-7,
            summary: ColumnGenerationSummary { stop: ColumnGenerationStop::Completed, phase_one_complete: true, pricing_converged: true,
                iterations: 2, pricing_calls: 1, column_count: 1, pricing_block: Some("block".into()), values: vec![], objective_value: Some(0.0) },
            master_report_json: Some(child.clone()), pricing_report_json: Some(child.clone()), final_report_json: Some(child.clone()) };
        let decoded = decode_column_generation_report(&serde_json::to_string(&report).unwrap(), &artifact).unwrap();
        assert_eq!(decoded.final_report_json.as_ref(), Some(&child));
        report.summary.objective_value = Some(1.0);
        assert!(decode_column_generation_report(&serde_json::to_string(&report).unwrap(), &artifact).is_err());
        report.summary.objective_value = None;
        assert!(decode_column_generation_report(&serde_json::to_string(&report).unwrap(), &artifact).is_err());
        report.schema_version = "99.0".into();
        assert_eq!(decode_column_generation_report(&serde_json::to_string(&report).unwrap(), &artifact).unwrap_err().code, "UnsupportedProtocolVersion");
    }
}
