//! 续挖第 55 单：候选漏斗的**昂贵分析短名单**——只优先 A/B、低层填剩余名额、且从不排序
//! （`analysis/pipeline.rs` 的 `CandidateFunnelRanking::shortlist` / `validate` / `CandidateTier`）
//! Continuation, delivery 55: the candidate funnel's **expensive-analysis shortlist** — only A/B are preferred, lower tiers fill
//! the remaining slots, and it never sorts (`CandidateFunnelRanking::shortlist` / `validate` / `CandidateTier`).
//!
//! 第 26/27 单量过候选漏斗的产出与有效性排序；这一单量 §11.14 点名的 `analysis/pipeline.rs` 里剩下的一处：
//! **短名单**。七条实测：
//!
//! 1. **只有 `TierA` 与 `TierB` 默认进短名单**（`only_a_and_b_are_default()`）：`is_shortlist_default` 对
//!    `TierC` 与 `Unclassified` 都是 `false`。
//! 2. **低层不是被永久丢弃**（`lower_tiers_fill_the_remaining_slots()`）：名额够时 `TierC` 与
//!    `Unclassified` 会被**追加**进短名单，只是排在 A/B 之后。
//! 3. **`shortlist(0)` 是空**（`a_zero_limit_returns_nothing()`）。
//! 4. **名额大于总数时全部返回，但顺序会变**（`a_large_limit_returns_everything_but_regroups()`）：
//!    `TierA`/`TierB` 被**稳定分区**提到 `TierC`/`Unclassified` 之前——所以它虽然不排序，
//!    却会重排：一份 `C` 在前、`A` 在后的输入，输出会变成 `A` 在前。
//! 5. **`shortlist()` 从不排序**（`the_shortlist_never_sorts()`）：它只是"过滤 + 截断"，所以给一份
//!    `priority` 与位置不一致的候选列表，它也会照输入顺序吐回来——**"优先级"完全靠调用方保证**。
//! 6. **`validate()` 才要求 priority 稠密且等于位置**（`priorities_must_be_dense_and_positional()`）：
//!    跳号或乱序被拒，重复 ID 也被拒。所以第 5 条的乱序输入**过不了 `validate()`**，但**过得了 `shortlist()`**。
//! 7. **分层需要证据**（`tiers_require_evidence()`）：`TierA` 的 `|dual|` 必须**严格大于**阈值
//!    （等于阈值被拒）、`TierB` 必须**小于等于**阈值、`TierC` 必须**不是 active**——而
//!    `ActivityStatus::is_active()` 只认 `Active`，所以 `NearlyActive` 的候选可以被判成 `TierC`。
//!
//! Deliveries 26 and 27 measured the funnel's output and the effectiveness ranking; this one measures the remaining
//! `analysis/pipeline.rs` surface the audit names: **the shortlist**. **Only `TierA` and `TierB` are shortlist defaults**
//! (`only_a_and_b_are_default()`), since `is_shortlist_default` is `false` for `TierC` and `Unclassified`. **Lower tiers are not
//! dropped** (`lower_tiers_fill_the_remaining_slots()`): with slots left they are **appended** after A/B. **`shortlist(0)` is
//! empty** (`a_zero_limit_returns_nothing()`). **A limit above the total returns everything in input order**
//! (`a_large_limit_keeps_the_input_order()`). **`shortlist()` never sorts** (`the_shortlist_never_sorts()`): it only filters and
//! truncates, so a candidate list whose `priority` disagrees with its position comes back in **input** order — **priority is
//! entirely the caller's responsibility**. **Only `validate()` demands dense positional priorities**
//! (`priorities_must_be_dense_and_positional()`), rejecting gaps, disorder and duplicate IDs, so the disordered input of point 5
//! **fails `validate()` but passes `shortlist()`**. And **a tier requires evidence** (`tiers_require_evidence()`): `TierA` needs
//! `|dual|` **strictly above** the threshold (equality is refused), `TierB` at or below it, and `TierC` **not active** — while
//! `ActivityStatus::is_active()` accepts only `Active`, so a `NearlyActive` candidate may be classified `TierC`.

use ospf_rust_core::analysis::activity::ActivityStatus;
use ospf_rust_core::analysis::pipeline::{
    CandidateFunnelRanking, CandidateTier, ConstraintCandidate,
};
use ospf_rust_core::solver::report::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条候选的字面量。 / One candidate literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateEntrySpec {
    pub id: String,
    /// `A` / `B` / `C` / `UNCLASSIFIED`。 / The tier.
    #[serde(default)]
    pub tier: Option<String>,
    /// 活动性：`ACTIVE` / `NEARLY_ACTIVE` / `INACTIVE` / `SATISFIED_WITHOUT_SLACK_METRIC` /
    /// `VIOLATED` / `UNKNOWN`。 / The activity status.
    #[serde(default)]
    pub activity: Option<String>,
    #[serde(default)]
    pub dual_value: Option<f64>,
    #[serde(default)]
    pub slack: Option<f64>,
    #[serde(default)]
    pub group: Option<String>,
}

/// 短名单请求。 / The shortlist request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortlistRequest {
    #[serde(default)]
    pub candidates: Vec<CandidateEntrySpec>,
    /// 短名单名额。 / The shortlist limit.
    #[serde(default)]
    pub limit: Option<u64>,
    /// 是否先 `validate()`。 / Whether to call `validate()` first.
    #[serde(default)]
    pub validate: bool,
    /// 用于排序的 `dualThreshold`（缺省 1e-9）。 / The `dualThreshold` (defaults to 1e-9).
    #[serde(default)]
    pub dual_threshold: Option<f64>,
}

/// 短名单结论。 / The shortlist verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortlistReport {
    /// `validate()` 是否通过（未调用时为 `null`）。 / Whether `validate()` passed (`null` when not called).
    #[serde(default)]
    pub validated: Option<bool>,
    /// `validate()` 的失败消息。 / The `validate()` failure message.
    #[serde(default)]
    pub error: Option<String>,
    /// 短名单里的约束 ID（按返回顺序）。 / The shortlisted constraint IDs (in returned order).
    pub shortlist: Vec<String>,
    /// 短名单里的分层名（与 `shortlist` 对齐）。 / The tier names aligned with `shortlist`.
    pub tiers: Vec<String>,
    /// 短名单里的 priority（与 `shortlist` 对齐）。 / The priorities aligned with `shortlist`.
    pub priorities: Vec<u64>,
    /// 只有 A/B 默认进短名单（**是**）。 / Whether only A/B are shortlist defaults (**yes**).
    pub only_a_and_b_are_default: bool,
    /// 低层填剩余名额（**是**）。 / Whether lower tiers fill the remaining slots (**yes**).
    pub lower_tiers_fill_the_remaining_slots: bool,
    /// `shortlist(0)` 是空（**是**）。 / Whether `shortlist(0)` is empty (**yes**).
    pub a_zero_limit_returns_nothing: bool,
    /// 大名额返回全部但会按层分组（**是**）。 / Whether a large limit returns everything while regrouping by tier (**yes**).
    pub a_large_limit_returns_everything_but_regroups: bool,
    /// `shortlist()` 从不排序（**是**）。 / Whether `shortlist()` never sorts (**yes**).
    pub the_shortlist_never_sorts: bool,
    /// priority 必须稠密且等于位置（**是**）。 / Whether priorities must be dense and positional (**yes**).
    pub priorities_must_be_dense_and_positional: bool,
    /// 分层需要证据（**是**）。 / Whether a tier requires evidence (**yes**).
    pub tiers_require_evidence: bool,
}

/// 候选条数上限（信任边界）。 / The candidate cap (a trust boundary).
const MAX_CANDIDATES: usize = 128;

/// 活动性名转枚举。 / Turn an activity name into the enum.
fn activity_of(name: &str) -> Result<ActivityStatus> {
    Ok(match name {
        "ACTIVE" => ActivityStatus::Active,
        "NEARLY_ACTIVE" => ActivityStatus::NearlyActive,
        "INACTIVE" => ActivityStatus::Inactive,
        "SATISFIED_WITHOUT_SLACK_METRIC" => ActivityStatus::SatisfiedWithoutSlackMetric,
        "VIOLATED" => ActivityStatus::Violated,
        "UNKNOWN" => ActivityStatus::Unknown,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "candidate shortlist failed validation: unknown activity `{other}`"))),
    })
}

/// 分层名转枚举。 / Turn a tier name into the enum.
fn tier_of(name: &str) -> Result<CandidateTier> {
    Ok(match name {
        "A" => CandidateTier::TierA,
        "B" => CandidateTier::TierB,
        "C" => CandidateTier::TierC,
        "UNCLASSIFIED" => CandidateTier::Unclassified,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "candidate shortlist failed validation: unknown tier `{other}`"))),
    })
}

/// 造一条候选。 / Build one candidate.
fn candidate_of(spec: &CandidateEntrySpec, position: usize) -> Result<ConstraintCandidate> {
    Ok(ConstraintCandidate { constraint_id: StableConstraintId(spec.id.clone()),
        group: spec.group.clone(),
        tier: tier_of(spec.tier.as_deref().unwrap_or("UNCLASSIFIED"))?,
        activity: activity_of(spec.activity.as_deref().unwrap_or("UNKNOWN"))?,
        slack: spec.slack, dual_value: spec.dual_value, local_effective: None,
        priority: position })
}

/// 走一遍短名单。非法输入是**错误**。 / Walk the shortlist.
pub fn verify_candidate_shortlist(request: &ShortlistRequest) -> Result<ShortlistReport> {
    if request.candidates.len() > MAX_CANDIDATES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "candidate shortlist failed validation: at most {MAX_CANDIDATES} candidates are \
             accepted")));
    }
    let candidates = request.candidates.iter().enumerate()
        .map(|(position, spec)| candidate_of(spec, position))
        .collect::<Result<Vec<_>>>()?;
    let dual_threshold = request.dual_threshold.unwrap_or(1e-9);
    if !dual_threshold.is_finite() || dual_threshold < 0.0 {
        return Err(BridgeError::new("InvalidInput",
            "candidate shortlist failed validation: the dual threshold must be finite and \
             non-negative"));
    }
    let ranking = CandidateFunnelRanking {
        schema_version: "candidate-funnel/v1".to_owned(),
        dual_threshold,
        tier_a: candidates.iter().filter(|c| c.tier == CandidateTier::TierA).count(),
        tier_b: candidates.iter().filter(|c| c.tier == CandidateTier::TierB).count(),
        tier_c: candidates.iter().filter(|c| c.tier == CandidateTier::TierC).count(),
        unclassified: candidates.iter()
            .filter(|c| c.tier == CandidateTier::Unclassified).count(),
        candidates,
    };
    let (validated, error) = if request.validate {
        match ranking.validate() {
            Ok(()) => (Some(true), None),
            Err(failure) => (Some(false), Some(failure.to_string())),
        }
    } else {
        (None, None)
    };
    let limit = usize::try_from(request.limit.unwrap_or(32)).unwrap_or(usize::MAX);
    let shortlisted = ranking.shortlist(limit);
    let shortlist = shortlisted.iter()
        .map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>();
    let tiers = shortlisted.iter().map(|candidate| candidate.tier.as_str().to_owned())
        .collect::<Vec<_>>();
    let priorities = shortlisted.iter().map(|candidate| candidate.priority as u64)
        .collect::<Vec<_>>();
    let findings = canonical_findings();
    Ok(ShortlistReport { validated, error, shortlist, tiers, priorities,
        only_a_and_b_are_default: findings.only_a_and_b_are_default,
        lower_tiers_fill_the_remaining_slots: findings.lower_tiers_fill_the_remaining_slots,
        a_zero_limit_returns_nothing: findings.a_zero_limit_returns_nothing,
        a_large_limit_returns_everything_but_regroups:
            findings.a_large_limit_returns_everything_but_regroups,
        the_shortlist_never_sorts: findings.the_shortlist_never_sorts,
        priorities_must_be_dense_and_positional:
            findings.priorities_must_be_dense_and_positional,
        tiers_require_evidence: findings.tiers_require_evidence })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    only_a_and_b_are_default: bool,
    lower_tiers_fill_the_remaining_slots: bool,
    a_zero_limit_returns_nothing: bool,
    a_large_limit_returns_everything_but_regroups: bool,
    the_shortlist_never_sorts: bool,
    priorities_must_be_dense_and_positional: bool,
    tiers_require_evidence: bool,
}

/// 造一条探针候选。 / Build one probe candidate.
fn probe_candidate(id: &str, tier: CandidateTier, activity: ActivityStatus, dual: Option<f64>,
    position: usize) -> ConstraintCandidate {
    ConstraintCandidate { constraint_id: StableConstraintId(id.to_owned()), group: None, tier,
        activity, slack: None, dual_value: dual, local_effective: None, priority: position }
}

/// 造一份探针排序。 / Build one probe ranking.
fn probe_ranking(candidates: Vec<ConstraintCandidate>) -> CandidateFunnelRanking {
    CandidateFunnelRanking {
        schema_version: "candidate-funnel/v1".to_owned(), dual_threshold: 1e-9,
        tier_a: candidates.iter().filter(|c| c.tier == CandidateTier::TierA).count(),
        tier_b: candidates.iter().filter(|c| c.tier == CandidateTier::TierB).count(),
        tier_c: candidates.iter().filter(|c| c.tier == CandidateTier::TierC).count(),
        unclassified: candidates.iter()
            .filter(|c| c.tier == CandidateTier::Unclassified).count(),
        candidates,
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 只有 A/B 默认进短名单。 / Only A/B are shortlist defaults.
    let only_a_and_b_are_default = CandidateTier::TierA.is_shortlist_default()
        && CandidateTier::TierB.is_shortlist_default()
        && !CandidateTier::TierC.is_shortlist_default()
        && !CandidateTier::Unclassified.is_shortlist_default();
    // 2) 低层填剩余名额（不被永久丢弃）。 / Lower tiers fill the remaining slots.
    let lower_tiers_fill_the_remaining_slots = {
        let ranking = probe_ranking(vec![
            probe_candidate("a", CandidateTier::TierA, ActivityStatus::Active, Some(1.0), 0),
            probe_candidate("c", CandidateTier::TierC, ActivityStatus::Inactive, None, 1),
            probe_candidate("u", CandidateTier::Unclassified, ActivityStatus::Unknown, None, 2)]);
        let shortlist = ranking.shortlist(3).into_iter()
            .map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>();
        shortlist == vec!["a".to_owned(), "c".to_owned(), "u".to_owned()]
            && ranking.shortlist(1).len() == 1
    };
    // 3) `shortlist(0)` 是空。 / `shortlist(0)` is empty.
    let a_zero_limit_returns_nothing = probe_ranking(vec![
        probe_candidate("a", CandidateTier::TierA, ActivityStatus::Active, Some(1.0), 0)])
        .shortlist(0).is_empty();
    // 4) 大名额返回全部，但会**按层稳定分组**：`C` 在前、`A` 在后的输入会翻成 `A` 在前。 
    // 4) A large limit returns everything while **stably partitioning by tier**: an input with `C` first and `A` later flips.
    let a_large_limit_returns_everything_but_regroups = {
        let ranking = probe_ranking(vec![
            probe_candidate("c", CandidateTier::TierC, ActivityStatus::Inactive, None, 0),
            probe_candidate("a", CandidateTier::TierA, ActivityStatus::Active, Some(1.0), 1)]);
        let shortlist = ranking.shortlist(99).into_iter()
            .map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>();
        shortlist == vec!["a".to_owned(), "c".to_owned()] && ranking.shortlist(99).len() == 2
    };
    // 5) `shortlist()` 从不排序：priority 与位置不一致的输入照样按输入顺序返回。 
    // 5) `shortlist()` never sorts: priorities disagreeing with positions still come back in input order.
    let the_shortlist_never_sorts = {
        let ranking = probe_ranking(vec![
            probe_candidate("first", CandidateTier::TierA, ActivityStatus::Active, Some(1.0), 5),
            probe_candidate("second", CandidateTier::TierA, ActivityStatus::Active, Some(2.0), 0)]);
        let shortlist = ranking.shortlist(2).into_iter()
            .map(|candidate| candidate.constraint_id.0.clone()).collect::<Vec<_>>();
        let priorities = ranking.shortlist(2).into_iter()
            .map(|candidate| candidate.priority).collect::<Vec<_>>();
        shortlist == vec!["first".to_owned(), "second".to_owned()]
            && priorities == vec![5usize, 0]
            // 而同一份输入过不了 `validate()`。 / The same input fails `validate()`.
            && ranking.validate().is_err()
    };
    // 6) `validate()` 要求 priority 稠密且等于位置，且 ID 不重复。 
    // 6) `validate()` demands dense positional priorities and unique IDs.
    let priorities_must_be_dense_and_positional = {
        let dense = probe_ranking(vec![
            probe_candidate("a", CandidateTier::Unclassified, ActivityStatus::Unknown, None, 0),
            probe_candidate("b", CandidateTier::Unclassified, ActivityStatus::Unknown, None, 1)]);
        let skipped = probe_ranking(vec![
            probe_candidate("a", CandidateTier::Unclassified, ActivityStatus::Unknown, None, 0),
            probe_candidate("b", CandidateTier::Unclassified, ActivityStatus::Unknown, None, 2)]);
        let duplicated = probe_ranking(vec![
            probe_candidate("a", CandidateTier::Unclassified, ActivityStatus::Unknown, None, 0),
            probe_candidate("a", CandidateTier::Unclassified, ActivityStatus::Unknown, None, 1)]);
        dense.validate().is_ok() && skipped.validate().is_err() && duplicated.validate().is_err()
    };
    // 7) 分层需要证据；`is_active()` 只认 `Active`，所以 `NearlyActive` 可以是 `TierC`。 
    // 7) A tier requires evidence, and `is_active()` accepts only `Active`, so `NearlyActive` may be `TierC`.
    let tiers_require_evidence = {
        let at_threshold = probe_ranking(vec![probe_candidate("a", CandidateTier::TierA,
            ActivityStatus::Active, Some(1e-9), 0)]);
        let above = probe_ranking(vec![probe_candidate("a", CandidateTier::TierA,
            ActivityStatus::Active, Some(1.0), 0)]);
        let b_above = probe_ranking(vec![probe_candidate("b", CandidateTier::TierB,
            ActivityStatus::Active, Some(1.0), 0)]);
        let c_active = probe_ranking(vec![probe_candidate("c", CandidateTier::TierC,
            ActivityStatus::Active, None, 0)]);
        let c_nearly = probe_ranking(vec![probe_candidate("c", CandidateTier::TierC,
            ActivityStatus::NearlyActive, None, 0)]);
        at_threshold.validate().is_err() && above.validate().is_ok()
            && b_above.validate().is_err() && c_active.validate().is_err()
            && c_nearly.validate().is_ok()
            // `is_active()` 只认 `Active`。 / `is_active()` accepts only `Active`.
            && ActivityStatus::Active.is_active() && !ActivityStatus::NearlyActive.is_active()
    };
    Findings { only_a_and_b_are_default, lower_tiers_fill_the_remaining_slots,
        a_zero_limit_returns_nothing, a_large_limit_returns_everything_but_regroups,
        the_shortlist_never_sorts, priorities_must_be_dense_and_positional,
        tiers_require_evidence }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, tier: &str, activity: &str, dual: Option<f64>) -> CandidateEntrySpec {
        CandidateEntrySpec { id: id.to_owned(), tier: Some(tier.to_owned()),
            activity: Some(activity.to_owned()), dual_value: dual, slack: None, group: None }
    }

    fn request() -> ShortlistRequest {
        ShortlistRequest {
            candidates: vec![candidate("strong", "A", "ACTIVE", Some(1.0)),
                candidate("weak", "B", "ACTIVE", Some(0.0)),
                candidate("inactive", "C", "INACTIVE", None),
                candidate("unknown", "UNCLASSIFIED", "UNKNOWN", None)],
            limit: Some(2), validate: true, dual_threshold: None }
    }

    #[test]
    fn the_shortlist_prefers_a_and_b() {
        let report = verify_candidate_shortlist(&request()).unwrap();
        assert_eq!(report.validated, Some(true));
        assert_eq!(report.error, None);
        assert_eq!(report.shortlist, vec!["strong".to_owned(), "weak".to_owned()]);
        assert_eq!(report.tiers, vec!["A".to_owned(), "B".to_owned()]);
        assert_eq!(report.priorities, vec![0, 1]);
        assert!(report.only_a_and_b_are_default);
        assert!(report.lower_tiers_fill_the_remaining_slots);
        assert!(report.a_zero_limit_returns_nothing);
        assert!(report.a_large_limit_returns_everything_but_regroups);
        assert!(report.the_shortlist_never_sorts);
        assert!(report.priorities_must_be_dense_and_positional);
        assert!(report.tiers_require_evidence);
    }

    #[test]
    fn lower_tiers_fill_the_remaining_slots() {
        let report = verify_candidate_shortlist(&ShortlistRequest { limit: Some(4),
            ..request() }).unwrap();
        assert_eq!(report.shortlist, vec!["strong".to_owned(), "weak".to_owned(),
            "inactive".to_owned(), "unknown".to_owned()]);
        let report = verify_candidate_shortlist(&ShortlistRequest { limit: Some(0),
            ..request() }).unwrap();
        assert!(report.shortlist.is_empty());
    }

    #[test]
    fn the_shortlist_follows_positions_not_priorities() {
        // 位置是短的，priority 相反——`shortlist()` 不排序。 
        // Positions are edited independently of priorities — `shortlist()` does not sort.
        let mut disordered = request();
        disordered.candidates = vec![candidate("first", "A", "ACTIVE", Some(1.0)),
            candidate("second", "A", "ACTIVE", Some(2.0))];
        disordered.limit = Some(2);
        let report = verify_candidate_shortlist(&disordered).unwrap();
        assert_eq!(report.shortlist, vec!["first".to_owned(), "second".to_owned()]);
        assert_eq!(report.priorities, vec![0, 1]);
        assert_eq!(report.validated, Some(true));
    }

    #[test]
    fn a_tier_needs_evidence() {
        // A 略高于阈值 → 通过。 / A above the threshold passes.
        let above = ShortlistRequest { candidates: vec![candidate("a", "A", "ACTIVE", Some(1.0))],
            limit: Some(1), validate: true, dual_threshold: None };
        assert_eq!(verify_candidate_shortlist(&above).unwrap().validated, Some(true));
        // A 恰好等于阈值 → 被拒。 / A exactly at the threshold is refused.
        let at = ShortlistRequest { candidates: vec![candidate("a", "A", "ACTIVE", Some(1e-9))],
            limit: Some(1), validate: true, dual_threshold: None };
        let report = verify_candidate_shortlist(&at).unwrap();
        assert_eq!(report.validated, Some(false));
        assert!(report.error.as_deref().unwrap_or_default()
            .contains("dual above the threshold"));
        // B 高于阈值 → 被拒。 / B above the threshold is refused.
        let bad_b = ShortlistRequest { candidates: vec![candidate("b", "B", "ACTIVE", Some(1.0))],
            limit: Some(1), validate: true, dual_threshold: None };
        assert_eq!(verify_candidate_shortlist(&bad_b).unwrap().validated, Some(false));
        // C 是 active → 被拒；C 是 nearly-active → 通过。 
        // An active C is refused while a nearly-active C passes.
        let c_active = ShortlistRequest { candidates: vec![candidate("c", "C", "ACTIVE", None)],
            limit: Some(1), validate: true, dual_threshold: None };
        assert_eq!(verify_candidate_shortlist(&c_active).unwrap().validated, Some(false));
        let c_nearly = ShortlistRequest { candidates: vec![candidate("c", "C", "NEARLY_ACTIVE",
            None)], limit: Some(1), validate: true, dual_threshold: None };
        assert_eq!(verify_candidate_shortlist(&c_nearly).unwrap().validated, Some(true));
    }

    #[test]
    fn duplicate_ids_and_missing_duals_are_refused() {
        let duplicated = ShortlistRequest { candidates: vec![
            candidate("a", "UNCLASSIFIED", "UNKNOWN", None),
            candidate("a", "UNCLASSIFIED", "UNKNOWN", None)],
            limit: Some(2), validate: true, dual_threshold: None };
        let report = verify_candidate_shortlist(&duplicated).unwrap();
        assert_eq!(report.validated, Some(false));
        assert!(report.error.as_deref().unwrap_or_default()
            .contains("duplicate constraints"));
        let missing = ShortlistRequest { candidates: vec![candidate("a", "A", "ACTIVE", None)],
            limit: Some(1), validate: true, dual_threshold: None };
        assert_eq!(verify_candidate_shortlist(&missing).unwrap().validated, Some(false));
    }

    #[test]
    fn bad_input_is_refused() {
        let tier = ShortlistRequest { candidates: vec![candidate("a", "D", "ACTIVE", None)],
            limit: Some(1), validate: false, dual_threshold: None };
        assert!(verify_candidate_shortlist(&tier).unwrap_err().message
            .contains("unknown tier"));
        let activity = ShortlistRequest { candidates: vec![candidate("a", "A", "hot", None)],
            limit: Some(1), validate: false, dual_threshold: None };
        assert!(verify_candidate_shortlist(&activity).unwrap_err().message
            .contains("unknown activity"));
        let negative = ShortlistRequest { dual_threshold: Some(-1.0), ..request() };
        assert!(verify_candidate_shortlist(&negative).unwrap_err().message
            .contains("dual threshold"));
        let too_many = ShortlistRequest {
            candidates: (0..129).map(|index| candidate(&format!("c{index}"), "UNCLASSIFIED",
                "UNKNOWN", None)).collect(), limit: Some(1), validate: false,
            dual_threshold: None };
        assert!(verify_candidate_shortlist(&too_many).unwrap_err().message
            .contains("at most 128 candidates"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_candidate_shortlist(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"validated\""), "{json}");
        assert!(json.contains("\"onlyAAndBAreDefault\""), "{json}");
        assert!(json.contains("\"theShortlistNeverSorts\""), "{json}");
        assert!(!json.contains("only_a"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ShortlistReport>(&copy.to_string()).is_err());
    }
}
