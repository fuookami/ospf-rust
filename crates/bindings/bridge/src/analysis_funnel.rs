//! 批次 B 第 2 单：候选漏斗排序的校验与短名单 / Batch B, delivery 2: candidate-funnel verification and shortlist.
//!
//! `CandidateFunnelRanking::validate()` 与 `shortlist(limit)` 都是纯数据能力：优先级必须稠密且与稳定 ID 对齐、
//! 不得重复、分层计数必须与候选列表一致、Tier A 必须"有对偶且高于阈值"、Tier B 必须"有对偶且不高于阈值"、
//! Tier C 必须不是 active。整套规则不需要求解器，因此可以跨线。
//! `CandidateFunnelRanking::validate()` and `shortlist(limit)` are pure-data capabilities: priorities must be dense and
//! aligned with stable IDs, no duplicates, tier counts must match the candidate list, tier A requires a dual above the
//! threshold, tier B a dual at or below it, and tier C must not be active. None of it needs a solver, so it can cross the
//! wire.

use ospf_rust_core::analysis::pipeline::{CandidateFunnelRanking, CandidateTier};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 短名单里的一条候选。 / One shortlisted candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortlistEntry {
    /// 原始约束稳定 ID。 / The stable original-constraint ID.
    pub constraint_id: String,
    /// 分层（`A`/`B`/`C`/`UNCLASSIFIED`）。 / The tier.
    pub tier: String,
    /// 排序位置，0 优先级最高。 / The rank position; 0 is the highest priority.
    pub priority: u32,
}

/// 候选漏斗的校验结论与摘要。 / The verification verdict and summary of a candidate funnel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateFunnelReport {
    /// 报告 schema。 / The report schema.
    pub schema_version: String,
    /// 实际使用的对偶阈值。 / The dual threshold actually used.
    pub dual_threshold: f64,
    /// 候选总数。 / The candidate count.
    pub candidate_count: u32,
    /// Tier A 数量。 / The tier-A count.
    pub tier_a: u32,
    /// Tier B 数量。 / The tier-B count.
    pub tier_b: u32,
    /// Tier C 数量。 / The tier-C count.
    pub tier_c: u32,
    /// 未分层数量。 / The unclassified count.
    pub unclassified: u32,
    /// 请求的短名单上限；未请求时为空。 / The requested shortlist limit; absent when not requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shortlist_limit: Option<u32>,
    /// 短名单（按领域自己的 `shortlist` 顺序）。 / The shortlist in the domain's own `shortlist` order.
    #[serde(default)]
    pub shortlist: Vec<ShortlistEntry>,
}

/// 校验一份候选漏斗排序，并按需给出短名单。校验失败即**错误**并原样带出领域原因。
/// Verify one candidate funnel ranking and, on request, produce the shortlist. A validation failure is an **error**
/// carrying the domain reason verbatim.
pub fn verify_candidate_funnel(json: &str, shortlist_limit: Option<u32>) -> Result<CandidateFunnelReport> {
    if json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the candidate funnel exceeds 8 MiB"));
    }
    let ranking: CandidateFunnelRanking = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("candidate funnel is not decodable: {error}"))
    })?;
    ranking.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("candidate funnel failed validation: {error}"))
    })?;
    // 短名单走领域自己的 `shortlist`：先强/退化候选，名额不足时才按列表顺序补 Tier C 与未分层。
    // The shortlist comes from the domain's own `shortlist`: strong and degenerate candidates first, and only when there
    // is room does it append tier C and unclassified ones in list order.
    let shortlist = match shortlist_limit {
        None => Vec::new(),
        Some(limit) => ranking
            .shortlist(usize::try_from(limit).unwrap_or(usize::MAX))
            .into_iter()
            .map(|candidate| ShortlistEntry {
                constraint_id: candidate.constraint_id.0.clone(),
                tier: tier_name(candidate.tier).to_owned(),
                priority: u32::try_from(candidate.priority).unwrap_or(u32::MAX),
            })
            .collect(),
    };
    Ok(CandidateFunnelReport {
        schema_version: ranking.schema_version.clone(),
        dual_threshold: ranking.dual_threshold,
        candidate_count: u32::try_from(ranking.candidates.len()).unwrap_or(u32::MAX),
        tier_a: u32::try_from(ranking.tier_a).unwrap_or(u32::MAX),
        tier_b: u32::try_from(ranking.tier_b).unwrap_or(u32::MAX),
        tier_c: u32::try_from(ranking.tier_c).unwrap_or(u32::MAX),
        unclassified: u32::try_from(ranking.unclassified).unwrap_or(u32::MAX),
        shortlist_limit,
        shortlist,
    })
}

/// 分层名，直接复用领域的 `as_str`。 / The tier name, reusing the domain's own `as_str`.
fn tier_name(tier: CandidateTier) -> &'static str { tier.as_str() }

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::analysis::activity::ActivityStatus;
    use ospf_rust_core::analysis::pipeline::{ConstraintCandidate, CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION};
    use ospf_rust_core::solver::StableConstraintId as ConstraintId;

    fn candidate(priority: usize, id: &str, tier: CandidateTier, activity: ActivityStatus,
        dual: Option<f64>) -> ConstraintCandidate {
        ConstraintCandidate {
            constraint_id: ConstraintId(id.to_owned()),
            group: Some("g".to_owned()),
            tier,
            activity,
            slack: Some(0.0),
            dual_value: dual,
            local_effective: None,
            priority,
        }
    }

    /// 一份自洽的漏斗：A（对偶 1.0 > 0.5）、B（对偶 0）、C（非 active）。
    /// A self-consistent funnel: A (dual 1.0 > 0.5), B (dual 0), C (not active).
    fn ranking() -> CandidateFunnelRanking {
        CandidateFunnelRanking {
            schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
            dual_threshold: 0.5,
            candidates: vec![
                candidate(0, "hot", CandidateTier::TierA, ActivityStatus::Active, Some(1.0)),
                candidate(1, "warm", CandidateTier::TierB, ActivityStatus::Active, Some(0.0)),
                candidate(2, "cold", CandidateTier::TierC, ActivityStatus::Inactive, None),
            ],
            tier_a: 1, tier_b: 1, tier_c: 1, unclassified: 0,
        }
    }

    fn encode(ranking: &CandidateFunnelRanking) -> String {
        serde_json::to_string(ranking).expect("serialize funnel")
    }

    #[test]
    fn valid_funnel_is_summarized_and_shortlisted() {
        let report = verify_candidate_funnel(&encode(&ranking()), None).unwrap();
        assert_eq!(report.candidate_count, 3);
        assert_eq!((report.tier_a, report.tier_b, report.tier_c, report.unclassified), (1, 1, 1, 0));
        assert_eq!(report.dual_threshold, 0.5);
        assert_eq!(report.shortlist_limit, None);
        assert!(report.shortlist.is_empty(), "no limit means no shortlist");

        // 名额 2：A 与 B 都在默认短名单里，Tier C 不进。
        // Limit 2: A and B are both in the default shortlist, C stays out.
        let report = verify_candidate_funnel(&encode(&ranking()), Some(2)).unwrap();
        assert_eq!(report.shortlist_limit, Some(2));
        assert_eq!(report.shortlist.iter().map(|entry| entry.constraint_id.as_str()).collect::<Vec<_>>(),
            vec!["hot", "warm"]);
        assert_eq!(report.shortlist[0].tier, "A");
        assert_eq!(report.shortlist[0].priority, 0);

        // 名额 1：只有 A。 / Limit 1: only A.
        let report = verify_candidate_funnel(&encode(&ranking()), Some(1)).unwrap();
        assert_eq!(report.shortlist.len(), 1);
        assert_eq!(report.shortlist[0].constraint_id, "hot");

        // 名额 5（超过总数）：Tier C 才被补进来，顺序在 A/B 之后。
        // Limit 5 (more than the total): tier C only then gets appended, after A/B.
        let report = verify_candidate_funnel(&encode(&ranking()), Some(5)).unwrap();
        assert_eq!(report.shortlist.iter().map(|entry| entry.constraint_id.as_str()).collect::<Vec<_>>(),
            vec!["hot", "warm", "cold"]);
        assert_eq!(report.shortlist[2].tier, "C");

        // 名额 0：空短名单。 / Limit 0: an empty shortlist.
        assert!(verify_candidate_funnel(&encode(&ranking()), Some(0)).unwrap().shortlist.is_empty());
    }

    #[test]
    fn broken_funnels_are_refused_with_the_domain_reason() {
        let cases: Vec<(&str, Box<dyn Fn(&mut CandidateFunnelRanking)>, &str)> = vec![
            ("sparse priorities", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.candidates[1].priority = 5;
            }), "dense"),
            ("duplicate constraints", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.candidates[1].constraint_id = ConstraintId("hot".to_owned());
            }), "duplicate"),
            ("tier counts drift", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.tier_a = 2;
            }), "counts"),
            ("tier A without a dual", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.candidates[0].dual_value = None;
            }), "dual value"),
            ("tier A dual at the threshold", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.candidates[0].dual_value = Some(0.5);
            }), "above the threshold"),
            ("tier B dual above the threshold", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.candidates[1].dual_value = Some(2.0);
            }), "at or below"),
            ("tier C that is active", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.candidates[2].activity = ActivityStatus::Active;
            }), "must not be active"),
            ("blank schema", Box::new(|ranking: &mut CandidateFunnelRanking| {
                ranking.schema_version = "  ".to_owned();
            }), "invalid"),
        ];
        for (label, mutate, expected) in cases {
            let mut broken = ranking();
            mutate(&mut broken);
            let error = verify_candidate_funnel(&encode(&broken), None)
                .unwrap_err_or_panic(label);
            assert_eq!(error.code, "InvalidInput", "{label}");
            assert!(error.message.contains(expected), "{label}: {}", error.message);
        }
        assert_eq!(verify_candidate_funnel("not json", None).unwrap_err().code, "InvalidInput");
        assert_eq!(verify_candidate_funnel(&"x".repeat(9 * 1024 * 1024), None).unwrap_err().code,
            "InvalidInput");
    }

    /// 让上面的循环在"本该失败却成功"时 panic，而不是静默跳过。
    /// Make the loop above panic when a case should have failed but did not, instead of silently skipping it.
    trait UnwrapErrOrPanic {
        fn unwrap_err_or_panic(self, label: &str) -> BridgeError;
    }

    impl UnwrapErrOrPanic for Result<CandidateFunnelReport> {
        fn unwrap_err_or_panic(self, label: &str) -> BridgeError {
            match self {
                Ok(_) => panic!("case `{label}` must be refused"),
                Err(error) => error,
            }
        }
    }
}
