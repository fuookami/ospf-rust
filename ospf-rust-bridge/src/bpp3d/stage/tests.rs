use std::collections::BTreeMap;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_bpp3d::application::replay_strict_grouped_packing;
use super::*;
use crate::bpp3d::pool::tests::fixture;

fn terminal(model: &MetaModel<f64>, stage: MetaModelStage, reason: TerminationReason) -> LinearStageReport<MetaModelStage> {
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let report = SolveReport::builder(ProblemStatus::Unknown, reason)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
    LinearStageReport { stage, model: compiled.model, report, objective_constant: compiled.objective_constant }
}

fn final_record(state: &ColumnGenerationApplicationState) -> LinearStageReport<MetaModelStage> {
    let (model, assignment) = MetaModelFinalExecutor::default().model_and_assignment_with_item_amounts(state, &[2]).unwrap();
    let mut primal = vec![0.0; model.num_tokens()];
    primal[assignment.x.as_ref().unwrap().model_index(&0, &1).unwrap()] = 1.0;
    primal[assignment.v.as_ref().unwrap().model_index(&0).unwrap()] = 1.0;
    let values = model.tokens_in_solver_order().into_iter().zip(primal).map(|(token, value)| (token.id(), value)).collect::<BTreeMap<_, _>>();
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let values = compiled.model.variables.iter().map(|token| values[&token.id()]).collect::<Vec<_>>();
    let objective = compiled.model.c.iter().zip(&values).map(|(cost, value)| cost * value).sum();
    let mut solution = SolveSolution::vector(values);
    solution.objective = Some(objective); solution.objective_value = Some(objective);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::TimeLimit).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
    LinearStageReport { stage: MetaModelStage::FinalMilp, model: compiled.model, report, objective_constant: compiled.objective_constant }
}

#[test]
fn final_stage_derives_actual_generated_selection_and_requires_separate_physical_replay() {
    let (source, state, expected) = fixture();
    let record = final_record(&state);
    let text = encode_bpp3d_stage_report(&source, &state, &record).unwrap();
    let decoded = decode_bpp3d_stage_report(&text, &source).unwrap();
    assert_eq!(decoded.artifact.child.solver_report_json, serde_json::to_string(&record.report).unwrap());
    assert_eq!(decoded.selection.as_ref(), Some(&expected));
    assert_eq!(decoded.report.termination_reason, TerminationReason::TimeLimit);
    assert_eq!(decoded.objective, record.report.solution.as_ref().unwrap().objective_value.map(|value| value + record.objective_constant));
    assert_eq!(replay_strict_grouped_packing(&decoded.state, decoded.selection.as_ref().unwrap(), &[2], 4096).unwrap()[0].items.len(), 2);

    let mut untrusted = state.clone();
    untrusted.layer_block_traces.get_mut(&1).unwrap()[0].nx = 1;
    let decoded = decode_bpp3d_stage_report(&encode_bpp3d_stage_report(&source, &untrusted, &record).unwrap(), &source).unwrap();
    assert_eq!(decoded.selection.as_ref(), Some(&expected));
    assert!(replay_strict_grouped_packing(&decoded.state, decoded.selection.as_ref().unwrap(), &[2], 4096).is_err());
}

#[test]
fn terminal_stages_preserve_status_exact_counters_and_require_infeasibility_proof() {
    let (source, state, _) = fixture();
    for stage in [MetaModelStage::RestrictedMaster, MetaModelStage::FinalMilp] {
        let model = if stage == MetaModelStage::RestrictedMaster {
            MetaModelRmpExecutor::default().model_with_item_amounts(&state, &[2]).unwrap()
        } else { MetaModelFinalExecutor::default().model_with_item_amounts(&state, &[2]).unwrap() };
        for reason in [TerminationReason::TimeLimit, TerminationReason::Cancelled, TerminationReason::Interrupted] {
            let record = terminal(&model, stage, reason);
            let text = encode_bpp3d_stage_report(&source, &state, &record).unwrap();
            let mut artifact: Bpp3dStageReportArtifact = serde_json::from_str(&text).unwrap();
            let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.child.solver_report_json).unwrap();
            report.statistics.nodes = Some(u64::MAX);
            report.provenance.thread_count = Some(9_007_199_254_740_993);
            artifact.child.solver_report_json = serde_json::to_string(&report).unwrap();
            let decoded = decode_bpp3d_stage_report(&serde_json::to_string(&artifact).unwrap(), &source).unwrap();
            assert_eq!(decoded.report.termination_reason, reason);
            assert_eq!(decoded.report.statistics.nodes, Some(u64::MAX));
            assert_eq!(decoded.report.provenance.thread_count, Some(9_007_199_254_740_993));
            assert!(decoded.objective.is_none() && decoded.selection.is_none());
        }
        let mut record = terminal(&model, stage, TerminationReason::TimeLimit);
        record.report = SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed).proof(SolveProof::infeasibility())
            .fingerprints(record.report.fingerprints.clone()).model_mapping(record.report.model_mapping.clone()).build().unwrap();
        let text = encode_bpp3d_stage_report(&source, &state, &record).unwrap();
        let mut artifact: Bpp3dStageReportArtifact = serde_json::from_str(&text).unwrap();
        let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.child.solver_report_json).unwrap();
        report.proof = None;
        artifact.child.solver_report_json = serde_json::to_string(&report).unwrap();
        assert!(decode_bpp3d_stage_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err());
    }
}

#[test]
fn stage_rejects_forged_identity_math_vectors_and_cross_source_pool() {
    let (source, state, _) = fixture();
    let text = encode_bpp3d_stage_report(&source, &state, &final_record(&state)).unwrap();
    let original: Bpp3dStageReportArtifact = serde_json::from_str(&text).unwrap();
    for variant in 0..8 {
        let mut artifact = original.clone();
        let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.child.solver_report_json).unwrap();
        match variant {
            0 => artifact.schema_version = "2.0".into(),
            1 => artifact.child.stage = Csp1dStageKind::Lp,
            2 => artifact.child.stage = Csp1dStageKind::Milp,
            3 => artifact.child.objective_constant += 1.0,
            4 => report.fingerprints.model = None,
            5 => report.solution.as_mut().unwrap().values.clear(),
            6 => report.solution.as_mut().unwrap().objective_value = Some(999.0),
            _ => { let mut pool: serde_json::Value = serde_json::from_str(&artifact.pool_json).unwrap(); pool["sourceFingerprint"] = "foreign".into(); artifact.pool_json = pool.to_string(); },
        }
        artifact.child.solver_report_json = serde_json::to_string(&report).unwrap();
        assert!(decode_bpp3d_stage_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err(), "variant {variant}");
    }
    let mut altered = state.clone();
    altered.layers[1].demand_coverage[0].coefficient = 1.0;
    let model = MetaModelFinalExecutor::default().model_with_item_amounts(&altered, &[2]).unwrap();
    let foreign = terminal(&model, MetaModelStage::FinalMilp, TerminationReason::TimeLimit);
    let mut artifact = original;
    artifact.child.model = encode_compiled_linear_model(&foreign.model).unwrap();
    artifact.child.solver_report_json = serde_json::to_string(&foreign.report).unwrap();
    artifact.child.objective_constant = foreign.objective_constant;
    assert!(decode_bpp3d_stage_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err());
    assert_eq!(decode_bpp3d_stage_report(&" ".repeat(MAX_BYTES + 1), &source).unwrap_err().code, "PayloadTooLarge");
}
