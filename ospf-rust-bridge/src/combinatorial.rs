//! 组合编排配置及无损报告 / Combinatorial orchestration configuration and lossless reports.

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{CombinatorialSolveReport, CURRENT_SOLVE_REPORT_SCHEMA_VERSION, linear_model_fingerprint};
use crate::{BridgeError, Result, invalid};

/// 显式原生后端身份 / Explicit native backend identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NativeBackend {
    /// SCIP 原生后端 / Native SCIP backend.
    Scip,
    /// Gurobi 原生后端 / Native Gurobi backend.
    Gurobi,
}

impl NativeBackend {
    /// 稳定后端 ID / Stable backend ID.
    pub fn id(self) -> &'static str { match self { Self::Scip => "scip", Self::Gurobi => "gurobi" } }
}

/// 串行编排的可靠数学终态策略 / Reliable mathematical-terminal policy for serial orchestration.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerialFallbackPolicy {
    /// 可靠终态停止；不依赖裸状态 / Stop at certified terminals, not bare statuses.
    #[default]
    StopOnCertifiedTerminal,
    /// 可靠终态后仍尝试下一个后端 / Try later backends after certified terminals.
    ContinueOnTerminal,
}

/// 宿主预置串行线性组合策略的类型化配置 / Typed configuration for the host's built-in serial linear composition strategy.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SerialLinearOptions {
    /// 有序、非空且不重复的原生后端列表 / Ordered nonempty list of distinct native backends.
    pub backends: Vec<NativeBackend>,
    /// 省略使用 Rust framework 默认策略 / Omission uses the Rust framework default policy.
    #[ts(optional)]
    pub fallback_policy: Option<SerialFallbackPolicy>,
}

impl SerialLinearOptions {
    /// 限制一条链为已知不同后端，不自动插入或跳过 / Restrict a chain to distinct known backends, without automatic insertion or skipping.
    pub fn validate(&self) -> Result<()> {
        if self.backends.is_empty() || self.backends.len() > 2 || self.backends.iter().collect::<BTreeSet<_>>().len() != self.backends.len() {
            return Err(BridgeError::new("InvalidInput", "serial linear backends must be a nonempty distinct list of scip and/or gurobi"));
        }
        Ok(())
    }
}

/// 对应 Rust framework 的并行选择模式 / Parallel selection mode corresponding to Rust framework.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ParallelCombinatorialMode {
    /// 接受首个可选报告并请求取消其余 worker，仍等待回收 / Accept the first selectable report and cancel other workers, still joining them.
    First,
    /// 等待全部后端并按模型目标方向选择 / Wait for all backends and select by the model objective direction.
    #[default]
    Best,
}

/// 宿主预置并行线性组合策略的配置 / Configuration for the host's built-in parallel linear composition strategy.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParallelLinearOptions {
    /// 有序、非空且不重复的原生后端列表；顺序用于打破平局 / Ordered nonempty distinct backends; order breaks ties.
    pub backends: Vec<NativeBackend>,
    /// 省略使用 Rust framework 的 Best 默认值 / Omission uses the Rust framework Best default.
    #[ts(optional)]
    pub mode: Option<ParallelCombinatorialMode>,
}

impl ParallelLinearOptions {
    /// 限制 worker 数量且不自动替换后端 / Bound worker count without automatic backend substitution.
    pub fn validate(&self) -> Result<()> {
        if self.backends.is_empty() || self.backends.len() > 2 || self.backends.iter().collect::<BTreeSet<_>>().len() != self.backends.len() {
            return Err(BridgeError::new("InvalidInput", "parallel linear backends must be a nonempty distinct list of scip and/or gurobi"));
        }
        Ok(())
    }
}

/// 校验聚合报告契约、schema 与提交模型身份；不独立证明最优性。
/// Validate aggregate contracts, schema and submitted model identity, without independently proving optimality.
pub fn decode_combinatorial_report(json: &str, model: &LinearTriadModel) -> Result<CombinatorialSolveReport<f64, u64>> {
    if json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "combinatorial report exceeds 8 MiB")); }
    let aggregate: CombinatorialSolveReport<f64, u64> = serde_json::from_str(json).map_err(invalid)?;
    if aggregate.report.schema_version != CURRENT_SOLVE_REPORT_SCHEMA_VERSION {
        return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported combinatorial report schema"));
    }
    aggregate.validate().map_err(invalid)?;
    if aggregate.report.fingerprints.model.as_ref() != Some(&linear_model_fingerprint(model).map_err(invalid)?) {
        return Err(BridgeError::new("FingerprintMismatch", "combinatorial report belongs to a different linear model"));
    }
    if let Some(solution) = &aggregate.report.solution {
        if solution.value.is_some() || solution.values.len() != model.num_variables() || solution.pool.iter().any(|values| values.len() != model.num_variables()) {
            return Err(invalid("combinatorial report solution shape differs from source model"));
        }
    }
    Ok(aggregate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveReport, SolveSolution, TerminationReason};

    #[test]
    fn parallel_configuration_is_bounded_typed_and_rejects_callbacks() {
        assert!(ParallelLinearOptions { backends: vec![], mode: None }.validate().is_err());
        assert!(ParallelLinearOptions { backends: vec![NativeBackend::Scip, NativeBackend::Scip], mode: None }.validate().is_err());
        for json in [r#"{"backends":["scip"],"mode":"fastest"}"#, r#"{"backends":["scip"],"callback":"code"}"#] {
            assert!(serde_json::from_str::<ParallelLinearOptions>(json).is_err());
        }
        let configuration: ParallelLinearOptions = serde_json::from_str(r#"{"backends":["scip","gurobi"],"mode":"best"}"#).unwrap();
        configuration.validate().unwrap();
        assert!(matches!(configuration.mode, Some(ParallelCombinatorialMode::Best)));
        assert!(matches!(ParallelCombinatorialMode::default(), ParallelCombinatorialMode::Best));
    }

    #[test]
    fn serial_configuration_is_explicit_and_aggregate_codec_preserves_trace_identity() {
        assert!(SerialLinearOptions { backends: vec![], fallback_policy: None }.validate().is_err());
        assert!(SerialLinearOptions { backends: vec![NativeBackend::Scip, NativeBackend::Scip], fallback_policy: None }.validate().is_err());
        assert!(serde_json::from_str::<SerialLinearOptions>(r#"{"backends":["fake"]}"#).is_err());
        assert!(serde_json::from_str::<SerialLinearOptions>(r#"{"backends":["scip"],"callback":"code"}"#).is_err());
        let model = LinearTriadModel::new("aggregate-codec");
        let report = SolveReport::<f64>::builder(ProblemStatus::Unknown, TerminationReason::BackendFailure)
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() }).build().unwrap();
        let mut aggregate = CombinatorialSolveReport::single(report, "attempt-1");
        aggregate.attempts[0].started_at_epoch_ms = Some(u64::MAX - 1);
        aggregate.attempts[0].completed_at_epoch_ms = Some(u64::MAX);
        let json = serde_json::to_string(&aggregate).unwrap();
        assert_eq!(decode_combinatorial_report(&json, &model).unwrap().attempts[0].completed_at_epoch_ms, Some(u64::MAX));
        assert!(json.contains(&u64::MAX.to_string()));
        aggregate.selected_attempt_id = Some("foreign-attempt".into());
        assert!(decode_combinatorial_report(&serde_json::to_string(&aggregate).unwrap(), &model).is_err());
        aggregate.selected_attempt_id = Some("attempt-1".into());
        let other = LinearTriadModel::new("different");
        assert_eq!(decode_combinatorial_report(&serde_json::to_string(&aggregate).unwrap(), &other).unwrap_err().code, "FingerprintMismatch");
    }

    #[test]
    fn aggregate_codec_rejects_wrong_source_shapes_and_domain_incumbents() {
        let model = LinearTriadModel::new("source-shape");
        for solution in [SolveSolution::vector(vec![1.0]), SolveSolution::incumbent(1.0)] {
            let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
                .solution(solution)
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() }).build().unwrap();
            let aggregate = CombinatorialSolveReport::single(report, "shape-attempt");
            aggregate.validate().unwrap();
            let error = decode_combinatorial_report(&serde_json::to_string(&aggregate).unwrap(), &model).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains("shape"));
        }
    }
}
