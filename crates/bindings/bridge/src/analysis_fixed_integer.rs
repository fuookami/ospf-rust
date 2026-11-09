//! 批次 B 第 6 单：固定整数 LP 局部灵敏度的配置与报告 / Batch B, delivery 6: fixed-integer LP sensitivity config and report.
//!
//! 两件纯数据能力合在一个入口：①`FixedIntegerLpSensitivityConfig::validate()` 校验容差；②
//! `LocalConstraintSensitivityReport::validate()` + `has_duals()` + `constraint(id)` 校验并投影报告。
//! 都不需要求解器。
//!
//! 两个实测发现（本桥接**不**擅自定义新规则，只把它们变成可见数据）：
//! * 报告校验**不查重复约束 ID**，而 `constraint(id)` 只返回第一条——重复 ID 会让查表有歧义，所以这里显式列出
//!   `duplicateConstraintIds`；
//! * 报告里的 `non-finite` 守卫在 JSON 线上不可达（serde 把 `NaN`/`Infinity` 写成 `null`），只有空白 schema 与空白
//!   约束 ID 是线上可触发的。
//!
//! Two pure-data capabilities in one entry: config tolerance validation, and report validation plus the `has_duals()` /
//! `constraint(id)` projections. Two measured findings are surfaced as data instead of inventing new rules here: the report
//! validation does **not** reject duplicate constraint IDs even though `constraint(id)` only returns the first match (so
//! `duplicateConstraintIds` is listed explicitly), and the non-finite guards are unreachable over JSON (serde writes
//! `NaN`/`Infinity` as `null`), leaving only blank schemas and blank constraint IDs triggerable on the wire.

use ospf_rust_core::analysis::fixed_integer::{
    FixedIntegerLpSensitivityConfig, LocalConstraintSensitivityReport,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 配置覆盖；省略字段用领域默认值（`1e-7` / `1e-9`）。
/// Config overrides; absent fields use the domain defaults (`1e-7` / `1e-9`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedIntegerConfigInput {
    /// 校验 incumbent 整数性的绝对容差。 / The absolute tolerance for incumbent integrality.
    pub integrality_tolerance: f64,
    /// 判断对偶是否为零的绝对容差。 / The absolute tolerance for a zero dual.
    pub dual_tolerance: f64,
}

/// 一条约束的局部灵敏度投影。 / One constraint's local-sensitivity projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedIntegerSensitivityRow {
    /// 原始约束稳定 ID。 / The stable original-constraint ID.
    pub constraint_id: String,
    /// 分析状态（`reachable`/`unreachable`/`unknown`/`unsupported`）。 / The analysis status.
    pub status: String,
    /// 语义范围（`fixed_integer_incumbent`）。 / The semantic scope.
    pub scope: String,
    /// 是否有可用 LP 对偶。 / Whether a usable LP dual exists.
    pub has_dual: bool,
    /// LP 对偶值。 / The LP dual value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dual_value: Option<f64>,
    /// 是否在该固定整数模式下局部有效。 / Whether it is locally effective under the fixed integer pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub local_effective: Option<bool>,
    /// 是否携带活动性证据。 / Whether activity evidence is attached.
    pub has_activity: bool,
}

/// 固定整数 LP 的配置校验与报告摘要。 / Config verification plus report summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedIntegerLpReport {
    /// 实际生效的整数性容差。 / The integrality tolerance in effect.
    pub integrality_tolerance: f64,
    /// 实际生效的对偶容差。 / The dual tolerance in effect.
    pub dual_tolerance: f64,
    /// 是否带上了报告（`false` 时下面所有 `Option` 字段都为空）。 / Whether a report was supplied.
    pub has_report: bool,
    /// 报告 schema。 / The report schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub schema_version: Option<String>,
    /// 灵敏度范围。 / The sensitivity scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scope: Option<String>,
    /// baseline 目标值。 / The baseline objective value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub baseline_objective: Option<f64>,
    /// 固定整数 LP 目标值。 / The fixed-integer LP objective value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lp_objective: Option<f64>,
    /// 固定整数变量数量。 / The number of fixed integer variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fixed_integer_count: Option<u32>,
    /// 整体分析状态。 / The overall analysis status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status: Option<String>,
    /// 是否至少有一个可用对偶（领域自己的 `has_duals`）。 / Whether at least one usable dual exists.
    pub has_duals: bool,
    /// 约束记录数。 / The constraint-record count.
    pub row_count: u32,
    /// **重复的约束 ID**（领域校验不拒，但会让 `constraint(id)` 查表有歧义）。
    /// **Duplicate constraint IDs** (domain validation allows them, but they make `constraint(id)` ambiguous).
    pub duplicate_constraint_ids: Vec<String>,
    /// 逐约束投影。 / The per-constraint projection.
    pub rows: Vec<FixedIntegerSensitivityRow>,
    /// 失败或降级原因。 / The failure or downgrade reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unavailable_reason: Option<String>,
}

/// 校验固定整数 LP 配置，并在给出报告时校验与投影报告。非法输入是**错误**，原因原样带出。
/// Verify the fixed-integer LP config and, when a report is supplied, verify and project it. An illegal input is an
/// **error** carrying the reason verbatim.
pub fn verify_fixed_integer_lp(
    config: Option<&FixedIntegerConfigInput>,
    report_json: Option<&str>,
) -> Result<FixedIntegerLpReport> {
    let config = match config {
        None => FixedIntegerLpSensitivityConfig::default(),
        Some(input) => FixedIntegerLpSensitivityConfig {
            integrality_tolerance: input.integrality_tolerance,
            dual_tolerance: input.dual_tolerance,
        },
    };
    config.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("fixed-integer LP config failed validation: {error}"))
    })?;
    let mut summary = FixedIntegerLpReport {
        integrality_tolerance: config.integrality_tolerance,
        dual_tolerance: config.dual_tolerance,
        has_report: false,
        schema_version: None, scope: None, baseline_objective: None, lp_objective: None,
        fixed_integer_count: None, status: None,
        has_duals: false,
        row_count: 0,
        duplicate_constraint_ids: Vec::new(),
        rows: Vec::new(),
        unavailable_reason: None,
    };
    let Some(json) = report_json else { return Ok(summary) };
    if json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the sensitivity report exceeds 8 MiB"));
    }
    let report: LocalConstraintSensitivityReport = serde_json::from_str(json).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("sensitivity report is not decodable: {error}"))
    })?;
    report.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("sensitivity report failed validation: {error}"))
    })?;
    // 重复 ID 由这里显式列出：领域校验放行，但查表会静默取第一条，调用方有权知道。
    // Duplicate IDs are listed explicitly here: domain validation allows them but `constraint(id)` silently returns the
    // first match, and a caller deserves to know.
    let mut seen = std::collections::BTreeSet::new();
    let mut duplicates = std::collections::BTreeSet::new();
    for constraint in &report.constraints {
        if !seen.insert(constraint.constraint_id.0.clone()) {
            duplicates.insert(constraint.constraint_id.0.clone());
        }
    }
    summary.has_report = true;
    summary.schema_version = Some(report.schema_version.clone());
    summary.scope = Some(report.scope.stable_name().to_owned());
    summary.baseline_objective = Some(report.baseline_objective);
    summary.lp_objective = report.lp_objective;
    summary.fixed_integer_count = Some(u32::try_from(report.fixed_integer_count).unwrap_or(u32::MAX));
    summary.status = Some(status_name(report.status).to_owned());
    summary.has_duals = report.has_duals();
    summary.row_count = u32::try_from(report.constraints.len()).unwrap_or(u32::MAX);
    summary.duplicate_constraint_ids = duplicates.into_iter().collect();
    summary.rows = report.constraints.iter().map(|constraint| FixedIntegerSensitivityRow {
        constraint_id: constraint.constraint_id.0.clone(),
        status: status_name(constraint.status).to_owned(),
        scope: constraint.scope.stable_name().to_owned(),
        has_dual: constraint.dual_value.is_some(),
        dual_value: constraint.dual_value,
        local_effective: constraint.local_effective,
        has_activity: constraint.activity.is_some(),
    }).collect();
    summary.unavailable_reason = report.unavailable_reason.clone();
    Ok(summary)
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
    use ospf_rust_core::analysis::fixed_integer::{LocalConstraintSensitivity, SensitivityScope};
    use ospf_rust_core::analysis::AnalysisStatus;
    use ospf_rust_core::solver::StableConstraintId as ConstraintId;

    const SCHEMA: &str = "1.0";

    fn report() -> LocalConstraintSensitivityReport {
        let row = |id: &str, dual: Option<f64>, local: Option<bool>| LocalConstraintSensitivity {
            constraint_id: ConstraintId(id.to_owned()),
            dual_value: dual,
            local_effective: local,
            status: AnalysisStatus::Reachable,
            scope: SensitivityScope::FixedIntegerIncumbent,
            activity: None,
        };
        LocalConstraintSensitivityReport {
            schema_version: SCHEMA.to_owned(),
            scope: SensitivityScope::FixedIntegerIncumbent,
            baseline_objective: 10.0,
            lp_objective: Some(12.5),
            fixed_integer_count: 3,
            constraints: vec![
                row("capacity", Some(1.5), Some(true)),
                row("shift", Some(0.0), Some(false)),
                row("budget", None, None),
            ],
            status: AnalysisStatus::Reachable,
            termination_reason: None,
            unavailable_reason: None,
        }
    }

    fn encode(report: &LocalConstraintSensitivityReport) -> String {
        serde_json::to_string(report).expect("serialize sensitivity report")
    }

    #[test]
    fn config_only_validates_and_echoes_the_tolerances() {
        let summary = verify_fixed_integer_lp(None, None).unwrap();
        assert!(!summary.has_report);
        // 省略配置时用领域默认值，而不是桥接自己编的默认值。
        // Without a config the domain defaults apply, not defaults invented by the bridge.
        let default = FixedIntegerLpSensitivityConfig::default();
        assert_eq!(summary.integrality_tolerance, default.integrality_tolerance);
        assert_eq!(summary.dual_tolerance, default.dual_tolerance);
        assert_eq!(summary.rows.len(), 0);
        assert!(!summary.has_duals);

        let summary = verify_fixed_integer_lp(
            Some(&FixedIntegerConfigInput { integrality_tolerance: 1e-4, dual_tolerance: 1e-6 }), None).unwrap();
        assert_eq!(summary.integrality_tolerance, 1e-4);
        assert_eq!(summary.dual_tolerance, 1e-6);
    }

    #[test]
    fn report_is_validated_and_projected() {
        let summary = verify_fixed_integer_lp(None, Some(&encode(&report()))).unwrap();
        assert!(summary.has_report);
        assert_eq!(summary.scope.as_deref(), Some("fixed_integer_incumbent"));
        assert_eq!(summary.status.as_deref(), Some("reachable"));
        assert_eq!(summary.fixed_integer_count, Some(3));
        assert_eq!(summary.lp_objective, Some(12.5));
        assert_eq!(summary.baseline_objective, Some(10.0));
        // `has_duals()` 只看"有没有对偶"，不看对偶是否为 0：`shift` 的 0.0 仍然算有对偶。
        // `has_duals()` only asks whether a dual exists, not whether it is zero: `shift`'s 0.0 still counts.
        assert!(summary.has_duals);
        assert_eq!(summary.row_count, 3);
        assert_eq!(summary.rows[0].constraint_id, "capacity");
        assert_eq!(summary.rows[0].dual_value, Some(1.5));
        assert_eq!(summary.rows[1].dual_value, Some(0.0));
        assert_eq!(summary.rows[1].local_effective, Some(false));
        assert_eq!(summary.rows[2].has_dual, false);
        assert_eq!(summary.rows[2].local_effective, None);
        assert_eq!(summary.rows[2].scope, "fixed_integer_incumbent");
        assert!(summary.duplicate_constraint_ids.is_empty());
        assert_eq!(summary.unavailable_reason, None);
    }

    #[test]
    fn duplicate_ids_are_surfaced_although_the_domain_allows_them() {
        // 领域 `validate()` 不拒绝重复 ID，`constraint(id)` 只返回第一条 —— 桥接把这件事变成可见数据。
        // Domain `validate()` does not reject duplicate IDs and `constraint(id)` returns only the first, so the bridge
        // turns that into visible data instead of silently hiding it.
        let mut duplicated = report();
        duplicated.constraints.push(LocalConstraintSensitivity {
            constraint_id: ConstraintId("capacity".to_owned()),
            dual_value: Some(9.0),
            local_effective: Some(false),
            status: AnalysisStatus::Reachable,
            scope: SensitivityScope::FixedIntegerIncumbent,
            activity: None,
        });
        // 领域自己确实放行。 / The domain really does accept it.
        assert!(duplicated.validate().is_ok());
        assert_eq!(duplicated.constraint(&ConstraintId("capacity".to_owned())).unwrap().dual_value, Some(1.5));

        let summary = verify_fixed_integer_lp(None, Some(&encode(&duplicated))).unwrap();
        assert_eq!(summary.row_count, 4);
        assert_eq!(summary.duplicate_constraint_ids, vec!["capacity".to_owned()]);
    }

    #[test]
    fn illegal_inputs_are_refused_with_the_domain_reason() {
        // 负容差：领域拒绝（`NaN`/`Infinity` 走不到线上，serde 会写成 `null`）。
        // A negative tolerance: refused by the domain (`NaN`/`Infinity` cannot cross the wire, serde writes `null`).
        let negative = FixedIntegerConfigInput { integrality_tolerance: -1.0, dual_tolerance: 1e-9 };
        let error = verify_fixed_integer_lp(Some(&negative), None).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("finite and non-negative"), "{}", error.message);

        // 空白 schema / 空白约束 ID：线上可触发的两条。 / Blank schema and blank constraint ID: the two reachable rules.
        let mut blank_schema = report();
        blank_schema.schema_version = "  ".to_owned();
        let error = verify_fixed_integer_lp(None, Some(&encode(&blank_schema))).unwrap_err();
        assert!(error.message.contains("invalid"), "{}", error.message);

        let mut blank_id = report();
        blank_id.constraints[0].constraint_id = ConstraintId(" ".to_owned());
        let error = verify_fixed_integer_lp(None, Some(&encode(&blank_id))).unwrap_err();
        assert!(error.message.contains("invalid"), "{}", error.message);

        assert_eq!(verify_fixed_integer_lp(None, Some("not json")).unwrap_err().code, "InvalidInput");
        assert_eq!(verify_fixed_integer_lp(None, Some(&"x".repeat(9 * 1024 * 1024))).unwrap_err().code,
            "InvalidInput");
    }
}
