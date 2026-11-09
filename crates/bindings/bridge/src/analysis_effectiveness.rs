//! 批次 B 第 3 单：有效性排序的校验与投影 / Batch B, delivery 3: effectiveness ranking verification and projections.
//!
//! `EffectivenessRanking::validate()` 与 `globally_effective()` / `locally_effective()` 都是纯数据能力：
//! 稳定 ID 非空且不重复、对偶/改善/边际/首次 delta 必须有限且 delta 必须为正、局部结论必须先有对偶、
//! 删除结论必须来自受支持的删除测试。它们不需要求解器，因此可以跨线。
//! `EffectivenessRanking::validate()` plus `globally_effective()` / `locally_effective()` are pure-data capabilities:
//! stable IDs non-blank and unique, dual/improvement/marginal/first-delta finite with a positive delta, a local verdict
//! requires a dual first, and a removal verdict requires a supported removal test. None of it needs a solver.

use ospf_rust_core::analysis::pipeline::{EffectivenessEntry, EffectivenessRanking};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一行有效性明细。 / One effectiveness detail row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessRow {
    /// 原始约束稳定 ID。 / The stable original-constraint ID.
    pub constraint_id: String,
    /// 漏斗分层。 / The funnel tier.
    pub tier: String,
    /// 该约束的分析状态。 / The analysis status for this constraint.
    pub status: String,
    /// 是否观察到全局改善（`maxImprovement > 0`）。 / Whether a global improvement was observed.
    pub globally_effective: bool,
    /// 是否局部有效（局部结论为真）。 / Whether it is locally effective.
    pub locally_effective: bool,
    /// 已证明的最大目标改善。 / The largest proven objective improvement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_improvement: Option<f64>,
    /// 单删是否改善目标。 / Whether single removal improved the objective.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub removal_effective: Option<bool>,
    /// 未形成结论的原因。 / Why no conclusion was formed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unavailable_reason: Option<String>,
}

/// 有效性排序的校验结论与投影。 / The verification verdict and projections of an effectiveness ranking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectivenessReport {
    /// 报告 schema。 / The report schema.
    pub schema_version: String,
    /// baseline 目标值。 / The baseline objective value.
    pub baseline_objective: f64,
    /// 条目总数。 / The entry count.
    pub entry_count: u32,
    /// 已证明有全局改善的约束 ID（领域自己的 `globally_effective` 投影）。
    /// Constraint IDs with a proven global improvement (the domain's own `globally_effective` projection).
    pub globally_effective_ids: Vec<String>,
    /// 局部有效的约束 ID（领域自己的 `locally_effective` 投影）。
    /// Locally effective constraint IDs (the domain's own `locally_effective` projection).
    pub locally_effective_ids: Vec<String>,
    /// 明细行。 / The detail rows.
    pub rows: Vec<EffectivenessRow>,
}

/// 校验一份有效性排序并给出投影。校验失败即**错误**并原样带出领域原因。
/// Verify one effectiveness ranking and expose its projections. A validation failure is an **error** carrying the domain
/// reason verbatim.
pub fn verify_effectiveness_ranking(json: &str) -> Result<EffectivenessReport> {
    if json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the effectiveness ranking exceeds 8 MiB"));
    }
    let ranking: EffectivenessRanking = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("effectiveness ranking is not decodable: {error}"))
    })?;
    ranking.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("effectiveness ranking failed validation: {error}"))
    })?;
    // 两个投影都走领域自己的方法：不在桥接里重写 "max_improvement > 0" 这类判据。
    // Both projections come from the domain's own methods; predicates such as "max_improvement > 0" are not re-written
    // in the bridge.
    let globally_effective_ids = ranking
        .globally_effective()
        .into_iter()
        .map(|entry| entry.constraint_id.0.clone())
        .collect();
    let locally_effective_ids = ranking
        .locally_effective()
        .into_iter()
        .map(|entry| entry.constraint_id.0.clone())
        .collect();
    Ok(EffectivenessReport {
        schema_version: ranking.schema_version.clone(),
        baseline_objective: ranking.baseline_objective,
        entry_count: u32::try_from(ranking.entries.len()).unwrap_or(u32::MAX),
        globally_effective_ids,
        locally_effective_ids,
        rows: ranking.entries.iter().map(row).collect(),
    })
}

/// 把一条领域条目投影成扁平行。 / Project one domain entry into a flat row.
fn row(entry: &EffectivenessEntry) -> EffectivenessRow {
    EffectivenessRow {
        constraint_id: entry.constraint_id.0.clone(),
        tier: entry.tier.as_str().to_owned(),
        status: status_name(entry.status).to_owned(),
        globally_effective: entry.is_globally_effective(),
        locally_effective: entry.local_effective == Some(true),
        max_improvement: entry.max_improvement,
        removal_effective: entry.removal_effective,
        unavailable_reason: entry.unavailable_reason.clone(),
    }
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
    use ospf_rust_core::analysis::activity::ActivityStatus;
    use ospf_rust_core::analysis::pipeline::{CandidateTier, CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION};
    use ospf_rust_core::AnalysisStatus;
    use ospf_rust_core::solver::StableConstraintId as ConstraintId;

    fn entry(constraint_id: &str, tier: CandidateTier) -> EffectivenessEntry {
        EffectivenessEntry {
            constraint_id: ConstraintId(constraint_id.to_owned()),
            group: Some("g".to_owned()),
            tier,
            activity: ActivityStatus::Active,
            dual_value: Some(1.0),
            local_effective: None,
            removal_effective: None,
            first_effective_delta: None,
            max_improvement: None,
            marginal_effect: None,
            status: AnalysisStatus::Unknown,
            unavailable_reason: None,
        }
    }

    /// 一份自洽的排序：`hot` 有全局改善且局部有效、`warm` 单删有改善但无全局改善、`cold` 无任何结论。
    /// A self-consistent ranking: `hot` has a global improvement and is locally effective, `warm` improves under single
    /// removal without a global improvement, `cold` has no conclusion at all.
    fn ranking() -> EffectivenessRanking {
        let mut hot = entry("hot", CandidateTier::TierA);
        hot.local_effective = Some(true);
        hot.max_improvement = Some(0.5);
        hot.first_effective_delta = Some(0.1);
        hot.marginal_effect = Some(5.0);
        hot.removal_effective = Some(true);
        hot.status = AnalysisStatus::Reachable;

        let mut warm = entry("warm", CandidateTier::TierB);
        warm.max_improvement = Some(0.0);
        warm.removal_effective = Some(true);
        warm.status = AnalysisStatus::Reachable;

        let mut cold = entry("cold", CandidateTier::TierC);
        cold.dual_value = None;
        cold.status = AnalysisStatus::Unsupported;
        cold.unavailable_reason = Some("no dual under the fixed pattern".to_owned());

        EffectivenessRanking {
            schema_version: CRITICAL_ANALYSIS_REPORT_SCHEMA_VERSION.to_owned(),
            baseline_objective: 10.0,
            entries: vec![hot, warm, cold],
        }
    }

    fn encode(ranking: &EffectivenessRanking) -> String {
        serde_json::to_string(ranking).expect("serialize ranking")
    }

    #[test]
    fn valid_ranking_projects_both_domain_views() {
        let report = verify_effectiveness_ranking(&encode(&ranking())).unwrap();
        assert_eq!(report.entry_count, 3);
        assert_eq!(report.baseline_objective, 10.0);
        // 全局投影只包含 max_improvement > 0 的那一条；`warm` 的 0.0 不算改善。
        // The global projection holds only the entry with max_improvement > 0; `warm`'s 0.0 is not an improvement.
        assert_eq!(report.globally_effective_ids, vec!["hot".to_owned()]);
        // 局部投影只看 local_effective 结论，与全局投影互不覆盖。
        // The local projection only looks at `local_effective` and never overwrites the global one.
        assert_eq!(report.locally_effective_ids, vec!["hot".to_owned()]);
        assert_eq!(report.rows.len(), 3);
        assert_eq!(report.rows[0].tier, "A");
        assert_eq!(report.rows[0].status, "reachable");
        assert!(report.rows[0].globally_effective);
        assert!(report.rows[0].locally_effective);
        assert_eq!(report.rows[0].max_improvement, Some(0.5));
        assert_eq!(report.rows[1].max_improvement, Some(0.0));
        assert!(!report.rows[1].globally_effective);
        assert_eq!(report.rows[2].tier, "C");
        assert_eq!(report.rows[2].status, "unsupported");
        assert_eq!(report.rows[2].unavailable_reason.as_deref(),
            Some("no dual under the fixed pattern"));
    }

    #[test]
    fn broken_rankings_are_refused_with_the_domain_reason() {
        let cases: Vec<(&str, Box<dyn Fn(&mut EffectivenessRanking)>, &str)> = vec![
            ("blank identity", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.entries[0].constraint_id = ConstraintId("  ".to_owned());
            }), "stable constraint identity"),
            ("duplicate constraints", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.entries[1].constraint_id = ConstraintId("hot".to_owned());
            }), "duplicate"),
            // 实测：`serde_json` 把 `f64::INFINITY`/`NaN` 序列化成 `null`，所以"非有限值"这条守卫**无法**通过线格式
            // 触发——它只在 Rust 内构造时可达。这里改用可达的负 delta，别假装能测到不可达的分支。
            // Measured: `serde_json` serializes `f64::INFINITY`/`NaN` as `null`, so the non-finite guard cannot be
            // triggered through the wire at all — it is only reachable from in-process Rust. A reachable negative delta is
            // used instead of pretending to test an unreachable branch.
            ("negative first delta", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.entries[0].first_effective_delta = Some(-0.5);
            }), "non-positive"),
            ("zero first delta", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.entries[0].first_effective_delta = Some(0.0);
            }), "non-positive"),
            ("local verdict without a dual", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.entries[2].local_effective = Some(true);
            }), "available dual"),
            ("removal verdict without a supported test", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.entries[2].status = AnalysisStatus::Unsupported;
                ranking.entries[2].removal_effective = Some(true);
            }), "supported removal test"),
            ("non-finite baseline", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.baseline_objective = f64::NAN;
            }), "invalid"),
            ("blank schema", Box::new(|ranking: &mut EffectivenessRanking| {
                ranking.schema_version = "   ".to_owned();
            }), "invalid"),
        ];
        for (label, mutate, expected) in cases {
            let mut broken = ranking();
            mutate(&mut broken);
            let outcome = verify_effectiveness_ranking(&encode(&broken));
            let error = match outcome {
                Ok(_) => panic!("case `{label}` must be refused"),
                Err(error) => error,
            };
            assert_eq!(error.code, "InvalidInput", "{label}");
            assert!(error.message.contains(expected), "{label}: {}", error.message);
        }
        assert_eq!(verify_effectiveness_ranking("not json").unwrap_err().code, "InvalidInput");
        assert_eq!(verify_effectiveness_ranking(&"x".repeat(9 * 1024 * 1024)).unwrap_err().code,
            "InvalidInput");
    }
}
