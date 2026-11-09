use wasm_bindgen_test::wasm_bindgen_test;
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use super::*;

const SOURCE: &str = r#"{"items":[{"id":"a","weight":{"value":1000,"unit":"g"},"required":true,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"18446744073709551615","maxWeight":{"value":1,"unit":"kg"},"fixedOccupied":false,"enabled":true},{"id":"q","maxCount":"1","maxWeight":{"value":0,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#;
const SOLUTION: &str = r#"{"assignments":{"a":"p"},"occupiedPositions":["p","q"],"selectedLayouts":{}}"#;

#[wasm_bindgen_test]
fn partition_application_control_replay_never_requires_a_native_clock_or_solver() {
    let context = DiscreteLoadingContext::new(SOURCE).unwrap(); let artifact = context.to_artifact_json().unwrap();
    let source: ospf_rust_bridge::ModelArtifact = serde_json::from_str(&artifact).unwrap();
    let core = SolveReport::<f64>::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit).build().unwrap();
    let application = ospf_rust_bridge::BppLoadingApplicationEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        registration_present: false, report_json: serde_json::to_string(&core).unwrap(), objective: None };
    let envelope = serde_json::json!({"schemaVersion":"1.0","sourceFingerprint":source.fingerprint,"timeLimitMs":1,
        "options":{"partition":{"partitions":[{"id":"whole","positionIds":["p","q"]}]}},
        "observations":[{"terminalCheck":true,"remainingNanos":"0","cancelled":false}],"calls":[],"applicationReportJson":serde_json::to_string(&application).unwrap()});
    let text = envelope.to_string(); let report = PartitionColumnGenerationReport::from_json(&text, &artifact).unwrap(); drop(context);
    assert_eq!(report.to_json(), text); assert!(report.solution().is_none()); assert_eq!(report.calls_json().unwrap(), "[]");
    assert!(report.summary_json().unwrap().contains("TIME_LIMIT"));
}

#[wasm_bindgen_test]
fn original_sparse_model_values_and_independent_results_work_without_native_clocks() {
    let context = DiscreteLoadingContext::new(SOURCE).unwrap();
    let artifact = context.to_artifact_json().unwrap(); let linear = context.to_linear_triad_model_json().unwrap();
    let restored = DiscreteLoadingContext::from_artifact(&artifact).unwrap(); assert_eq!(restored.to_linear_triad_model_json().unwrap(), linear);
    assert!(artifact.contains("18446744073709551615"));
    let values = context.solution_values(SOLUTION).unwrap(); assert_eq!(values, vec![1.0, 1.0, 1.0]);
    let result = context.extract_solution(&values).unwrap(); assert_eq!(result.to_json().unwrap(), context.verify_solution(SOLUTION).unwrap().to_json().unwrap());
    assert!(context.extract_solution(&[1.0]).is_err()); assert!(context.extract_solution(&[f64::NAN, 1.0, 1.0]).is_err());
    drop(context); drop(restored); assert!(result.to_json().unwrap().contains("\"loadedCount\":\"1\""));
    assert!(DiscreteLoadingContext::new(&SOURCE.replace("1000", "1001")).unwrap().verify_solution(SOLUTION).is_err());
}

#[wasm_bindgen_test]
fn lossless_report_identity_and_u64_metadata_are_portable_and_results_are_independent() {
    let context = DiscreteLoadingContext::new(SOURCE).unwrap(); let artifact = context.to_artifact_json().unwrap();
    let model = ospf_rust_bridge::decode_model(&serde_json::from_str(&context.to_linear_triad_model_json().unwrap()).unwrap()).unwrap();
    let mut solution = SolveSolution::vector(context.solution_values(SOLUTION).unwrap()); solution.objective_value = Some(0.0);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    let text = serde_json::to_string(&report).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
    let report = context.read_report(&text).unwrap(); let restored = DiscreteLoadingReport::from_json(&text, &artifact).unwrap();
    let result = report.solution().unwrap(); drop(context); drop(report); assert_eq!(restored.to_json(), text);
    assert!(result.to_json().unwrap().contains("\"loadedCount\":\"1\""));
    let mut bad: serde_json::Value = serde_json::from_str(&text).unwrap(); bad["fingerprints"]["model"] = serde_json::Value::Null;
    assert!(DiscreteLoadingReport::from_json(&bad.to_string(), &artifact).is_err());
}

#[wasm_bindgen_test]
fn objective_registration_and_application_report_readers_remain_portable_and_lossless() {
    let context = DiscreteLoadingContext::with_objective(SOURCE, r#"{"itemCosts":{"a":-5},"positionCosts":{"p":1,"q":2},"layoutCosts":[]}"#).unwrap();
    let artifact = context.to_artifact_json().unwrap();
    let model = ospf_rust_bridge::decode_model(&serde_json::from_str(&context.to_application_model_json().unwrap()).unwrap()).unwrap();
    let mut solution = SolveSolution::vector(context.solution_values(SOLUTION).unwrap()); solution.objective_value = Some(-2.0);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    let raw = serde_json::to_string(&report).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
    let source: ospf_rust_bridge::ModelArtifact = serde_json::from_str(&artifact).unwrap();
    let text = serde_json::json!({"schemaVersion":"1.0","sourceFingerprint":source.fingerprint,"registrationPresent":true,"reportJson":raw,"objective":-2.0}).to_string();
    let report = DiscreteLoadingApplicationReport::from_json(&text, &artifact).unwrap();
    assert_eq!(report.to_json(), text); assert_eq!(report.solver_report_json(), raw); assert!(report.registration_present());
    let result = report.solution().unwrap(); drop(context); drop(report); assert!(result.to_json().unwrap().contains("\"loadedCount\":\"1\""));
}

#[wasm_bindgen_test]
fn continuous_resources_keep_explicit_units_and_cannot_be_omitted_or_silently_zeroed() {
    let source = SOURCE.replace("\"maxWeight\":{\"value\":1", "\"maxWeight\":{\"value\":2");
    let resources = r#"[{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1000,"unit":"g"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#;
    let context = DiscreteLoadingContext::with_resources(&source, "null", resources).unwrap();
    assert!(context.solution_values(SOLUTION).is_err());
    let weights = r#"{"ballast":{"value":500,"unit":"g"}}"#;
    let values = context.solution_values_with_resources(SOLUTION, weights).unwrap(); assert_eq!(values[3], 0.5);
    let result = context.verify_solution_with_resources(SOLUTION, weights).unwrap();
    assert_eq!(result.to_json().unwrap(), context.extract_solution(&values).unwrap().to_json().unwrap());
    assert!(context.verify_solution_with_resources(SOLUTION, r#"{"ballast":{"value":0.49,"unit":"kg"}}"#).is_err());
    let restored = DiscreteLoadingContext::from_artifact(&context.to_artifact_json().unwrap()).unwrap();
    assert_eq!(restored.to_application_model_json().unwrap(), context.to_application_model_json().unwrap());
    drop(context); drop(restored);
    let data: serde_json::Value = serde_json::from_str(&result.to_json().unwrap()).unwrap();
    assert_eq!(data["loadedWeight"]["value"], 1.0); assert_eq!(data["resources"]["totalWeight"]["value"], 1.5);
}

#[wasm_bindgen_test]
fn benders_reader_preserves_actual_nested_text_and_proof_free_resource_incumbents() {
    let source = SOURCE.replace("\"maxWeight\":{\"value\":1", "\"maxWeight\":{\"value\":2");
    let resources = r#"[{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1000,"unit":"g"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#;
    let context = DiscreteLoadingContext::with_resources(&source, "null", resources).unwrap(); let artifact = context.to_artifact_json().unwrap();
    let source: ospf_rust_bridge::ModelArtifact = serde_json::from_str(&artifact).unwrap();
    let model = ospf_rust_bridge::decode_model(&serde_json::from_str(&context.to_application_model_json().unwrap()).unwrap()).unwrap();
    let values = context.solution_values_with_resources(SOLUTION, r#"{"ballast":{"value":500,"unit":"g"}}"#).unwrap();
    let mut solution = SolveSolution::vector(values.clone()); solution.objective_value = Some(1.0);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::IterationLimit).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    let raw = serde_json::to_string(&report).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
    let application = serde_json::json!({"schemaVersion":"1.0","sourceFingerprint":source.fingerprint,"registrationPresent":true,"reportJson":raw,"objective":1.0}).to_string();
    let decomposition = serde_json::json!({"schemaVersion":"1.0","fingerprint":ospf_rust_bridge::encode_model(&model).unwrap().fingerprint,"tolerance":1e-7,
        "masterReportJson":raw,"subproblemReportJson":null,"summary":{"stop":"iterationLimit","terminalProblemStatus":null,"terminalTerminationReason":null,
        "values":values,"objectiveValue":1.0,"bestBound":0.0,"iterations":1,"feasibilityCuts":0,"optimalityCuts":0,"backendError":null}}).to_string();
    let text = serde_json::json!({"schemaVersion":"1.0","sourceFingerprint":source.fingerprint,"options":{"recourseResources":["ballast"],"iterationLimit":1},
        "applicationReportJson":application,"decompositionReportJson":decomposition,"decompositionError":null,"fallbackReportJson":null,"fallbackUsed":false}).to_string();
    let read = DiscreteLoadingBendersReport::from_json(&text, &artifact).unwrap(); assert_eq!(read.to_json(), text); assert_eq!(read.solver_report_json(), raw);
    assert_eq!(read.decomposition_report_json(), Some(decomposition)); assert!(!read.fallback_used());
    assert!(!read.source_finishing_present()); assert!(!read.source_finishing_invoked()); assert_eq!(read.acceleration_application_report_json(), application);
    let mut final_report = report; final_report.proof = Some(ospf_rust_core::solver::SolveProof::optimality());
    final_report.termination_reason = TerminationReason::Completed; final_report.solution_presence = ospf_rust_core::solver::SolutionPresence::Optimal;
    let final_raw = serde_json::to_string(&final_report).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
    let selected = serde_json::json!({"schemaVersion":"1.0","sourceFingerprint":source.fingerprint,"registrationPresent":true,"reportJson":final_raw,"objective":1.0}).to_string();
    let mut envelope: serde_json::Value = serde_json::from_str(&text).unwrap();
    envelope["options"]["sourceMilpFinishing"] = serde_json::json!({"reserveTimeMs":1000});
    envelope["certification"] = serde_json::json!({"previousApplicationReportJson":application,"reportJson":final_raw,"error":null});
    envelope["applicationReportJson"] = selected.into();
    let finished = DiscreteLoadingBendersReport::from_json(&envelope.to_string(), &artifact).unwrap();
    assert!(finished.source_finishing_present()); assert!(finished.source_finishing_invoked()); assert_eq!(finished.certification_report_json(), Some(final_raw));
    assert_eq!(finished.acceleration_application_report_json(), application); assert_eq!(finished.certification_error_json().unwrap(), None);
    let result = read.solution().unwrap(); drop(read); drop(context); assert!(result.to_json().unwrap().contains("\"resources\""));
}

#[wasm_bindgen_test]
fn owned_partition_lifecycle_preserves_units_resources_and_explicit_invalidation_without_clocks() {
    use super::partition::PartitionLoadingContext;
    let source = SOURCE.replace("\"maxWeight\":{\"value\":1", "\"maxWeight\":{\"value\":2");
    let context = DiscreteLoadingContext::with_resources(&source, "null", r#"[{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1,"unit":"kg"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#).unwrap();
    let source = context.to_artifact_json().unwrap(); drop(context);
    let mut context = PartitionLoadingContext::new(&source, r#"{"partitions":[{"id":"whole","positionIds":["p","q"]}],"refinableResources":["ballast"]}"#).unwrap();
    let decisions: Vec<ospf_rust_bridge::BppLoadingPartitionDecision> = serde_json::from_str(&context.decisions_json().unwrap()).unwrap();
    let values = decisions.iter().map(|decision| (decision.id.clone(), 1.0)).collect::<std::collections::BTreeMap<_, _>>();
    context.add_columns(&serde_json::json!([{"partitionId":"whole","decisions":values}]).to_string()).unwrap();
    let master = context.master(r#"{"integer":true,"refineResources":true}"#).unwrap();
    let model = ospf_rust_bridge::decode_compiled_model(&serde_json::from_str(&master.to_model_json().unwrap()).unwrap()).unwrap();
    let mut values = vec![0.0; model.num_variables()];
    let columns: std::collections::BTreeMap<String, u32> = serde_json::from_str(&master.column_indices_json().unwrap()).unwrap();
    for index in columns.values() { values[*index as usize] = 1.0; }
    let globals: Vec<ospf_rust_bridge::BppLoadingPartitionGlobalBinding> = serde_json::from_str(&master.global_bindings_json().unwrap()).unwrap();
    let resource = decisions.iter().find(|decision| decision.resource_id.is_some()).unwrap();
    let binding = globals.iter().find(|binding| binding.source_index == resource.variable_index).unwrap(); values[binding.master_index as usize] = 0.5;
    let result = context.extract_solution(&master, &values).unwrap();
    context.materialize_completed_resources(&master, &values).unwrap(); assert!(context.source_values(&master, &values).is_err());
    assert_eq!(serde_json::from_str::<Vec<serde_json::Value>>(&context.columns_json().unwrap()).unwrap().len(), 2);
    drop(context); assert!(master.to_model_json().is_ok()); assert!(result.to_json().unwrap().contains("\"value\":0.5"));
}

#[wasm_bindgen_test]
fn partition_lp_gate_binds_full_certificates_with_u64_counters_on_wasm32() {
    use super::partition::PartitionLoadingContext;
    let source = DiscreteLoadingContext::new(SOURCE).unwrap().to_artifact_json().unwrap();
    let mut context = PartitionLoadingContext::new(&source, r#"{"partitions":[{"id":"whole","positionIds":["p","q"]}]}"#).unwrap();
    let decisions: Vec<ospf_rust_bridge::BppLoadingPartitionDecision> = serde_json::from_str(&context.decisions_json().unwrap()).unwrap();
    let values = decisions.iter().map(|decision| (decision.id.clone(), 1.0)).collect::<std::collections::BTreeMap<_, _>>();
    context.add_columns(&serde_json::json!([{"partitionId":"whole","decisions":values}]).to_string()).unwrap();
    let master = context.master("{}").unwrap(); let model = ospf_rust_bridge::decode_compiled_model(&serde_json::from_str(&master.to_model_json().unwrap()).unwrap()).unwrap();
    let mut solution = SolveSolution::vector(vec![1.0]); solution.objective_value = Some(0.0); solution.dual_solution = Some(vec![0.0; model.num_constraints()]);
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    let text = serde_json::to_string(&report).unwrap().replace("\"nodes\":null", "\"nodes\":18446744073709551615");
    context.accept_lp_report(&master, &text).unwrap(); assert!(context.pricing(&master, "whole", false).is_ok());
    assert!(context.accept_lp_report(&master, "{}").is_err()); assert!(context.pricing(&master, "whole", false).is_err());
}
