//! 批次 B：`core/analysis` 的**冲突解释校验** / Batch B: conflict-explanation verification from `core/analysis`.
//!
//! `ConflictExplanation::validate()` 是纯数据的领域校验：目标来源必须与目标一致、成员不得重复、不得把目标本身
//! 当作成员、`Irreducible` 必须是"已验证不可达 + 至少一个阻塞成员"、`background_blocking` 必须真的没有任何
//! 约束成员。这些规则全部可以脱离求解器执行，因此可以跨线。
//! `ConflictExplanation::validate()` is pure-data domain validation: the target source must match the target, members
//! must not repeat, the target itself must not be a member, `Irreducible` requires a verified-unreachable target with at
//! least one blocking member, and `background_blocking` must really have no constraint member. All of that runs without a
//! solver, so it can cross the wire.

use ospf_rust_core::analysis::{ConflictExplanation, ConflictMinimality, ConflictValidity};
use ospf_rust_core::analysis::AnalysisStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 冲突按组汇总的一行。 / One conflict group-summary row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConflictGroupEntry {
    /// 组名；未分组为空。 / The group name; absent when ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<String>,
    /// 组内剩余约束数。 / The number of remaining constraints in the group.
    pub count: u32,
}

/// 冲突解释的校验结论与摘要。 / The verification verdict and summary of a conflict explanation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConflictExplanationSummary {
    /// 报告 schema。 / The report schema.
    pub schema_version: String,
    /// 聚合结论（`reachable`/`unreachable`/`unknown`/`unsupported`）。
    /// The aggregate conclusion.
    pub status: String,
    /// 有效性（`verified`/`unknown`）。 / The validity.
    pub validity: String,
    /// 最小性（`irreducible`/`partial`/`notChecked`）。 / The minimality.
    pub minimality: String,
    /// 冲突成员数。 / The conflict member count.
    pub member_count: u32,
    /// 仅原始约束的稳定 ID，按字典序。 / The stable IDs of constraint members only, in lexicographic order.
    pub constraint_ids: Vec<String>,
    /// 固定背景证据数。 / The fixed-background evidence count.
    pub fixed_background_count: u32,
    /// 删除复验记录数。 / The deletion-verification record count.
    pub verification_count: u32,
    /// 按组汇总。 / The per-group summary.
    pub group_summaries: Vec<ConflictGroupEntry>,
    /// 目标/删除复验的真实求解次数。 / The real solve count for target and deletion checks.
    pub resolve_count: u32,
    /// 被记忆化省下的求解次数。 / The number of solves avoided by memoization.
    pub memoized_checks: u32,
    /// 实际使用的冲突提取层级。 / The conflict-extraction tier actually used.
    pub extraction_tier: String,
    /// 阻塞是否仅来自固定背景。 / Whether blocking comes only from the fixed background.
    pub background_blocking: bool,
    /// 失败或部分完成的原因。 / Why the analysis failed or was partial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unavailable_reason: Option<String>,
}

/// 校验一份冲突解释并给出摘要。校验失败即**错误**并原样带出领域原因，不返回"校验未通过"的假成功。
/// Verify one conflict explanation and summarize it. A validation failure is an **error** carrying the domain reason
/// verbatim; it never returns a fake "did not validate" success.
pub fn verify_conflict_explanation(json: &str) -> Result<ConflictExplanationSummary> {
    if json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the conflict explanation exceeds 8 MiB"));
    }
    let explanation: ConflictExplanation = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("conflict explanation is not decodable: {error}"))
    })?;
    explanation.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("conflict explanation failed validation: {error}"))
    })?;
    // 约束 ID 走领域自己的投影（`constraint_ids()`），不在这里重新筛一遍类型——两套筛选迟早会不一致。
    // Constraint IDs come from the domain's own projection (`constraint_ids()`) instead of being re-filtered here;
    // two filters would drift apart.
    let constraint_ids = explanation.constraint_ids().into_iter().map(|id| id.0).collect();
    Ok(ConflictExplanationSummary {
        schema_version: explanation.schema_version.clone(),
        status: status_name(explanation.status).to_owned(),
        validity: validity_name(explanation.validity).to_owned(),
        minimality: minimality_name(explanation.minimality).to_owned(),
        member_count: u32::try_from(explanation.members.len()).unwrap_or(u32::MAX),
        constraint_ids,
        fixed_background_count: u32::try_from(explanation.fixed_background.len()).unwrap_or(u32::MAX),
        verification_count: u32::try_from(explanation.verifications.len()).unwrap_or(u32::MAX),
        group_summaries: explanation.group_summaries.iter().map(|summary| ConflictGroupEntry {
            group: summary.group.clone(),
            count: u32::try_from(summary.count).unwrap_or(u32::MAX),
        }).collect(),
        resolve_count: u32::try_from(explanation.resolve_count).unwrap_or(u32::MAX),
        memoized_checks: u32::try_from(explanation.memoized_checks).unwrap_or(u32::MAX),
        extraction_tier: format!("{:?}", explanation.extraction_tier),
        background_blocking: explanation.background_blocking,
        unavailable_reason: explanation.unavailable_reason.clone(),
    })
}

/// 分析状态名。 / The analysis status name.
fn status_name(status: AnalysisStatus) -> &'static str {
    match status {
        AnalysisStatus::Reachable => "reachable",
        AnalysisStatus::Unreachable => "unreachable",
        AnalysisStatus::Unknown => "unknown",
        AnalysisStatus::Unsupported => "unsupported",
    }
}

/// 有效性名。 / The validity name.
fn validity_name(validity: ConflictValidity) -> &'static str {
    match validity {
        ConflictValidity::Verified => "verified",
        ConflictValidity::Unknown => "unknown",
    }
}

/// 最小性名。 / The minimality name.
fn minimality_name(minimality: ConflictMinimality) -> &'static str {
    match minimality {
        ConflictMinimality::Irreducible => "irreducible",
        ConflictMinimality::Partial => "partial",
        ConflictMinimality::NotChecked => "notChecked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::analysis::{
        ConflictExtractionTier, ConflictGroupSummary, DiagnosticSource, ObjectiveTarget,
        TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::{ProofStatus, StableConstraintId};

    const SCHEMA: &str = ospf_rust_core::analysis::conflict::CONFLICT_REPORT_SCHEMA_VERSION;

    fn target() -> ObjectiveTarget {
        ObjectiveTarget::at_least("objective", 5.0).expect("target")
    }

    /// 一份**自洽的最小冲突解释**：不可约 + 已验证不可达 + 一个约束成员。
    /// A **self-consistent minimal conflict explanation**: irreducible, verified unreachable, one constraint member.
    fn explanation() -> ConflictExplanation {
        let target = target();
        ConflictExplanation {
            schema_version: SCHEMA.to_owned(),
            target: target.clone(),
            target_feasibility: TargetFeasibilityReport {
                schema_version: SCHEMA.to_owned(),
                target: target.clone(),
                effective_integer_bound: None,
                source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
                status: AnalysisStatus::Unreachable,
                solution: None,
                exact_objective: None,
                objective_value: None,
                proof_status: ProofStatus::Verified,
                termination_reason: None,
                solve_time: None,
                target_fixed: true,
                message: None,
            },
            status: AnalysisStatus::Unreachable,
            available_evidence: Vec::new(),
            fixed_background: Vec::new(),
            members: vec![DiagnosticSource::Constraint { id: StableConstraintId("capacity".to_owned()) }],
            target_source: DiagnosticSource::ObjectiveTarget { target },
            validity: ConflictValidity::Verified,
            minimality: ConflictMinimality::Irreducible,
            verifications: Vec::new(),
            group_summaries: vec![ConflictGroupSummary { group: Some("capacity".to_owned()), count: 1 }],
            resolve_count: 2,
            memoized_checks: 1,
            extraction_tier: ConflictExtractionTier::AssumptionExtraction,
            unavailable_reason: None,
            background_blocking: false,
        }
    }

    fn encode(explanation: &ConflictExplanation) -> String {
        serde_json::to_string(explanation).expect("serialize conflict explanation")
    }

    #[test]
    fn valid_explanation_is_summarized_from_real_fields() {
        let summary = verify_conflict_explanation(&encode(&explanation())).unwrap();
        assert_eq!(summary.status, "unreachable");
        assert_eq!(summary.validity, "verified");
        assert_eq!(summary.minimality, "irreducible");
        assert_eq!(summary.member_count, 1);
        assert_eq!(summary.constraint_ids, vec!["capacity".to_owned()]);
        assert_eq!(summary.group_summaries,
            vec![ConflictGroupEntry { group: Some("capacity".to_owned()), count: 1 }]);
        assert_eq!(summary.resolve_count, 2);
        assert_eq!(summary.memoized_checks, 1);
        assert_eq!(summary.extraction_tier, "AssumptionExtraction");
        assert!(!summary.background_blocking);
        assert_eq!(summary.unavailable_reason, None);
        assert_eq!(summary.schema_version, SCHEMA);
    }

    #[test]
    fn broken_explanations_are_refused_with_the_domain_reason() {
        // 重复成员：同一约束出现两次。 / A duplicate member: the same constraint twice.
        let mut duplicate = explanation();
        duplicate.members.push(DiagnosticSource::Constraint { id: StableConstraintId("capacity".to_owned()) });
        let error = verify_conflict_explanation(&encode(&duplicate)).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("duplicate"), "{}", error.message);

        // target source 与 target 不一致。 / The target source disagrees with the target.
        let mut mismatched = explanation();
        mismatched.target = ObjectiveTarget::at_most("objective", 1.0).unwrap();
        let error = verify_conflict_explanation(&encode(&mismatched)).unwrap_err();
        assert!(error.message.contains("target source"), "{}", error.message);

        // 目标本身当成员：拒绝。 / The target used as a member: refused.
        let mut target_member = explanation();
        target_member.members.push(DiagnosticSource::ObjectiveTarget { target: target() });
        let error = verify_conflict_explanation(&encode(&target_member)).unwrap_err();
        assert!(error.message.contains("original model evidence"), "{}", error.message);

        // 不可约但没有成员：拒绝。 / Irreducible with no member: refused.
        let mut empty_irreducible = explanation();
        empty_irreducible.members.clear();
        let error = verify_conflict_explanation(&encode(&empty_irreducible)).unwrap_err();
        assert!(error.message.contains("at least one blocking"), "{}", error.message);

        // 不可约但未验证不可达：拒绝。 / Irreducible without a verified unreachable target: refused.
        let mut unverified = explanation();
        unverified.status = AnalysisStatus::Unknown;
        let error = verify_conflict_explanation(&encode(&unverified)).unwrap_err();
        assert!(error.message.contains("verified unreachable"), "{}", error.message);

        // background_blocking 却带着约束成员：拒绝。 / `background_blocking` while carrying a constraint member.
        let mut background = explanation();
        background.background_blocking = true;
        let error = verify_conflict_explanation(&encode(&background)).unwrap_err();
        assert!(error.message.contains("background blocking"), "{}", error.message);

        // 不是 JSON / 超限：入口拒绝。 / Not JSON / over the limit: refused at the entry.
        assert_eq!(verify_conflict_explanation("not json").unwrap_err().code, "InvalidInput");
        assert_eq!(verify_conflict_explanation(&"x".repeat(9 * 1024 * 1024)).unwrap_err().code, "InvalidInput");
    }
}
