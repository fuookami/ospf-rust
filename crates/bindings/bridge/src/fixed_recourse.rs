//! 固定连续追索的类型化配置与无损结果 / Typed fixed-continuous-recourse configuration and lossless results.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::{LinearModelSnapshot, MetaModel, ObjectiveCategory};
use ospf_rust_core::solver::{CURRENT_SOLVE_REPORT_SCHEMA_VERSION, ProblemStatus, SolveReport, TerminationReason};
use ospf_rust_framework::solver::{FixedRecourseResult, LinearBendersPartitionOptions, LinearBendersStage};
use crate::{BridgeError, ModelArtifact, NativeBackend, Result, decode_meta_model, invalid};

/// 源变量的原生阶段 / Native stage of a source variable.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FixedRecourseStage {
    /// 留在主问题 / Retain in the master.
    Master,
    /// 固定矩阵连续追索 / Fixed-matrix continuous recourse.
    ContinuousRecourse,
}

/// 按源交换模型的列索引指定阶段，不传进程内 VariableId / Stage by source exchange column index, never a process-local VariableId.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StagedVariable {
    /// 源变量列索引 / Source-variable column index.
    pub variable_index: u32,
    /// 阶段 / Stage.
    pub stage: FixedRecourseStage,
}

/// 原生固定追索预置策略；不接受自制切割、裸对偶或回调 / Native fixed-recourse strategy, accepting neither caller-built cuts, bare duals nor callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedRecourseOptions {
    /// 显式原生后端 / Explicit native backend.
    pub backend: NativeBackend,
    /// 省略沿用 Rust 默认划分：整数及非零目标变量留在 master / Omission keeps integers and nonzero-objective variables in master, following Rust defaults.
    #[ts(optional)]
    pub stages: Option<Vec<StagedVariable>>,
    /// 最大 10000 轮；省略使用 Rust 默认 128 / Maximum 10000 rounds; omission uses the Rust default of 128.
    #[ts(optional)]
    pub iteration_limit: Option<u32>,
    /// 有限且位于 (0, 0.5)；省略使用 Rust 默认容差 / Finite and in (0, 0.5); omission uses Rust's default tolerance.
    #[ts(optional)]
    pub tolerance: Option<f64>,
    /// 缺证书时使用剩余总预算恢复，不替换无效的已有证据 / Recover missing certificates within the remaining total budget, without replacing invalid existing evidence.
    #[ts(optional)]
    pub recover_missing_certificates: Option<bool>,
}

impl FixedRecourseOptions {
    /// 校验工作量和独立阶段声明 / Validate work bounds and distinct stage declarations.
    pub fn validate(&self) -> Result<()> {
        if self.iteration_limit.is_some_and(|limit| limit == 0 || limit > 10_000)
            || self.tolerance.is_some_and(|value| !value.is_finite() || value <= 0.0 || value >= 0.5) {
            return Err(invalid("invalid fixed-recourse iteration limit or tolerance"));
        }
        if let Some(stages) = &self.stages {
            if stages.len() > 100_000 { return Err(BridgeError::new("PayloadTooLarge", "too many staged variables")); }
            if stages.iter().map(|stage| stage.variable_index).collect::<BTreeSet<_>>().len() != stages.len() {
                return Err(invalid("duplicate fixed-recourse variable stage"));
            }
        }
        Ok(())
    }

    /// 仅在源模型验证后把索引映射为本实例的真实变量身份 / Map indices to this instance's real variable identities only after source-model validation.
    pub fn partition_options(&self, source: &MetaModel<f64>) -> Result<LinearBendersPartitionOptions> {
        self.validate()?;
        let mut stages = BTreeMap::new();
        for entry in self.stages.iter().flatten() {
            let variable = source.tokens().get(entry.variable_index as usize).ok_or_else(|| invalid("unknown staged source-variable index"))?.id();
            stages.insert(variable, match entry.stage {
                FixedRecourseStage::Master => LinearBendersStage::Master,
                FixedRecourseStage::ContinuousRecourse => LinearBendersStage::ContinuousRecourse,
            });
        }
        Ok(LinearBendersPartitionOptions { stages })
    }
}

/// 原生算法停止原因；数学子问题终态保留在原始报告，不伪造成源模型证明 / Native stop reason; mathematical child terminals remain in original reports, not fabricated source proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FixedRecourseStop {
    /// 原域上下界在容差内收敛 / Source-domain bounds converged within tolerance.
    Converged,
    /// 主问题实际终态 / Actual master terminal.
    MasterTermination,
    /// 追索或证书恢复实际终态 / Actual recourse or certificate-recovery terminal.
    SubproblemTermination,
    /// 缺少证书 / Missing certificate.
    CertificateMissing,
    /// 证书复核失败 / Invalid certificate.
    CertificateInvalid,
    /// 当前数值边界不支持 / Unsupported at the current numeric boundary.
    Unsupported,
    /// 无新分离割 / No separating new cut.
    CutStall,
    /// 迭代耗尽 / Iteration limit reached.
    IterationLimit,
    /// 取消 / Cancelled.
    Cancelled,
    /// 总预算耗尽 / Total time budget exhausted.
    TimeLimit,
    /// 保留 incumbent 后发生后端失败 / Backend failure after retaining an incumbent.
    BackendFailure,
}

/// SDK 可安全读取的结果投影，不是独立最优性证明 / SDK-safe result projection, not an independent optimality proof.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedRecourseSummary {
    /// 实际停止原因 / Actual stop reason.
    pub stop: FixedRecourseStop,
    /// 原生终态的 core 问题状态；证书恢复的终态也不丢失 / Core problem status of the native terminal, including certificate recovery terminals.
    pub terminal_problem_status: Option<String>,
    /// 原生终态的 core 终止原因 / Core termination reason of the native terminal.
    pub terminal_termination_reason: Option<String>,
    /// 按源交换列顺序的完整 incumbent，无解时为空 / Complete incumbent in source exchange column order, empty without an incumbent.
    pub values: Vec<f64>,
    /// 源目标方向的 incumbent 界 / Incumbent bound in the source objective direction.
    pub objective_value: Option<f64>,
    /// 源目标方向的最优界：最小化为下界，最大化为上界 / Best bound: lower for minimization, upper for maximization.
    pub best_bound: Option<f64>,
    /// 已执行轮数 / Executed rounds.
    pub iterations: u32,
    /// 已提交可行性割数 / Committed feasibility cuts.
    pub feasibility_cuts: u32,
    /// 已提交最优性割数 / Committed optimality cuts.
    pub optimality_cuts: u32,
    /// incumbent 后保留的原始错误 / Original error retained after an incumbent.
    pub backend_error: Option<BridgeError>,
}

/// 保留原始子报告文本的结果封装 / Result envelope retaining original child report text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedRecourseReport {
    schema_version: String,
    fingerprint: String,
    tolerance: f64,
    /// 原始主问题报告文本 / Original master report text.
    pub master_report_json: Option<String>,
    /// 原始追索报告文本 / Original recourse report text.
    pub subproblem_report_json: Option<String>,
    /// 安全投影 / Safe projection.
    pub summary: FixedRecourseSummary,
}

impl FixedRecourseReport {
    /// 编码时实际使用的验证容差。 / Validation tolerance actually used during encoding.
    pub fn tolerance(&self) -> f64 { self.tolerance }
}

/// 共享 Rust 编码边界；不把 master/sub 报告重新标记为源模型证明 / Shared Rust encoding boundary, never relabeling master/sub reports as source proofs.
pub fn encode_fixed_recourse_result(
    result: &FixedRecourseResult,
    source: &MetaModel<f64>,
    artifact: &ModelArtifact,
    tolerance: f64,
) -> Result<String> {
    encode_fixed_recourse_snapshot_result(result, &source.linear_model_snapshot().map_err(invalid)?, artifact, tolerance)
}

/// 按最初注册快照的真实身份编码，不使用可能属于全新回退会话的 token。 / Encode with original snapshot identities, never tokens from a potentially fresh fallback session.
pub fn encode_fixed_recourse_snapshot_result(
    result: &FixedRecourseResult,
    source: &LinearModelSnapshot<f64>,
    artifact: &ModelArtifact,
    tolerance: f64,
) -> Result<String> {
    use ospf_rust_framework::solver::FixedRecourseStop as Stop;
    let sign = if source.objective_category == ObjectiveCategory::Maximum { -1.0 } else { 1.0 };
    let values = if result.objective.is_some() {
        source.variables.iter().map(|state| result.values.get(&state.variable).copied().ok_or_else(|| invalid("incomplete source incumbent"))).collect::<Result<Vec<_>>>()?
    } else { vec![] };
    let stop = match result.stop {
        Stop::Converged => FixedRecourseStop::Converged,
        Stop::MasterTermination(..) => FixedRecourseStop::MasterTermination,
        Stop::SubproblemTermination(..) => FixedRecourseStop::SubproblemTermination,
        Stop::CertificateMissing => FixedRecourseStop::CertificateMissing,
        Stop::CertificateInvalid => FixedRecourseStop::CertificateInvalid,
        Stop::Unsupported => FixedRecourseStop::Unsupported,
        Stop::CutStall => FixedRecourseStop::CutStall,
        Stop::IterationLimit => FixedRecourseStop::IterationLimit,
        Stop::Cancelled => FixedRecourseStop::Cancelled,
        Stop::TimeLimit => FixedRecourseStop::TimeLimit,
        Stop::BackendFailure(..) => FixedRecourseStop::BackendFailure,
    };
    let terminal_problem_status = match result.stop {
        Stop::MasterTermination(status, _) | Stop::SubproblemTermination(status, _) => Some(enum_text(status)?),
        _ => None,
    };
    let terminal_termination_reason = match result.stop {
        Stop::MasterTermination(_, reason) | Stop::SubproblemTermination(_, reason) | Stop::BackendFailure(reason) => Some(enum_text(reason)?),
        _ => None,
    };
    let report = FixedRecourseReport {
        schema_version: "1.0".into(), fingerprint: artifact.fingerprint.clone(), tolerance,
        master_report_json: result.master_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?,
        subproblem_report_json: result.subproblem_report.as_ref().map(serde_json::to_string).transpose().map_err(invalid)?,
        summary: FixedRecourseSummary { stop, terminal_problem_status, terminal_termination_reason, values, objective_value: result.objective.map(|value| sign * value), best_bound: result.lower_bound.map(|value| sign * value),
            iterations: u32::try_from(result.iterations).map_err(invalid)?, feasibility_cuts: u32::try_from(result.feasibility_cuts).map_err(invalid)?,
            optimality_cuts: u32::try_from(result.optimality_cuts).map_err(invalid)?, backend_error: result.backend_error.as_ref().map(|error| BridgeError::new(error.solver_error_class().code(), error)) },
    };
    let json = serde_json::to_string(&report).map_err(invalid)?;
    decode_fixed_recourse_report(&json, artifact)?;
    Ok(json)
}

fn enum_text(value: impl Serialize) -> Result<String> {
    serde_json::to_value(value).map_err(invalid)?.as_str().map(str::to_owned).ok_or_else(|| invalid("expected a core enum string"))
}

/// 检查原域完整赋值、目标、报告 schema 与指纹，不独立重放切割或证明 bound / Check complete source assignment, objective, report schemas and identity, without independently replaying cuts or proving bounds.
pub fn decode_fixed_recourse_report(json: &str, artifact: &ModelArtifact) -> Result<FixedRecourseReport> {
    if json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "fixed-recourse report exceeds 8 MiB")); }
    let report: FixedRecourseReport = serde_json::from_str(json).map_err(invalid)?;
    if report.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported fixed-recourse report schema")); }
    let source = decode_meta_model(artifact)?;
    if report.fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "fixed-recourse report belongs to a different model")); }
    if !report.tolerance.is_finite() || report.tolerance <= 0.0 || report.tolerance >= 0.5 { return Err(invalid("invalid report tolerance")); }
    let summary = &report.summary;
    if let Some(status) = &summary.terminal_problem_status {
        serde_json::from_value::<ProblemStatus>(serde_json::Value::String(status.clone())).map_err(invalid)?;
    }
    if let Some(reason) = &summary.terminal_termination_reason {
        serde_json::from_value::<TerminationReason>(serde_json::Value::String(reason.clone())).map_err(invalid)?;
    }
    let child_terminal = matches!(summary.stop, FixedRecourseStop::MasterTermination | FixedRecourseStop::SubproblemTermination);
    let backend_terminal = summary.stop == FixedRecourseStop::BackendFailure;
    if summary.terminal_problem_status.is_some() != child_terminal || summary.terminal_termination_reason.is_some() != (child_terminal || backend_terminal)
        || (backend_terminal && summary.backend_error.is_none()) || (summary.backend_error.is_some() && !matches!(summary.stop, FixedRecourseStop::BackendFailure | FixedRecourseStop::Cancelled)) {
        return Err(invalid("inconsistent native terminal metadata"));
    }
    if summary.iterations > 10_000 || u64::from(summary.feasibility_cuts) + u64::from(summary.optimality_cuts) > u64::from(summary.iterations)
        || summary.best_bound.is_some_and(|value| !value.is_finite()) { return Err(invalid("invalid fixed-recourse counters or bound")); }
    if (summary.iterations > 0 && report.master_report_json.is_none()) || ((summary.feasibility_cuts > 0 || summary.optimality_cuts > 0) && report.subproblem_report_json.is_none()) {
        return Err(invalid("missing original fixed-recourse child report"));
    }
    let snapshot = source.linear_model_snapshot().map_err(invalid)?;
    let sign = if snapshot.objective_category == ObjectiveCategory::Maximum { -1.0 } else { 1.0 };
    if let Some(objective) = summary.objective_value {
        if !objective.is_finite() || summary.values.len() != source.tokens().len() { return Err(invalid("invalid complete source incumbent shape")); }
        let values = source.tokens().iter().zip(&summary.values).map(|(token, value)| (token.id(), *value)).collect();
        snapshot.validate_solution(&values, &report.tolerance).map_err(invalid)?;
        if sign * snapshot.evaluate_objective_flt64(&values).map_err(invalid)?.total_value().map_err(invalid)? != objective {
            return Err(invalid("source incumbent objective mismatch"));
        }
        if let Some(bound) = summary.best_bound {
            let gap = sign * (objective - bound);
            let tolerance = report.tolerance * objective.abs().max(bound.abs()).max(1.0);
            if !gap.is_finite() || gap < -tolerance || (summary.stop == FixedRecourseStop::Converged && gap > tolerance) { return Err(invalid("inconsistent source incumbent/bound")); }
        }
    } else if !summary.values.is_empty() { return Err(invalid("source values without an incumbent objective")); }
    if summary.stop == FixedRecourseStop::Converged && (summary.objective_value.is_none() || summary.best_bound.is_none() || summary.iterations == 0) {
        return Err(invalid("convergence needs a complete incumbent and bound"));
    }
    for json in [&report.master_report_json, &report.subproblem_report_json].into_iter().flatten() {
        let child: SolveReport<f64, u64> = serde_json::from_str(json).map_err(invalid)?;
        if child.schema_version != CURRENT_SOLVE_REPORT_SCHEMA_VERSION { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported child report schema")); }
        child.validate().map_err(invalid)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::{ConstraintRelation, ObjectiveCategory};
    use ospf_rust_core::solver::{SolveFingerprints, SolveSolution, linear_model_fingerprint};
    use ospf_rust_framework::solver::FixedRecourseStop as Stop;

    fn source(maximize: bool) -> MetaModel<f64> {
        let mut input = ospf_rust_framework::solver::remote::SerializedLinearModel::empty("fixed-recourse-source");
        use ospf_rust_framework::solver::remote::{SerializedVariable, SerializedVariableType};
        for (index, name, variable_type) in [(0, "x", SerializedVariableType::Binary), (1, "y", SerializedVariableType::Continuous)] {
            input.variables.push(SerializedVariable { index, name: name.into(), stable_id: name.into(), lower_bound: 0.0, upper_bound: 1.0, variable_type });
        }
        let mut source = crate::compile_linear_meta(&input).unwrap();
        source.add_linear_constraint(&[(0, 1.0), (1, 1.0)], ConstraintRelation::GreaterEqual, 1.0, "coupling").unwrap();
        source.set_linear_objective(vec![0.5, 1.0], if maximize { ObjectiveCategory::Maximum } else { ObjectiveCategory::Minimum });
        source
    }

    #[test]
    fn stage_configuration_uses_checked_source_columns_and_never_stages_integer_recourse() {
        let source = source(false);
        let mut options = FixedRecourseOptions { backend: NativeBackend::Scip, stages: None, iteration_limit: None, tolerance: None, recover_missing_certificates: None };
        for index in [0, 2] {
            options.stages = Some(vec![StagedVariable { variable_index: index, stage: FixedRecourseStage::ContinuousRecourse }]);
            let partition = options.partition_options(&source).and_then(|options| ospf_rust_framework::solver::LinearBendersPartition::build(&source, options).map_err(invalid));
            assert!(partition.is_err());
        }
        options.stages = Some(vec![StagedVariable { variable_index: 1, stage: FixedRecourseStage::ContinuousRecourse }]);
        let partition = ospf_rust_framework::solver::LinearBendersPartition::build(&source, options.partition_options(&source).unwrap()).unwrap();
        assert_eq!(partition.recourse_variables().len(), 1);
        let duplicate = options.stages.as_ref().unwrap()[0].clone();
        options.stages.as_mut().unwrap().push(duplicate);
        assert!(options.validate().is_err());
        assert!(serde_json::from_str::<FixedRecourseOptions>(r#"{"backend":"scip","callback":"code"}"#).is_err());
        for value in [0, 10_001] { options.stages = None; options.iteration_limit = Some(value); assert!(options.validate().is_err()); }
    }

    #[test]
    fn result_codec_validates_source_values_direction_and_exact_child_text() {
        for maximize in [false, true] {
            let source = source(maximize);
            let model = source.try_to_linear_triad_model().unwrap();
            let artifact = crate::encode_model(&model).unwrap();
            let values = if maximize { vec![1.0, 1.0] } else { vec![1.0, 0.0] };
            let objective = if maximize { -1.5 } else { 0.5 };
            let child = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(SolveSolution::vector(values.clone()))
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() }).build().unwrap();
            let result = FixedRecourseResult { backend_error: None, stop: Stop::Converged,
                values: source.tokens().iter().zip(&values).map(|(token, value)| (token.id(), *value)).collect(),
                objective: Some(objective), lower_bound: Some(objective), master_report: Some(child.clone()), subproblem_report: Some(child),
                iterations: 1, feasibility_cuts: 0, optimality_cuts: 0 };
            let json = encode_fixed_recourse_result(&result, &source, &artifact, 1e-7).unwrap();
            let mut report = decode_fixed_recourse_report(&json, &artifact).unwrap();
            assert_eq!(report.summary.objective_value, Some(objective.abs()));
            assert_eq!(report.summary.best_bound, Some(objective.abs()));
            let precise = report.master_report_json.as_ref().unwrap().replace("\"randomSeed\":null", "\"randomSeed\":18446744073709551615")
                .replace("\"nodes\":null", "\"nodes\":18446744073709551615");
            report.master_report_json = Some(precise.clone());
            let restored = decode_fixed_recourse_report(&serde_json::to_string(&report).unwrap(), &artifact).unwrap();
            assert_eq!(restored.master_report_json.unwrap(), precise);
            for field in ["values", "objectiveValue", "bestBound"] {
                let mut corrupt: serde_json::Value = serde_json::from_str(&json).unwrap();
                corrupt["summary"][field] = if field == "values" { serde_json::json!([0.0, 0.0]) } else { serde_json::json!(100.0) };
                assert!(decode_fixed_recourse_report(&serde_json::to_string(&corrupt).unwrap(), &artifact).is_err());
            }
            let foreign = ModelArtifact { fingerprint: "wrong".into(), ..artifact.clone() };
            assert_eq!(decode_fixed_recourse_report(&json, &foreign).unwrap_err().code, "FingerprintMismatch");
        }
    }
}
