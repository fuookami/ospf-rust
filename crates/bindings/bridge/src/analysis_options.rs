//! 续挖第 65 单：分析流水线**阶段配置**的四道闸门——flag 不校验、`candidateLimit` 是快照、`validate()` 按阶段顺序短路
//! （`analysis/pipeline.rs` 的 `CriticalConstraintAnalysisOptions` 及其五个子配置）
//! Continuation, delivery 65: the four gates of the analysis pipeline's **stage configuration** — the run flags are never
//! validated, `candidateLimit` is a snapshot, and `validate()` short-circuits in stage order (`CriticalConstraintAnalysisOptions`
//! and its five sub-configurations in `analysis/pipeline.rs`).
//!
//! §11.14 点名 `analysis/pipeline.rs` 的可编码部分。前一单量视图，这一单量**编排配置**——它不含任何求解器调用，
//! 却能决定后续每一段跑不跑。七条实测：
//!
//! 1. **四个 `run_*` 默认全真**（`all_four_stage_flags_default_true()`）。
//! 2. **`candidate_limit` 是**快照**而不是引用**（`the_candidate_limit_is_snapshotted_from_the_funnel()`）：
//!    `Default` 时把它拷成 `funnel.default_candidate_limit`，之后改 `funnel` 不会回头改它。
//! 3. **`candidate_limit = 0` 合法**（`a_zero_candidate_limit_is_legal()`）：`validate()` 根本不看它，
//!    按文档就是"只做筛选"。
//! 4. **四个 `run_*` 也从不被校验**（`the_four_run_flags_are_never_validated()`）：全关掉 `validate()` 照样通过——
//!    "一段都不跑的流水线"是**合法配置**。
//! 5. **`validate()` 按阶段顺序短路**（`validation_runs_in_stage_order()`）：funnel 坏 + 扰动也坏时，
//!    报的是 **funnel** 的文案；先修 funnel 才看得见扰动的问题。
//! 6. **流水线与 conflict 阶段各带一套 CP 参数**（`the_pipeline_and_the_conflict_stage_carry_separate_cp_options()`）：
//!    `options.solve_options` 与 `options.conflict.solve_options` 互不影响，坏一套不牵连另一套。
//! 7. **扰动策略要求 `deltas` 与 `adaptive` 恰好二选一**（`the_perturbation_policy_requires_exactly_one_source()`）：
//!    "两个都空"和"两个都给定"**报同一条文案**（判据写成 `is_empty() == is_none()`）。
//!
//! The §11.14 audit names the codable part of `analysis/pipeline.rs`. The previous delivery measured the views; this one
//! measures **the orchestration configuration** — no solver call anywhere, yet it decides whether each later stage runs.
//! **All four `run_*` flags default true.** **`candidate_limit` is a snapshot, not a reference**: `Default` copies it from
//! `funnel.default_candidate_limit`, and later funnel edits never reach it. **`candidate_limit = 0` is legal**: `validate()`
//! never looks at it, and the doc says zero means screening only. **The four `run_*` flags are never validated either**: turning
//! every stage off still passes — "a pipeline that runs nothing" is a **legal configuration**. **`validate()` short-circuits
//! in stage order**: with a broken funnel *and* a broken perturbation, the reported message is the **funnel's**. **The pipeline
//! and the conflict stage carry separate CP option sets**: breaking one never touches the other. And **the perturbation policy
//! demands exactly one of `deltas`/`adaptive`**: "both empty" and "both given" share one message because the check is written
//! `is_empty() == is_none()`.

use std::time::Duration;

use ospf_rust_core::analysis::{
    CandidateFunnelConfig, ConflictAnalysisOptions, ConstraintPerturbationPolicy,
    FixedIntegerLpSensitivityConfig,
};
use ospf_rust_core::solver::ConstraintProgrammingSolveOptions;
use ospf_rust_core::{AdaptivePerturbationConfig, CriticalConstraintAnalysisOptions};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 阶段配置探针请求。 / The stage-configuration probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisOptionsRequest {
    /// funnel 的对偶阈值。 / The funnel's dual threshold.
    #[serde(default)]
    pub dual_threshold: Option<f64>,
    /// funnel 的默认候选上限。 / The funnel's default candidate limit.
    #[serde(default)]
    pub funnel_default_limit: Option<u64>,
    /// 流水线自己的候选上限。 / The pipeline's own candidate limit.
    #[serde(default)]
    pub candidate_limit: Option<u64>,
    /// 四个 `run_*` 开关（缺省全真）。 / The four `run_*` switches (all true by default).
    #[serde(default)]
    pub run_fixed_integer: Option<bool>,
    #[serde(default)]
    pub run_perturbation: Option<bool>,
    #[serde(default)]
    pub run_target_analysis: Option<bool>,
    #[serde(default)]
    pub run_conflict_analysis: Option<bool>,
    /// CP 枚举上限。 / The CP enumeration limit.
    #[serde(default)]
    pub enumeration_limit: Option<u64>,
    /// CP conflict 开关。 / The CP conflict switches.
    #[serde(default)]
    pub request_conflict: Option<bool>,
    #[serde(default)]
    pub shrink_conflict: Option<bool>,
    /// CP conflict 重求解预算。 / The CP conflict resolve budget.
    #[serde(default)]
    pub max_conflict_resolves: Option<u64>,
    /// 扰动显式序列（给空数组表示"空"）。 / The explicit perturbation deltas (an empty array means "empty").
    #[serde(default)]
    pub deltas: Option<Vec<f64>>,
    /// 是否给自适应策略。 / Whether an adaptive strategy is supplied.
    #[serde(default)]
    pub adaptive: Option<bool>,
    /// 扰动求解次数上限。 / The perturbation solve cap.
    #[serde(default)]
    pub max_solves: Option<u64>,
}

/// 阶段配置探针结论。 / The stage-configuration probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisOptionsReport {
    /// funnel 阈值读数。 / The funnel threshold reading.
    pub dual_threshold: String,
    /// funnel 的默认上限读数。 / The funnel's default limit reading.
    pub funnel_default_limit: u64,
    /// 流水线候选上限读数。 / The pipeline's candidate limit reading.
    pub candidate_limit: u64,
    /// 四个 `run_*` 读数。 / The four `run_*` readings.
    pub run_fixed_integer: bool,
    pub run_perturbation: bool,
    pub run_target_analysis: bool,
    pub run_conflict_analysis: bool,
    /// CP 参数读数。 / The CP option readings.
    pub enumeration_limit: u64,
    pub request_conflict: bool,
    pub shrink_conflict: bool,
    pub max_conflict_resolves: u64,
    /// 扰动读数。 / The perturbation readings.
    pub delta_count: u64,
    pub has_adaptive: bool,
    pub max_solves: u64,
    /// 固定整数容差读数。 / The fixed-integer tolerance readings.
    pub integrality_tolerance: String,
    pub dual_tolerance: String,
    /// 这份配置自己的 `validate()` 结论（`None` 表示通过）。 / This configuration's own verdict (`None` = accepted).
    pub validate_message: Option<String>,
    /// 顺序探针报出的文案（funnel 与扰动同时坏时）。 / The message the order probe reports.
    pub order_probe_message: String,
    /// **只**坏 conflict 阶段的 CP 参数时的文案。 / The message when only the conflict stage's CP options are broken.
    pub conflict_only_probe_message: String,
    /// **只**坏流水线的 CP 参数时的文案。 / The message when only the pipeline's CP options are broken.
    pub pipeline_only_probe_message: String,
    /// 四个 `run_*` 默认全真（**是**）。 / Whether the four flags default true (**yes**).
    pub all_four_stage_flags_default_true: bool,
    /// 候选上限是快照（**是**）。 / Whether the limit is a snapshot (**yes**).
    pub the_candidate_limit_is_snapshotted_from_the_funnel: bool,
    /// 零候选上限合法（**是**）。 / Whether a zero limit is legal (**yes**).
    pub a_zero_candidate_limit_is_legal: bool,
    /// 四个开关从不被校验（**是**）。 / Whether the four flags are never validated (**yes**).
    pub the_four_run_flags_are_never_validated: bool,
    /// `validate()` 按阶段顺序短路（**是**）。 / Whether validation short-circuits in stage order (**yes**).
    pub validation_runs_in_stage_order: bool,
    /// 流水线与 conflict 各带一套 CP 参数（**是**）。 / Whether the two CP option sets are separate (**yes**).
    pub the_pipeline_and_the_conflict_stage_carry_separate_cp_options: bool,
    /// 扰动策略要求恰好一个来源（**是**）。 / Whether the policy demands exactly one source (**yes**).
    pub the_perturbation_policy_requires_exactly_one_source: bool,
}

/// 自适应扰动配置。 / The adaptive perturbation configuration.
fn adaptive_config() -> AdaptivePerturbationConfig {
    AdaptivePerturbationConfig::default()
}

/// 从请求造一份配置。 / Build a configuration from the request.
fn build(request: &AnalysisOptionsRequest) -> Result<CriticalConstraintAnalysisOptions<'static>> {
    let mut options = CriticalConstraintAnalysisOptions::default();
    options.funnel = CandidateFunnelConfig {
        dual_threshold: request.dual_threshold.unwrap_or(1e-9),
        default_candidate_limit: usize::try_from(request.funnel_default_limit.unwrap_or(32))
            .map_err(|_| BridgeError::new("InvalidInput",
                "stage options failed validation: funnel limit overflows usize"))?,
    };
    if let Some(limit) = request.candidate_limit {
        options.candidate_limit = usize::try_from(limit)
            .map_err(|_| BridgeError::new("InvalidInput",
                "stage options failed validation: candidate limit overflows usize"))?;
    }
    options.run_fixed_integer = request.run_fixed_integer.unwrap_or(true);
    options.run_perturbation = request.run_perturbation.unwrap_or(true);
    options.run_target_analysis = request.run_target_analysis.unwrap_or(true);
    options.run_conflict_analysis = request.run_conflict_analysis.unwrap_or(true);
    options.solve_options = ConstraintProgrammingSolveOptions {
        enumeration_limit: usize::try_from(request.enumeration_limit.unwrap_or(10_000))
            .map_err(|_| BridgeError::new("InvalidInput",
                "stage options failed validation: enumeration limit overflows usize"))?,
        request_conflict: request.request_conflict.unwrap_or(false),
        shrink_conflict: request.shrink_conflict.unwrap_or(false),
        max_conflict_resolves: usize::try_from(request.max_conflict_resolves.unwrap_or(128))
            .map_err(|_| BridgeError::new("InvalidInput",
                "stage options failed validation: conflict budget overflows usize"))?,
        ..ConstraintProgrammingSolveOptions::new()
    };
    let mut policy = ConstraintPerturbationPolicy::default();
    if let Some(deltas) = &request.deltas {
        policy.deltas = deltas.clone();
    }
    if request.adaptive.unwrap_or(false) {
        policy.adaptive = Some(adaptive_config());
    }
    policy.max_solves = usize::try_from(request.max_solves.unwrap_or(32))
        .map_err(|_| BridgeError::new("InvalidInput",
            "stage options failed validation: solve cap overflows usize"))?;
    options.perturbation = policy;
    options.fixed_integer = FixedIntegerLpSensitivityConfig::default();
    Ok(options)
}

/// 走一遍阶段配置探针。非法输入是**错误**。 / Walk the stage-configuration probe.
pub fn verify_analysis_options(request: &AnalysisOptionsRequest) -> Result<AnalysisOptionsReport> {
    if request.deltas.as_ref().is_some_and(|deltas| deltas.len() > 16) {
        return Err(BridgeError::new("InvalidInput",
            "stage options failed validation: at most 16 deltas are accepted"));
    }
    let options = build(request)?;
    let validate_message = options.validate().err().map(|error| error.to_string());
    // 顺序探针：funnel 与扰动**同时**坏，看先报谁。 
    // The order probe breaks the funnel and the perturbation **at once** to see which one speaks first.
    let order_probe_message = {
        let mut broken = CriticalConstraintAnalysisOptions::default();
        broken.funnel = CandidateFunnelConfig { dual_threshold: -1.0, default_candidate_limit: 32 };
        broken.perturbation = ConstraintPerturbationPolicy {
            deltas: Vec::new(), adaptive: None, removal_test: false, max_solves: 0,
            time_budget: None, objective_tolerance: 1e-7 };
        broken.validate().err().map(|error| error.to_string())
            .unwrap_or_else(|| "accepted".to_owned())
    };
    // 只坏 conflict 阶段自带的 CP 参数。 / Break only the conflict stage's own CP options.
    let conflict_only_probe_message = {
        let mut broken = CriticalConstraintAnalysisOptions::default();
        broken.conflict = ConflictAnalysisOptions {
            solve_options: ConstraintProgrammingSolveOptions { enumeration_limit: 0,
                ..ConstraintProgrammingSolveOptions::new() },
            ..ConflictAnalysisOptions::default() };
        broken.validate().err().map(|error| error.to_string())
            .unwrap_or_else(|| "accepted".to_owned())
    };
    // 只坏流水线的 CP 参数。 / Break only the pipeline's CP options.
    let pipeline_only_probe_message = {
        let mut broken = CriticalConstraintAnalysisOptions::default();
        broken.solve_options = ConstraintProgrammingSolveOptions { enumeration_limit: 0,
            ..ConstraintProgrammingSolveOptions::new() };
        broken.validate().err().map(|error| error.to_string())
            .unwrap_or_else(|| "accepted".to_owned())
    };
    let findings = canonical_findings();
    Ok(AnalysisOptionsReport {
        dual_threshold: options.funnel.dual_threshold.to_string(),
        funnel_default_limit: options.funnel.default_candidate_limit as u64,
        candidate_limit: options.candidate_limit as u64,
        run_fixed_integer: options.run_fixed_integer,
        run_perturbation: options.run_perturbation,
        run_target_analysis: options.run_target_analysis,
        run_conflict_analysis: options.run_conflict_analysis,
        enumeration_limit: options.solve_options.enumeration_limit as u64,
        request_conflict: options.solve_options.request_conflict,
        shrink_conflict: options.solve_options.shrink_conflict,
        max_conflict_resolves: options.solve_options.max_conflict_resolves as u64,
        delta_count: options.perturbation.deltas.len() as u64,
        has_adaptive: options.perturbation.adaptive.is_some(),
        max_solves: options.perturbation.max_solves as u64,
        integrality_tolerance: options.fixed_integer.integrality_tolerance.to_string(),
        dual_tolerance: options.fixed_integer.dual_tolerance.to_string(),
        validate_message, order_probe_message, conflict_only_probe_message,
        pipeline_only_probe_message,
        all_four_stage_flags_default_true: findings.all_four_stage_flags_default_true,
        the_candidate_limit_is_snapshotted_from_the_funnel:
            findings.the_candidate_limit_is_snapshotted_from_the_funnel,
        a_zero_candidate_limit_is_legal: findings.a_zero_candidate_limit_is_legal,
        the_four_run_flags_are_never_validated:
            findings.the_four_run_flags_are_never_validated,
        validation_runs_in_stage_order: findings.validation_runs_in_stage_order,
        the_pipeline_and_the_conflict_stage_carry_separate_cp_options:
            findings.the_pipeline_and_the_conflict_stage_carry_separate_cp_options,
        the_perturbation_policy_requires_exactly_one_source:
            findings.the_perturbation_policy_requires_exactly_one_source })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    all_four_stage_flags_default_true: bool,
    the_candidate_limit_is_snapshotted_from_the_funnel: bool,
    a_zero_candidate_limit_is_legal: bool,
    the_four_run_flags_are_never_validated: bool,
    validation_runs_in_stage_order: bool,
    the_pipeline_and_the_conflict_stage_carry_separate_cp_options: bool,
    the_perturbation_policy_requires_exactly_one_source: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 四个 `run_*` 默认全真。 / All four `run_*` flags default true.
    let all_four_stage_flags_default_true = {
        let options = CriticalConstraintAnalysisOptions::default();
        options.run_fixed_integer && options.run_perturbation && options.run_target_analysis
            && options.run_conflict_analysis && options.validate().is_ok()
    };
    // 2) 候选上限是快照：改 funnel 不回灌。 / The limit is a snapshot: funnel edits never flow back.
    let the_candidate_limit_is_snapshotted_from_the_funnel = {
        let mut options = CriticalConstraintAnalysisOptions::default();
        let snapshot = options.candidate_limit;
        options.funnel.default_candidate_limit = 7;
        options.candidate_limit == snapshot && snapshot == 32 && options.candidate_limit != 7
    };
    // 3) 零候选上限合法。 / A zero candidate limit is legal.
    let a_zero_candidate_limit_is_legal = {
        let zeroed = CriticalConstraintAnalysisOptions::default().with_candidate_limit(0);
        zeroed.candidate_limit == 0 && zeroed.validate().is_ok()
            && CriticalConstraintAnalysisOptions::default().candidate_limit == 32
    };
    // 4) 四个开关从不被校验：全关掉仍通过。 / The four flags are never validated: all off still passes.
    let the_four_run_flags_are_never_validated = {
        let mut options = CriticalConstraintAnalysisOptions::default();
        options.run_fixed_integer = false;
        options.run_perturbation = false;
        options.run_target_analysis = false;
        options.run_conflict_analysis = false;
        options.candidate_limit = 0;
        options.validate().is_ok()
    };
    // 5) `validate()` 按阶段顺序短路：funnel 先说话。 
    // 5) `validate()` short-circuits in stage order: the funnel speaks first.
    let validation_runs_in_stage_order = {
        let mut broken = CriticalConstraintAnalysisOptions::default();
        broken.funnel = CandidateFunnelConfig { dual_threshold: -1.0, default_candidate_limit: 32 };
        broken.perturbation = ConstraintPerturbationPolicy { deltas: Vec::new(),
            adaptive: None, removal_test: false, max_solves: 0, time_budget: None,
            objective_tolerance: 1e-7 };
        let both = broken.validate().err().expect("both stages are broken").to_string();
        let funnel_only = CandidateFunnelConfig { dual_threshold: -1.0, default_candidate_limit: 32 }
            .validate().err().expect("the funnel is broken").to_string();
        let perturbation_only = broken.perturbation.validate().err()
            .expect("the perturbation is broken").to_string();
        both == funnel_only && both != perturbation_only
    };
    // 6) 两套 CP 参数互不影响。 / The two CP option sets are independent.
    let the_pipeline_and_the_conflict_stage_carry_separate_cp_options = {
        let mut conflict_broken = CriticalConstraintAnalysisOptions::default();
        conflict_broken.conflict = ConflictAnalysisOptions {
            solve_options: ConstraintProgrammingSolveOptions { enumeration_limit: 0,
                ..ConstraintProgrammingSolveOptions::new() },
            ..ConflictAnalysisOptions::default() };
        let mut pipeline_broken = CriticalConstraintAnalysisOptions::default();
        pipeline_broken.solve_options = ConstraintProgrammingSolveOptions {
            enumeration_limit: 0, ..ConstraintProgrammingSolveOptions::new() };
        // 两套各自能坏、也各自能单独修好。 / Each set breaks on its own and each is fixed on its own.
        conflict_broken.validate().is_err() && pipeline_broken.validate().is_err()
            && {
                conflict_broken.solve_options = ConstraintProgrammingSolveOptions::new();
                conflict_broken.validate().is_err()
            }
            && {
                pipeline_broken.solve_options = ConstraintProgrammingSolveOptions::new();
                pipeline_broken.validate().is_ok()
            }
    };
    // 7) 扰动策略要求恰好一个来源。 / The policy demands exactly one source.
    let the_perturbation_policy_requires_exactly_one_source = {
        let both_empty = ConstraintPerturbationPolicy { deltas: Vec::new(), adaptive: None,
            removal_test: false, max_solves: 32, time_budget: None, objective_tolerance: 1e-7 };
        let both_given = ConstraintPerturbationPolicy { deltas: vec![1.0],
            adaptive: Some(adaptive_config()), removal_test: false, max_solves: 32,
            time_budget: None, objective_tolerance: 1e-7 };
        let empty_message = both_empty.validate().err().expect("both empty is refused")
            .to_string();
        let given_message = both_given.validate().err().expect("both given is refused")
            .to_string();
        empty_message == given_message
            && empty_message.contains("either explicit deltas or an adaptive strategy")
            && ConstraintPerturbationPolicy::single_delta(1.0).validate().is_ok()
            && ConstraintPerturbationPolicy::default().validate().is_ok()
    };
    Findings { all_four_stage_flags_default_true, the_candidate_limit_is_snapshotted_from_the_funnel,
        a_zero_candidate_limit_is_legal, the_four_run_flags_are_never_validated,
        validation_runs_in_stage_order,
        the_pipeline_and_the_conflict_stage_carry_separate_cp_options,
        the_perturbation_policy_requires_exactly_one_source }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AnalysisOptionsRequest {
        AnalysisOptionsRequest { dual_threshold: None, funnel_default_limit: None,
            candidate_limit: None, run_fixed_integer: None, run_perturbation: None,
            run_target_analysis: None, run_conflict_analysis: None, enumeration_limit: None,
            request_conflict: None, shrink_conflict: None, max_conflict_resolves: None,
            deltas: None, adaptive: None, max_solves: None }
    }

    #[test]
    fn the_defaults_are_all_on_and_the_limit_is_a_snapshot() {
        let report = verify_analysis_options(&request()).unwrap();
        assert_eq!(report.candidate_limit, 32);
        assert_eq!(report.funnel_default_limit, 32);
        assert_eq!(report.dual_threshold, "0.000000001");
        assert!(report.run_fixed_integer && report.run_perturbation);
        assert!(report.run_target_analysis && report.run_conflict_analysis);
        assert_eq!(report.enumeration_limit, 10_000);
        assert!(!report.request_conflict && !report.shrink_conflict);
        assert_eq!(report.max_conflict_resolves, 128);
        assert_eq!(report.delta_count, 1);
        assert!(!report.has_adaptive);
        assert_eq!(report.max_solves, 32);
        assert_eq!(report.integrality_tolerance, "0.0000001");
        assert_eq!(report.dual_tolerance, "0.000000001");
        assert_eq!(report.validate_message, None);
        assert!(report.all_four_stage_flags_default_true);
        assert!(report.the_candidate_limit_is_snapshotted_from_the_funnel);
        assert!(report.a_zero_candidate_limit_is_legal);
        assert!(report.the_four_run_flags_are_never_validated);
        assert!(report.validation_runs_in_stage_order);
        assert!(report.the_pipeline_and_the_conflict_stage_carry_separate_cp_options);
        assert!(report.the_perturbation_policy_requires_exactly_one_source);
    }

    #[test]
    fn the_order_and_isolation_probes_report_the_funnel_and_both_cp_sets() {
        let report = verify_analysis_options(&request()).unwrap();
        assert!(report.order_probe_message.contains("dual threshold"), "{}",
            report.order_probe_message);
        assert!(report.conflict_only_probe_message.contains("enumeration limit"), "{}",
            report.conflict_only_probe_message);
        assert_eq!(report.conflict_only_probe_message, report.pipeline_only_probe_message);
    }

    #[test]
    fn a_broken_funnel_is_reported_and_a_zero_limit_is_not() {
        let report = verify_analysis_options(&AnalysisOptionsRequest {
            dual_threshold: Some(-1.0), ..request() }).unwrap();
        assert!(report.validate_message.as_deref().unwrap().contains("dual threshold"));
        let zero = verify_analysis_options(&AnalysisOptionsRequest { candidate_limit: Some(0),
            ..request() }).unwrap();
        assert_eq!(zero.candidate_limit, 0);
        assert_eq!(zero.validate_message, None);
    }

    #[test]
    fn the_perturbation_source_is_exclusive() {
        let both = verify_analysis_options(&AnalysisOptionsRequest { deltas: Some(vec![1.0]),
            adaptive: Some(true), ..request() }).unwrap();
        assert!(both.validate_message.as_deref().unwrap().contains("either explicit deltas"));
        let neither = verify_analysis_options(&AnalysisOptionsRequest { deltas: Some(Vec::new()),
            ..request() }).unwrap();
        assert_eq!(neither.validate_message, both.validate_message);
        assert_eq!(neither.delta_count, 0);
        let adaptive = verify_analysis_options(&AnalysisOptionsRequest { deltas: Some(Vec::new()),
            adaptive: Some(true), ..request() }).unwrap();
        assert_eq!(adaptive.validate_message, None);
        assert!(adaptive.has_adaptive);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_analysis_options(&AnalysisOptionsRequest {
            deltas: Some(vec![1.0; 17]), ..request() }).unwrap_err().message
            .contains("at most 16 deltas"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_analysis_options(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"candidateLimit\""), "{json}");
        assert!(json.contains("\"theCandidateLimitIsSnapshottedFromTheFunnel\""), "{json}");
        assert!(json.contains("\"runConflictAnalysis\""), "{json}");
        assert!(!json.contains("candidate_limit"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<AnalysisOptionsReport>(&copy.to_string()).is_err());
    }
}
