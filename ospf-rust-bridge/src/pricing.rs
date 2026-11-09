//! 认证 LP 定价对偶：只接受已独立验证的最优 LP 证书，裸对偶无法构造。
//! Certified LP pricing duals: only an independently verified optimal LP certificate is accepted; raw duals are not.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_core::solver::{SolveReport, linear_model_mapping};
use ospf_rust_framework::model::pricing_duals::VerifiedPricingDuals;

use crate::{BridgeError, ModelArtifact, Result, invalid};

/// 单条约束的认证对偶；身份来自原模型的稳定约束 ID，不是 solver 行号。
/// Certified dual for one constraint; the identity is the model's stable constraint ID, not a solver row number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CertifiedPricingDual {
    /// 稳定约束身份。 / Stable constraint identity.
    pub constraint_id: String,
    /// 认证行对偶。 / Certified row dual.
    pub dual: f64,
}

/// 认证定价对偶集合，绑定到产生它的模型指纹。 / Certified pricing duals bound to the model fingerprint that produced them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearPricingDuals {
    /// 被认证的模型指纹；模型一旦改变，旧证书即失效。 / Certified model fingerprint; a changed model invalidates the certificate.
    pub model_fingerprint: String,
    /// 按原模型行顺序排列的认证对偶。 / Certified duals in original model row order.
    pub duals: Vec<CertifiedPricingDual>,
}

/// 由真实线性模型与最优 LP 报告构造认证对偶。
/// Build certified duals from a real linear model and an optimal LP report.
///
/// 这里不重算对偶、不填充缺失证据：`VerifiedPricingDuals` 会对照**当前**模型复核指纹、维度、
/// primal 可行性、dual 可行性与目标一致性，任何一项不满足都直接失败。
/// Nothing is recomputed and no missing evidence is filled in: `VerifiedPricingDuals` re-checks the fingerprint,
/// dimensions, primal feasibility, dual feasibility and objective agreement against the **current** model, and fails
/// on any mismatch.
pub fn verify_pricing_duals(artifact: &ModelArtifact, report_json: &str) -> Result<LinearPricingDuals> {
    if report_json.len() > 8 * 1024 * 1024 {
        return Err(BridgeError::new("PayloadTooLarge", "pricing report exceeds 8 MiB"));
    }
    let model = crate::decode_model(artifact)?;
    // 传输层报告的计数字段是 u64，本平台 usize 与 u64 同宽，因此可以直接构造证书要求的类型。
    // Transport reports carry u64 counters; on this platform usize is as wide as u64, so the certificate's type
    // constructs directly.
    let report: SolveReport<f64> = serde_json::from_str(report_json).map_err(invalid)?;
    if report.schema_version != "1.0" {
        return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported pricing report schema"));
    }
    report.validate().map_err(invalid)?;
    let certified = VerifiedPricingDuals::new(&model, &report).map_err(invalid)?;
    // 证书对当前模型再次复核，防止调用方拿旧报告配新模型。
    // Re-check the certificate against the current model so an old report cannot be paired with a new model.
    certified.validate_for(&model).map_err(invalid)?;
    let mapping = linear_model_mapping(&model).map_err(invalid)?;
    let duals = certified.duals();
    if duals.len() != mapping.constraints_by_row.len() {
        return Err(BridgeError::new("InvalidReport", "certified duals do not cover every model row"));
    }
    let rows = duals.iter().zip(mapping.constraints_by_row.iter()).map(|(dual, constraint_id)| {
        if !dual.is_finite() {
            return Err(BridgeError::new("InvalidReport", "certified dual is not finite"));
        }
        Ok(CertifiedPricingDual { constraint_id: constraint_id.clone(), dual: *dual })
    }).collect::<Result<Vec<_>>>()?;
    Ok(LinearPricingDuals { model_fingerprint: certified.fingerprint().value.clone(), duals: rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::{ConstraintRelation, MetaModel, ObjectiveCategory};
    use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint};
    use ospf_rust_core::variable::{Continuous, VariableRange};

    /// 手工构造一个带认证对偶的最优 LP 报告；全部字段都与真实模型一致。
    /// Hand-build an optimal LP report carrying certified duals whose fields all agree with the real model.
    fn fixture() -> (ModelArtifact, String) {
        let mut meta = MetaModel::<f64>::new("pricing-bridge");
        let x = meta.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 10.0)).unwrap();
        meta.add_linear_constraint(&[(x, 1.0)], ConstraintRelation::LessEqual, 2.0, "capacity").unwrap();
        meta.set_linear_objective(vec![1.0], ObjectiveCategory::Maximum);
        let model = meta.try_to_linear_triad_model().unwrap();
        let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
            .solution(SolveSolution { value: None, values: vec![2.0], stable_values: Default::default(),
                objective: Some(2.0), objective_value: Some(2.0), dual_solution: Some(vec![1.0]),
                quadratic_dual_solution: None, pool: Vec::new() })
            .proof(SolveProof::optimality())
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
            .build().unwrap();
        let artifact = crate::encode_model(&model).unwrap();
        (artifact, serde_json::to_string(&report).unwrap())
    }

    #[test]
    fn certified_duals_are_keyed_by_stable_identities() {
        let (artifact, report) = fixture();
        let certified = verify_pricing_duals(&artifact, &report).unwrap();
        assert_eq!(certified.model_fingerprint, artifact.fingerprint);
        assert_eq!(certified.duals.len(), 1);
        assert!(certified.duals[0].constraint_id.contains("capacity"), "got {:?}", certified.duals[0].constraint_id);
        assert_eq!(certified.duals[0].dual, 1.0);
    }

    #[test]
    fn a_certificate_never_transfers_to_another_model() {
        let (artifact, report) = fixture();
        // 同一模型换个名字即产生新指纹，旧证书必须失效。
        // Renaming the model yields a new fingerprint, so the old certificate must be rejected.
        let mut other = MetaModel::<f64>::new("pricing-bridge-other");
        let x = other.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 10.0)).unwrap();
        other.add_linear_constraint(&[(x, 1.0)], ConstraintRelation::LessEqual, 2.0, "capacity").unwrap();
        other.set_linear_objective(vec![1.0], ObjectiveCategory::Maximum);
        let foreign = crate::encode_model(&other.try_to_linear_triad_model().unwrap()).unwrap();
        assert!(verify_pricing_duals(&foreign, &report).is_err());
        // 缺少对偶、对偶维度不符、非有限对偶都必须拒绝，不能凭空补 0。
        // A missing dual, a wrong dual dimension and a non-finite dual must all fail; zeroes are never invented.
        for broken in [
            report.replace("\"dualSolution\":[1.0]", "\"dualSolution\":null"),
            report.replace("\"dualSolution\":[1.0]", "\"dualSolution\":[]"),
            report.replace("\"dualSolution\":[1.0]", "\"dualSolution\":[1e999]"),
        ] {
            assert_ne!(broken, report, "fixture no longer contains the expected dual text");
            assert!(verify_pricing_duals(&artifact, &broken).is_err());
        }
        // 非报告文本与缺 schema 同样拒绝。 / Non-report text and a missing schema are rejected too.
        assert!(verify_pricing_duals(&artifact, "{}").is_err());
        assert!(verify_pricing_duals(&artifact, "not json").is_err());
    }
}
