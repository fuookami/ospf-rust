//! 单轮 BPP3D 完整执行的源绑定 codec；受限模型、启发式生成与装箱结论不提升为源问题证明。
//! Source-bound codec for complete single-round BPP3D executions, without promoting restricted-model, heuristic-generation or packing conclusions to source-wide proofs.

use std::collections::BTreeMap;
use serde::Serialize;
use ospf_rust_core::solver::{ProblemStatus, SolveReport, require_optimal_lp_certificate_for_linear_model};
use ospf_rust_framework::solver::{LinearStageReport, MetaModelStage};
use ospf_rust_framework_bpp3d::{
    application::{Bpp3dOneRoundEvidence, ColumnGenerationApplicationState, replay_strict_grouped_packing},
    domain::layer_assignment::PreciseAssignmentSelection,
};
use super::pool::{decode_bpp3d_column_pool, encode_bpp3d_column_pool};
use super::stage::{encode_stage_child, revalidate_stage};
use super::{Bpp3dPacking, MAX_BYTES, decode_bpp3d_problem, packed_bins};
use crate::{BridgeError, Csp1dStageKind, Csp1dStageReportArtifact, ModelArtifact, Result, invalid};
use ts_rs::TS;

/// 严格物理回放的展开上限，与原领域单轮配置的同一上界。 / Strict-replay expansion limit, the same bound as the original one-round configuration.
pub(crate) const MAX_REPLAY_PLACEMENTS: usize = 100_000;

/// 单轮实际子阶段类型，不冒充源问题阶段。 / Actual one-round child-stage kind, never impersonating a source-problem stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Bpp3dExecutionStageKind {
    /// 受限主 LP。 / Restricted master LP.
    Rmp,
    /// 当前完整池上的最终整数模型。 / Final integer model over the complete pool.
    Final,
}

/// 单轮执行中的实际子阶段：实际输入列池与后端接受的原模型／原报告。
/// Actual child stage of a one-round execution: the actual input pool and the accepted original model/report.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dExecutionStageArtifact {
    /// 子阶段类型。 / Child-stage kind.
    pub kind: Bpp3dExecutionStageKind,
    /// 该阶段真实求解所用的源绑定列池文本。 / Source-bound pool text actually solved by this stage.
    pub pool_json: String,
    /// 后端接受的实际模型与原报告；提交、门禁或报告预算失败时为空。
    /// Accepted actual model and original report; absent on pre-submission, gate or report-budget failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<Csp1dStageReportArtifact>,
}

/// 有界、版本化的单轮执行封装；不携带可伪造的全局最优、生成完备或生产声明。
/// Bounded, versioned one-round execution envelope without forgeable global-optimality, complete-generation or production assertions.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bpp3dExecutionReportArtifact {
    /// 独立主版本。 / Independent major version.
    pub schema_version: String,
    /// 源 canonical 问题身份。 / Canonical source-problem identity.
    pub source_fingerprint: String,
    /// 原运行实际使用的严格回放展开上限。 / Strict-replay expansion limit actually used by the original run.
    pub replay_limit: u32,
    /// 按实际调用顺序保存的子阶段；初始 RMP、刷新 RMP、可选 final。
    /// Child stages in actual invocation order: the initial RMP, the refreshed RMP and an optional final child.
    pub stages: Vec<Bpp3dExecutionStageArtifact>,
    /// 原流程失败文本；None 表示原流程成功。 / Original flow failure text; None means the original flow succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    /// 原流程结果 info 中与连续半径相关的条目（选中数量、每个变量的半径/平方/分段、诊断），按原文本原样保留。
    /// Continuous-radius entries from the original flow result info (selected count, per-variable radius/squared/segment
    /// and diagnostics), preserved verbatim.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub continuous_radius: BTreeMap<String, String>,
}

/// 经源模型及实际报告共同复验的单个子阶段摘要。 / One child-stage summary jointly revalidated against the source model and the actual report.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bpp3dExecutionStageSummary {
    /// 子阶段类型。 / Child-stage kind.
    pub kind: Bpp3dExecutionStageKind,
    /// 该阶段的完整列数。 / Complete column count of this stage.
    pub column_count: u32,
    /// 该阶段列池的轮次。 / Column-pool iteration of this stage.
    pub iteration: u32,
    /// 实际子模型指纹，不是重新编译模型身份。 / Actual child fingerprint, not the recompiled model's identity.
    #[ts(optional)]
    pub model_fingerprint: Option<String>,
    /// 原子问题状态。 / Original child problem status.
    #[ts(optional)]
    pub problem_status: Option<String>,
    /// 原终止原因。 / Original termination reason.
    #[ts(optional)]
    pub termination_reason: Option<String>,
    /// 仅原池内认证最优，不是源问题最优。 / Certified optimal over the original pool only, not source-wide optimality.
    pub restricted_optimal: bool,
    /// 仅原池内认证不可行，不是源问题不可行。 / Certified infeasible over the original pool only, not source-wide infeasibility.
    pub restricted_infeasible: bool,
    /// 是否有通过原模型复验的完整解。 / Whether a complete incumbent passed original-model validation.
    pub has_incumbent: bool,
    /// 派生目标，独立常数仅计一次。 / Derived objective counting the separate constant once.
    #[ts(optional)]
    pub objective_value: Option<f64>,
}

/// 全流程派生摘要，不能通过输入字段冒充全局证明。 / Derived workflow summary that cannot impersonate global proofs through input fields.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bpp3dExecutionSummary {
    /// 源问题身份。 / Source-problem identity.
    pub source_fingerprint: String,
    /// 原流程是否完成；失败时仍保留已取得证据。 / Whether the original flow completed; acquired evidence survives failures.
    pub completed: bool,
    /// 始终 false，既有生成器是启发式。 / Always false, as the existing generators are heuristic.
    pub candidate_complete: bool,
    /// 最后阶段看到的完整列数。 / Complete column count seen by the last stage.
    pub column_count: u32,
    /// 本轮新增列数。 / Columns appended in this round.
    pub generated_count: u32,
    /// 实际子阶段数。 / Actual child-stage count.
    pub stage_count: u32,
    /// 实际 final 子阶段索引，未执行则为空。 / Actual final-child index, absent when not executed.
    #[ts(optional)]
    pub final_stage: Option<u32>,
    /// 是否由真实整数向量导出了源可行装箱。 / Whether source-feasible packing was derived from the actual integer vector.
    pub has_packing: bool,
    /// 原流程失败文本。 / Original flow failure text.
    #[ts(optional)]
    pub failure: Option<String>,
}

/// 独立拥有、经源及全部子阶段复验的执行报告；仍不认证求解器或生成器来源。
/// Independently owned execution report revalidated against its source and every child stage, still without authenticating solver or generator provenance.
#[derive(Debug, Clone)]
pub struct Bpp3dExecutionReport {
    /// 原始封装，内部模型／报告文本保留字节。 / Original artifact retaining inner model/report text bytes.
    pub artifact: Bpp3dExecutionReportArtifact,
    /// 验证器派生摘要。 / Validator-derived summary.
    pub summary: Bpp3dExecutionSummary,
    /// 与 stages 同序的派生摘要。 / Derived summaries in stages order.
    pub stages: Vec<Bpp3dExecutionStageSummary>,
    /// 仅严格分组物理回放成功时导出。 / Derived only when strict grouped physical replay succeeds.
    pub packing: Option<Bpp3dPacking>,
    /// 从真实 final 整数向量提取的分配；物理失败仍保留。 / Selection extracted from the actual final vector, retained on physical failures.
    pub selection: Option<PreciseAssignmentSelection>,
}

struct DecodedStage {
    kind: Bpp3dExecutionStageKind,
    pool_json: String,
    state: ColumnGenerationApplicationState,
    report: Option<SolveReport<f64, u64>>,
    model_fingerprint: Option<String>,
    objective: Option<f64>,
    selection: Option<PreciseAssignmentSelection>,
}

fn json_name<T: Serialize>(value: T) -> Result<String> {
    serde_json::to_value(value).map_err(invalid)?.as_str().map(str::to_owned)
        .ok_or_else(|| invalid("BPP3D execution status is not a JSON string"))
}

impl DecodedStage {
    fn summary(&self) -> Result<Bpp3dExecutionStageSummary> {
        let (problem_status, termination_reason) = match &self.report {
            Some(report) => (Some(json_name(report.problem_status)?), Some(json_name(report.termination_reason)?)),
            None => (None, None),
        };
        Ok(Bpp3dExecutionStageSummary { kind: self.kind, column_count: u32::try_from(self.state.layers.len()).map_err(invalid)?,
            iteration: u32::try_from(self.state.iteration).map_err(invalid)?, model_fingerprint: self.model_fingerprint.clone(),
            problem_status, termination_reason, restricted_optimal: self.report.as_ref().is_some_and(|report| report.is_optimal()),
            restricted_infeasible: self.report.as_ref().is_some_and(|report| report.problem_status == ProblemStatus::Infeasible),
            has_incumbent: self.report.as_ref().is_some_and(|report| report.solution.is_some()), objective_value: self.objective })
    }
}

/// 解码单个子阶段；空报告阶段只复验列池，不伪造模型或求解结论。
/// Decode one child stage; report-free stages revalidate only the pool and never fabricate models or solve conclusions.
fn decode_stage(stage: &Bpp3dExecutionStageArtifact, source: &ModelArtifact) -> Result<DecodedStage> {
    let expected = match stage.kind { Bpp3dExecutionStageKind::Rmp => Csp1dStageKind::Lp, Bpp3dExecutionStageKind::Final => Csp1dStageKind::FinalMilp };
    let Some(child) = &stage.child else {
        return Ok(DecodedStage { kind: stage.kind, pool_json: stage.pool_json.clone(),
            state: decode_bpp3d_column_pool(&stage.pool_json, source)?, report: None,
            model_fingerprint: None, objective: None, selection: None });
    };
    if child.stage != expected { return Err(invalid("BPP3D execution child label contradicts its stage kind")); }
    let decoded = revalidate_stage(&stage.pool_json, child, source)?;
    // 最优 LP 结论只能来自可精确复算的对偶证书，不能仅凭报告标签。
    // Optimal-LP claims must rest on an exactly rechecked dual certificate, never on a report label alone.
    if stage.kind == Bpp3dExecutionStageKind::Rmp && decoded.report.is_optimal() {
        require_optimal_lp_certificate_for_linear_model(&decoded.report, &decoded.actual).map_err(invalid)?;
    }
    Ok(DecodedStage { kind: stage.kind, pool_json: stage.pool_json.clone(), state: decoded.state,
        report: Some(decoded.report), model_fingerprint: Some(child.model.model_fingerprint.value.clone()),
        objective: decoded.objective, selection: decoded.selection })
}

/// 继续到后续阶段的前提：该 RMP 是已认证最优 LP，且其证书已在解码阶段复验。
/// Precondition for any later stage: this RMP is a certified optimal LP whose certificate was already rechecked while decoding.
fn require_optimal_rmp(stage: &DecodedStage) -> Result<()> {
    let Some(report) = &stage.report else { return Err(invalid("BPP3D execution continued from an RMP without an accepted report")); };
    if !report.is_optimal() { return Err(invalid("BPP3D execution continued from an RMP that is not a certified optimal LP")); }
    Ok(())
}

/// 编码真实单轮证据后立即用源问题及原流程结果复验，不写回身份或替代失败语义。
/// Encode actual one-round evidence and immediately revalidate it against the source and the original flow outcome, without rewriting identity or replacing failure semantics.
pub fn encode_bpp3d_execution_report(
    source: &ModelArtifact,
    replay_limit: usize,
    evidence: &Bpp3dOneRoundEvidence,
) -> Result<String> {
    if !(1..=MAX_REPLAY_PLACEMENTS).contains(&replay_limit) { return Err(invalid("BPP3D replay limit must be within 1..=100000")); }
    let child = |record: &LinearStageReport<MetaModelStage>| encode_stage_child(record);
    let mut stages = evidence.rmp.iter().map(|record| Ok(Bpp3dExecutionStageArtifact {
        kind: Bpp3dExecutionStageKind::Rmp, pool_json: encode_bpp3d_column_pool(source, &record.state)?,
        child: record.report.as_ref().map(child).transpose()?,
    })).collect::<Result<Vec<_>>>()?;
    if let Some(final_stage) = &evidence.final_stage {
        stages.push(Bpp3dExecutionStageArtifact { kind: Bpp3dExecutionStageKind::Final,
            pool_json: encode_bpp3d_column_pool(source, &final_stage.state)?, child: final_stage.report.as_ref().map(child).transpose()? });
    }
    // 连续半径解只在原流程成功时存在于 result info；按前缀过滤，避免把无关条目混进报告。
    // Continuous-radius solutions only exist in the result info of a successful flow; filter by prefix so unrelated
    // entries never leak into the report.
    let continuous_radius = evidence.flow.as_ref().ok().map(|flow| flow.result.info.iter()
        .filter(|(key, _)| key.starts_with("continuous_radius_") || key.starts_with("r_"))
        .map(|(key, value)| (key.clone(), value.clone())).collect::<BTreeMap<_, _>>()).unwrap_or_default();
    let artifact = Bpp3dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        replay_limit: u32::try_from(replay_limit).map_err(invalid)?, stages,
        failure: evidence.flow.as_ref().err().map(|errors| errors.join("; ")), continuous_radius };
    let text = serde_json::to_string(&artifact).map_err(invalid)?;
    if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP3D execution report exceeds 8 MiB")); }
    decode_bpp3d_execution_report(&text, source)?;
    Ok(text)
}

/// 复验两条 RMP、最终子阶段、原报告、生命周期与严格物理回放，再从实际 final 向量提取源绑定装箱。
/// Revalidate both RMPs, the final child, the original reports, the lifecycle and strict physical replay, then derive source-bound packing from the actual final vector.
pub fn decode_bpp3d_execution_report(text: &str, source: &ModelArtifact) -> Result<Bpp3dExecutionReport> {
    if text.len() > MAX_BYTES { return Err(BridgeError::new("PayloadTooLarge", "BPP3D execution report exceeds 8 MiB")); }
    let artifact: Bpp3dExecutionReportArtifact = serde_json::from_str(text).map_err(invalid)?;
    if artifact.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported BPP3D execution schema")); }
    if artifact.source_fingerprint != source.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "BPP3D execution belongs to a different source problem")); }
    let replay_limit = usize::try_from(artifact.replay_limit).map_err(invalid)?;
    if !(1..=MAX_REPLAY_PLACEMENTS).contains(&replay_limit) { return Err(invalid("BPP3D execution replay limit is out of range")); }
    let kinds = artifact.stages.iter().map(|stage| stage.kind).collect::<Vec<_>>();
    match kinds.as_slice() {
        [Bpp3dExecutionStageKind::Rmp] | [Bpp3dExecutionStageKind::Rmp, Bpp3dExecutionStageKind::Rmp]
        | [Bpp3dExecutionStageKind::Rmp, Bpp3dExecutionStageKind::Rmp, Bpp3dExecutionStageKind::Final] => {}
        _ => return Err(invalid("BPP3D execution stages must be the initial RMP, the refreshed RMP and an optional final child")),
    }
    let (problem, request, _) = decode_bpp3d_problem(source)?;
    let stages = artifact.stages.iter().map(|stage| decode_stage(stage, source)).collect::<Result<Vec<_>>>()?;
    let completed = artifact.failure.is_none();
    for (index, stage) in stages.iter().enumerate() {
        if stage.report.is_none() && (completed || index + 1 != stages.len()) {
            return Err(invalid("BPP3D stage without an accepted report must terminate a failed execution"));
        }
    }
    // 初始 RMP 只能使用源初始前缀；刷新 RMP 与 final 必须共用本轮的同一完整池。
    // The initial RMP may use only the source initial prefix; the refreshed RMP and final must share the same complete pool.
    let initial = request.initial_layers.len();
    let first = &stages[0];
    if first.state.iteration != 0 || first.state.layers.len() != initial {
        return Err(invalid("BPP3D initial RMP must solve exactly the source initial prefix at iteration 0"));
    }
    if let Some(refreshed) = stages.get(1) {
        if refreshed.state.iteration != 1 || refreshed.state.layers.len() < initial {
            return Err(invalid("BPP3D refreshed RMP must solve the single-round pool at iteration 1"));
        }
        require_optimal_rmp(first)?;
        if let Some(final_stage) = stages.get(2) {
            if final_stage.pool_json != refreshed.pool_json { return Err(invalid("BPP3D final child must reuse the refreshed complete pool verbatim")); }
            require_optimal_rmp(refreshed)?;
        }
    }
    if completed && stages.len() != 3 { return Err(invalid("completed BPP3D execution must retain both RMPs and the final child")); }
    let amounts = request.item_amounts.iter().map(|(_, amount)| *amount).collect::<Vec<_>>();
    let selection = stages.get(2).and_then(|stage| stage.selection.clone());
    let packing = match selection.as_ref() {
        Some(selection) => {
            let stage = stages.get(2).ok_or_else(|| invalid("BPP3D selection without a final child"))?;
            match replay_strict_grouped_packing(&stage.state, selection, &amounts, replay_limit) {
                Ok(packed) => Some(Bpp3dPacking { source_fingerprint: source.fingerprint.clone(), bins: packed_bins(&problem, selection, packed)? }),
                Err(_) if completed => return Err(invalid("completed BPP3D execution failed strict grouped physical replay")),
                // 物理失败保留原分配与诊断，不伪造装箱。 / Physical failures keep the original selection and diagnostics without fabricating packing.
                Err(_) => None,
            }
        }
        None if completed => return Err(invalid("completed BPP3D execution requires a final incumbent and selection")),
        None => None,
    };
    // 半径条目是原流程 info 的逐字副本，复验只能做自洽检查：选中数量与 `_selected_<var>` 键数一致，
    // 且所有半径类数值键都是有限非负数。文本从不被当作权威解，只拒绝明显伪造或残缺的载荷。
    // The radius entries are verbatim copies of the original flow info; validation can only check self-consistency:
    // the selected count matches the `_selected_<var>` keys and every radius-like numeric key is finite and nonnegative.
    // The text is never treated as authoritative; only obviously forged or incomplete payloads are rejected.
    if !artifact.continuous_radius.is_empty() {
        // 原型数量决定应有几个选中解；选中数量不得超过它，且每个数值键必须是有限非负数。
        // The prototype count bounds how many solutions can be selected; the selected count must not exceed it and
        // every numeric key must be finite and nonnegative.
        let prototypes = artifact.continuous_radius.get("continuous_radius_prototype_count")
            .ok_or_else(|| invalid("BPP3D radius entries require the prototype count"))?
            .parse::<usize>().map_err(|_| invalid("BPP3D radius prototype count must be a canonical integer"))?;
        let count = artifact.continuous_radius.get("continuous_radius_selected_count")
            .ok_or_else(|| invalid("BPP3D radius entries require the selected count"))?
            .parse::<usize>().map_err(|_| invalid("BPP3D radius selected count must be a canonical integer"))?;
        if count > prototypes { return Err(invalid("BPP3D radius selected count exceeds the prototype count")); }
        for (key, value) in &artifact.continuous_radius {
            let key = key.as_str();
            if key.ends_with("_count") || key.ends_with("_variables") || key.ends_with("_keys")
                || key.ends_with("_policy") || key.ends_with("_diagnostics") { continue; }
            let parsed = value.parse::<f64>().map_err(|_| invalid("BPP3D radius entry must be a finite decimal"))?;
            if !parsed.is_finite() || parsed < 0.0 { return Err(invalid("BPP3D radius entry must be finite and nonnegative")); }
        }
    }
    let last = stages.last().ok_or_else(|| invalid("BPP3D execution has no stages"))?;
    let summary = Bpp3dExecutionSummary { source_fingerprint: source.fingerprint.clone(), completed, candidate_complete: false,
        column_count: u32::try_from(last.state.layers.len()).map_err(invalid)?,
        generated_count: u32::try_from(last.state.layers.len().saturating_sub(initial)).map_err(invalid)?,
        stage_count: u32::try_from(stages.len()).map_err(invalid)?, final_stage: stages.get(2).map(|_| 2u32),
        has_packing: packing.is_some(), failure: artifact.failure.clone() };
    let summaries = stages.iter().map(DecodedStage::summary).collect::<Result<Vec<_>>>()?;
    Ok(Bpp3dExecutionReport { artifact, summary, stages: summaries, packing, selection })
}

#[cfg(test)]
mod tests;
