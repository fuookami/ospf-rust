//! 续挖第 98 单：**有效性排序的"排序"从不被校验**——`EffectivenessRanking` 的文档说"按已证明的最大改善降序排列"，
//! 而 `validate()` 只看身份、有限性与两条前提，**完全不看顺序**；`is_globally_effective` 要求 `max_improvement > 0.0`
//! （`Some(0.0)` 与负数都算"没有全局收益"，不报错）；局部有效**只认 `Some(true)`**；`first_effective_delta` 必须**为正**；
//! "有局部结论却没有对偶"与"`Unsupported` 却给了单删结论"被拒绝；而 **`unavailable_reason` 与 `status` 之间没有任何
//! 约束**（`Reachable` 带原因、`Unsupported` 不带原因都合法）
//! （`analysis/pipeline.rs` 的 `EffectivenessEntry` / `EffectivenessRanking`）
//! Continuation, delivery 98: **the effectiveness ranking's "ranking" is never validated** — the doc claims entries are
//! ordered by proven maximum improvement while `validate()` only inspects identities, finiteness and two preconditions and
//! **never the order**; `is_globally_effective` requires `max_improvement > 0.0` (so `Some(0.0)` and negatives both read as
//! "no global benefit" without an error); local effectiveness **only accepts `Some(true)`**; `first_effective_delta` must be
//! **positive**; "a local verdict without a dual" and "a removal verdict under `Unsupported`" are refused; and
//! **`unavailable_reason` is tied to nothing** (a `Reachable` entry may carry a reason and an `Unsupported` one may omit it)
//! (`EffectivenessEntry` / `EffectivenessRanking` in `analysis/pipeline.rs`).
//!
//! 第 97 单量的是候选漏斗，这一单量**有效性排序**（同一点名的另一半）。八条实测：
//!
//! 1. **全局有效性要 `max_improvement > 0.0`**（`global_effectiveness_needs_a_positive_improvement()`）：
//!    `Some(0.0)`、负数、`None` 都是"没有收益"，且**不报错**。
//! 2. **局部有效只认 `Some(true)`**（`local_effectiveness_needs_exactly_some_true()`）。
//! 3. **顺序从不被校验**（`the_order_of_entries_is_never_validated()`）：把 `max_improvement` 排成升序照样通过。本单最值钱的一条。
//! 4. **`first_effective_delta` 必须为正**（`the_first_delta_must_be_positive()`）：0 与负数都被拒。
//! 5. **有局部结论必须有对偶**（`a_local_verdict_requires_a_dual()`）。
//! 6. **单删结论不能挂在 `Unsupported` 上**（`a_removal_verdict_requires_a_supported_status()`）。
//! 7. **`unavailable_reason` 与 `status` 之间没有约束**（`the_reason_is_never_tied_to_the_status()`）。
//! 8. **身份为空或重复会被拒**（`blank_and_duplicate_identities_are_refused()`）。
//!
//! Delivery 97 measured the candidate funnel; this one measures **the effectiveness ranking**, the other half of the same
//! audit entry. **Global effectiveness needs a positive improvement** and zero, negatives and `None` all read as "no
//! benefit" without an error. **Local effectiveness only accepts `Some(true)`.** **The order is never validated** — the
//! most valuable finding here. **`first_effective_delta` must be positive.** **A local verdict requires a dual** and **a
//! removal verdict cannot sit under `Unsupported`.** **`unavailable_reason` is tied to nothing.** And **blank or duplicate
//! identities are refused.**

use ospf_rust_core::analysis::pipeline::{CandidateTier, EffectivenessEntry, EffectivenessRanking};
use ospf_rust_core::analysis::{ActivityStatus, AnalysisStatus};
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条有效性条目的输入。 / One effectiveness entry's input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessProbeSpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 局部对偶值。 / The local dual value.
    #[serde(default)]
    pub dual: Option<f64>,
    /// 局部有效性结论。 / The local effectiveness verdict.
    #[serde(default)]
    pub local_effective: Option<bool>,
    /// 单删结论文。 / The removal verdict.
    #[serde(default)]
    pub removal_effective: Option<bool>,
    /// 首次观察到改善的 delta。 / The first delta that improved.
    #[serde(default)]
    pub first_effective_delta: Option<f64>,
    /// 已证明的最大目标改善。 / The largest proven objective improvement.
    #[serde(default)]
    pub max_improvement: Option<f64>,
    /// 每单位 delta 的边际效应。 / The marginal effect per unit delta.
    #[serde(default)]
    pub marginal_effect: Option<f64>,
    /// 分析状态名（四个之一，默认 `Unknown`）。 / The analysis status name, one of four.
    #[serde(default)]
    pub status: Option<String>,
    /// 未形成结论的原因。 / Why no conclusion was formed.
    #[serde(default)]
    pub unavailable_reason: Option<String>,
}

/// 一条有效性条目的读数。 / One effectiveness entry's reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessProbeRow {
    /// 稳定身份。 / The stable identity.
    pub id: String,
    /// `is_globally_effective()` 的读数。 / What `is_globally_effective()` says.
    pub globally_effective: bool,
    /// 是否出现在 `globally_effective()` 里。 / Whether it appears in `globally_effective()`.
    pub in_global_list: bool,
    /// 是否出现在 `locally_effective()` 里。 / Whether it appears in `locally_effective()`.
    pub in_local_list: bool,
}

/// 有效性排序探针请求。 / The effectiveness-ranking probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessProbeRequest {
    /// baseline 目标值（默认 0）。 / The baseline objective (default zero).
    #[serde(default)]
    pub baseline_objective: Option<f64>,
    /// schema 版本（默认 `probe`）。 / The schema version (default `probe`).
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 条目列表（顺序即给出的顺序）。 / The entries, in the order given.
    #[serde(default)]
    pub entries: Vec<EffectivenessProbeSpec>,
}

/// 有效性排序探针结论。 / The effectiveness-ranking probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessProbeReport {
    /// 每条条目的读数。 / Every entry's reading.
    pub rows: Vec<EffectivenessProbeRow>,
    /// 全局有效的身份（按给出顺序）。 / The globally effective identities, in the order given.
    pub globally_effective: Vec<String>,
    /// 局部有效的身份（按给出顺序）。 / The locally effective identities, in the order given.
    pub locally_effective: Vec<String>,
    /// 给出的排序是否通过 `validate()`。 / Whether the given ranking passes `validate()`.
    pub ranking_valid: bool,
    /// 把条目顺序颠倒之后是否仍通过。 / Whether the reversed order also passes.
    pub reversed_still_valid: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub global_effectiveness_needs_a_positive_improvement: bool,
    pub local_effectiveness_needs_exactly_some_true: bool,
    pub the_order_of_entries_is_never_validated: bool,
    pub the_first_delta_must_be_positive: bool,
    pub a_local_verdict_requires_a_dual: bool,
    pub a_removal_verdict_requires_a_supported_status: bool,
    pub the_reason_is_never_tied_to_the_status: bool,
    pub blank_and_duplicate_identities_are_refused: bool,
}

/// 按名字取分析状态。 / Resolve an analysis status by name.
fn status_named(name: &str) -> Option<AnalysisStatus> {
    match name {
        "Reachable" => Some(AnalysisStatus::Reachable),
        "Unreachable" => Some(AnalysisStatus::Unreachable),
        "Unknown" => Some(AnalysisStatus::Unknown),
        "Unsupported" => Some(AnalysisStatus::Unsupported),
        _ => None,
    }
}

/// 把一条输入铺成条目。 / Lay one input out as an entry.
fn entry_of(spec: &EffectivenessProbeSpec) -> Result<EffectivenessEntry> {
    let status = match spec.status.as_deref() {
        None => AnalysisStatus::Unknown,
        Some(name) => status_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "effectiveness probe failed validation: unknown analysis status"))?,
    };
    Ok(EffectivenessEntry {
        constraint_id: StableConstraintId(spec.id.clone()),
        group: None,
        tier: CandidateTier::Unclassified,
        activity: ActivityStatus::Unknown,
        dual_value: spec.dual,
        local_effective: spec.local_effective,
        removal_effective: spec.removal_effective,
        first_effective_delta: spec.first_effective_delta,
        max_improvement: spec.max_improvement,
        marginal_effect: spec.marginal_effect,
        status,
        unavailable_reason: spec.unavailable_reason.clone(),
    })
}

/// 造一份排序。 / Build one ranking.
fn ranking_of(schema: &str, baseline: f64, entries: Vec<EffectivenessEntry>)
    -> EffectivenessRanking
{
    EffectivenessRanking { schema_version: schema.to_owned(), baseline_objective: baseline,
        entries }
}

/// 走一遍有效性排序探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_effectiveness_probe(request: &EffectivenessProbeRequest) -> Result<EffectivenessProbeReport> {
    if request.entries.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "effectiveness probe failed validation: \
            at most eight entries are accepted"));
    }
    let baseline = request.baseline_objective.unwrap_or(0.0);
    if !baseline.is_finite() {
        return Err(BridgeError::new("InvalidInput", "effectiveness probe failed validation: \
            the baseline objective must be finite"));
    }
    let schema = request.schema_version.clone().unwrap_or_else(|| "probe".to_owned());
    let findings = canonical_findings();
    let entries = request.entries.iter().map(entry_of).collect::<Result<Vec<_>>>()?;
    let ranking = ranking_of(&schema, baseline, entries.clone());
    let ranking_valid = ranking.validate().is_ok();
    let mut reversed = entries.clone();
    reversed.reverse();
    let reversed_still_valid = ranking_of(&schema, baseline, reversed).validate().is_ok();
    let globally: Vec<String> = ranking.globally_effective().iter()
        .map(|entry| entry.constraint_id.0.clone()).collect();
    let locally: Vec<String> = ranking.locally_effective().iter()
        .map(|entry| entry.constraint_id.0.clone()).collect();
    let rows = entries.iter().map(|entry| EffectivenessProbeRow {
        id: entry.constraint_id.0.clone(),
        globally_effective: entry.is_globally_effective(),
        in_global_list: globally.contains(&entry.constraint_id.0),
        in_local_list: locally.contains(&entry.constraint_id.0),
    }).collect();
    Ok(EffectivenessProbeReport {
        rows,
        globally_effective: globally,
        locally_effective: locally,
        ranking_valid,
        reversed_still_valid,
        global_effectiveness_needs_a_positive_improvement:
            findings.global_effectiveness_needs_a_positive_improvement,
        local_effectiveness_needs_exactly_some_true:
            findings.local_effectiveness_needs_exactly_some_true,
        the_order_of_entries_is_never_validated: findings.the_order_of_entries_is_never_validated,
        the_first_delta_must_be_positive: findings.the_first_delta_must_be_positive,
        a_local_verdict_requires_a_dual: findings.a_local_verdict_requires_a_dual,
        a_removal_verdict_requires_a_supported_status:
            findings.a_removal_verdict_requires_a_supported_status,
        the_reason_is_never_tied_to_the_status: findings.the_reason_is_never_tied_to_the_status,
        blank_and_duplicate_identities_are_refused:
            findings.blank_and_duplicate_identities_are_refused,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    global_effectiveness_needs_a_positive_improvement: bool,
    local_effectiveness_needs_exactly_some_true: bool,
    the_order_of_entries_is_never_validated: bool,
    the_first_delta_must_be_positive: bool,
    a_local_verdict_requires_a_dual: bool,
    a_removal_verdict_requires_a_supported_status: bool,
    the_reason_is_never_tied_to_the_status: bool,
    blank_and_duplicate_identities_are_refused: bool,
}

/// 造一条最小条目。 / Build one minimal entry.
fn bare(id: &str) -> EffectivenessEntry {
    EffectivenessEntry {
        constraint_id: StableConstraintId(id.to_owned()),
        group: None,
        tier: CandidateTier::Unclassified,
        activity: ActivityStatus::Unknown,
        dual_value: None,
        local_effective: None,
        removal_effective: None,
        first_effective_delta: None,
        max_improvement: None,
        marginal_effect: None,
        status: AnalysisStatus::Unknown,
        unavailable_reason: None,
    }
}

/// 一次校验。 / One validation.
fn valid(id: &str, tweak: impl FnOnce(&mut EffectivenessEntry)) -> bool {
    let mut entry = bare(id);
    tweak(&mut entry);
    ranking_of("probe", 0.0, vec![entry]).validate().is_ok()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 全局有效性要正的改善。 / Global effectiveness needs a positive improvement.
        let global_effectiveness_needs_a_positive_improvement = {
            let effective = |value: Option<f64>| {
                let mut entry = bare("c");
                entry.max_improvement = value;
                entry.is_globally_effective()
            };
            // 只有严格正才算；0 与负数都不是"收益"。 / Only strictly positive counts; zero and negatives do not.
            effective(Some(1e-12)) && !effective(Some(0.0)) && !effective(Some(-1.0))
                && !effective(None)
                // 而**这些都不会被 `validate()` 拒绝**——负改善是合法数据。 
                // None of these are refused by `validate()`, so a negative improvement is legitimate data.
                && valid("c", |entry| entry.max_improvement = Some(-1.0))
        };
        // 2) 局部有效只认 `Some(true)`。 / Local effectiveness only accepts `Some(true)`.
        let local_effectiveness_needs_exactly_some_true = {
            let listed = |verdict: Option<bool>| {
                let mut entry = bare("c");
                entry.dual_value = Some(1.0);
                entry.local_effective = verdict;
                ranking_of("probe", 0.0, vec![entry]).locally_effective().len() == 1
            };
            listed(Some(true)) && !listed(Some(false)) && !listed(None)
        };
        // 3) 顺序从不被校验。 / The order is never validated.
        let the_order_of_entries_is_never_validated = {
            // 文档说"按最大改善降序"，但升序排列照样通过。 
            // The doc claims a descending order by maximum improvement, yet an ascending one validates too.
            let mut first = bare("low");
            first.max_improvement = Some(1.0);
            let mut second = bare("high");
            second.max_improvement = Some(9.0);
            ranking_of("probe", 0.0, vec![first.clone(), second.clone()]).validate().is_ok()
                && ranking_of("probe", 0.0, vec![second, first]).validate().is_ok()
                // 对照：身份重复才被拒。 / Control: only duplicate identities are refused.
                && ranking_of("probe", 0.0, vec![bare("same"), bare("same")]).validate().is_err()
        };
        // 4) `first_effective_delta` 必须为正。 / `first_effective_delta` must be positive.
        let the_first_delta_must_be_positive = {
            valid("c", |entry| entry.first_effective_delta = Some(0.5))
                && !valid("c", |entry| entry.first_effective_delta = Some(0.0))
                && !valid("c", |entry| entry.first_effective_delta = Some(-0.5))
        };
        // 5) 有局部结论必须有对偶。 / A local verdict requires a dual.
        let a_local_verdict_requires_a_dual = {
            !valid("c", |entry| entry.local_effective = Some(true))
                && valid("c", |entry| { entry.local_effective = Some(true);
                    entry.dual_value = Some(0.0); })
                // 反过来：只有对偶、没有结论，合法。 
                // Conversely, a dual without a verdict is fine.
                && valid("c", |entry| entry.dual_value = Some(0.0))
        };
        // 6) 单删结论不能挂在 `Unsupported` 上。 / A removal verdict cannot sit under `Unsupported`.
        let a_removal_verdict_requires_a_supported_status = {
            !valid("c", |entry| { entry.removal_effective = Some(false);
                entry.status = AnalysisStatus::Unsupported; })
                // 其它三个状态都允许。 / The other three statuses are all allowed.
                && valid("c", |entry| { entry.removal_effective = Some(false);
                    entry.status = AnalysisStatus::Reachable; })
                && valid("c", |entry| { entry.removal_effective = Some(false);
                    entry.status = AnalysisStatus::Unknown; })
                // 而 `Unsupported` 不带结论也合法。 / `Unsupported` without a verdict is fine too.
                && valid("c", |entry| entry.status = AnalysisStatus::Unsupported)
        };
        // 7) `unavailable_reason` 与 `status` 之间没有约束。 / The reason is tied to nothing.
        let the_reason_is_never_tied_to_the_status = {
            // `Reachable` 带原因、`Unsupported` 不带原因，两者都通过。 
            // A `Reachable` entry with a reason and an `Unsupported` one without both validate.
            valid("c", |entry| { entry.status = AnalysisStatus::Reachable;
                entry.unavailable_reason = Some("why not".to_owned()); })
                && valid("c", |entry| { entry.status = AnalysisStatus::Unsupported;
                    entry.unavailable_reason = None; })
        };
        // 8) 身份为空或重复会被拒。 / Blank and duplicate identities are refused.
        let blank_and_duplicate_identities_are_refused = {
            !valid("   ", |_| {})
                && !valid("", |_| {})
                && ranking_of("probe", 0.0, vec![bare("a"), bare("a")]).validate().is_err()
                && ranking_of("probe", 0.0, vec![bare("a"), bare("b")]).validate().is_ok()
                // 空 schema 与非有限 baseline 也被拒。 / An empty schema and a non-finite baseline are refused too.
                && ranking_of("  ", 0.0, Vec::new()).validate().is_err()
                && ranking_of("probe", f64::NAN, Vec::new()).validate().is_err()
        };
        Findings {
            global_effectiveness_needs_a_positive_improvement,
            local_effectiveness_needs_exactly_some_true,
            the_order_of_entries_is_never_validated,
            the_first_delta_must_be_positive,
            a_local_verdict_requires_a_dual,
            a_removal_verdict_requires_a_supported_status,
            the_reason_is_never_tied_to_the_status,
            blank_and_duplicate_identities_are_refused,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, improvement: Option<f64>) -> EffectivenessProbeSpec {
        EffectivenessProbeSpec { id: id.to_owned(), dual: None, local_effective: None,
            removal_effective: None, first_effective_delta: None, max_improvement: improvement,
            marginal_effect: None, status: None, unavailable_reason: None }
    }

    fn request() -> EffectivenessProbeRequest {
        EffectivenessProbeRequest { baseline_objective: Some(10.0),
            schema_version: Some("probe".to_owned()),
            entries: vec![spec("a", Some(3.0)), spec("b", Some(0.0)), spec("c", None)] }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_effectiveness_probe(&request()).unwrap();
        assert!(report.global_effectiveness_needs_a_positive_improvement);
        assert!(report.local_effectiveness_needs_exactly_some_true);
        assert!(report.the_order_of_entries_is_never_validated);
        assert!(report.the_first_delta_must_be_positive);
        assert!(report.a_local_verdict_requires_a_dual);
        assert!(report.a_removal_verdict_requires_a_supported_status);
        assert!(report.the_reason_is_never_tied_to_the_status);
        assert!(report.blank_and_duplicate_identities_are_refused);
        // 只有 `Some(3.0)` 进全局列表；0 与 `None` 都不进。 
        // Only `Some(3.0)` reaches the global list; zero and `None` do not.
        assert_eq!(report.globally_effective, vec!["a".to_owned()]);
        assert!(report.ranking_valid);
        assert!(report.reversed_still_valid);
        assert_eq!(report.rows[0].globally_effective, true);
        assert_eq!(report.rows[1].globally_effective, false);
        assert_eq!(report.rows[2].globally_effective, false);
        assert!(report.locally_effective.is_empty());
    }

    #[test]
    fn a_negative_improvement_is_neither_effective_nor_refused() {
        let report = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![spec("a", Some(-5.0))], ..request() }).unwrap();
        assert_eq!(report.rows[0].globally_effective, false);
        assert!(report.ranking_valid);
        assert!(report.globally_effective.is_empty());
    }

    #[test]
    fn local_effectiveness_needs_a_dual() {
        let without = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { local_effective: Some(true),
                ..spec("a", None) }], ..request() }).unwrap();
        assert!(!without.ranking_valid);
        let with = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { local_effective: Some(true), dual: Some(0.5),
                ..spec("a", None) }], ..request() }).unwrap();
        assert!(with.ranking_valid);
        assert_eq!(with.locally_effective, vec!["a".to_owned()]);
        assert!(with.globally_effective.is_empty());
    }

    #[test]
    fn a_developer_delta_must_be_positive() {
        let zero = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { first_effective_delta: Some(0.0),
                ..spec("a", None) }], ..request() }).unwrap();
        assert!(!zero.ranking_valid);
        let negative = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { first_effective_delta: Some(-1.0),
                ..spec("a", None) }], ..request() }).unwrap();
        assert!(!negative.ranking_valid);
        let positive = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { first_effective_delta: Some(0.5),
                ..spec("a", None) }], ..request() }).unwrap();
        assert!(positive.ranking_valid);
    }

    #[test]
    fn a_removal_verdict_under_unsupported_is_refused() {
        let refused = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { removal_effective: Some(false),
                status: Some("Unsupported".to_owned()), ..spec("a", None) }], ..request() })
            .unwrap();
        assert!(!refused.ranking_valid);
        let allowed = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { removal_effective: Some(false),
                status: Some("Reachable".to_owned()), ..spec("a", None) }], ..request() })
            .unwrap();
        assert!(allowed.ranking_valid);
    }

    #[test]
    fn duplicate_and_blank_identities_are_refused() {
        let duplicate = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![spec("same", None), spec("same", None)], ..request() }).unwrap();
        assert!(!duplicate.ranking_valid);
        let blank = verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![spec("   ", None)], ..request() }).unwrap();
        assert!(!blank.ranking_valid);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: (0..9).map(|index| spec(&format!("c{index}"), None)).collect(),
            ..request() }).unwrap_err().message.contains("at most eight entries"));
        assert!(verify_effectiveness_probe(&EffectivenessProbeRequest {
            baseline_objective: Some(f64::NAN), ..request() }).unwrap_err()
            .message.contains("baseline objective must be finite"));
        assert!(verify_effectiveness_probe(&EffectivenessProbeRequest {
            entries: vec![EffectivenessProbeSpec { status: Some("Nearly".to_owned()),
                ..spec("a", None) }], ..request() }).unwrap_err()
            .message.contains("unknown analysis status"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_effectiveness_probe(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"globallyEffective\""), "{json}");
        assert!(json.contains("\"reversedStillValid\""), "{json}");
        assert!(json.contains("\"theOrderOfEntriesIsNeverValidated\""), "{json}");
        assert!(!json.contains("ranking_valid"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<EffectivenessProbeReport>(&copy.to_string()).is_err());
    }
}
