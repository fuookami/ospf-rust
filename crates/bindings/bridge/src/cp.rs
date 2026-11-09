//! 精确 CP 快照和分析边界 / Exact CP snapshot and analysis boundary.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::analysis::{ActivityConfig, ActivityStatus as CoreStatus};
use ospf_rust_core::model::constraint_programming::ConstraintProgrammingSnapshot;
use ospf_rust_core::solver::{CURRENT_SOLVE_REPORT_SCHEMA_VERSION, SolveReport};
use crate::{BridgeError, ModelArtifact, Result, invalid};

/// CP 约束的身份和元数据 / CP constraint identity and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintOptions {
    /// 稳定约束身份 / Stable constraint identity.
    pub id: String,
    /// 展示名称，省略时使用 ID / Display name, defaulting to the ID.
    #[ts(optional)]
    pub name: Option<String>,
    /// 约束组 / Constraint group.
    #[ts(optional)]
    pub group: Option<String>,
    /// 来源信息 / Origin metadata.
    #[ts(optional)]
    pub origin: Option<String>,
}

/// 可移植活动性分析配置 / Portable activity analysis options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityOptions {
    /// active 绝对容差 / Absolute active tolerance.
    #[ts(optional)]
    pub tolerance: Option<f64>,
    /// nearly-active 绝对容差 / Absolute nearly-active tolerance.
    #[ts(optional)]
    pub nearly_active_tolerance: Option<f64>,
    /// 相对容差 / Relative tolerance.
    #[ts(optional)]
    pub relative_tolerance: Option<f64>,
    /// 是否分析变量边界 / Whether to analyze variable bounds.
    #[ts(optional)]
    pub include_variable_bounds: Option<bool>,
}

impl ActivityOptions {
    /// 使用 core 默认值并校验 / Apply core defaults and validate.
    pub fn config(self) -> Result<ActivityConfig> {
        let defaults = ActivityConfig::default();
        let config = ActivityConfig {
            tolerance: self.tolerance.unwrap_or(defaults.tolerance),
            nearly_active_tolerance: self.nearly_active_tolerance.unwrap_or(defaults.nearly_active_tolerance),
            relative_tolerance: self.relative_tolerance.unwrap_or(defaults.relative_tolerance),
            include_variable_bounds: self.include_variable_bounds.unwrap_or(defaults.include_variable_bounds),
        };
        config.validate().map_err(invalid)?;
        Ok(config)
    }
}

/// SDK 活动性分类，使用穷尽转换对齐 core / SDK activity classification, exhaustively mapped from core.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivityStatus {
    /// 贴边 / Active.
    Active,
    /// 接近边界 / Nearly active.
    NearlyActive,
    /// 不贴边 / Inactive.
    Inactive,
    /// 满足但无自然 slack / Satisfied without a natural slack metric.
    SatisfiedWithoutSlackMetric,
    /// 违反 / Violated.
    Violated,
    /// 未知 / Unknown,
    Unknown,
}

impl From<CoreStatus> for ActivityStatus {
    fn from(value: CoreStatus) -> Self {
        match value {
            CoreStatus::Active => Self::Active,
            CoreStatus::NearlyActive => Self::NearlyActive,
            CoreStatus::Inactive => Self::Inactive,
            CoreStatus::SatisfiedWithoutSlackMetric => Self::SatisfiedWithoutSlackMetric,
            CoreStatus::Violated => Self::Violated,
            CoreStatus::Unknown => Self::Unknown,
        }
    }
}

/// 用现有带摘要的 canonical CP codec 编码，不经 JS 数值解析。
/// Encode with the existing digest-bearing canonical CP codec, without JS numeric parsing.
pub fn encode_cp_snapshot(snapshot: &ConstraintProgrammingSnapshot) -> Result<ModelArtifact> {
    let json = snapshot.to_canonical_json().map_err(invalid)?;
    Ok(ModelArtifact {
        model_json: String::from_utf8(json).map_err(invalid)?, fingerprint: snapshot.fingerprint.value.clone(),
    })
}

/// 校验 CP schema、canonical 字节、身份、摘要及外层指纹。
/// Validate CP schema, canonical bytes, identities, digest and the outer fingerprint.
pub fn decode_cp_snapshot(artifact: &ModelArtifact) -> Result<ConstraintProgrammingSnapshot> {
    if artifact.model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "CP snapshot exceeds 8 MiB")); }
    let snapshot = ConstraintProgrammingSnapshot::from_canonical_json(artifact.model_json.as_bytes()).map_err(invalid)?;
    if snapshot.fingerprint.value != artifact.fingerprint {
        return Err(BridgeError::new("FingerprintMismatch", "CP snapshot fingerprint mismatch"));
    }
    Ok(snapshot)
}

/// 校验精确 CP 报告、源快照指纹及 incumbent，不经浮点转换。
/// Validate an exact CP report, source fingerprint and incumbent without floating conversion.
pub fn decode_cp_report(json: &str, snapshot: &ConstraintProgrammingSnapshot) -> Result<SolveReport<i64, u64>> {
    if json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "CP report exceeds 8 MiB")); }
    let report: SolveReport<i64, u64> = serde_json::from_str(json).map_err(invalid)?;
    if report.schema_version != CURRENT_SOLVE_REPORT_SCHEMA_VERSION {
        return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported CP report schema"));
    }
    report.validate().map_err(invalid)?;
    if report.fingerprints.model.as_ref() != Some(&snapshot.fingerprint) {
        return Err(BridgeError::new("FingerprintMismatch", "CP report belongs to a different snapshot"));
    }
    if let Some(solution) = &report.solution {
        if solution.values.len() != snapshot.variables.len() || solution.stable_values.len() != snapshot.variables.len() || solution.value.is_some() {
            return Err(invalid("CP report must contain complete canonical and stable assignments"));
        }
        for (entry, value) in snapshot.variables.iter().zip(&solution.values) {
            if solution.stable_values.get(&entry.variable.stable_id) != Some(value) {
                return Err(invalid("CP report canonical and stable assignments differ"));
            }
        }
        snapshot.validate_assignment(&solution.stable_values).map_err(invalid)?;
        if snapshot.objective_value(&solution.stable_values).map_err(invalid)? != solution.objective {
            return Err(invalid("CP report objective differs from source evaluation"));
        }
        for values in &solution.pool {
            if values.len() != snapshot.variables.len() { return Err(invalid("CP report pool assignment shape mismatch")); }
            let assignment = snapshot.variables.iter().zip(values).map(|(entry, value)| (entry.variable.stable_id.clone(), *value)).collect();
            snapshot.validate_assignment(&assignment).map_err(invalid)?;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::constraint_programming::{ConstraintProgrammingModel, IntegerDomain, IntegerExpression, IntegerObjective, IntegerVariable};
    use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveSolution, TerminationReason};

    #[test]
    fn exact_cp_codec_reuses_canonical_digest_and_rejects_tampering() {
        let mut model = ConstraintProgrammingModel::new("exact");
        model.add_variable(IntegerVariable::new("x"), IntegerDomain::range(9_007_199_254_740_993, i64::MAX).unwrap()).unwrap();
        let artifact = encode_cp_snapshot(&model.freeze().unwrap()).unwrap();
        assert!(artifact.model_json.contains("9007199254740993"));
        assert_eq!(encode_cp_snapshot(&decode_cp_snapshot(&artifact).unwrap()).unwrap().model_json, artifact.model_json);
        let mut wrong = artifact.clone();
        wrong.fingerprint = "wrong".into();
        assert_eq!(decode_cp_snapshot(&wrong).unwrap_err().code, "FingerprintMismatch");
        wrong = artifact.clone();
        wrong.model_json = wrong.model_json.replace("9007199254740993", "9007199254740994");
        assert!(decode_cp_snapshot(&wrong).is_err());
        wrong = artifact;
        wrong.model_json.push(' ');
        assert!(decode_cp_snapshot(&wrong).is_err());
    }

    #[test]
    fn cp_reports_verify_exact_source_values_objective_schema_and_fingerprint() {
        let high = 9_007_199_254_740_993;
        let mut model = ConstraintProgrammingModel::new("exact-report");
        let x = IntegerVariable::new("x");
        model.add_variable(x.clone(), IntegerDomain::range(high, high + 5).unwrap()).unwrap();
        model.set_objective(IntegerObjective::minimize_with_id("cost", IntegerExpression::variable(x.clone())));
        let snapshot = model.freeze().unwrap();
        let mut solution = SolveSolution::vector(vec![high]);
        solution.stable_values.insert(x.stable_id.clone(), high);
        solution.objective = Some(high);
        let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
            .solution(solution).fingerprints(SolveFingerprints { model: Some(snapshot.fingerprint.clone()), ..Default::default() }).build().unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("9007199254740993"));
        assert_eq!(decode_cp_report(&json, &snapshot).unwrap().solution.unwrap().objective, Some(high));
        let mut wrong = report.clone();
        wrong.solution.as_mut().unwrap().objective = Some(high + 1);
        assert!(decode_cp_report(&serde_json::to_string(&wrong).unwrap(), &snapshot).is_err());
        wrong = report.clone();
        wrong.solution.as_mut().unwrap().values[0] += 1;
        assert!(decode_cp_report(&serde_json::to_string(&wrong).unwrap(), &snapshot).is_err());
        wrong = report.clone();
        wrong.fingerprints.model = None;
        assert_eq!(decode_cp_report(&serde_json::to_string(&wrong).unwrap(), &snapshot).unwrap_err().code, "FingerprintMismatch");
        wrong = report;
        wrong.schema_version = "99.0".into();
        assert_eq!(decode_cp_report(&serde_json::to_string(&wrong).unwrap(), &snapshot).unwrap_err().code, "UnsupportedProtocolVersion");
    }
}
