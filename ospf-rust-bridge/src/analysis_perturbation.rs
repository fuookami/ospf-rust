//! 批次 B 第 7 单：RHS 扰动策略与报告 / Batch B, delivery 7: RHS perturbation policy and report.
//!
//! `ConstraintPerturbationPolicy` 的构造器（`single_delta` / `multiple_deltas` / `adaptive`）与 `validate()`，加上
//! `ConstraintPerturbationReport::{validate,has_global_improvement,last_observation}`，全是纯数据能力。
//!
//! 最容易被误读的一条是**互斥**：显式 delta 序列与自适应策略**必须恰有其一**（领域实现是
//! `deltas.is_empty() == adaptive.is_none()` → 报错）。两边都给、两边都不给，都是错误，而不是"优先用某个"。
//!
//! The policy constructors and `validate()`, plus the report's `validate` / `has_global_improvement` /
//! `last_observation`, are all pure-data capabilities. The easiest rule to misread is the **exclusivity**: an explicit delta
//! sequence and an adaptive strategy must be exactly one of the two (the domain checks
//! `deltas.is_empty() == adaptive.is_none()`), so supplying both or neither is an error rather than a silent preference.

use std::time::Duration;

use ospf_rust_core::analysis::perturbation::{
    AdaptivePerturbationConfig, ConstraintPerturbationPolicy, ConstraintPerturbationReport,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 自适应扰动配置输入。 / Adaptive perturbation config input.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdaptivePerturbationInput {
    /// 初始正扰动。 / The initial positive perturbation.
    pub initial_delta: f64,
    /// 每次扩大倍数（必须 > 1）。 / The growth factor (must be above 1).
    pub growth_factor: f64,
    /// 最大允许扰动（不得小于初始值）。 / The maximum perturbation (not below the initial one).
    pub max_delta: f64,
    /// 是否在首次有效后二分细化。 / Whether to refine the first effective interval.
    pub refine_threshold: bool,
    /// 二分停止宽度（必须为正）。 / The binary-search stopping width (must be positive).
    pub refinement_tolerance: f64,
}

/// 扰动策略请求（可选带一份报告）。 / The policy request, optionally carrying a report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerturbationPolicyRequest {
    /// 显式扰动序列；与 `adaptive` 互斥。 / The explicit delta sequence; mutually exclusive with `adaptive`.
    #[serde(default)]
    pub deltas: Vec<f64>,
    /// 自适应策略；与 `deltas` 互斥。 / The adaptive strategy; mutually exclusive with `deltas`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub adaptive: Option<AdaptivePerturbationInput>,
    /// 是否执行删除重优化。 / Whether to run the removal reoptimization.
    #[serde(default)]
    pub removal_test: bool,
    /// 最多求解次数；省略用领域默认 32。 / The maximum solve count; absent means the domain default of 32.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_solves: Option<u32>,
    /// 总时间预算（毫秒）；省略表示无预算。 / The total time budget in milliseconds; absent means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub time_budget_ms: Option<u64>,
    /// 目标改善容差；省略用领域默认 `1e-7`。 / The objective-improvement tolerance; absent means the domain default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub objective_tolerance: Option<f64>,
    /// 可选的扰动报告 JSON 文本。 / The optional perturbation-report JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub report: Option<String>,
}

/// 自适应配置回显。 / The adaptive config echo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdaptivePerturbationEcho {
    /// 初始正扰动。 / The initial positive perturbation.
    pub initial_delta: f64,
    /// 扩大倍数。 / The growth factor.
    pub growth_factor: f64,
    /// 最大允许扰动。 / The maximum perturbation.
    pub max_delta: f64,
    /// 是否二分细化。 / Whether refinement is enabled.
    pub refine_threshold: bool,
    /// 二分停止宽度。 / The binary-search stopping width.
    pub refinement_tolerance: f64,
}

/// 一条扰动观察的投影。 / One perturbation-observation projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerturbationObservationRow {
    /// 正扰动量。 / The positive perturbation amount.
    pub delta: f64,
    /// 扰动后的 RHS。 / The perturbed RHS.
    pub perturbed_rhs: f64,
    /// 求解状态。 / The solve status.
    pub status: String,
    /// 新目标值。 / The new objective value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub objective: Option<f64>,
    /// 按方向归一化的改善量。 / The improvement normalized by direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub improvement: Option<f64>,
    /// 是否已证明改善。 / Whether an improvement is proven.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub improved: Option<bool>,
    /// 是否有最优性证明。 / Whether an optimality proof is available.
    pub objective_proven: bool,
}

/// 报告摘要。 / The report summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerturbationReportSummary {
    /// 报告 schema。 / The report schema.
    pub schema_version: String,
    /// 原始约束稳定 ID。 / The stable original-constraint ID.
    pub constraint_id: String,
    /// baseline 目标值。 / The baseline objective value.
    pub baseline_objective: f64,
    /// 总体分析状态。 / The overall analysis status.
    pub status: String,
    /// 实际求解次数。 / The number of solver calls.
    pub solve_count: u32,
    /// 观察条数。 / The observation count.
    pub observation_count: u32,
    /// 是否有删除观察。 / Whether a removal observation is present.
    pub has_removal: bool,
    /// 删除是否被证明有效。 / Whether removal is proven effective.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub removal_effective: Option<bool>,
    /// 自适应结论的 `kind`（来自领域的 serde 标签）。 / The adaptive outcome `kind` (from the domain's serde tag).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub adaptive_outcome: Option<String>,
    /// 是否观察到已证明的目标改善（领域自己的 `has_global_improvement`）。
    /// Whether a proven improvement was observed (the domain's own `has_global_improvement`).
    pub has_global_improvement: bool,
    /// 最后一次观察在列表中的下标。 / The index of the last observation in the list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_observation_index: Option<u32>,
    /// 未形成结论的原因。 / Why no complete conclusion was formed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unavailable_reason: Option<String>,
    /// 观察明细。 / The observation details.
    pub observations: Vec<PerturbationObservationRow>,
}

/// 扰动策略校验结论（可选带报告摘要）。 / The policy verdict, optionally with a report summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerturbationVerification {
    /// 模式：`explicit`（显式序列）或 `adaptive`（自适应）。 / The mode: `explicit` or `adaptive`.
    pub mode: String,
    /// 显式 delta 条数（自适应模式为 0）。 / The explicit delta count (0 in adaptive mode).
    pub delta_count: u32,
    /// 归一化后的显式序列。 / The normalized explicit sequence.
    pub deltas: Vec<f64>,
    /// 归一化后的自适应配置。 / The normalized adaptive config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub adaptive: Option<AdaptivePerturbationEcho>,
    /// 是否执行删除重优化。 / Whether the removal reoptimization runs.
    pub removal_test: bool,
    /// 实际生效的最大求解次数。 / The maximum solve count in effect.
    pub max_solves: u32,
    /// 实际生效的时间预算（毫秒）。 / The time budget in effect, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub time_budget_ms: Option<u64>,
    /// 实际生效的目标改善容差。 / The objective-improvement tolerance in effect.
    pub objective_tolerance: f64,
    /// 报告摘要；未给报告时为空。 / The report summary; absent when no report was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub report: Option<PerturbationReportSummary>,
}

/// 校验一份扰动策略，并在给出报告时校验与投影报告。非法输入是**错误**，原因原样带出。
/// Verify a perturbation policy and, when a report is supplied, verify and project it. An illegal input is an **error**
/// carrying the reason verbatim.
pub fn verify_perturbation_policy(request: &PerturbationPolicyRequest) -> Result<PerturbationVerification> {
    // 显式序列与自适应策略互斥。领域在 `validate()` 里查这条，但两个领域构造器都会**丢掉**另一半
    // （`adaptive()` 把 `deltas` 清空、`multiple_deltas()` 把 `adaptive` 置空），所以先在这里拒绝，
    // 否则调用方给的 deltas 会被静默丢弃。
    // The explicit sequence and the adaptive strategy are mutually exclusive. The domain checks this in `validate()`,
    // but both domain constructors **drop** the other half (`adaptive()` clears `deltas`, `multiple_deltas()` nulls
    // `adaptive`), so the ambiguity is rejected here first; otherwise the caller's deltas would vanish silently.
    if request.adaptive.is_some() && !request.deltas.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "perturbation policy failed validation: a perturbation policy must contain either explicit deltas or an adaptive strategy"));
    }
    // 走领域的构造器而不是结构体字面量：构造器会带上领域默认值（`max_solves = 32` 等），
    // 用字面量就得在这里抄一遍默认值，两处默认迟早不一致。
    // Use the domain constructors rather than a struct literal: the constructors carry the domain defaults
    // (`max_solves = 32` and so on); a literal would copy those defaults here and the two would drift apart.
    let mut policy = match request.adaptive {
        Some(adaptive) => ConstraintPerturbationPolicy::adaptive(AdaptivePerturbationConfig {
            initial_delta: adaptive.initial_delta,
            growth_factor: adaptive.growth_factor,
            max_delta: adaptive.max_delta,
            refine_threshold: adaptive.refine_threshold,
            refinement_tolerance: adaptive.refinement_tolerance,
        }),
        None => ConstraintPerturbationPolicy::multiple_deltas(request.deltas.iter().copied()),
    };
    policy = policy.with_removal_test(request.removal_test);
    if let Some(max_solves) = request.max_solves {
        policy = policy.with_max_solves(usize::try_from(max_solves).unwrap_or(usize::MAX));
    }
    if let Some(milliseconds) = request.time_budget_ms {
        policy = policy.with_time_budget(Duration::from_millis(milliseconds));
    }
    if let Some(tolerance) = request.objective_tolerance {
        policy = policy.with_objective_tolerance(tolerance);
    }
    policy.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("perturbation policy failed validation: {error}"))
    })?;
    let report = request.report.as_deref().map(verify_report).transpose()?;
    Ok(PerturbationVerification {
        mode: if policy.adaptive.is_some() { "adaptive" } else { "explicit" }.to_owned(),
        delta_count: u32::try_from(policy.deltas.len()).unwrap_or(u32::MAX),
        deltas: policy.deltas.clone(),
        adaptive: policy.adaptive.map(|adaptive| AdaptivePerturbationEcho {
            initial_delta: adaptive.initial_delta,
            growth_factor: adaptive.growth_factor,
            max_delta: adaptive.max_delta,
            refine_threshold: adaptive.refine_threshold,
            refinement_tolerance: adaptive.refinement_tolerance,
        }),
        removal_test: policy.removal_test,
        max_solves: u32::try_from(policy.max_solves).unwrap_or(u32::MAX),
        time_budget_ms: policy.time_budget.map(|budget| u64::try_from(budget.as_millis()).unwrap_or(u64::MAX)),
        objective_tolerance: policy.objective_tolerance,
        report,
    })
}

/// 校验并投影一份扰动报告。 / Verify and project one perturbation report.
fn verify_report(json: &str) -> Result<PerturbationReportSummary> {
    if json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the perturbation report exceeds 8 MiB"));
    }
    let report: ConstraintPerturbationReport = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("perturbation report is not decodable: {error}"))
    })?;
    report.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("perturbation report failed validation: {error}"))
    })?;
    Ok(PerturbationReportSummary {
        schema_version: report.schema_version.clone(),
        constraint_id: report.constraint_id.0.clone(),
        baseline_objective: report.baseline_objective,
        status: status_name(report.status).to_owned(),
        solve_count: u32::try_from(report.solve_count).unwrap_or(u32::MAX),
        observation_count: u32::try_from(report.observations.len()).unwrap_or(u32::MAX),
        has_removal: report.removal.is_some(),
        removal_effective: report.removal.as_ref().and_then(|removal| removal.effective),
        // 自适应结论走领域的 serde 标签，而不是 `Debug` 字符串：那是线格式契约，改不动。
        // The adaptive outcome uses the domain's serde tag rather than a `Debug` string: that is the wire contract.
        adaptive_outcome: report.adaptive_outcome
            .and_then(|outcome| serde_json::to_value(outcome).ok())
            .and_then(|value| value.get("kind").and_then(|kind| kind.as_str()).map(str::to_owned)),
        has_global_improvement: report.has_global_improvement(),
        last_observation_index: report
            .last_observation()
            .and_then(|last| report.observations.iter().position(|observation| observation == last))
            .and_then(|position| u32::try_from(position).ok()),
        unavailable_reason: report.unavailable_reason.clone(),
        observations: report.observations.iter().map(|observation| PerturbationObservationRow {
            delta: observation.delta,
            perturbed_rhs: observation.perturbed_rhs,
            status: status_name(observation.status).to_owned(),
            objective: observation.objective,
            improvement: observation.improvement,
            improved: observation.improved,
            objective_proven: observation.objective_proven,
        }).collect(),
    })
}

/// 分析状态名。 / The analysis status name.
fn status_name(status: ospf_rust_core::analysis::AnalysisStatus) -> &'static str {
    use ospf_rust_core::analysis::AnalysisStatus;
    match status {
        AnalysisStatus::Reachable => "reachable",
        AnalysisStatus::Unreachable => "unreachable",
        AnalysisStatus::Unknown => "unknown",
        AnalysisStatus::Unsupported => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::analysis::perturbation::{
        AdaptivePerturbationOutcome, ConstraintPerturbationObservation, RemovalTestObservation,
    };
    use ospf_rust_core::analysis::AnalysisStatus;
    use ospf_rust_core::solver::report::{TerminationReason};
    use ospf_rust_core::solver::StableConstraintId as ConstraintId;

    fn request() -> PerturbationPolicyRequest {
        PerturbationPolicyRequest {
            deltas: vec![1.0],
            adaptive: None, removal_test: false, max_solves: None, time_budget_ms: None,
            objective_tolerance: None, report: None,
        }
    }

    fn observation(delta: f64, improved: Option<bool>) -> ConstraintPerturbationObservation {
        ConstraintPerturbationObservation {
            delta,
            perturbed_rhs: 10.0 + delta,
            status: AnalysisStatus::Reachable,
            termination_reason: TerminationReason::Completed,
            objective: Some(9.0),
            objective_change: Some(-1.0),
            improvement: Some(1.0),
            improved,
            integer_pattern_changed: Some(false),
            solve_time: Duration::from_millis(2),
            objective_proven: true,
        }
    }

    fn report() -> ConstraintPerturbationReport {
        ConstraintPerturbationReport {
            schema_version: "1.0".to_owned(),
            constraint_id: ConstraintId("capacity".to_owned()),
            baseline_objective: 10.0,
            observations: vec![observation(1.0, Some(false)), observation(2.0, Some(true))],
            removal: Some(RemovalTestObservation {
                status: AnalysisStatus::Reachable,
                termination_reason: TerminationReason::Completed,
                objective: Some(8.0), objective_change: Some(-2.0), improvement: Some(2.0),
                effective: Some(false), integer_pattern_changed: Some(false),
                solve_time: Duration::from_millis(2), objective_proven: true,
            }),
            adaptive_outcome: Some(AdaptivePerturbationOutcome::NoObservedEffect),
            status: AnalysisStatus::Reachable,
            solve_count: 3,
            unavailable_reason: None,
        }
    }

    fn encode(report: &ConstraintPerturbationReport) -> String {
        serde_json::to_string(report).expect("serialize perturbation report")
    }

    #[test]
    fn explicit_policy_uses_domain_defaults_and_builders() {
        let verification = verify_perturbation_policy(&request()).unwrap();
        assert_eq!(verification.mode, "explicit");
        assert_eq!(verification.delta_count, 1);
        assert_eq!(verification.deltas, vec![1.0]);
        // 未指定的字段用领域默认值：`max_solves = 32`、`objective_tolerance = 1e-7`、无时间预算。
        // Unspecified fields use the domain defaults: 32 solves, a 1e-7 tolerance and no time budget.
        let default = ConstraintPerturbationPolicy::default();
        assert_eq!(verification.max_solves, default.max_solves as u32);
        assert_eq!(verification.objective_tolerance, default.objective_tolerance);
        assert_eq!(verification.time_budget_ms, None);
        assert!(!verification.removal_test);
        assert_eq!(verification.adaptive, None);
        assert_eq!(verification.report, None);

        let mut built = request();
        built.deltas = vec![1.0, 2.0, 4.0];
        built.max_solves = Some(8);
        built.time_budget_ms = Some(500);
        built.objective_tolerance = Some(1e-3);
        built.removal_test = true;
        let verification = verify_perturbation_policy(&built).unwrap();
        assert_eq!(verification.deltas, vec![1.0, 2.0, 4.0]);
        assert_eq!(verification.delta_count, 3);
        assert_eq!(verification.max_solves, 8);
        assert_eq!(verification.time_budget_ms, Some(500));
        assert_eq!(verification.objective_tolerance, 1e-3);
        assert!(verification.removal_test);
    }

    #[test]
    fn adaptive_policy_is_mutually_exclusive_with_explicit_deltas() {
        let adaptive = AdaptivePerturbationInput {
            initial_delta: 1.0, growth_factor: 2.0, max_delta: 8.0,
            refine_threshold: false, refinement_tolerance: 1e-6,
        };
        let mut adaptive_only = request();
        adaptive_only.deltas = Vec::new();
        adaptive_only.adaptive = Some(adaptive);
        let verification = verify_perturbation_policy(&adaptive_only).unwrap();
        assert_eq!(verification.mode, "adaptive");
        // 自适应模式下显式序列必须为空——那是领域的互斥规则，不是"忽略"。
        // In adaptive mode the explicit sequence must be empty; that is the domain's exclusivity rule, not a silent
        // ignore.
        assert_eq!(verification.delta_count, 0);
        assert!(verification.deltas.is_empty());
        assert_eq!(verification.adaptive, Some(AdaptivePerturbationEcho {
            initial_delta: 1.0, growth_factor: 2.0, max_delta: 8.0,
            refine_threshold: false, refinement_tolerance: 1e-6 }));

        // 两边都给：错误，而不是"优先自适应"。 / Supplying both is an error, not "prefer adaptive".
        let mut both = request();
        both.adaptive = Some(adaptive);
        let error = verify_perturbation_policy(&both).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("either explicit deltas or an adaptive strategy"), "{}", error.message);

        // 两边都不给：同样是错误。 / Supplying neither is the same error.
        let mut neither = request();
        neither.deltas = Vec::new();
        let error = verify_perturbation_policy(&neither).unwrap_err();
        assert!(error.message.contains("either explicit deltas or an adaptive strategy"), "{}", error.message);
    }

    #[test]
    fn illegal_policies_are_refused_with_the_domain_reason() {
        let cases: Vec<(&str, Box<dyn Fn(&mut PerturbationPolicyRequest)>, &str)> = vec![
            ("zero delta", Box::new(|request: &mut PerturbationPolicyRequest| {
                request.deltas = vec![0.0];
            }), "strictly positive"),
            ("negative delta", Box::new(|request: &mut PerturbationPolicyRequest| {
                request.deltas = vec![-1.0];
            }), "strictly positive"),
            ("zero solves", Box::new(|request: &mut PerturbationPolicyRequest| {
                request.max_solves = Some(0);
            }), "max solves must be positive"),
            ("negative tolerance", Box::new(|request: &mut PerturbationPolicyRequest| {
                request.objective_tolerance = Some(-1e-7);
            }), "max solves must be positive"),
            ("zero time budget", Box::new(|request: &mut PerturbationPolicyRequest| {
                request.time_budget_ms = Some(0);
            }), "time budget must be positive"),
            ("growth factor one", Box::new(|request: &mut PerturbationPolicyRequest| {
                request.deltas = Vec::new();
                request.adaptive = Some(AdaptivePerturbationInput {
                    initial_delta: 1.0, growth_factor: 1.0, max_delta: 8.0,
                    refine_threshold: true, refinement_tolerance: 1e-7 });
            }), "finite and ordered"),
            ("max below initial", Box::new(|request: &mut PerturbationPolicyRequest| {
                request.deltas = Vec::new();
                request.adaptive = Some(AdaptivePerturbationInput {
                    initial_delta: 4.0, growth_factor: 2.0, max_delta: 2.0,
                    refine_threshold: true, refinement_tolerance: 1e-7 });
            }), "finite and ordered"),
        ];
        for (label, mutate, expected) in cases {
            let mut broken = request();
            mutate(&mut broken);
            let error = match verify_perturbation_policy(&broken) {
                Ok(_) => panic!("case `{label}` must be refused"),
                Err(error) => error,
            };
            assert_eq!(error.code, "InvalidInput", "{label}");
            assert!(error.message.contains(expected), "{label}: {}", error.message);
        }
    }

    #[test]
    fn report_is_validated_and_projected() {
        let mut with_report = request();
        with_report.report = Some(encode(&report()));
        let verification = verify_perturbation_policy(&with_report).unwrap();
        let summary = verification.report.expect("report summary");
        assert_eq!(summary.constraint_id, "capacity");
        assert_eq!(summary.observation_count, 2);
        assert_eq!(summary.solve_count, 3);
        assert!(summary.has_removal);
        assert_eq!(summary.removal_effective, Some(false));
        // 第二次观察的 `improved = true` 让全局改善成立。 / The second observation's `improved = true` proves it.
        assert!(summary.has_global_improvement);
        assert_eq!(summary.last_observation_index, Some(1));
        assert_eq!(summary.adaptive_outcome.as_deref(), Some("noObservedEffect"));
        assert_eq!(summary.status, "reachable");
        assert_eq!(summary.observations[1].delta, 2.0);
        assert_eq!(summary.observations[1].improved, Some(true));
        assert!(summary.observations[0].objective_proven);

        // `solve_count` 必须等于观察数 + 是否有删除观察：这是领域不变量。
        // `solve_count` must equal the observations plus whether a removal ran: a domain invariant.
        let mut drifted = report();
        drifted.solve_count = 9;
        let mut broken = request();
        broken.report = Some(encode(&drifted));
        let error = verify_perturbation_policy(&broken).unwrap_err();
        assert!(error.message.contains("solve count"), "{}", error.message);

        // 空白 schema / 非 JSON。 / Blank schema and non-JSON.
        let mut blank = report();
        blank.schema_version = " ".to_owned();
        let mut broken = request();
        broken.report = Some(encode(&blank));
        assert_eq!(verify_perturbation_policy(&broken).unwrap_err().code, "InvalidInput");
        let mut broken = request();
        broken.report = Some("not json".to_owned());
        assert_eq!(verify_perturbation_policy(&broken).unwrap_err().code, "InvalidInput");
    }
}
