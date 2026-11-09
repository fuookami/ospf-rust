//! 保留实际子模型的 CSP1D 阶段报告 codec；不等价于源问题证明。
//! CSP1D stage-report codec retaining the actual child model, not a source-problem proof.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::SolveReport;
use ospf_rust_framework::solver::remote::{CompiledLinearModelArtifact, decode_compiled_linear_model, encode_compiled_linear_model};
use ospf_rust_framework_csp1d::{Csp1dSolveStage, Csp1dStageReport};
use crate::{BridgeError, Result, invalid};

/// 实际提交子模型的阶段，不携带源问题最优性断言。 / Actual submitted child's stage, without source-problem optimality claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp1dStageKind {
    /// 受限 LP。 / Restricted LP.
    Lp,
    /// 普通候选 MILP。 / Ordinary candidate MILP.
    Milp,
    /// 当前列池最终整数模型。 / Final integer model over the current column pool.
    FinalMilp,
}

/// 独立子报告封装；只有 Rust 解析内部模型/报告文本，精确 u64 不经 JS number。
/// Independent child-report artifact; only Rust parses inner model/report text, bypassing JS number for exact u64.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dStageReportArtifact {
    /// 独立阶段协议版本。 / Independent stage-protocol version.
    pub schema_version: String,
    /// 实际执行阶段。 / Actual executed stage.
    pub stage: Csp1dStageKind,
    /// 实际提交模型，包含模型局部行身份和精确类型。 / Actual submitted model, including model-local row identity and exact types.
    pub model: CompiledLinearModelArtifact,
    /// 原始统一报告文本，不改写映射、指纹或证明。 / Original unified-report text, without rewriting mapping, fingerprints or proofs.
    pub solver_report_json: String,
    /// 未进入子模型的有限目标常数，不能隐式加进原始报告。 / Finite objective constant outside the child model, never implicitly added to the raw report.
    pub objective_constant: f64,
}

/// 从实际执行记录编码，校验整个子模型与报告后返回；不重新编译源 context。
/// Encode an actual execution record after full child-model/report validation, without recompiling its source context.
pub fn encode_csp1d_stage_report(record: &Csp1dStageReport) -> Result<Csp1dStageReportArtifact> {
    let artifact = Csp1dStageReportArtifact { schema_version: "1.0".into(),
        stage: match record.stage { Csp1dSolveStage::Lp => Csp1dStageKind::Lp, Csp1dSolveStage::Milp => Csp1dStageKind::Milp,
            Csp1dSolveStage::FinalMilp => Csp1dStageKind::FinalMilp },
        model: encode_compiled_linear_model(&record.model).map_err(invalid)?,
        solver_report_json: serde_json::to_string(&record.report).map_err(invalid)?, objective_constant: record.objective_constant };
    decode_csp1d_stage_report(&artifact)?;
    Ok(artifact)
}

/// 复验实际模型身份、完整可行向量、整数性、目标及最优 LP 对偶；u64 元数据不降为 usize。
/// Revalidate actual identity, full feasible vectors, integrality, objective and optimal LP duals, without narrowing u64 metadata to usize.
pub fn decode_csp1d_stage_report(artifact: &Csp1dStageReportArtifact) -> Result<(LinearTriadModel, SolveReport<f64, u64>)> {
    if artifact.schema_version != "1.0" || artifact.model.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported CSP1D stage or compiled-model schema")); }
    if artifact.solver_report_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "CSP1D child report exceeds 8 MiB")); }
    if !artifact.objective_constant.is_finite() { return Err(invalid("nonfinite CSP1D child objective constant")); }
    let model = decode_compiled_linear_model(&artifact.model).map_err(invalid)?;
    let report = crate::linear_report::decode_linear_report(&model, &artifact.solver_report_json, artifact.stage == Csp1dStageKind::Lp)?;
    Ok((model, report))
}

#[cfg(test)]
mod tests;
