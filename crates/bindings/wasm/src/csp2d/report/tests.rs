use wasm_bindgen_test::wasm_bindgen_test;
use ospf_rust_bridge::*;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework::solver::remote::encode_compiled_linear_model;
use ospf_rust_framework_csp2d::domain::produce::{Csp2dMasterProblem, Csp2dPatternAssignment, ProduceContext, ProduceModelInput, ProduceStage};
use crate::csp2d::Csp2dCuttingContext;
use super::*;

fn problem() -> Csp2dProblem {
    serde_json::from_str(r#"{"products":[{"id":"a","name":"A","geometry":{"width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"allowRotate":false}],"materials":[{"id":"s","name":"S","shape":{"kind":"sheet","geometry":{"width":{"value":4,"unit":"m"},"height":{"value":2,"unit":"m"}}},"availableBatches":"9007199254740991","process":"freeform","maxSegmentHeight":null,"products":[{"productId":"a","count":"4"}]}],"demands":[{"kind":"count","productId":"a","count":"0","upperBound":null}],"initialColumns":[{"id":"seed","materialId":"s","region":{"origin":{"x":{"value":0,"unit":"m"},"y":{"value":0,"unit":"m"}},"size":{"width":{"value":2,"unit":"m"},"height":{"value":1,"unit":"m"}}},"placements":[{"shape":{"kind":"rectangle","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"position":{"x":{"value":0,"unit":"m"},"y":{"value":0,"unit":"m"}},"rotated":false,"itemId":"a"},{"shape":{"kind":"rectangle","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"position":{"x":{"value":1,"unit":"m"},"y":{"value":0,"unit":"m"}},"rotated":false,"itemId":"a"}],"cutTree":null}],"kerf":{"value":0,"unit":"m"},"objective":{"areaWeight":1,"wasteWeight":0,"heightWeight":0,"usageWeight":0}}"#).unwrap()
}

#[wasm_bindgen_test]
fn certified_stage_reading_and_independent_exact_production_work_without_native_clocks() {
    let problem = problem(); let source = encode_csp2d_problem(problem.clone()).unwrap(); let input = problem.domain().unwrap();
    let master = Csp2dMasterProblem { assignment: Csp2dPatternAssignment { plans: input.initial_columns.clone(), usage_upper_bounds: vec![0] },
        demands: input.demands, material_availability: [("s".into(), 9_007_199_254_740_991)].into_iter().collect() };
    let mut stages = Vec::new();
    for kind in [Csp2dStageKind::Rmp, Csp2dStageKind::Integer] {
        let mut model = MetaModel::new("wasm-csp2d-child");
        ProduceContext { objective: input.objective.clone() }.register_model(&mut model, ProduceModelInput { master: &master,
            stage: if kind == Csp2dStageKind::Rmp { ProduceStage::Rmp } else { ProduceStage::Integer }, kerf: &input.requests[0].kerf }).unwrap();
        let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
        let amount = if kind == Csp2dStageKind::Rmp { 0.0 } else { 9_007_199_254_740_991.0 };
        let mut solution = SolveSolution::vector(vec![amount]); solution.objective_value = Some(compiled.model.c[0] * amount);
        if kind == Csp2dStageKind::Rmp { solution.dual_solution = Some(vec![0.0, 0.0]); }
        let report: SolveReport<f64> = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
            .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
        let mut exact: SolveReport<f64, u64> = serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        if kind == Csp2dStageKind::Rmp { exact.proof = Some(SolveProof::optimality()); exact.solution_presence = ospf_rust_core::solver::SolutionPresence::Optimal; }
        exact.statistics.nodes = Some(u64::MAX);
        stages.push(Csp2dStageReportArtifact { stage: kind, pool_size: 1, model: encode_compiled_linear_model(&compiled.model).unwrap(),
            solver_report_json: serde_json::to_string(&exact).unwrap(), objective_constant: compiled.objective_constant });
    }
    let artifact = Csp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        configuration: Csp2dOptions { backend: NativeBackend::Scip, generators: None, iteration_limit: Some(0), candidates_per_iteration: None, pool_limit: None,
            final_integer_enabled: None, reduced_cost_tolerance: None, report_limit: None }, columns: problem.initial_columns, stages, iterations: Vec::new(),
        stop: Csp2dExecutionStop::IterationLimit, final_stage: Some(1) };
    let text = serde_json::to_string(&artifact).unwrap(); let source_text = serde_json::to_string(&source).unwrap();
    let context = Csp2dCuttingContext::from_artifact(&source_text).unwrap(); let report = context.read_execution_report(&text).unwrap();
    let other = Csp2dExecutionReportResult::from_json(&text, &source_text).unwrap();
    assert!(report.stage_report_json(0).unwrap().contains("18446744073709551615")); assert_eq!(report.to_json(), text);
    assert!(report.stage_json(u32::MAX).unwrap().is_none());
    let production = report.production().unwrap(); drop(context); drop(report); drop(other);
    assert!(production.to_json().unwrap().contains("18014398509481982"));
    let mut forged = artifact; forged.final_stage = Some(u32::MAX);
    assert!(Csp2dExecutionReportResult::from_json(&serde_json::to_string(&forged).unwrap(), &source_text).is_err());
}
