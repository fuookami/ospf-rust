//! 批次 B 第 4 单：关键约束总报告的校验与阻塞摘要 / Batch B, delivery 4: the aggregate report and its blocking summary.
//!
//! `CriticalConstraintAnalysisReport::validate()` 强制两条红线：报告不得携带 solver 生成物（`solver_row`、
//! `solver_column`、`auxiliary_variable`、`native_handle` 这些痕迹一律拒绝），并且缺失/未证明阶段不得把总体结论
//! 升格为已证明。`has_minimal_blocking_set()` 与 `blocking_summary()` 也都是纯数据方法，不需要求解器。
//! `CriticalConstraintAnalysisReport::validate()` enforces two red lines: the report must never carry solver-generated
//! artifacts (any trace of `solver_row`, `solver_column`, `auxiliary_variable`, `native_handle` is refused), and a
//! missing or unproven stage must not upgrade the overall conclusion to proven. `has_minimal_blocking_set()` and
//! `blocking_summary()` are pure-data methods too, needing no solver.

use ospf_rust_core::analysis::pipeline::CriticalConstraintAnalysisReport;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// baseline 投影。 / The baseline projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalBaseline {
    /// 目标身份。 / The objective identity.
    pub objective_id: String,
    /// 优化方向。 / The optimization direction.
    pub sense: String,
    /// 是否已证明最优。 / Whether optimality was proven.
    pub proven_optimal: bool,
    /// 已证明的基线目标值。 / The proven baseline objective value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub optimal_value: Option<f64>,
}

/// 哪些阶段真实存在（`false` 表示该阶段未执行，**不表示**该阶段无效）。
/// Which stages are actually present (`false` means the stage did not run; it does **not** mean it was ineffective).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalStagePresence {
    /// 是否有活动性阶段。 / Whether the activity stage is present.
    pub activity: bool,
    /// 是否有固定整数 LP 局部灵敏度阶段。 / Whether the fixed-integer LP stage is present.
    pub local_sensitivity: bool,
    /// 是否有候选漏斗。 / Whether the candidate funnel is present.
    pub funnel: bool,
    /// 是否有有效性排序。 / Whether the effectiveness ranking is present.
    pub effectiveness: bool,
    /// 是否有目标可行性阶段。 / Whether the target-feasibility stage is present.
    pub target: bool,
    /// 是否有冲突解释。 / Whether the conflict explanation is present.
    pub conflict: bool,
}

/// 关键约束总报告的校验结论与摘要。 / The verdict and summary of the aggregate report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalAnalysisReport {
    /// 报告 schema。 / The report schema.
    pub schema_version: String,
    /// 整条链路的总体结论。 / The overall conclusion for the whole chain.
    pub status: String,
    /// 基线投影。 / The baseline projection.
    pub baseline: CriticalBaseline,
    /// 阶段存在性。 / Stage presence.
    pub stages: CriticalStagePresence,
    /// 是否存在已证明的最小阻塞集合。 / Whether a proven minimal blocking set exists.
    pub has_minimal_blocking_set: bool,
    /// 业务可读的阻塞摘要（领域自己的 `blocking_summary`，中英双语）。
    /// The business-readable blocking summary (the domain's own bilingual `blocking_summary`).
    pub blocking_summary: String,
    /// 约束组汇总行数。 / The group-summary row count.
    pub group_summary_count: u32,
    /// 未完成阶段的原因。 / The reasons stages did not complete.
    pub unavailable_reasons: Vec<String>,
}

/// 校验一份关键约束总报告并给出摘要。校验失败即**错误**并原样带出领域原因。
/// Verify one aggregate report and summarize it. A validation failure is an **error** carrying the domain reason
/// verbatim.
pub fn verify_critical_analysis(json: &str) -> Result<CriticalAnalysisReport> {
    if json.len() > 32 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the analysis report exceeds 32 MiB"));
    }
    let report: CriticalConstraintAnalysisReport = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("analysis report is not decodable: {error}"))
    })?;
    report.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("analysis report failed validation: {error}"))
    })?;
    Ok(CriticalAnalysisReport {
        schema_version: report.schema_version.clone(),
        status: status_name(report.status).to_owned(),
        baseline: CriticalBaseline {
            objective_id: report.baseline.objective_id.clone(),
            sense: report.baseline.sense.clone(),
            proven_optimal: report.baseline.proven_optimal,
            optimal_value: report.baseline.optimal_value,
        },
        stages: CriticalStagePresence {
            activity: report.activity.is_some(),
            local_sensitivity: report.local_sensitivity.is_some(),
            funnel: report.funnel.is_some(),
            effectiveness: report.effectiveness.is_some(),
            target: report.target.is_some(),
            conflict: report.conflict.is_some(),
        },
        // 这两项都直接调用领域方法，不在桥接里重写"是否最小"或摘要措辞。
        // Both come straight from domain methods; neither the minimality predicate nor the wording is re-written here.
        has_minimal_blocking_set: report.has_minimal_blocking_set(),
        blocking_summary: report.blocking_summary(),
        group_summary_count: u32::try_from(report.group_summaries.len()).unwrap_or(u32::MAX),
        unavailable_reasons: report.unavailable_reasons.clone(),
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
    use ospf_rust_core::analysis::pipeline::{AnalysisBaselineSummary, CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION};
    use ospf_rust_core::analysis::{
        AnalysisStatus, ConflictExplanation, ConflictExtractionTier, ConflictGroupSummary,
        ConflictMinimality, ConflictValidity, DiagnosticSource, ObjectiveTarget, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};

    fn baseline(proven_optimal: bool, optimal_value: Option<f64>) -> AnalysisBaselineSummary {
        AnalysisBaselineSummary {
            objective_id: "objective".to_owned(),
            sense: "MINIMUM".to_owned(),
            optimal_value,
            status: AnalysisStatus::Unreachable,
            proven_optimal,
        }
    }

    /// 只有 baseline、其余阶段全缺的报告：**合法**，因为缺失阶段只表示为 `None`。
    /// A report with only the baseline and every other stage missing: **valid**, because a missing stage is only `None`.
    fn bare_report() -> CriticalConstraintAnalysisReport {
        CriticalConstraintAnalysisReport {
            schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
            baseline: baseline(false, None),
            activity: None, local_sensitivity: None, funnel: None, effectiveness: None,
            target: None, conflict: None,
            group_summaries: Vec::new(),
            status: AnalysisStatus::Unknown,
            unavailable_reasons: vec!["activity stage was not run".to_owned()],
        }
    }

    fn conflict_explanation(background_blocking: bool) -> ConflictExplanation {
        let target = ObjectiveTarget::at_least("objective", 5.0).expect("target");
        let members = if background_blocking {
            Vec::new()
        } else {
            vec![DiagnosticSource::Constraint { id: StableConstraintId("capacity".to_owned()) }]
        };
        ConflictExplanation {
            schema_version: ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION.to_owned(),
            target: target.clone(),
            target_feasibility: TargetFeasibilityReport {
                schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
                target: target.clone(),
                effective_integer_bound: None,
                source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
                status: AnalysisStatus::Unreachable, solution: None, exact_objective: None,
                objective_value: None, proof_status: ProofStatus::Verified, termination_reason: None,
                solve_time: None, target_fixed: true, message: None,
            },
            status: AnalysisStatus::Unreachable,
            available_evidence: Vec::new(),
            fixed_background: if background_blocking {
                vec![DiagnosticSource::Constraint { id: StableConstraintId("bound".to_owned()) }]
            } else {
                Vec::new()
            },
            members,
            target_source: DiagnosticSource::ObjectiveTarget { target },
            validity: ConflictValidity::Verified,
            // 背景阻塞不存在约束级最小集合，所以这里必须**不是** `Irreducible`——不可约结论要求至少一个阻塞成员，
            // 而背景阻塞的成员列表按定义是空的。
            // Background blocking has no constraint-level minimal set, so this must **not** be `Irreducible`: an
            // irreducible verdict requires at least one blocking member and the member list is empty by definition.
            minimality: if background_blocking { ConflictMinimality::NotChecked } else { ConflictMinimality::Irreducible },
            verifications: Vec::new(),
            group_summaries: if background_blocking {
                Vec::new()
            } else {
                vec![ConflictGroupSummary { group: Some("capacity".to_owned()), count: 1 }]
            },
            resolve_count: 2, memoized_checks: 0,
            extraction_tier: ConflictExtractionTier::AssumptionExtraction,
            unavailable_reason: None,
            background_blocking,
        }
    }

    fn encode(report: &CriticalConstraintAnalysisReport) -> String {
        serde_json::to_string(report).expect("serialize report")
    }

    #[test]
    fn bare_report_is_valid_and_says_blocking_was_not_run() {
        let summary = verify_critical_analysis(&encode(&bare_report())).unwrap();
        assert_eq!(summary.status, "unknown");
        assert!(!summary.has_minimal_blocking_set);
        // 没有冲突阶段：领域原话是"未执行"，而不是"没有阻塞"。
        // No conflict stage: the domain says "was not run", not "no blocking".
        assert!(summary.blocking_summary.contains("未执行"), "{}", summary.blocking_summary);
        assert!(summary.blocking_summary.contains("was not run"), "{}", summary.blocking_summary);
        assert_eq!(summary.stages, CriticalStagePresence {
            activity: false, local_sensitivity: false, funnel: false,
            effectiveness: false, target: false, conflict: false });
        assert_eq!(summary.unavailable_reasons, vec!["activity stage was not run".to_owned()]);
        assert_eq!(summary.group_summary_count, 0);
        assert_eq!(summary.baseline.proven_optimal, false);
        assert_eq!(summary.baseline.optimal_value, None);
    }

    #[test]
    fn blocking_summary_and_minimality_come_from_the_domain() {
        let mut with_conflict = bare_report();
        with_conflict.conflict = Some(conflict_explanation(false));
        with_conflict.status = AnalysisStatus::Unreachable;
        let summary = verify_critical_analysis(&encode(&with_conflict)).unwrap();
        assert!(summary.has_minimal_blocking_set, "irreducible + verified must count as minimal");
        assert!(summary.blocking_summary.contains("最小阻塞集合"), "{}", summary.blocking_summary);
        assert!(summary.blocking_summary.contains("capacity: 1"), "{}", summary.blocking_summary);
        assert_eq!(summary.stages.conflict, true);

        // 背景阻塞：摘要必须点名"固定背景"，且不存在约束级最小集合。
        // Background blocking: the summary must name the fixed background and state that no constraint-level set exists.
        let mut background = bare_report();
        background.conflict = Some(conflict_explanation(true));
        background.status = AnalysisStatus::Unreachable;
        let summary = verify_critical_analysis(&encode(&background)).unwrap();
        assert!(summary.blocking_summary.contains("固定背景"), "{}", summary.blocking_summary);
        assert!(summary.blocking_summary.contains("不存在约束级最小阻塞集合"),
            "{}", summary.blocking_summary);
        // 背景阻塞下 `has_minimal_blocking_set` **为假**：没有成员就不可能有约束级最小集合。
        // 这不是"没证明"，而是"这类阻塞根本不存在约束级集合"。
        // With background blocking `has_minimal_blocking_set` is **false**: no members means no constraint-level
        // minimal set. This is not "unproven" — such blocking simply has no constraint-level set at all.
        assert!(!summary.has_minimal_blocking_set);
    }

    #[test]
    fn broken_reports_are_refused_with_the_domain_reason() {
        let cases: Vec<(&str, Box<dyn Fn(&mut CriticalConstraintAnalysisReport)>, &str)> = vec![
            ("blank schema", Box::new(|report: &mut CriticalConstraintAnalysisReport| {
                report.schema_version = "  ".to_owned();
            }), "schema version"),
            ("blank objective identity", Box::new(|report: &mut CriticalConstraintAnalysisReport| {
                report.baseline.objective_id = "   ".to_owned();
            }), "objective identity"),
            ("blank sense", Box::new(|report: &mut CriticalConstraintAnalysisReport| {
                report.baseline.sense = String::new();
            }), "objective identity"),
            ("proven baseline without a value", Box::new(|report: &mut CriticalConstraintAnalysisReport| {
                report.baseline = baseline(true, None);
            }), "requires an objective value"),
            ("solver-generated evidence", Box::new(|report: &mut CriticalConstraintAnalysisReport| {
                report.unavailable_reasons.push("leaked solver_row handle".to_owned());
            }), "must not expose solver-generated evidence"),
            ("invalid nested conflict", Box::new(|report: &mut CriticalConstraintAnalysisReport| {
                let mut explanation = conflict_explanation(false);
                explanation.members.clear();
                report.conflict = Some(explanation);
            }), "at least one blocking"),
        ];
        for (label, mutate, expected) in cases {
            let mut broken = bare_report();
            mutate(&mut broken);
            let error = match verify_critical_analysis(&encode(&broken)) {
                Ok(_) => panic!("case `{label}` must be refused"),
                Err(error) => error,
            };
            assert_eq!(error.code, "InvalidInput", "{label}");
            assert!(error.message.contains(expected), "{label}: {}", error.message);
        }
        assert_eq!(verify_critical_analysis("not json").unwrap_err().code, "InvalidInput");
        assert_eq!(verify_critical_analysis(&"x".repeat(33 * 1024 * 1024)).unwrap_err().code,
            "InvalidInput");
    }
}
