//! 续挖第 99 单：**统一分析报告的 `validate()` 不检查"总体结论"，红线只写在文档里**——报告可以把 `status` 报成
//! `Reachable`、`proven_optimal` 为真，而活动性/可行性/冲突各阶段**一个都没跑**，`validate()` 照样通过；
//! 它真正检查的是**字符串层面**的东西：**用 `Debug` 格式化整份报告再做禁用词子串扫描**（于是调用方自己的
//! 一句理由里出现 `solver_row` 也会被拒），空 schema/身份被拒，已证明最优必须有目标值，而**存在阶段会被逐个委托校验**；
//! 另外 `has_minimal_blocking_set()` 在没有冲突解释时恒为假，`blocking_summary()` 在没有冲突时给的是"未执行"
//! （`analysis/pipeline.rs` 的 `CriticalConstraintAnalysisReport`）
//! Continuation, delivery 99: **the unified analysis report's `validate()` never inspects the overall conclusion** — a
//! report may claim `Reachable` with `proven_optimal` true while the activity, feasibility and conflict stages **never ran**,
//! and validation still passes; what it does check is **textual**: it **formats the whole report with `Debug` and scans for
//! forbidden substrings** (so a caller's own reason mentioning `solver_row` is refused), refuses a blank schema or identity,
//! demands a value for a proven optimum, and **delegates to every stage that is present**; meanwhile
//! `has_minimal_blocking_set()` is false without a conflict explanation and `blocking_summary()` says "not run"
//! (`CriticalConstraintAnalysisReport` in `analysis/pipeline.rs`).
//!
//! 第 98 单量的是有效性排序，这一单量**统一分析报告**。八条实测：
//!
//! 1. **没有阶段也能带"已证明"的总体结论通过校验**（`an_empty_report_with_a_proven_status_still_validates()`）：
//!    文档里那条红线（阶段缺失时不得升格为已证明）**没有实现**。本单最值钱的一条。
//! 2. **`proven_optimal` 为真必须有目标值**（`a_proven_baseline_requires_a_value()`）。
//! 3. **禁用词扫描读的是整份报告的 `Debug` 文本**（`the_forbidden_word_scan_reads_every_string()`）：
//!    调用方写的一句 `unavailable_reason` 里带 `solver_row` 就被拒。
//! 4. **空 schema 与空身份被拒**（`blank_schema_and_baseline_fields_are_refused()`）。
//! 5. **存在的阶段会被逐个委托校验**（`the_report_delegates_to_each_present_stage()`）：
//!    一个坏的候选漏斗能让整份报告失效。
//! 6. **没有冲突解释就没有最小阻塞集合**（`a_missing_conflict_reports_no_minimal_blocking_set()`）。
//! 7. **没有冲突解释时摘要就是"未执行"**（`a_missing_conflict_summarises_as_not_run()`），文本逐字固定。
//! 8. **总体状态从不与阶段对照**（`the_status_is_never_checked_against_the_stages()`）：
//!    八个状态名都能配上"一个阶段都没有"的报告。
//!
//! Delivery 98 measured the effectiveness ranking; this one measures **the unified analysis report**. **A report with no
//! stages can still claim a proven conclusion** — the documented red line is not implemented, the most valuable finding
//! here. **A proven baseline requires a value.** **The forbidden-word scan reads the whole report's `Debug` text**, so a
//! caller's own reason mentioning `solver_row` is refused. **Blank schemas and identities are refused.** **Present stages
//! are delegated to.** **No conflict explanation means no minimal blocking set.** **Its summary is literally "not run".**
//! And **the overall status is never checked against the stages.**

use ospf_rust_core::analysis::pipeline::{
    AnalysisBaselineSummary, CandidateFunnelRanking, CriticalConstraintAnalysisReport,
};
use ospf_rust_core::analysis::AnalysisStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 统一分析报告探针请求。 / The unified-report probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalReportRequest {
    /// schema 版本（默认 `probe`）。 / The schema version (default `probe`).
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 目标身份（默认 `objective`）。 / The objective identity.
    #[serde(default)]
    pub objective_id: Option<String>,
    /// 优化方向（默认 `minimize`）。 / The optimisation sense.
    #[serde(default)]
    pub sense: Option<String>,
    /// 总体状态名（四个之一，默认 `Unknown`）。 / The overall status name, one of four.
    #[serde(default)]
    pub status: Option<String>,
    /// 基线是否已证明最优。 / Whether the baseline proved optimality.
    #[serde(default)]
    pub proven_optimal: Option<bool>,
    /// 基线目标值。 / The baseline objective value.
    #[serde(default)]
    pub optimal_value: Option<f64>,
    /// 未完成阶段的原因。 / Reasons stages did not complete.
    #[serde(default)]
    pub unavailable_reasons: Vec<String>,
    /// 注入一个**坏**的候选漏斗（空 schema）。 / Inject a **broken** funnel with a blank schema.
    #[serde(default)]
    pub inject_broken_funnel: Option<bool>,
}

/// 统一分析报告探针结论。 / The unified-report probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalReportVerdict {
    /// `validate()` 是否通过。 / Whether `validate()` passes.
    pub valid: bool,
    /// `has_minimal_blocking_set()` 的读数。 / What `has_minimal_blocking_set()` says.
    pub minimal_blocking_set: bool,
    /// `blocking_summary()` 的文本。 / What `blocking_summary()` says.
    pub blocking_summary: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub an_empty_report_with_a_proven_status_still_validates: bool,
    pub a_proven_baseline_requires_a_value: bool,
    pub the_forbidden_word_scan_reads_every_string: bool,
    pub blank_schema_and_baseline_fields_are_refused: bool,
    pub the_report_delegates_to_each_present_stage: bool,
    pub a_missing_conflict_reports_no_minimal_blocking_set: bool,
    pub a_missing_conflict_summarises_as_not_run: bool,
    pub the_status_is_never_checked_against_the_stages: bool,
}

/// 四个状态名。 / The four status names.
const STATUS_NAMES: [&str; 4] = ["Reachable", "Unreachable", "Unknown", "Unsupported"];

/// 按名字取状态。 / Resolve a status by name.
fn status_named(name: &str) -> Option<AnalysisStatus> {
    match name {
        "Reachable" => Some(AnalysisStatus::Reachable),
        "Unreachable" => Some(AnalysisStatus::Unreachable),
        "Unknown" => Some(AnalysisStatus::Unknown),
        "Unsupported" => Some(AnalysisStatus::Unsupported),
        _ => None,
    }
}

/// 报告里的"未执行"文本（逐字固定）。 / The "not run" text, which is fixed verbatim.
const NOT_RUN: &str = "阻塞分析未执行 / Blocking analysis was not run";

/// 造一份报告：默认不带任何阶段。 / Build a report; by default it carries no stage at all.
fn report_of(schema: &str, objective: &str, sense: &str, status: AnalysisStatus,
    proven_optimal: bool, optimal_value: Option<f64>, reasons: Vec<String>,
    funnel: Option<CandidateFunnelRanking>) -> CriticalConstraintAnalysisReport
{
    CriticalConstraintAnalysisReport {
        schema_version: schema.to_owned(),
        baseline: AnalysisBaselineSummary {
            objective_id: objective.to_owned(),
            sense: sense.to_owned(),
            optimal_value,
            status,
            proven_optimal,
        },
        activity: None,
        local_sensitivity: None,
        funnel,
        effectiveness: None,
        target: None,
        conflict: None,
        group_summaries: Vec::new(),
        status,
        unavailable_reasons: reasons,
    }
}

/// 走一遍统一报告探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_critical_report(request: &CriticalReportRequest) -> Result<CriticalReportVerdict> {
    if request.unavailable_reasons.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "critical report probe failed validation: \
            at most eight reasons are accepted"));
    }
    let schema = request.schema_version.clone().unwrap_or_else(|| "probe".to_owned());
    let objective = request.objective_id.clone().unwrap_or_else(|| "objective".to_owned());
    let sense = request.sense.clone().unwrap_or_else(|| "minimize".to_owned());
    let status = match request.status.as_deref() {
        None => AnalysisStatus::Unknown,
        Some(name) => status_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "critical report probe failed validation: unknown status name"))?,
    };
    let findings = canonical_findings();
    let funnel = if request.inject_broken_funnel.unwrap_or(false) {
        // 空 schema 的排序本身不合法，用来证明报告会委托校验。 
        // A ranking with a blank schema is invalid on its own, which proves the report delegates.
        Some(CandidateFunnelRanking { schema_version: String::new(), dual_threshold: 1e-9,
            candidates: Vec::new(), tier_a: 0, tier_b: 0, tier_c: 0, unclassified: 0 })
    } else {
        None
    };
    let report = report_of(&schema, &objective, &sense, status,
        request.proven_optimal.unwrap_or(false), request.optimal_value,
        request.unavailable_reasons.clone(), funnel);
    Ok(CriticalReportVerdict {
        valid: report.validate().is_ok(),
        minimal_blocking_set: report.has_minimal_blocking_set(),
        blocking_summary: report.blocking_summary(),
        an_empty_report_with_a_proven_status_still_validates:
            findings.an_empty_report_with_a_proven_status_still_validates,
        a_proven_baseline_requires_a_value: findings.a_proven_baseline_requires_a_value,
        the_forbidden_word_scan_reads_every_string:
            findings.the_forbidden_word_scan_reads_every_string,
        blank_schema_and_baseline_fields_are_refused:
            findings.blank_schema_and_baseline_fields_are_refused,
        the_report_delegates_to_each_present_stage:
            findings.the_report_delegates_to_each_present_stage,
        a_missing_conflict_reports_no_minimal_blocking_set:
            findings.a_missing_conflict_reports_no_minimal_blocking_set,
        a_missing_conflict_summarises_as_not_run: findings.a_missing_conflict_summarises_as_not_run,
        the_status_is_never_checked_against_the_stages:
            findings.the_status_is_never_checked_against_the_stages,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    an_empty_report_with_a_proven_status_still_validates: bool,
    a_proven_baseline_requires_a_value: bool,
    the_forbidden_word_scan_reads_every_string: bool,
    blank_schema_and_baseline_fields_are_refused: bool,
    the_report_delegates_to_each_present_stage: bool,
    a_missing_conflict_reports_no_minimal_blocking_set: bool,
    a_missing_conflict_summarises_as_not_run: bool,
    the_status_is_never_checked_against_the_stages: bool,
}

/// 一份"什么都没跑"的报告。 / A report where nothing ran.
fn empty(status: AnalysisStatus, proven_optimal: bool, optimal_value: Option<f64>)
    -> CriticalConstraintAnalysisReport
{
    report_of("probe", "objective", "minimize", status, proven_optimal, optimal_value,
        Vec::new(), None)
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 没有阶段也能带"已证明"的结论通过。 / No stages at all can still claim a proven conclusion.
        let an_empty_report_with_a_proven_status_still_validates = {
            let report = empty(AnalysisStatus::Reachable, true, Some(1.0));
            // 阶段全空。 / Every stage is absent.
            let stages_absent = report.activity.is_none() && report.target.is_none()
                && report.conflict.is_none() && report.funnel.is_none();
            stages_absent && report.validate().is_ok()
        };
        // 2) 已证明最优必须有目标值。 / A proven optimum requires a value.
        let a_proven_baseline_requires_a_value = {
            empty(AnalysisStatus::Reachable, true, None).validate().is_err()
                // 没证明最优就不需要值。 / Without the proof no value is needed.
                && empty(AnalysisStatus::Unknown, false, None).validate().is_ok()
        };
        // 3) 禁用词扫描读整份报告的 `Debug` 文本。 / The scan reads the whole report's `Debug` text.
        let the_forbidden_word_scan_reads_every_string = {
            // 调用方写的一句原因里带禁用词就被拒。 
            // A caller's own reason carrying a forbidden word is refused.
            let poisoned = report_of("probe", "objective", "minimize", AnalysisStatus::Unknown,
                false, None, vec!["the solver_row was hidden".to_owned()], None);
            poisoned.validate().is_err()
                // 对照：只用 "solver" 这个词没问题。 / Control: the bare word "solver" is fine.
                && report_of("probe", "objective", "minimize", AnalysisStatus::Unknown, false,
                    None, vec!["the solver was unavailable".to_owned()], None).validate().is_ok()
                // 其余三个禁用词同样被拒。 / The other three forbidden words are refused too.
                && ["solver_column", "auxiliary_variable", "native_handle"].iter().all(|word| {
                    report_of("probe", "objective", "minimize", AnalysisStatus::Unknown, false,
                        None, vec![(*word).to_owned()], None).validate().is_err()
                })
        };
        // 4) 空 schema 与空身份被拒。 / Blank schemas and identities are refused.
        let blank_schema_and_baseline_fields_are_refused = {
            report_of("  ", "objective", "minimize", AnalysisStatus::Unknown, false, None,
                Vec::new(), None).validate().is_err()
                && report_of("probe", "", "minimize", AnalysisStatus::Unknown, false, None,
                    Vec::new(), None).validate().is_err()
                && report_of("probe", "objective", " ", AnalysisStatus::Unknown, false, None,
                    Vec::new(), None).validate().is_err()
        };
        // 5) 存在的阶段会被委托校验。 / Present stages are delegated to.
        let the_report_delegates_to_each_present_stage = {
            let mut report = empty(AnalysisStatus::Unknown, false, None);
            report.funnel = Some(CandidateFunnelRanking { schema_version: String::new(),
                dual_threshold: 1e-9, candidates: Vec::new(), tier_a: 0, tier_b: 0, tier_c: 0,
                unclassified: 0 });
            // 报告本身没别的问题，坏漏斗就足以让它失效。 
            // Nothing else is wrong with the report; the broken funnel alone invalidates it.
            report.validate().is_err()
        };
        // 6) 没有冲突解释就没有最小阻塞集合。 / No conflict explanation means no minimal blocking set.
        let a_missing_conflict_reports_no_minimal_blocking_set = {
            !empty(AnalysisStatus::Unreachable, false, None).has_minimal_blocking_set()
        };
        // 7) 没有冲突解释时摘要逐字是"未执行"。 / Without a conflict the summary is literally "not run".
        let a_missing_conflict_summarises_as_not_run = {
            empty(AnalysisStatus::Unknown, false, None).blocking_summary() == NOT_RUN
                // 换成别的状态也一样。 / Another status gives the same text.
                && empty(AnalysisStatus::Unreachable, true, Some(0.0)).blocking_summary() == NOT_RUN
        };
        // 8) 总体状态从不与阶段对照。 / The overall status is never checked against the stages.
        let the_status_is_never_checked_against_the_stages = {
            // 四个状态名 × "一个阶段都没有" 全都通过。 
            // All four status names combined with "no stage at all" validate.
            STATUS_NAMES.iter().all(|name| {
                let status = status_named(name).expect("status name");
                empty(status, false, None).validate().is_ok()
            })
                // 而"什么都不缺"配 `Unsupported` 也通过。 
                // A fully populated report with `Unsupported` also passes.
                && { let mut full = empty(AnalysisStatus::Unsupported, false, None);
                    full.funnel = Some(CandidateFunnelRanking {
                        schema_version: "probe".to_owned(), dual_threshold: 1e-9,
                        candidates: Vec::new(), tier_a: 0, tier_b: 0, tier_c: 0,
                        unclassified: 0 });
                    full.validate().is_ok() }
        };
        Findings {
            an_empty_report_with_a_proven_status_still_validates,
            a_proven_baseline_requires_a_value,
            the_forbidden_word_scan_reads_every_string,
            blank_schema_and_baseline_fields_are_refused,
            the_report_delegates_to_each_present_stage,
            a_missing_conflict_reports_no_minimal_blocking_set,
            a_missing_conflict_summarises_as_not_run,
            the_status_is_never_checked_against_the_stages,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CriticalReportRequest {
        CriticalReportRequest { schema_version: Some("probe".to_owned()),
            objective_id: Some("objective".to_owned()), sense: Some("minimize".to_owned()),
            status: Some("Reachable".to_owned()), proven_optimal: Some(true),
            optimal_value: Some(1.0), unavailable_reasons: Vec::new(),
            inject_broken_funnel: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let verdict = verify_critical_report(&request()).unwrap();
        assert!(verdict.valid);
        assert!(verdict.an_empty_report_with_a_proven_status_still_validates);
        assert!(verdict.a_proven_baseline_requires_a_value);
        assert!(verdict.the_forbidden_word_scan_reads_every_string);
        assert!(verdict.blank_schema_and_baseline_fields_are_refused);
        assert!(verdict.the_report_delegates_to_each_present_stage);
        assert!(verdict.a_missing_conflict_reports_no_minimal_blocking_set);
        assert!(verdict.a_missing_conflict_summarises_as_not_run);
        assert!(verdict.the_status_is_never_checked_against_the_stages);
        // 没有冲突解释 → 没有最小阻塞集合，摘要逐字是"未执行"。 
        // Without a conflict there is no minimal blocking set and the summary is literally "not run".
        assert!(!verdict.minimal_blocking_set);
        assert_eq!(verdict.blocking_summary, NOT_RUN);
    }

    #[test]
    fn a_proven_baseline_without_a_value_is_refused() {
        let verdict = verify_critical_report(&CriticalReportRequest { optimal_value: None,
            ..request() }).unwrap();
        assert!(!verdict.valid);
        // 不声明已证明就不需要值。 / Without the claim no value is needed.
        let unproven = verify_critical_report(&CriticalReportRequest { proven_optimal: Some(false),
            optimal_value: None, ..request() }).unwrap();
        assert!(unproven.valid);
    }

    #[test]
    fn a_forbidden_word_in_the_callers_own_reason_is_refused() {
        for word in ["solver_row", "solver_column", "auxiliary_variable", "native_handle"] {
            let verdict = verify_critical_report(&CriticalReportRequest {
                unavailable_reasons: vec![format!("hidden {word}")], ..request() }).unwrap();
            assert!(!verdict.valid, "{word}");
        }
        // 裸的 "solver" 没问题。 / The bare word "solver" is fine.
        let allowed = verify_critical_report(&CriticalReportRequest {
            unavailable_reasons: vec!["the solver was unavailable".to_owned()], ..request() })
            .unwrap();
        assert!(allowed.valid);
    }

    #[test]
    fn blank_schema_and_identities_are_refused() {
        assert!(!verify_critical_report(&CriticalReportRequest {
            schema_version: Some("   ".to_owned()), ..request() }).unwrap().valid);
        assert!(!verify_critical_report(&CriticalReportRequest {
            objective_id: Some(String::new()), ..request() }).unwrap().valid);
        assert!(!verify_critical_report(&CriticalReportRequest {
            sense: Some(" ".to_owned()), ..request() }).unwrap().valid);
    }

    #[test]
    fn a_broken_stage_invalidates_the_whole_report() {
        let verdict = verify_critical_report(&CriticalReportRequest {
            inject_broken_funnel: Some(true), ..request() }).unwrap();
        assert!(!verdict.valid);
    }

    #[test]
    fn every_status_passes_with_no_stage_at_all() {
        for name in ["Reachable", "Unreachable", "Unknown", "Unsupported"] {
            let verdict = verify_critical_report(&CriticalReportRequest {
                status: Some(name.to_owned()), proven_optimal: Some(false),
                optimal_value: None, ..request() }).unwrap();
            assert!(verdict.valid, "{name}");
        }
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_critical_report(&CriticalReportRequest {
            unavailable_reasons: (0..9).map(|index| format!("r{index}")).collect(),
            ..request() }).unwrap_err().message.contains("at most eight reasons"));
        assert!(verify_critical_report(&CriticalReportRequest {
            status: Some("Nearly".to_owned()), ..request() }).unwrap_err()
            .message.contains("unknown status name"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let verdict = verify_critical_report(&request()).unwrap();
        let json = serde_json::to_string(&verdict).unwrap();
        assert!(json.contains("\"minimalBlockingSet\""), "{json}");
        assert!(json.contains("\"blockingSummary\""), "{json}");
        assert!(json.contains("\"anEmptyReportWithAProvenStatusStillValidates\""), "{json}");
        assert!(!json.contains("blocking_summary"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CriticalReportVerdict>(&copy.to_string()).is_err());
    }
}
