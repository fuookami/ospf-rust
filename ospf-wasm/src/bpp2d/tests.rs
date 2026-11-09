use wasm_bindgen_test::wasm_bindgen_test;
use ospf_rust_core::model::MetaModel;
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework::solver::remote::encode_compiled_linear_model;
use ospf_rust_framework_bpp2d::domain::layer_assignment::{LayerAssignmentContext, LayerAssignmentModelInput};
use super::*;

fn context() -> Bpp2dPackingContext {
    Bpp2dPackingContext::new(r#"{"items":[{"id":"a","name":"A","shape":{"kind":"rectangle","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"allowRotate":false,"generationCount":"3"}],"sheets":[{"id":"s","geometry":{"width":{"value":2,"unit":"m"},"height":{"value":1,"unit":"m"}},"isMain":true}],"demands":[{"kind":"count","itemId":"a","count":"3","upperBound":null}],"initialLayers":[{"sheetId":"s","envelope":{"width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"placements":[{"shape":{"kind":"rectangle","width":{"value":1,"unit":"m"},"height":{"value":1,"unit":"m"}},"position":{"x":{"value":0,"unit":"m"},"y":{"value":0,"unit":"m"}},"rotated":false,"itemId":"a"}]}],"kerf":{"value":0,"unit":"m"},"objective":{"sheetAreaWeight":1,"sheetAmountWeight":0.000001,"sheetTypeCostWeight":0,"sheetTypeCosts":{},"rmpAreaCoefficient":1}}"#).unwrap()
}

#[wasm_bindgen_test]
fn source_bound_lp_certificates_and_integer_production_are_portable_without_native_clocks() {
    let context = context();
    let (problem, input) = decode_bpp2d_problem(&context.artifact).unwrap();
    let mut stages = Vec::new();
    for kind in [Bpp2dStageKind::Rmp, Bpp2dStageKind::Integer] {
        let mut model = MetaModel::new("wasm-bpp2d-child");
        LayerAssignmentContext::new(input.objective.clone()).register_model(&mut model, LayerAssignmentModelInput {
            layers: &input.request.existing_layers, demands: &input.demands, stage: match kind { Bpp2dStageKind::Rmp => ospf_rust_framework_bpp2d::domain::layer_assignment::LayerAssignmentStage::Rmp,
                _ => ospf_rust_framework_bpp2d::domain::layer_assignment::LayerAssignmentStage::Integer },
        }).unwrap();
        let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
        let mut solution = SolveSolution::vector(vec![3.0]);
        solution.objective_value = Some(compiled.model.c[0] * 3.0);
        if kind == Bpp2dStageKind::Rmp { solution.dual_solution = Some(vec![-2.0]); }
        let report: SolveReport<f64> = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
            .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
        let mut exact: SolveReport<f64, u64> = serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        exact.statistics.nodes = Some(u64::MAX);
        stages.push(Bpp2dStageReportArtifact { stage: kind, pool_size: 1, model: encode_compiled_linear_model(&compiled.model).unwrap(),
            solver_report_json: serde_json::to_string(&exact).unwrap(), objective_constant: compiled.objective_constant });
    }
    let artifact = Bpp2dExecutionReportArtifact { schema_version: "1.0".into(), source_fingerprint: context.artifact.fingerprint.clone(),
        configuration: Bpp2dOptions { backend: NativeBackend::Scip, generators: None, iteration_limit: Some(0), candidates_per_iteration: None, pool_limit: None,
            final_milp_enabled: None, reduced_cost_tolerance: None, report_limit: None }, columns: problem.initial_layers, stages, iterations: Vec::new(),
        stop: Bpp2dExecutionStop::IterationLimit, final_stage: Some(1) };
    let text = serde_json::to_string(&artifact).unwrap();
    let report = context.read_execution_report(&text).unwrap();
    assert!(report.stage_report_json(0).unwrap().contains("18446744073709551615"));
    assert!(report.stage_json(u32::MAX).unwrap().is_none());
    let production = report.production().unwrap();
    let packed = report.packing().unwrap().unwrap();
    let copy = context.pack_layers(&serde_json::to_string(&report.value.artifact.columns).unwrap()).unwrap();
    drop(context); drop(report);
    assert_eq!(production.value.produced_counts["a"], "3");
    assert_eq!(packed.value.item_count, "3");
    assert_eq!(packed.value.material_counts["a"], "3");
    assert_eq!(packed.value.patterns.len(), 1);
    assert_eq!(packed.value.remnant_area.value, 3.0);
    assert_eq!(copy.value.item_count, "1");
    assert!(packed.to_json().unwrap().contains("\"loadingOrder\":0"));
    let mut tampered = artifact; tampered.final_stage = Some(u32::MAX);
    assert!(Bpp2dExecutionReportResult::from_json(&serde_json::to_string(&tampered).unwrap(), &context_source()).is_err());
}

fn context_source() -> String { context().to_artifact_json().unwrap() }

#[wasm_bindgen_test]
fn compressed_packing_keeps_u128_totals_beyond_js_safe_integers_on_wasm32() {
    let context = context();
    let (mut problem, _) = decode_bpp2d_problem(&context.artifact).unwrap();
    let mut second = problem.initial_layers[0].placements[0].clone();
    second.position.x.value = 1.0;
    problem.initial_layers[0].envelope.width.value = 2.0;
    problem.initial_layers[0].placements.push(second);
    let source = encode_bpp2d_problem(problem.clone()).unwrap();
    let packed = Bpp2dPackingResult { value: pack_bpp2d_layers(&source, &problem.initial_layers,
        &[Bpp2dUsage { column_index: 0, count: "9007199254740991".into() }]).unwrap() };
    drop(context);
    assert_eq!(packed.value.patterns.len(), 1);
    assert_eq!(packed.value.item_count, "18014398509481982");
    assert!(packed.to_json().unwrap().contains("\"itemCount\":\"18014398509481982\""));
}

#[wasm_bindgen_test]
fn source_and_layout_validation_reject_foreign_identity_and_disallowed_units() {
    let context = context();
    let source = context.to_artifact_json().unwrap();
    let restored = Bpp2dPackingContext::from_artifact(&source).unwrap();
    let (problem, _) = decode_bpp2d_problem(&restored.artifact).unwrap();
    restored.validate_layers(&serde_json::to_string(&problem.initial_layers).unwrap()).unwrap();
    let mut layers = problem.initial_layers; layers[0].sheet_id = "foreign".into();
    assert!(restored.validate_layers(&serde_json::to_string(&layers).unwrap()).is_err());
    let mut artifact = restored.artifact; artifact.fingerprint = "foreign".into();
    assert!(Bpp2dPackingContext::from_artifact(&serde_json::to_string(&artifact).unwrap()).is_err());
}
