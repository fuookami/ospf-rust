//! 复验 BPP3D 真实子报告与源模型，不将受限模型证明提升为源问题证明。
//! Revalidate actual BPP3D child reports against source models without promoting restricted proofs to source-wide claims.

use serde::{Deserialize, Serialize};
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{ProblemStatus, SolveReport, require_infeasibility_certificate_for_model};
use ospf_rust_framework::solver::{LinearStageReport, MetaModelStage, remote::encode_compiled_linear_model};
use ospf_rust_framework_bpp3d::{
    application::{ColumnGenerationApplicationState, MetaModelFinalExecutor, MetaModelRmpExecutor},
    domain::layer_assignment::PreciseAssignmentSelection,
};
use super::{MAX_BYTES, decode_bpp3d_column_pool, decode_bpp3d_problem, encode_bpp3d_column_pool};
use crate::{BridgeError, Csp1dStageKind, Csp1dStageReportArtifact, ModelArtifact, Result, decode_csp1d_stage_report, invalid};

/// 源绑定列池及实际子报告；内部模型和报告保留原始身份与精确整数。
/// Source-bound pool and actual child report, retaining original model/report identities and exact integers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dStageReportArtifact {
    /// 阶段协议版本。 / Stage protocol version.
    pub schema_version: String,
    /// 按原源重建初始层的列池文本。 / Pool text whose initial layers are rebuilt from the original source.
    pub pool_json: String,
    /// 实际编译模型及原始子报告。 / Actual compiled model and original child report.
    pub child: Csp1dStageReportArtifact,
}

/// 经源模型与实际子报告复验的阶段；最终分配仍须严格物理回放。
/// Stage revalidated against its source model and actual child report; final selections still require strict physical replay.
#[derive(Debug, Clone)]
pub struct Bpp3dStageReport {
    /// 原始封装，保留子模型及报告文本。 / Original artifact retaining child model and report text.
    pub artifact: Bpp3dStageReportArtifact,
    /// 复验后的源绑定列池。 / Revalidated source-bound pool.
    pub state: ColumnGenerationApplicationState,
    /// 复验后的原始报告，不认证求解器来源。 / Revalidated original report, without authenticating solver provenance.
    pub report: SolveReport<f64, u64>,
    /// 包含独立目标常数的源模型目标；无 incumbent 时为空。 / Source objective including the separate constant, absent without an incumbent.
    pub objective: Option<f64>,
    /// 从真实 final 整数向量提取的完整箱／层分配；不声明物理可行。
    /// Complete bin/layer selection extracted from the actual final integer vector, without claiming physical feasibility.
    pub selection: Option<PreciseAssignmentSelection>,
}

/// 复验后的阶段证据；子模型保留实际身份，分配仍须严格物理回放。
/// Revalidated stage evidence whose child model keeps its actual identity; selections still require strict physical replay.
pub(crate) struct RevalidatedStage {
    /// 复验后的源绑定列池。 / Revalidated source-bound pool.
    pub state: ColumnGenerationApplicationState,
    /// 实际编译子模型，保留原报告身份。 / Actual compiled child model retaining the original report identity.
    pub actual: LinearTriadModel,
    /// 原报告，不认证求解器来源。 / Original report, without authenticating solver provenance.
    pub report: SolveReport<f64, u64>,
    /// 源目标，无 incumbent 时为空。 / Source objective, absent without an incumbent.
    pub objective: Option<f64>,
    /// 从真实整数向量提取的分配，不声明物理可行。 / Selection extracted from the actual integer vector, without physical-feasibility claims.
    pub selection: Option<PreciseAssignmentSelection>,
}

/// 编码真实子模型及原报告，供阶段与完整执行共用。 / Encode the actual child model and original report for stages and full executions.
pub(crate) fn encode_stage_child(record: &LinearStageReport<MetaModelStage>) -> Result<Csp1dStageReportArtifact> {
    Ok(Csp1dStageReportArtifact {
        schema_version: "1.0".into(),
        stage: match record.stage { MetaModelStage::RestrictedMaster => Csp1dStageKind::Lp, MetaModelStage::FinalMilp => Csp1dStageKind::FinalMilp },
        model: encode_compiled_linear_model(&record.model).map_err(invalid)?,
        solver_report_json: serde_json::to_string(&record.report).map_err(invalid)?,
        objective_constant: record.objective_constant,
    })
}

/// 编码真实子模型及原报告，并立即使用源和列池复验；不重新求解。
/// Encode the actual child model and original report and immediately revalidate against source and pool, without re-solving.
pub fn encode_bpp3d_stage_report(
    source: &ModelArtifact,
    state: &ColumnGenerationApplicationState,
    record: &LinearStageReport<MetaModelStage>,
) -> Result<String> {
    let artifact = Bpp3dStageReportArtifact {
        schema_version: "1.0".into(),
        pool_json: encode_bpp3d_column_pool(source, state)?,
        child: encode_stage_child(record)?,
    };
    let text = serde_json::to_string(&artifact).map_err(invalid)?;
    decode_bpp3d_stage_report(&text, source)?;
    Ok(text)
}

/// 从源和列池重建原阶段模型，复验实际报告、完整数学模型及 incumbent；不认证生成器或求解器来源。
/// Rebuild the original stage from source and pool, revalidate the actual report, full mathematics and incumbent, without authenticating generator or solver provenance.
pub fn decode_bpp3d_stage_report(text: &str, source: &ModelArtifact) -> Result<Bpp3dStageReport> {
    if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP3D stage report exceeds 8 MiB")); }
    let artifact: Bpp3dStageReportArtifact = serde_json::from_str(text).map_err(invalid)?;
    if artifact.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP3D stage schema")); }
    let stage = revalidate_stage(&artifact.pool_json, &artifact.child, source)?;
    Ok(Bpp3dStageReport { artifact, state: stage.state, report: stage.report, objective: stage.objective, selection: stage.selection })
}

/// 复验阶段文本之外的共用门禁：源绑定列池、实际模型数学与原报告身份。
/// Shared gate for stage text: source-bound pool, actual model mathematics and original report identity.
pub(crate) fn revalidate_stage(pool_json: &str, child: &Csp1dStageReportArtifact, source: &ModelArtifact) -> Result<RevalidatedStage> {
    let (_, request, _) = decode_bpp3d_problem(source)?;
    let state = decode_bpp3d_column_pool(pool_json, source)?;
    let amounts = request.item_amounts.iter().map(|(_, amount)| *amount).collect::<Vec<_>>();
    let (model, assignment) = match child.stage {
        Csp1dStageKind::Lp => (MetaModelRmpExecutor::default().model_with_item_amounts(&state, &amounts).map_err(invalid)?, None),
        Csp1dStageKind::FinalMilp => {
            let (model, assignment) = MetaModelFinalExecutor::default().model_and_assignment_with_item_amounts(&state, &amounts).map_err(invalid)?;
            (model, Some(assignment))
        }
        Csp1dStageKind::Milp => return Err(invalid("BPP3D stage must be a restricted LP or final MILP")),
    };
    let (actual, report) = decode_csp1d_stage_report(child)?;
    if report.problem_status == ProblemStatus::Infeasible {
        require_infeasibility_certificate_for_model(&report, &child.model.model_fingerprint).map_err(invalid)?;
    }
    let snapshot = model.linear_model_snapshot().map_err(invalid)?;
    let compiled = snapshot.compile_flt64().map_err(invalid)?;
    if child.objective_constant != compiled.objective_constant { return Err(invalid("BPP3D child objective constant differs from its source")); }
    let permutation = crate::source_model::compare_source_model(&actual, &compiled.model)?;
    let (objective, selection) = if let Some(solution) = &report.solution {
        let values = compiled.model.variables.iter().enumerate().map(|(column, token)| (token.id(), solution.values[permutation[column]])).collect();
        snapshot.validate_solution(&values, &1e-7).map_err(invalid)?;
        let objective = snapshot.evaluate_objective_flt64(&values).map_err(invalid)?.total_value().map_err(invalid)?;
        let selection = assignment.map(|assignment| {
            let primal = model.tokens_in_solver_order().into_iter().map(|token| values.get(&token.id()).copied()
                .ok_or_else(|| invalid("BPP3D source model token is missing from the incumbent"))).collect::<Result<Vec<_>>>()?;
            assignment.extract_selection(&primal, 1e-7).map_err(invalid)
        }).transpose()?;
        (Some(objective), selection)
    } else { (None, None) };
    Ok(RevalidatedStage { state, actual, report, objective, selection })
}

#[cfg(test)]
mod tests;
