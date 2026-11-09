use std::collections::{BTreeMap, HashMap};
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason,
    linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework::solver::{LinearStageReport, MetaModelStage};
use ospf_rust_framework_bpp3d::application::{
    Bpp3dFinalStageEvidence, Bpp3dOneRoundEvidence, Bpp3dRmpStageEvidence, ColumnGenerationApplicationFlowResult,
    ColumnGenerationApplicationState, ColumnGenerationFinalExecution, ColumnGenerationResult, ColumnGenerationRmpExecution,
    ColumnGenerationState, LayerAssignmentFinalExecution, MetaModelFinalExecutor, MetaModelRmpExecutor,
};
use ospf_rust_framework_bpp3d::domain::layer_assignment::PreciseAssignmentSelection;
use super::{Bpp3dExecutionReportArtifact, Bpp3dExecutionStageKind, MAX_REPLAY_PLACEMENTS, decode_bpp3d_execution_report, encode_bpp3d_execution_report};
use crate::bpp3d::MAX_BYTES;
use crate::bpp3d::pool::tests::fixture;
use crate::{Csp1dStageKind, ModelArtifact, decode_bpp3d_problem, encode_bpp3d_column_pool};

type Record = LinearStageReport<MetaModelStage>;

/// 与 pool 夹具同源的初始前缀状态，只保留源初始列与放置 trace。 / Initial-prefix state of the pool fixture, keeping only source initials and placement traces.
fn initial_state(refreshed: &ColumnGenerationApplicationState) -> ColumnGenerationApplicationState {
    let initial = refreshed.initial_layers.len();
    let mut state = refreshed.clone();
    state.layers.truncate(initial);
    state.layer_block_traces.clear();
    state.layer_placement_traces.retain(|index, _| *index < initial);
    state.iteration = 0;
    state
}

fn record(stage: MetaModelStage, model: ospf_rust_core::model::MetaModel<f64>, values: Vec<f64>, duals: Vec<f64>) -> Record {
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let mut solution = SolveSolution::vector(values);
    let objective = compiled.model.c.iter().zip(&solution.values).map(|(cost, value)| cost * value).sum();
    solution.objective = Some(objective);
    solution.objective_value = Some(objective);
    solution.dual_solution = (!duals.is_empty()).then_some(duals);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
        .proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
    LinearStageReport { stage, model: compiled.model, report, objective_constant: compiled.objective_constant }
}

/// 已认证最优 LP 的报告；对偶与原编译模型真实一致。 / Report of a certified optimal LP whose duals really match the compiled model.
fn rmp(state: &ColumnGenerationApplicationState, values: Vec<f64>, duals: Vec<f64>) -> Record {
    record(MetaModelStage::RestrictedMaster, MetaModelRmpExecutor::default().model_with_item_amounts(state, &[2]).unwrap(), values, duals)
}

/// 无 incumbent 的原始终态报告，保留身份但不补对偶或方案。 / Original incumbent-free terminal report retaining identity without invented duals or plans.
fn terminal(state: &ColumnGenerationApplicationState, reason: TerminationReason) -> Record {
    let model = MetaModelRmpExecutor::default().model_with_item_amounts(state, &[2]).unwrap();
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let report = SolveReport::builder(ProblemStatus::Unknown, reason)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
    LinearStageReport { stage: MetaModelStage::RestrictedMaster, model: compiled.model, report, objective_constant: compiled.objective_constant }
}

/// 真实 final 整数向量：仅启用 bin 0 的指定层。 / Actual final integer vector enabling only the given layer of bin 0.
fn final_selection(state: &ColumnGenerationApplicationState, layer: usize) -> Record {
    let (model, assignment) = MetaModelFinalExecutor::default().model_and_assignment_with_item_amounts(state, &[2]).unwrap();
    let mut primal = vec![0.0; model.num_tokens()];
    primal[assignment.x.as_ref().unwrap().model_index(&0, &layer).unwrap()] = 1.0;
    primal[assignment.v.as_ref().unwrap().model_index(&0).unwrap()] = 1.0;
    let token_values = model.tokens_in_solver_order().into_iter().zip(primal).map(|(token, value)| (token.id(), value)).collect::<BTreeMap<_, _>>();
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let values = compiled.model.variables.iter().map(|token| token_values[&token.id()]).collect::<Vec<_>>();
    let mut solution = SolveSolution::vector(values);
    let objective = compiled.model.c.iter().zip(&solution.values).map(|(cost, value)| cost * value).sum();
    solution.objective = Some(objective);
    solution.objective_value = Some(objective);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
        .proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
    LinearStageReport { stage: MetaModelStage::FinalMilp, model: compiled.model, report, objective_constant: compiled.objective_constant }
}

fn flow() -> ColumnGenerationApplicationFlowResult {
    ColumnGenerationApplicationFlowResult { result: ColumnGenerationResult { state: ColumnGenerationState::new(), layers: Vec::new(),
        packing_result: None, render_loading_plans: Vec::new(), info: HashMap::new() }, rmp: ColumnGenerationRmpExecution::default(),
        final_execution: ColumnGenerationFinalExecution::default(), selected_radius_solutions: Vec::new() }
}

fn rmp_stage(state: ColumnGenerationApplicationState, record: Option<Record>) -> Bpp3dRmpStageEvidence {
    Bpp3dRmpStageEvidence { state, amounts: Some(vec![2]), execution: ColumnGenerationRmpExecution::default(), report: record }
}

fn final_stage(state: ColumnGenerationApplicationState, record: Option<Record>, selection: Option<PreciseAssignmentSelection>) -> Bpp3dFinalStageEvidence {
    Bpp3dFinalStageEvidence { state, amounts: Some(vec![2]), execution: LayerAssignmentFinalExecution {
        execution: ColumnGenerationFinalExecution { final_solved: record.is_some(), ..Default::default() }, selection }, report: record }
}

/// 完整三阶段证据：初始 RMP 只覆盖源前缀，刷新 RMP 与 final 共用含生成块的完整池。 / Complete three-stage evidence: the initial RMP covers the source prefix only while the refreshed RMP and final share the pooled block column.
fn completed() -> (ModelArtifact, Bpp3dOneRoundEvidence, PreciseAssignmentSelection) {
    let (source, refreshed, selection) = fixture();
    let initial = initial_state(&refreshed);
    let evidence = Bpp3dOneRoundEvidence { flow: Ok(flow()),
        rmp: vec![rmp_stage(initial.clone(), Some(rmp(&initial, vec![2.0], vec![0.0, -2.0]))),
            rmp_stage(refreshed.clone(), Some(rmp(&refreshed, vec![0.0, 1.0], vec![0.0, -1.0])))],
        final_stage: Some(final_stage(refreshed.clone(), Some(final_selection(&refreshed, 1)), Some(selection.clone()))) };
    (source, evidence, selection)
}

#[test]
fn completed_round_roundtrips_stages_and_derives_strict_packing() {
    let (source, evidence, selection) = completed();
    let text = encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap();
    let report = decode_bpp3d_execution_report(&text, &source).unwrap();
    assert!(report.summary.completed && !report.summary.candidate_complete);
    assert_eq!((report.summary.column_count, report.summary.generated_count, report.summary.stage_count), (2, 1, 3));
    assert_eq!(report.summary.final_stage, Some(2));
    assert_eq!(report.selection.as_ref(), Some(&selection));
    assert!(report.summary.has_packing);
    let packing = report.packing.as_ref().unwrap();
    assert_eq!(packing.source_fingerprint, source.fingerprint);
    assert_eq!(packing.bins.len(), 1);
    assert_eq!(packing.bins[0].placements.len(), 2);
    assert_eq!((report.stages[0].kind, report.stages[2].kind), (Bpp3dExecutionStageKind::Rmp, Bpp3dExecutionStageKind::Final));
    assert!(report.stages[0].restricted_optimal && report.stages[2].has_incumbent);
    assert_eq!(report.artifact.stages[1].pool_json, report.artifact.stages[2].pool_json);
    assert_eq!(report.artifact.stages[1].pool_json, encode_bpp3d_column_pool(&source, &fixture().1).unwrap());
    assert_eq!(encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap(), text);
}

#[test]
fn execution_preserves_original_reports_exact_u64_and_complete_pool() {
    let (source, evidence, _) = completed();
    let mut artifact: Bpp3dExecutionReportArtifact = serde_json::from_str(&encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap()).unwrap();
    let mut child = artifact.stages[0].child.clone().unwrap();
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&child.solver_report_json).unwrap();
    report.statistics.nodes = Some(u64::MAX);
    report.provenance.thread_count = Some(9_007_199_254_740_993);
    child.solver_report_json = serde_json::to_string(&report).unwrap();
    artifact.stages[0].child = Some(child);
    let text = serde_json::to_string(&artifact).unwrap();
    let decoded = decode_bpp3d_execution_report(&text, &source).unwrap();
    assert_eq!(decoded.artifact.stages[0].child.as_ref().unwrap().solver_report_json,
        artifact.stages[0].child.as_ref().unwrap().solver_report_json);
    let restored: SolveReport<f64, u64> = serde_json::from_str(&decoded.artifact.stages[0].child.as_ref().unwrap().solver_report_json).unwrap();
    assert_eq!((restored.statistics.nodes, restored.provenance.thread_count), (Some(u64::MAX), Some(9_007_199_254_740_993)));
    assert!(decoded.packing.is_some());
}

#[test]
fn report_free_and_physical_failures_keep_original_evidence() {
    let (source, refreshed, _) = fixture();
    let initial = initial_state(&refreshed);
    let stopped = Bpp3dOneRoundEvidence { flow: Err(vec!["RestrictedMasterProblem: registration_failed".into()]),
        rmp: vec![rmp_stage(initial.clone(), None)], final_stage: None };
    let text = encode_bpp3d_execution_report(&source, 4096, &stopped).unwrap();
    let report = decode_bpp3d_execution_report(&text, &source).unwrap();
    assert!(!report.summary.completed && report.packing.is_none() && report.selection.is_none());
    assert_eq!(report.stages[0].model_fingerprint, None);
    assert_eq!(report.summary.failure.as_deref(), Some("RestrictedMasterProblem: registration_failed"));

    // 物理失败保留真实分配与原报告，只撤回装箱结论。 / Physical failures keep the actual selection and original report, retracting only the packing conclusion.
    let (source, evidence, selection) = completed();
    let mut artifact: Bpp3dExecutionReportArtifact = serde_json::from_str(&encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap()).unwrap();
    let mut pool: serde_json::Value = serde_json::from_str(&artifact.stages[1].pool_json).unwrap();
    pool["generated"][0]["blocks"][0]["grid"][0] = "1".into();
    let pool = pool.to_string();
    artifact.stages[1].pool_json = pool.clone();
    artifact.stages[2].pool_json = pool;
    assert!(decode_bpp3d_execution_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err());
    artifact.failure = Some("FinalMilp: physical_validation_failed".into());
    let report = decode_bpp3d_execution_report(&serde_json::to_string(&artifact).unwrap(), &source).unwrap();
    assert!(!report.summary.completed && report.packing.is_none());
    assert_eq!(report.selection.as_ref(), Some(&selection));
}

#[test]
fn no_new_columns_round_keeps_the_initial_pool_and_still_requires_failure_evidence() {
    let (source, refreshed, _) = fixture();
    let initial = initial_state(&refreshed);
    let empty = ColumnGenerationApplicationState { layers: initial.layers.clone(), iteration: 1, ..initial.clone() };
    let mut evidence = Bpp3dOneRoundEvidence { flow: Err(vec!["RestrictedMasterProblem: restricted master lacks a certified optimal LP".into()]),
        rmp: vec![rmp_stage(initial.clone(), Some(rmp(&initial, vec![2.0], vec![0.0, -2.0]))),
            rmp_stage(empty.clone(), Some(terminal(&empty, TerminationReason::TimeLimit)))],
        final_stage: None };
    let text = encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap();
    let report = decode_bpp3d_execution_report(&text, &source).unwrap();
    assert_eq!((report.summary.generated_count, report.summary.stage_count, report.summary.final_stage), (0, 2, None));
    assert_eq!(report.stages[1].termination_reason.as_deref(), Some("TIME_LIMIT"));
    assert!(!report.stages[1].restricted_optimal && report.packing.is_none());

    // 少了 final 却声称完成，或刷新阶段偷换列池，都必须拒绝。
    // Claiming completion without the final child, or swapping the refreshed pool, must be rejected.
    evidence.flow = Ok(flow());
    assert!(encode_bpp3d_execution_report(&source, 4096, &evidence).is_err());
    evidence.flow = Err(vec!["stop".into()]);
    let mut artifact: Bpp3dExecutionReportArtifact = serde_json::from_str(&encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap()).unwrap();
    artifact.stages[1].pool_json = artifact.stages[0].pool_json.clone();
    assert!(decode_bpp3d_execution_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err());
}

#[test]
fn lifecycle_and_budget_tampering_is_rejected() {
    let (source, evidence, _) = completed();
    let text = encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap();
    let original: Bpp3dExecutionReportArtifact = serde_json::from_str(&text).unwrap();
    let read = |artifact: &Bpp3dExecutionReportArtifact| decode_bpp3d_execution_report(&serde_json::to_string(artifact).unwrap(), &source);
    for variant in 0..16 {
        let mut artifact = original.clone();
        match variant {
            0 => artifact.schema_version = "2.0".into(),
            1 => artifact.source_fingerprint = "foreign".into(),
            2 => artifact.replay_limit = 0,
            3 => artifact.replay_limit = 100_001,
            4 => artifact.stages.swap(0, 1),
            5 => artifact.stages.swap(1, 2),
            6 => { let last = artifact.stages.pop().unwrap(); artifact.stages.insert(1, last); },
            7 => { let first = artifact.stages[0].clone(); artifact.stages.push(first); },
            8 => artifact.stages[1].pool_json = artifact.stages[0].pool_json.clone(),
            9 => artifact.stages[2].pool_json = artifact.stages[0].pool_json.clone(),
            10 => artifact.stages[0].child = None,
            11 => artifact.stages[1].child.as_mut().unwrap().stage = Csp1dStageKind::FinalMilp,
            12 => artifact.stages[2].child.as_mut().unwrap().stage = Csp1dStageKind::Lp,
            13 => artifact.stages.truncate(1),
            14 => artifact.stages[2].child.as_mut().unwrap().objective_constant += 1.0,
            _ => artifact.stages[1].child.as_mut().unwrap().solver_report_json = "{}".into(),
        }
        assert!(read(&artifact).is_err(), "variant {variant}");
    }
    assert_eq!(decode_bpp3d_execution_report(&" ".repeat(MAX_BYTES + 1), &source).unwrap_err().code, "PayloadTooLarge");
    assert!(encode_bpp3d_execution_report(&source, 0, &evidence).is_err());
    assert!(encode_bpp3d_execution_report(&source, MAX_REPLAY_PLACEMENTS + 1, &evidence).is_err());
    let mut foreign = source.clone();
    foreign.fingerprint = "foreign".into();
    assert_eq!(decode_bpp3d_execution_report(&text, &foreign).unwrap_err().code, "FingerprintMismatch");
}

#[test]
fn optimal_lp_claims_need_a_real_dual_certificate() {
    let (source, evidence, _) = completed();
    let text = encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap();
    for stage in 0..2 {
        let mut artifact: Bpp3dExecutionReportArtifact = serde_json::from_str(&text).unwrap();
        let mut child = artifact.stages[stage].child.clone().unwrap();
        let mut report: SolveReport<f64, u64> = serde_json::from_str(&child.solver_report_json).unwrap();
        report.solution.as_mut().unwrap().dual_solution.as_mut().unwrap().fill(0.0);
        child.solver_report_json = serde_json::to_string(&report).unwrap();
        artifact.stages[stage].child = Some(child);
        assert!(decode_bpp3d_execution_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err(), "stage {stage}");
    }
}

#[test]
fn initial_rmp_may_not_use_generated_columns() {
    let (source, evidence, _) = completed();
    let text = encode_bpp3d_execution_report(&source, 4096, &evidence).unwrap();
    let mut artifact: Bpp3dExecutionReportArtifact = serde_json::from_str(&text).unwrap();
    artifact.stages[0].pool_json = encode_bpp3d_column_pool(&source, &fixture().1).unwrap();
    assert!(decode_bpp3d_execution_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err());
    let (_, request, _) = decode_bpp3d_problem(&source).unwrap();
    assert_eq!(request.item_amounts, vec![("item".to_string(), 2)]);
}
