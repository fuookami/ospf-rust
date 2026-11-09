//! 已编码源进入真实 BPP3D 原生流程的验收；这不是 HTTP 或 N-API 执行入口。 / Acceptance of encoded sources in the real BPP3D native flow; this is not an HTTP or N-API execution entry.

#![cfg(any(feature = "scip", feature = "gurobi10"))]

use std::sync::Arc;
use ospf_rust_bridge::{Bpp3dAssignment, Bpp3dOrientation, Bpp3dProblem, decode_bpp3d_execution_report, decode_bpp3d_problem, decode_bpp3d_stage_report, encode_bpp3d_execution_report, encode_bpp3d_problem, encode_bpp3d_stage_report, pack_bpp3d_layers};
use ospf_rust_core::solver::{LinearSolver, require_optimal_lp_certificate_for_linear_model};
use ospf_rust_framework::solver::meta_model_backend::ReportMetaModelBackend;
use ospf_rust_framework_bpp3d::{
    application::{Bpp3dOneRoundOptions, ColumnGenerationApplicationService, SolverBackedMetaModelRmpExecutor, TraceValidatedMetaModelFinalExecutor, replay_strict_grouped_packing},
    domain::layer_generation::{BlockLayerGenerator, LayerGenerationContext},
};

fn source() -> Bpp3dProblem {
    serde_json::from_str(r#"{
        "items":[{"id":"item","name":"Item","width":{"value":100,"unit":"cm"},"height":{"value":1,"unit":"m"},"depth":{"value":1,"unit":"m"},"weight":{"value":500,"unit":"g"},"amount":"2","orientations":["upright"]}],
        "bins":[{"id":"one","typeCode":"type","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"},"depth":{"value":1,"unit":"m"},"capacity":{"value":1,"unit":"kg"},"isMain":true},
                {"id":"two","typeCode":"type","width":{"value":100,"unit":"cm"},"height":{"value":1,"unit":"m"},"depth":{"value":1,"unit":"m"},"capacity":{"value":1000,"unit":"g"},"isMain":true}],
        "initialLayers":[{"id":"layer","binId":"one","depth":{"value":1,"unit":"m"},"placements":[{"itemId":"item","position":{"x":{"value":0,"unit":"m"},"y":{"value":0,"unit":"m"},"z":{"value":0,"unit":"m"}},"orientation":"upright"}]}]
    }"#).unwrap()
}

fn solve(solver: Arc<dyn LinearSolver>) {
    let artifact = encode_bpp3d_problem(source()).unwrap();
    let (_, request, traces) = decode_bpp3d_problem(&artifact).unwrap();
    let backend = ReportMetaModelBackend::new(solver, Default::default()).unwrap();
    let rmp = SolverBackedMetaModelRmpExecutor::new(Default::default(), backend.clone());
    let final_executor = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend.clone());
    let result = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_traced_initial_layers(
        request, traces, LayerGenerationContext::new(), &rmp, &final_executor,
    ).unwrap();
    assert!(result.final_execution.final_solved, "{:?}", result.final_execution.info);
    assert_eq!(result.final_execution.packed_bins.len(), 2);
    assert!(result.final_execution.packed_bins.iter().all(|bin| bin.items.len() == 1 && bin.items[0].item.id.as_str() == "item"));
    let packed = pack_bpp3d_layers(&artifact, &[
        Bpp3dAssignment { bin_id: "one".into(), layer_id: "layer".into() },
        Bpp3dAssignment { bin_id: "two".into(), layer_id: "layer".into() },
    ]).unwrap();
    assert_eq!(packed.bins.len(), result.final_execution.packed_bins.len());
    for (index, (expected, actual)) in packed.bins.iter().zip(&result.final_execution.packed_bins).enumerate() {
        assert_eq!(actual.name, format!("assigned-bin-{index}"));
        assert_eq!(expected.placements.len(), actual.items.len());
        for (expected, actual) in expected.placements.iter().zip(&actual.items) {
            assert_eq!(expected.item_id, actual.item.id.as_str());
            assert_eq!(expected.orientation, Bpp3dOrientation::from(actual.orientation));
            assert_eq!([expected.position.x.value, expected.position.y.value, expected.position.z.value],
                [actual.position.x.value, actual.position.y.value, actual.position.z.value]);
        }
    }
    let reports = backend.reports();
    assert_eq!(reports.len(), 3);
    for report in &reports[..2] { require_optimal_lp_certificate_for_linear_model(&report.report, &report.model).unwrap(); }
    assert!(reports[2].model.has_integer_variables() && reports[2].report.has_incumbent());
}

fn stage_contract(solver: Arc<dyn LinearSolver>) {
    let mut problem = source();
    problem.bins.truncate(1);
    problem.bins[0].width.value = 2.0;
    let source = encode_bpp3d_problem(problem).unwrap();
    let (_, request, traces) = decode_bpp3d_problem(&source).unwrap();
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(BlockLayerGenerator::new()));
    let evidence = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_evidence(
        request, traces, generation, solver, Default::default(),
    ).unwrap();
    assert!(evidence.flow.is_ok(), "{:?}", evidence.flow.err());
    assert_eq!(evidence.rmp.len(), 2);
    for stage in &evidence.rmp {
        let record = stage.report.as_ref().unwrap();
        let text = encode_bpp3d_stage_report(&source, &stage.state, record).unwrap();
        let decoded = decode_bpp3d_stage_report(&text, &source).unwrap();
        assert_eq!(decoded.report.fingerprints.model, record.report.fingerprints.model);
        assert!(decoded.report.is_optimal());
        assert!(decoded.selection.is_none());
        let mut tampered = decoded.artifact;
        let mut report = decoded.report;
        report.solution.as_mut().unwrap().dual_solution.as_mut().unwrap().fill(0.0);
        tampered.child.solver_report_json = serde_json::to_string(&report).unwrap();
        assert!(decode_bpp3d_stage_report(&serde_json::to_string(&tampered).unwrap(), &source).is_err());
    }
    let final_stage = evidence.final_stage.as_ref().unwrap();
    let record = final_stage.report.as_ref().unwrap();
    let text = encode_bpp3d_stage_report(&source, &final_stage.state, record).unwrap();
    let decoded = decode_bpp3d_stage_report(&text, &source).unwrap();
    assert_eq!(decoded.selection, final_stage.execution.selection);
    let selection = decoded.selection.as_ref().unwrap();
    assert_eq!(selection.assignments.len(), 1);
    assert_eq!(decoded.state.layers[selection.assignments[0].layer_index].from, "block_layer_generator");
    assert_eq!(replay_strict_grouped_packing(&decoded.state, selection, &[2], 4096).unwrap()[0].items.len(), 2);
}

/// 单轮完整执行报告的源绑定往返：真实证据、真实 final 分配与严格物理装箱。
/// Source-bound roundtrip of a complete one-round execution report with real evidence, the actual final selection and strict physical packing.
fn execution_contract(solver: Arc<dyn LinearSolver>) {
    let mut problem = source();
    problem.bins.truncate(1);
    problem.bins[0].width.value = 2.0;
    let source = encode_bpp3d_problem(problem).unwrap();
    let (_, request, traces) = decode_bpp3d_problem(&source).unwrap();
    let mut generation = LayerGenerationContext::new();
    generation.add_generator(Box::new(BlockLayerGenerator::new()));
    let options = Bpp3dOneRoundOptions::default();
    let limit = options.validation.max_placements;
    let evidence = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_evidence(
        request, traces, generation, solver, options,
    ).unwrap();
    assert!(evidence.flow.is_ok(), "{:?}", evidence.flow.err());
    assert_eq!(evidence.rmp.len(), 2);
    let text = encode_bpp3d_execution_report(&source, limit, &evidence).unwrap();
    let report = decode_bpp3d_execution_report(&text, &source).unwrap();
    assert!(report.summary.completed && report.summary.has_packing && !report.summary.candidate_complete);
    assert_eq!(report.summary.stage_count, 3);
    assert!(report.summary.generated_count >= 1 && report.summary.column_count == 1 + report.summary.generated_count);
    let final_stage = evidence.final_stage.as_ref().unwrap();
    assert_eq!(report.selection.as_ref(), final_stage.execution.selection.as_ref());
    let packing = report.packing.as_ref().unwrap();
    assert_eq!(packing.bins.len(), final_stage.execution.execution.packed_bins.len());
    for (packed, actual) in packing.bins.iter().zip(&final_stage.execution.execution.packed_bins) {
        assert_eq!(packed.placements.len(), actual.items.len());
        assert!(packed.placements.iter().all(|placement| placement.item_id == "item"));
    }
    assert_eq!(encode_bpp3d_execution_report(&source, limit, &evidence).unwrap(), text);
}

/// 单箱单层无法覆盖分组需求时，final 认证不可行只保留证据，不提升为源问题不可行。
/// When one bin and one layer cannot cover the grouped demand, the certified-infeasible final keeps evidence without promoting source-wide infeasibility.
fn infeasible_final_contract(solver: Arc<dyn LinearSolver>) {
    let mut problem = source();
    problem.bins.truncate(1);
    let source = encode_bpp3d_problem(problem).unwrap();
    let (_, request, traces) = decode_bpp3d_problem(&source).unwrap();
    let options = Bpp3dOneRoundOptions::default();
    let limit = options.validation.max_placements;
    let evidence = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_evidence(
        request, traces, LayerGenerationContext::new(), solver, options,
    ).unwrap();
    assert!(evidence.flow.is_err(), "{:?}", evidence.flow);
    assert_eq!(evidence.rmp.len(), 2);
    let final_stage = evidence.final_stage.as_ref().expect("final must still be attempted");
    assert_eq!(final_stage.report.as_ref().unwrap().report.problem_status, ospf_rust_core::solver::ProblemStatus::Infeasible);
    let text = encode_bpp3d_execution_report(&source, limit, &evidence).unwrap();
    let report = decode_bpp3d_execution_report(&text, &source).unwrap();
    assert!(!report.summary.completed && report.summary.failure.is_some());
    assert!(report.summary.final_stage == Some(2) && !report.summary.has_packing);
    assert!(report.stages[2].restricted_infeasible && !report.stages[2].has_incumbent);
    assert!(report.selection.is_none() && report.packing.is_none());
}

#[cfg(feature = "scip")]
#[test]
fn scip_runs_original_grouped_flow_from_source_artifact() {
    use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
    let solver = Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults()));
    solve(solver.clone());
    stage_contract(solver.clone());
    execution_contract(solver.clone());
    infeasible_final_contract(solver);
}

#[cfg(feature = "gurobi10")]
#[test]
fn gurobi_runs_original_grouped_flow_from_source_artifact() {
    use ospf_rust_core_plugin_gurobi::{GurobiConfig, GurobiSolver};
    let solver = Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false)));
    solve(solver.clone());
    stage_contract(solver.clone());
    execution_contract(solver.clone());
    infeasible_final_contract(solver);
}
