//! 批次 B 第 8 单：目标构造与可行性报告 / Batch B, delivery 8: target construction and feasibility report.
//!
//! 这是批次 B 里"业务意图 → 精确 CP 边界"的那一层，全部是纯函数：
//!
//! * `ObjectiveTarget::at_least` / `at_most` / `at_least_relative` / `at_most_relative` /
//!   `at_least_business_unit` / `at_most_business_unit`——六种真实构造器，构造时即校验；
//! * `exact_integer_bound`：`AtLeast` 取 **ceil**、`AtMost` 取 **floor**，超出 i64 范围返回 `None`（不是错误）；
//! * `target_constraint_id` / `stable_id`：稳定的 target 身份；
//! * `TargetFeasibilityReport::{validate,reachable,is_proven}`。
//!
//! 把 ceil/floor 与 i64 边界规则搬进 TS 是最容易搬错的一件事，所以这一层必须跨线。
//! This is batch B's "business intent → exact CP bound" layer, all pure functions: six real constructors, the ceil/floor
//! integer-bound derivation (`AtLeast` ceils, `AtMost` floors, out-of-i64-range yields `None` rather than an error), the
//! stable identities, and the report's `validate` / `reachable` / `is_proven`. Re-implementing the ceil/floor and range
//! rules in TypeScript is the easiest thing to get wrong, which is exactly why this layer crosses the wire.

use ospf_rust_core::analysis::target::{exact_integer_bound, target_constraint_id, TargetFeasibilityReport};
use ospf_rust_core::analysis::ObjectiveTarget;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 目标的构造方式。三种模式对应领域里六种构造器；`relation` 决定走上下界的哪一支。
/// How to build the target. The three modes cover the domain's six constructors; `relation` picks the upper or lower
/// branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum TargetSpec {
    /// 绝对边界：`value` 即目标值。 / An absolute bound: `value` is the target.
    #[serde(rename_all = "camelCase")]
    Absolute {
        /// 目标身份。 / The objective identity.
        objective_id: String,
        /// `atLeast` 或 `atMost`。 / Either `atLeast` or `atMost`.
        relation: String,
        /// 绝对目标值。 / The absolute target value.
        value: f64,
    },
    /// 相对变化：`baseline × (1 + fraction)`。 / A relative change: `baseline × (1 + fraction)`.
    #[serde(rename_all = "camelCase")]
    Relative {
        /// 目标身份。 / The objective identity.
        objective_id: String,
        /// `atLeast` 或 `atMost`。 / Either `atLeast` or `atMost`.
        relation: String,
        /// 基线目标值。 / The baseline objective value.
        baseline: f64,
        /// 相对变化比例（`0.05` 表示改善 5%）。 / The fractional change (`0.05` means a 5% improvement).
        fraction: f64,
    },
    /// 业务单位增量：`baseline + delta`。 / A business-unit delta: `baseline + delta`.
    #[serde(rename_all = "camelCase")]
    BusinessUnit {
        /// 目标身份。 / The objective identity.
        objective_id: String,
        /// `atLeast` 或 `atMost`。 / Either `atLeast` or `atMost`.
        relation: String,
        /// 基线目标值。 / The baseline objective value.
        baseline: f64,
        /// 业务单位增量。 / The business-unit delta.
        delta: f64,
    },
}

/// 归一化后的目标。 / The normalized target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetEcho {
    /// 目标身份。 / The objective identity.
    pub objective_id: String,
    /// 方向（`AtLeast`/`AtMost`）。 / The direction.
    pub relation: String,
    /// 归一化后的绝对目标值。 / The normalized absolute target value.
    pub value: f64,
    /// 稳定 target 身份（`objective-target:<id>:<relation>:<value>`）。 / The stable target identity.
    pub stable_id: String,
    /// 插入 CP 模型时使用的稳定约束 ID。 / The stable constraint ID used when inserting into a CP model.
    pub constraint_id: String,
    /// 精确整数边界；超出 i64 范围时为空（**不是错误**）。 / The exact integer bound; absent when out of i64 range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exact_integer_bound: Option<i64>,
}

/// 可行性报告摘要。 / The feasibility report summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetReportSummary {
    /// 报告 schema。 / The report schema.
    pub schema_version: String,
    /// 可达性结论（`reachable`/`unreachable`/`unknown`/`unsupported`）。 / The reachability conclusion.
    pub status: String,
    /// 三值可达性：`true`/`false`，未知时为 `null`。 / Ternary reachability: true, false, or null when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reachable: Option<bool>,
    /// 是否有证明。 / Whether the conclusion is proven.
    pub is_proven: bool,
    /// 报告中记录的实际整数边界。 / The integer bound recorded in the report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effective_integer_bound: Option<i64>,
    /// 是否带见证解。 / Whether a witness solution is attached.
    pub has_solution: bool,
    /// 浮点兼容目标值。 / The floating-point compatibility objective value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub objective_value: Option<f64>,
    /// 精确目标值。 / The exact objective value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exact_objective: Option<i64>,
    /// 后端证明状态（`none`/`claimed`/`verified`）。 / The backend proof status.
    pub proof_status: String,
    /// 是否确实插入了 target 约束。 / Whether a target constraint was actually inserted.
    pub target_fixed: bool,
    /// 报告自带的 target 来源稳定 ID（可用来核对与请求 target 是否一致）。
    /// The report's own target-source stable ID (usable to check agreement with the requested target).
    pub target_source_id: String,
    /// 面向诊断的详情。 / The diagnostic detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message: Option<String>,
}

/// 目标构造与报告校验的结论。 / The target construction and report verification verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetVerification {
    /// 归一化后的目标。 / The normalized target.
    pub target: TargetEcho,
    /// 报告摘要；未给报告时为空。 / The report summary; absent when no report was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub report: Option<TargetReportSummary>,
}

/// 按请求构造目标，并在给出报告时校验与投影报告。非法输入是**错误**，原因原样带出。
/// Build the requested target and, when a report is supplied, verify and project it. An illegal input is an **error**
/// carrying the reason verbatim.
pub fn verify_target(spec: &TargetSpec, report_json: Option<&str>) -> Result<TargetVerification> {
    // 六种构造器全部走领域实现：构造时就校验身份与有限性，桥接不重复实现那两条规则。
    // All six constructors go through the domain: they validate the identity and finiteness on construction, so the
    // bridge does not re-implement those two rules.
    let target = match spec {
        TargetSpec::Absolute { objective_id, relation, value } => match relation.as_str() {
            "atLeast" => ObjectiveTarget::at_least(objective_id.as_str(), *value),
            "atMost" => ObjectiveTarget::at_most(objective_id.as_str(), *value),
            other => return Err(unsupported_relation(other)),
        },
        TargetSpec::Relative { objective_id, relation, baseline, fraction } => match relation.as_str() {
            "atLeast" => ObjectiveTarget::at_least_relative(objective_id.as_str(), *baseline, *fraction),
            "atMost" => ObjectiveTarget::at_most_relative(objective_id.as_str(), *baseline, *fraction),
            other => return Err(unsupported_relation(other)),
        },
        TargetSpec::BusinessUnit { objective_id, relation, baseline, delta } => match relation.as_str() {
            "atLeast" => ObjectiveTarget::at_least_business_unit(objective_id.as_str(), *baseline, *delta),
            "atMost" => ObjectiveTarget::at_most_business_unit(objective_id.as_str(), *baseline, *delta),
            other => return Err(unsupported_relation(other)),
        },
    }
    .map_err(|error| {
        BridgeError::new("InvalidInput", &format!("objective target failed validation: {error}"))
    })?;
    let echo = TargetEcho {
        objective_id: target.objective_id().as_str().to_owned(),
        relation: format!("{:?}", target.relation()),
        value: target.value(),
        stable_id: target.stable_id(),
        constraint_id: target_constraint_id(&target).0,
        // 超出 i64 范围时领域返回 `None`：那是"无法用整数精确表示"，不是错误。
        // The domain returns `None` beyond the i64 range: that means "not exactly representable as an integer", not an
        // error.
        exact_integer_bound: exact_integer_bound(&target),
    };
    let report = report_json.map(|json| verify_report(json, &target, &echo)).transpose()?;
    Ok(TargetVerification { target: echo, report })
}

/// 关系名非法。 / An unsupported relation name.
fn unsupported_relation(relation: &str) -> BridgeError {
    BridgeError::new("InvalidInput", &format!(
        "objective target failed validation: unsupported relation `{relation}` (expected `atLeast` or `atMost`)"))
}

/// 校验并投影一份可行性报告。 / Verify and project one feasibility report.
fn verify_report(json: &str, target: &ObjectiveTarget, echo: &TargetEcho) -> Result<TargetReportSummary> {
    if json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the target report exceeds 8 MiB"));
    }
    let report: TargetFeasibilityReport = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("target report is not decodable: {error}"))
    })?;
    report.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("target report failed validation: {error}"))
    })?;
    // 领域校验已经保证 `source` 等于由 `target` 推出的来源；这里再显式核对一次"报告里的 target 与请求的 target 一致"，
    // 因为请求只给了 `objective_id`/`relation`/`value`，报告可能来自另一个 target。
    // Domain validation already guarantees the source matches the report's own target; this explicitly checks that the
    // report's target agrees with the *requested* one, since the request only carried id/relation/value.
    if report.target.stable_id() != target.stable_id() {
        return Err(BridgeError::new("InvalidInput", &format!(
            "target report describes `{}` but the request asked for `{}`",
            report.target.stable_id(), target.stable_id())));
    }
    Ok(TargetReportSummary {
        schema_version: report.schema_version.clone(),
        status: status_name(report.status).to_owned(),
        reachable: report.reachable(),
        is_proven: report.is_proven(),
        effective_integer_bound: report.effective_integer_bound,
        has_solution: report.solution.is_some(),
        objective_value: report.objective_value,
        exact_objective: report.exact_objective,
        proof_status: proof_status_name(report.proof_status),
        target_fixed: report.target_fixed,
        target_source_id: report.source.stable_id(),
        message: report.message.clone(),
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

/// 证明状态名。 / The proof status name.
fn proof_status_name(status: ospf_rust_core::solver::report::ProofStatus) -> String {
    use ospf_rust_core::solver::report::ProofStatus;
    match status {
        ProofStatus::None => "none",
        ProofStatus::Claimed => "claimed",
        ProofStatus::Verified => "verified",
    }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::analysis::{
        AnalysisStatus, DiagnosticSource, TargetFeasibilityReport,
    };
    use ospf_rust_core::solver::report::ProofStatus;

    fn absolute(relation: &str, value: f64) -> TargetSpec {
        TargetSpec::Absolute { objective_id: "objective".to_owned(), relation: relation.to_owned(), value }
    }

    fn report_for(target: &ObjectiveTarget, status: AnalysisStatus) -> TargetFeasibilityReport {
        TargetFeasibilityReport {
            schema_version: "1.0".to_owned(),
            target: target.clone(),
            effective_integer_bound: exact_integer_bound(target),
            source: DiagnosticSource::ObjectiveTarget { target: target.clone() },
            status,
            solution: None,
            exact_objective: None,
            objective_value: None,
            proof_status: ProofStatus::Verified,
            termination_reason: None,
            solve_time: None,
            target_fixed: true,
            message: None,
        }
    }

    #[test]
    fn absolute_targets_take_ceil_for_at_least_and_floor_for_at_most() {
        // 这是最容易在 TS 里搬错的一条：`AtLeast` 向上取整、`AtMost` 向下取整。
        // The easiest rule to get wrong in TypeScript: `AtLeast` ceils and `AtMost` floors.
        let verification = verify_target(&absolute("atLeast", 4.2), None).unwrap();
        assert_eq!(verification.target.value, 4.2);
        assert_eq!(verification.target.exact_integer_bound, Some(5));
        assert_eq!(verification.target.relation, "AtLeast");
        assert_eq!(verification.target.objective_id, "objective");
        assert!(verification.target.constraint_id.contains(&verification.target.stable_id));

        let verification = verify_target(&absolute("atMost", 4.8), None).unwrap();
        assert_eq!(verification.target.exact_integer_bound, Some(4));
        assert_eq!(verification.target.relation, "AtMost");

        // 负值同理：`-4.2` 上取整得 `-4`，`-4.8` 下取整得 `-5`。
        // The same holds for negatives: -4.2 ceils to -4 and -4.8 floors to -5.
        assert_eq!(verify_target(&absolute("atLeast", -4.2), None).unwrap()
            .target.exact_integer_bound, Some(-4));
        assert_eq!(verify_target(&absolute("atMost", -4.8), None).unwrap()
            .target.exact_integer_bound, Some(-5));

        // 超出 i64 范围：返回 `None` 而不是错误。 / Beyond the i64 range: `None` rather than an error.
        let verification = verify_target(&absolute("atLeast", 1e30), None).unwrap();
        assert_eq!(verification.target.exact_integer_bound, None);
        assert_eq!(verification.target.value, 1e30);
    }

    #[test]
    fn relative_and_business_unit_modes_fold_into_an_absolute_value() {
        let relative = TargetSpec::Relative {
            objective_id: "objective".to_owned(), relation: "atLeast".to_owned(),
            baseline: 100.0, fraction: 0.05 };
        let verification = verify_target(&relative, None).unwrap();
        assert_eq!(verification.target.value, 105.0);
        assert_eq!(verification.target.exact_integer_bound, Some(105));

        // 负比例同样成立（"改善 10%" 对最小化目标意味着更大的空间，这里只验证算术）。
        // A negative fraction works too (only the arithmetic is verified here).
        let relative = TargetSpec::Relative {
            objective_id: "objective".to_owned(), relation: "atMost".to_owned(),
            baseline: 100.0, fraction: -0.1 };
        assert_eq!(verify_target(&relative, None).unwrap().target.value, 90.0);

        let business = TargetSpec::BusinessUnit {
            objective_id: "objective".to_owned(), relation: "atLeast".to_owned(),
            baseline: 100.0, delta: 5.0 };
        let verification = verify_target(&business, None).unwrap();
        assert_eq!(verification.target.value, 105.0);
        assert_eq!(verification.target.exact_integer_bound, Some(105));
    }

    #[test]
    fn report_is_checked_against_the_requested_target() {
        let target = ObjectiveTarget::at_least("objective", 5.0).unwrap();
        let json = serde_json::to_string(&report_for(&target, AnalysisStatus::Reachable)).unwrap();
        // `Reachable` 却没有见证解：领域拒绝。 / `Reachable` without a witness: refused by the domain.
        let error = verify_target(&absolute("atLeast", 5.0), Some(&json)).unwrap_err();
        assert!(error.message.contains("witness"), "{}", error.message);

        let json = serde_json::to_string(&report_for(&target, AnalysisStatus::Unreachable)).unwrap();
        let verification = verify_target(&absolute("atLeast", 5.0), Some(&json)).unwrap();
        let summary = verification.report.expect("report summary");
        assert_eq!(summary.status, "unreachable");
        assert_eq!(summary.reachable, Some(false));
        assert_eq!(summary.effective_integer_bound, Some(5));
        assert!(!summary.has_solution);
        assert_eq!(summary.proof_status, "verified");
        assert!(summary.target_fixed);
        assert!(summary.target_source_id.starts_with("objective-target:"));

        // 报告描述的目标与请求不符：拒绝并点名两边。 / A report describing a different target: refused, naming both.
        let error = verify_target(&absolute("atLeast", 7.0), Some(&json)).unwrap_err();
        assert!(error.message.contains("but the request asked for"), "{}", error.message);

        // 未知状态的 `reachable` 是三值里的 `None`。 / For an unknown status `reachable` is the ternary `None`.
        let mut unknown = report_for(&target, AnalysisStatus::Unknown);
        unknown.effective_integer_bound = None;
        let json = serde_json::to_string(&unknown).unwrap();
        assert_eq!(verify_target(&absolute("atLeast", 5.0), Some(&json)).unwrap()
            .report.expect("report").reachable, None);
    }

    #[test]
    fn illegal_specs_are_refused_with_the_domain_reason() {
        // 空白目标身份：领域拒绝。 / A blank objective identity: refused by the domain.
        let blank = TargetSpec::Absolute {
            objective_id: "   ".to_owned(), relation: "atLeast".to_owned(), value: 1.0 };
        let error = verify_target(&blank, None).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("must not be blank"), "{}", error.message);

        // 未知方向：桥接拒绝并列出合法值。 / An unknown relation: refused by the bridge, listing the legal values.
        let error = verify_target(&absolute("greater", 1.0), None).unwrap_err();
        assert!(error.message.contains("unsupported relation"), "{}", error.message);

        // 报告 schema 空白 / 非 JSON。 / A blank report schema and non-JSON.
        let target = ObjectiveTarget::at_least("objective", 5.0).unwrap();
        let mut blank = report_for(&target, AnalysisStatus::Unreachable);
        blank.schema_version = "  ".to_owned();
        let error = verify_target(&absolute("atLeast", 5.0),
            Some(&serde_json::to_string(&blank).unwrap())).unwrap_err();
        assert!(error.message.contains("failed validation"), "{}", error.message);
        assert_eq!(verify_target(&absolute("atLeast", 5.0), Some("not json")).unwrap_err().code,
            "InvalidInput");

        // `effective_integer_bound` 不是由 target 精确推出的值：领域拒绝。
        // An `effective_integer_bound` that is not the exact bound implied by the target: refused by the domain.
        let mut drifted = report_for(&target, AnalysisStatus::Unreachable);
        drifted.effective_integer_bound = Some(99);
        let error = verify_target(&absolute("atLeast", 5.0),
            Some(&serde_json::to_string(&drifted).unwrap())).unwrap_err();
        assert!(error.message.contains("exact target bound"), "{}", error.message);
    }
}
