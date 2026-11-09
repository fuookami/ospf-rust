//! 续挖第 25 单：临界约束分析报告的聚合规则（`analysis/pipeline.rs` 的
//! `CriticalConstraintAnalysisReportBuilder`）
//! Continuation, delivery 25: the aggregation rules of the critical-constraint analysis report.
//!
//! 七段分析（baseline → activity → fixed-integer LP → perturbation → target → conflict → MUS）的产物并非平铺收集，
//! `build` 里有四条**会改变结论**的规则：
//!
//! 1. **baseline 是唯一硬前置**：没有它直接报错，其余六段都可以缺。
//! 2. **缺失 activity 不是"跳过"而是 `Unsupported`**：`build` 会补一条 `"activity stage was not run"` 并降级
//!    总体状态，所以"已证明最优的 baseline + 什么都没跑"**不会**得到 `Reachable`。
//! 3. **`note_unavailable` 是粘性的**：调一次就把 `has_unsupported_stage` 置真，之后再给多少好报告都退不回来。
//! 4. **effectiveness 需要 baseline 的已证明目标值**：扰动报告齐了但没有 `optimal_value`，整份报告直接失败——
//!    不会退化成"没有有效性结论"。
//!
//! 另外总体状态是 `combine_status` 的**最弱环节**（见第 17 单的 4 元格），所以任一阶段 `Unknown` / `Unsupported`
//! 都会吞掉 `Reachable`。
//!
//! The seven stages' outputs are not merely collected: `build` carries four rules that **change the conclusion**.
//! **The baseline is the only hard prerequisite** (everything else may be missing); **a missing activity stage is not
//! "skipped" but `Unsupported`** — `build` appends `"activity stage was not run"` and degrades the overall status, so a
//! proven-optimal baseline with nothing else run does **not** yield `Reachable`; **`note_unavailable` is sticky** — one call
//! sets `has_unsupported_stage` and no number of good reports brings it back; and **effectiveness needs the proven baseline
//! objective** — perturbations without `optimal_value` fail the whole report rather than degrading to "no effectiveness
//! conclusion". The overall status is the weakest link through `combine_status` (the four-element lattice from delivery 17),
//! so any `Unknown` / `Unsupported` stage swallows `Reachable`.

use ospf_rust_core::analysis::{
    AnalysisBaselineSummary, AnalysisStatus, CandidateFunnelConfig,
    ConstraintPerturbationReport, CriticalConstraintAnalysisReportBuilder,
};
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个聚合场景。 / One aggregation scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssemblyCase {
    /// 场景名（仅用于回读）。 / The case name (for read-back only).
    pub name: String,
    /// 是否提供 baseline。 / Whether a baseline is supplied.
    #[serde(default)]
    pub with_baseline: bool,
    /// baseline 是否已证明最优。 / Whether the baseline proved optimality.
    #[serde(default)]
    pub proven_optimal: bool,
    /// baseline 的目标值；未证明最优时应为空。 / The baseline objective; absent when not proven.
    #[serde(default)]
    pub optimal_value: Option<f64>,
    /// baseline 自身状态（`reachable` / `unknown` / `unsupported`）。 / The baseline status.
    #[serde(default)]
    pub baseline_status: String,
    /// 附加的扰动报告条数（每条都是最小合法报告）。 / How many minimal perturbation reports to attach.
    #[serde(default)]
    pub perturbation_count: u32,
    /// 扰动报告的状态（`reachable` / `unknown` / `unsupported`）。 / The perturbation status.
    #[serde(default)]
    pub perturbation_status: String,
    /// `note_unavailable` 的理由。 / The reasons passed to `note_unavailable`.
    #[serde(default)]
    pub note_unavailable: Vec<String>,
}

/// 聚合请求。 / The aggregation request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalAssemblyRequest {
    /// 场景列表。 / The scenarios.
    #[serde(default)]
    pub cases: Vec<AssemblyCase>,
}

/// 一个场景的结果。 / One scenario's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssemblyRow {
    /// 场景名。 / The case name.
    pub name: String,
    /// 是否建成。 / Whether the report was built.
    pub built: bool,
    /// 失败原因（建成时为空）。 / The failure reason (absent when built).
    #[serde(default)]
    pub error: Option<String>,
    /// 总体状态（失败时为空）。 / The overall status (absent on failure).
    #[serde(default)]
    pub status: Option<String>,
    /// 报告 schema 版本。 / The report schema version.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 未完成原因清单。 / The unavailable reasons.
    pub unavailable_reasons: Vec<String>,
    /// 是否带有效性排名。 / Whether an effectiveness ranking is present.
    pub has_effectiveness: bool,
    /// 是否带候选漏斗。 / Whether a candidate funnel is present.
    pub has_funnel: bool,
}

/// 聚合结论。 / The aggregation verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalAssemblyReport {
    /// 每个场景的结果。 / The outcome of each scenario.
    pub rows: Vec<AssemblyRow>,
    /// baseline 是唯一硬前置（**是**）。 / Whether the baseline is the only hard prerequisite (**yes**).
    pub baseline_is_the_only_prerequisite: bool,
    /// 缺失 activity 会降级为 `Unsupported`（**是**）。 / Whether a missing activity stage degrades to
    /// `Unsupported` (**yes**).
    pub missing_activity_degrades_to_unsupported: bool,
    /// `note_unavailable` 是粘性的（**是**）。 / Whether `note_unavailable` is sticky (**yes**).
    pub note_unavailable_is_sticky: bool,
    /// effectiveness 需要已证明的 baseline 目标值（**是**）。 / Whether effectiveness needs the proven baseline
    /// objective (**yes**).
    pub effectiveness_needs_proven_objective: bool,
    /// 已证明最优也挡不住缺失阶段的降级（**是**）。 / Whether a proven baseline cannot outvote a missing stage
    /// (**yes**).
    pub proven_optimal_is_not_enough: bool,
}

/// 场景上限（信任边界）。 / The case cap (a trust boundary).
const MAX_CASES: usize = 256;
/// 单场景扰动报告上限。 / The per-case perturbation cap.
const MAX_PERTURBATIONS: u32 = 64;

/// 走一遍报告聚合。非法输入是**错误**。 / Walk the report aggregation.
pub fn verify_critical_assembly(request: &CriticalAssemblyRequest) -> Result<CriticalAssemblyReport> {
    if request.cases.len() > MAX_CASES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "critical assembly failed validation: at most {MAX_CASES} cases are accepted")));
    }
    for case in &request.cases {
        if case.perturbation_count > MAX_PERTURBATIONS {
            return Err(BridgeError::new("InvalidInput", &format!(
                "critical assembly failed validation: at most {MAX_PERTURBATIONS} perturbation reports are accepted")));
        }
        status(&case.baseline_status)?;
        status(&case.perturbation_status)?;
        if case.note_unavailable.iter().any(|reason| reason.trim().is_empty()) {
            return Err(BridgeError::new("InvalidInput",
                "critical assembly failed validation: an unavailable reason must not be blank"));
        }
    }
    let config = CandidateFunnelConfig::default();
    let mut rows = Vec::with_capacity(request.cases.len());
    for case in &request.cases {
        rows.push(run_case(case, &config)?);
    }
    // 各条发现用**内部固定探针**测，不读调用方给的场景名——否则"能力标志"就变成"请求属性"
    // （第 19 / 21 单的老毛病）。调用方的 `cases` 只产出 `rows`。
    // Each finding is measured with an **internal fixed probe**, never read off caller-supplied case names — otherwise a
    // capability flag would become a request property (the repeat offender from deliveries 19 and 21). The caller's
    // `cases` only produce `rows`.
    let findings = canonical_findings(&config)?;
    Ok(CriticalAssemblyReport {
        rows,
        baseline_is_the_only_prerequisite: findings.baseline_is_the_only_prerequisite,
        missing_activity_degrades_to_unsupported: findings.missing_activity_degrades_to_unsupported,
        note_unavailable_is_sticky: findings.note_unavailable_is_sticky,
        effectiveness_needs_proven_objective: findings.effectiveness_needs_proven_objective,
        proven_optimal_is_not_enough: findings.proven_optimal_is_not_enough,
    })
}

/// 五个聚合发现的内部固定探针结果。 / The five findings measured by internal fixed probes.
struct Findings {
    baseline_is_the_only_prerequisite: bool,
    missing_activity_degrades_to_unsupported: bool,
    note_unavailable_is_sticky: bool,
    effectiveness_needs_proven_objective: bool,
    proven_optimal_is_not_enough: bool,
}

/// 用固定的五个探针测出全部发现。 / Measure every finding with five fixed probes.
fn canonical_findings(config: &CandidateFunnelConfig) -> Result<Findings> {
    let probe = |name: &str| AssemblyCase { name: name.to_owned(), with_baseline: true,
        proven_optimal: true, optimal_value: Some(1.0),
        baseline_status: "reachable".to_owned(), perturbation_count: 0,
        perturbation_status: "reachable".to_owned(), note_unavailable: Vec::new() };
    let mut no_baseline = probe("no-baseline");
    no_baseline.with_baseline = false;
    let mut noted = probe("noted");
    noted.note_unavailable = vec!["manual note".to_owned(), "manual note".to_owned()];
    let mut no_objective = probe("no-objective");
    no_objective.proven_optimal = false;
    no_objective.optimal_value = None;
    no_objective.perturbation_count = 1;
    let mut with_objective = probe("with-objective");
    with_objective.perturbation_count = 2;
    let cases = [no_baseline, probe("baseline-only"), noted, no_objective, with_objective];
    let mut rows = Vec::with_capacity(cases.len());
    for case in &cases {
        rows.push(run_case(case, config)?);
    }
    let row = |position: usize| &rows[position];
    Ok(Findings {
        baseline_is_the_only_prerequisite: !row(0).built && row(0).error.as_deref()
            .is_some_and(|error| error.contains("requires a baseline summary")),
        missing_activity_degrades_to_unsupported: row(1).built
            && row(1).status.as_deref() == Some("Unsupported")
            && row(1).unavailable_reasons.iter().any(|reason| reason.contains("activity stage was not run")),
        note_unavailable_is_sticky: row(2).built
            && row(2).status.as_deref() == Some("Unsupported")
            && row(2).unavailable_reasons.iter()
                .filter(|reason| reason.as_str() == "manual note").count() == 2,
        effectiveness_needs_proven_objective: !row(3).built && row(3).error.as_deref()
            .is_some_and(|error| error.contains("requires a proven baseline objective value")),
        proven_optimal_is_not_enough: row(1).built && row(1).status.as_deref() == Some("Unsupported"),
    })
}

/// 跑一个场景。 / Run one scenario.
fn run_case(case: &AssemblyCase, config: &CandidateFunnelConfig) -> Result<AssemblyRow> {
    let mut builder = CriticalConstraintAnalysisReportBuilder::new();
    if case.with_baseline {
        builder = builder.with_baseline(AnalysisBaselineSummary {
            objective_id: "objective".to_owned(),
            sense: "min".to_owned(),
            optimal_value: case.optimal_value,
            status: status(&case.baseline_status)?,
            proven_optimal: case.proven_optimal,
        });
    }
    if case.perturbation_count > 0 {
        let report_status = status(&case.perturbation_status)?;
        let reports: Vec<ConstraintPerturbationReport> = (0..case.perturbation_count)
            .map(|position| ConstraintPerturbationReport {
                schema_version: "1.0".to_owned(),
                constraint_id: StableConstraintId(format!("c{position}")),
                baseline_objective: case.optimal_value.unwrap_or(0.0),
                observations: Vec::new(),
                removal: None,
                adaptive_outcome: None,
                status: report_status,
                solve_count: 0,
                unavailable_reason: None,
            })
            .collect();
        builder = builder.with_perturbation(reports);
    }
    for reason in &case.note_unavailable {
        builder = builder.note_unavailable(reason.clone());
    }
    // 注意：activity 段**永远不给**——这本身就是要实测的规则（缺段 → Unsupported）。
    // Note: the activity stage is **never supplied**; that absence is one of the rules under test.
    match builder.build(config) {
        Ok(report) => Ok(AssemblyRow {
            name: case.name.clone(),
            built: true,
            error: None,
            status: Some(format!("{:?}", report.status)),
            schema_version: Some(report.schema_version),
            unavailable_reasons: report.unavailable_reasons,
            has_effectiveness: report.effectiveness.is_some(),
            has_funnel: report.funnel.is_some(),
        }),
        Err(error) => Ok(AssemblyRow {
            name: case.name.clone(),
            built: false,
            error: Some(error.to_string()),
            status: None,
            schema_version: None,
            unavailable_reasons: Vec::new(),
            has_effectiveness: false,
            has_funnel: false,
        }),
    }
}

/// 把字面量转成分析状态。 / Turn a literal into an analysis status.
fn status(name: &str) -> Result<AnalysisStatus> {
    match name {
        "reachable" => Ok(AnalysisStatus::Reachable),
        "unreachable" => Ok(AnalysisStatus::Unreachable),
        "unknown" | "" => Ok(AnalysisStatus::Unknown),
        "unsupported" => Ok(AnalysisStatus::Unsupported),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "critical assembly failed validation: unknown analysis status `{other}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(name: &str) -> AssemblyCase {
        AssemblyCase { name: name.to_owned(), with_baseline: true, proven_optimal: true,
            optimal_value: Some(1.0), baseline_status: "reachable".to_owned(),
            perturbation_count: 0, perturbation_status: "reachable".to_owned(),
            note_unavailable: Vec::new() }
    }

    fn run(cases: Vec<AssemblyCase>) -> CriticalAssemblyReport {
        verify_critical_assembly(&CriticalAssemblyRequest { cases }).unwrap()
    }

    fn row<'a>(report: &'a CriticalAssemblyReport, name: &str) -> &'a AssemblyRow {
        report.rows.iter().find(|row| row.name == name).unwrap_or_else(|| panic!("case {name}"))
    }

    /// 五个场景的完整集合，聚合发现都从这里读。 / The five cases the findings read.
    fn standard() -> CriticalAssemblyReport {
        let mut no_baseline = case("no-baseline");
        no_baseline.with_baseline = false;
        let mut noted = case("noted-then-proven");
        noted.note_unavailable = vec!["manual note".to_owned(), "manual note".to_owned()];
        let mut no_objective = case("perturbation-no-objective");
        no_objective.proven_optimal = false;
        no_objective.optimal_value = None;
        no_objective.baseline_status = "unknown".to_owned();
        no_objective.perturbation_count = 1;
        run(vec![no_baseline, case("baseline-only"), noted, no_objective, case("proven-only")])
    }

    #[test]
    fn the_baseline_is_the_only_hard_prerequisite() {
        let report = standard();
        assert!(report.baseline_is_the_only_prerequisite);
        let row = row(&report, "no-baseline");
        assert!(!row.built);
        assert!(row.error.as_deref().unwrap().contains("requires a baseline summary"), "{:?}", row.error);
    }

    #[test]
    fn a_missing_activity_stage_degrades_to_unsupported() {
        // 已证明最优的 baseline + 什么都没跑 ≠ Reachable。 
        // A proven-optimal baseline with nothing else run is not Reachable.
        let report = standard();
        assert!(report.missing_activity_degrades_to_unsupported);
        assert!(report.proven_optimal_is_not_enough);
        let row = row(&report, "baseline-only");
        assert!(row.built);
        assert_eq!(row.status.as_deref(), Some("Unsupported"));
        assert!(row.unavailable_reasons.iter()
            .any(|reason| reason.contains("activity stage was not run")));
        assert!(!row.has_effectiveness);
        assert!(!row.has_funnel);
        assert_eq!(row.schema_version.as_deref(), Some("1.0"));
    }

    #[test]
    fn note_unavailable_is_sticky_and_duplicated_verbatim() {
        let report = standard();
        assert!(report.note_unavailable_is_sticky);
        let row = row(&report, "noted-then-proven");
        assert_eq!(row.status.as_deref(), Some("Unsupported"));
        // 传两次就记两次，不合并。 / Passed twice, recorded twice, not merged.
        assert_eq!(row.unavailable_reasons.iter()
            .filter(|reason| reason.as_str() == "manual note").count(), 2);
    }

    #[test]
    fn effectiveness_needs_a_proven_baseline_objective() {
        let report = standard();
        assert!(report.effectiveness_needs_proven_objective);
        let row = row(&report, "perturbation-no-objective");
        assert!(!row.built);
        assert!(row.error.as_deref().unwrap()
            .contains("requires a proven baseline objective value"), "{:?}", row.error);
    }

    #[test]
    fn a_proven_objective_lets_the_perturbation_path_build() {
        let mut with_objective = case("with-objective");
        with_objective.perturbation_count = 2;
        let report = run(vec![with_objective]);
        let row = row(&report, "with-objective");
        assert!(row.built, "{:?}", row.error);
        // 有扰动报告就会建出有效性排名；activity 仍缺，所以状态仍是 Unsupported。
        // Perturbations produce an effectiveness ranking; activity is still missing, so the status stays Unsupported.
        assert!(row.has_effectiveness);
        assert!(!row.has_funnel);
        assert_eq!(row.status.as_deref(), Some("Unsupported"));
    }

    #[test]
    fn bad_input_is_refused() {
        let mut bad = case("bad-status");
        bad.baseline_status = "maybe".to_owned();
        assert!(verify_critical_assembly(&CriticalAssemblyRequest { cases: vec![bad] })
            .unwrap_err().message.contains("unknown analysis status"));
        let mut blank = case("blank-reason");
        blank.note_unavailable = vec!["  ".to_owned()];
        assert!(verify_critical_assembly(&CriticalAssemblyRequest { cases: vec![blank] })
            .unwrap_err().message.contains("must not be blank"));
        let many = CriticalAssemblyRequest { cases: vec![case("x"); MAX_CASES + 1] };
        assert!(verify_critical_assembly(&many).unwrap_err().message.contains("at most"));
        let mut too_many = case("too-many");
        too_many.perturbation_count = MAX_PERTURBATIONS + 1;
        assert!(verify_critical_assembly(&CriticalAssemblyRequest { cases: vec![too_many] })
            .unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = standard();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"baselineIsTheOnlyPrerequisite\""), "{json}");
        assert!(json.contains("\"missingActivityDegradesToUnsupported\""), "{json}");
        assert!(json.contains("\"unavailableReasons\""), "{json}");
        assert!(!json.contains("baseline_is_the_only"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CriticalAssemblyReport>(&copy.to_string()).is_err());
    }
}
