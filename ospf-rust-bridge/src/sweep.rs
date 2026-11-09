//! 同一模型的多后端扫描：每个 case 独立求解，结果按 case ID 稳定排序。
//! Multi-backend sweep over one model: each case solves independently and results are sorted stably by case ID.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, ReportSummary, Result};

/// 一个扫描 case：同一模型，不同后端或预算。
/// One sweep case: the same model under a different backend or budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearSweepCase {
    /// 稳定 case ID；结果按它排序，必须互不相同。 / Stable case ID; results are sorted by it and it must be unique.
    pub case_id: String,
    /// 要使用的后端名，例如 `scip`、`gurobi`。 / Backend name such as `scip` or `gurobi`.
    pub solver: String,
    /// 该 case 的单次时间上限（毫秒）；省略表示沿用调用方共享预算。
    /// Per-case time limit in milliseconds; omission inherits the caller's shared budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub time_limit_ms: Option<u64>,
}

impl LinearSweepCase {
    /// 单个扫描允许的 case 上限，防止一次请求无限放大工作量。
    /// Upper bound on cases per sweep, so one request cannot multiply work without limit.
    pub const MAX_CASES: usize = 16;

    /// 校验 case 列表；错误一次性暴露，不静默丢弃任何 case。
    /// Validate the case list; problems surface at once and no case is silently dropped.
    pub fn validate(cases: &[Self]) -> Result<()> {
        if cases.is_empty() {
            return Err(BridgeError::new("InvalidInput", "linear sweep requires at least one case"));
        }
        if cases.len() > Self::MAX_CASES {
            return Err(BridgeError::new("InvalidInput", "linear sweep accepts at most 16 cases"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for case in cases {
            if case.case_id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "linear sweep case ID must not be empty"));
            }
            if case.solver.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "linear sweep case solver must not be empty"));
            }
            if case.time_limit_ms == Some(0) {
                return Err(BridgeError::new("InvalidInput", "linear sweep case time limit must be positive"));
            }
            if !seen.insert(case.case_id.as_str()) {
                return Err(BridgeError::new("InvalidInput", "linear sweep repeats a case ID"));
            }
        }
        Ok(())
    }
}

/// 单个 case 的结果：要么带摘要，要么带错误，绝不同时给两者或都不给。
/// Result of one case: either a summary or an error, never both and never neither.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearSweepCaseResult {
    /// 与请求一致的 case ID。 / The case ID from the request.
    pub case_id: String,
    /// 求解摘要；成功时给出。 / Solve summary, present on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub summary: Option<ReportSummary>,
    /// 失败错误码；失败时给出。 / Failure code, present on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_code: Option<String>,
    /// 失败说明。 / Failure message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_message: Option<String>,
}

/// 扫描报告：模型指纹 + 按 case ID 排序的逐 case 结果。
/// Sweep report: the model fingerprint plus per-case results sorted by case ID.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearSweepReport {
    /// 被扫描的模型指纹，所有 case 共用。 / Fingerprint of the swept model, shared by every case.
    pub model_fingerprint: String,
    /// 与请求 case 数一致的逐 case 结果。 / Per-case results, one per requested case.
    pub cases: Vec<LinearSweepCaseResult>,
}

impl LinearSweepReport {
    /// 校验报告不变量：每个 case 恰好有一个结论。 / Validate report invariants: each case reaches exactly one verdict.
    pub fn validate(&self) -> Result<()> {
        if self.model_fingerprint.trim().is_empty() {
            return Err(BridgeError::new("InvalidReport", "sweep report has no model fingerprint"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for case in &self.cases {
            if !seen.insert(case.case_id.as_str()) {
                return Err(BridgeError::new("InvalidReport", "sweep report repeats a case ID"));
            }
            match (&case.summary, &case.error_code) {
                (Some(_), None) | (None, Some(_)) => {}
                _ => return Err(BridgeError::new("InvalidReport", "sweep report case must carry exactly one verdict")),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_cases_reject_duplicates_empty_identities_and_zero_budgets() {
        let valid = LinearSweepCase { case_id: "a".into(), solver: "scip".into(), time_limit_ms: Some(1000) };
        assert!(LinearSweepCase::validate(&[valid.clone()]).is_ok());
        for broken in [
            vec![],
            vec![LinearSweepCase { case_id: String::new(), ..valid.clone() }],
            vec![LinearSweepCase { solver: String::new(), ..valid.clone() }],
            vec![LinearSweepCase { time_limit_ms: Some(0), ..valid.clone() }],
            vec![valid.clone(), valid.clone()],
            (0..=LinearSweepCase::MAX_CASES)
                .map(|index| LinearSweepCase { case_id: format!("c{index}"), ..valid.clone() })
                .collect(),
        ] {
            assert!(LinearSweepCase::validate(&broken).is_err(), "{broken:?}");
        }
    }

    #[test]
    fn sweep_reports_never_carry_two_or_zero_verdicts() {
        let summary = || ReportSummary { problem_status: "feasible".into(), termination_reason: "completed".into(),
            objective_value: None, values: Vec::new(), has_optimality_proof: false, relative_gap: None, absolute_gap: None,
            nodes: None, solve_time_ms: 0 };
        let case = |summary, error_code| LinearSweepCaseResult { case_id: "a".into(),
            summary, error_code, error_message: None };
        let report = |cases| LinearSweepReport { model_fingerprint: "f".into(), cases };
        assert!(report(vec![case(Some(summary()), None)]).validate().is_ok());
        assert!(report(vec![case(None, Some("ExecutionFailure".into()))]).validate().is_ok());
        // 两个结论或没有结论都不允许。 / Either two verdicts or none is rejected.
        assert!(report(vec![case(Some(summary()), Some("ExecutionFailure".into()))]).validate().is_err());
        assert!(report(vec![case(None, None)]).validate().is_err());
        // 重复 case ID 与缺指纹同样拒绝。 / A repeated case ID and a missing fingerprint are rejected too.
        assert!(report(vec![case(Some(summary()), None), case(Some(summary()), None)]).validate().is_err());
        assert!(LinearSweepReport { model_fingerprint: String::new(), cases: Vec::new() }.validate().is_err());
    }
}
