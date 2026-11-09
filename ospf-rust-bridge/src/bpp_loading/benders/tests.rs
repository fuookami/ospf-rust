use std::collections::BTreeMap;
use std::time::Duration;
use ospf_rust_core::solver::{SolveFingerprints, SolveHandle, SolveProof, SolveSolution, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework::solver::{FixedRecourseConfig, FixedRecourseResult, FixedRecourseStop as Stop, LinearBendersPartition};
use ospf_rust_framework_bpp::application::{DiscreteLoadingBendersApplicationService, DiscreteLoadingBendersOptions, DiscreteLoadingSolveResult};
use crate::{PhysicalQuantity, QuantityUnit};
use super::*;

fn context() -> BppLoadingContext {
    let problem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1,"unit":"kg"},"required":false,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
    let objective = serde_json::from_str(r#"{"itemCosts":{"a":-5},"positionCosts":{"p":1},"layoutCosts":[]}"#).unwrap();
    let resources = serde_json::from_str(r#"[{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1,"unit":"kg"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#).unwrap();
    BppLoadingContext::with_resources(problem, Some(objective), resources).unwrap()
}

fn options() -> BppLoadingBendersOptions {
    serde_json::from_str(r#"{"recourseResources":["ballast"]}"#).unwrap()
}

fn values(context: &BppLoadingContext) -> Vec<f64> {
    context.solution_values_with_resources(&serde_json::from_str(r#"{"assignments":{"a":"p"},"occupiedPositions":["p"],"selectedLayouts":{}}"#).unwrap(),
        &BTreeMap::from([("ballast".into(), PhysicalQuantity { value: 0.5, unit: QuantityUnit::Kilogram })])).unwrap()
}

fn report(model: &ospf_rust_core::model::intermediate::LinearTriadModel, values: Vec<f64>, proof: bool) -> SolveReport<f64> {
    let mut solution = SolveSolution::vector(values); solution.objective_value = Some(-3.0);
    let mut builder = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(model).unwrap()));
    if proof { builder = builder.proof(SolveProof::optimality()); }
    builder.build().unwrap()
}

#[test]
fn explicit_recourse_resources_map_to_real_continuous_variables_and_reject_invalid_configuration() {
    let context = context(); let valid = options(); valid.validate(&context).unwrap();
    let partition = context.benders_partition_options(&valid.recourse_resources).unwrap();
    assert_eq!(partition.stages.len(), 1);
    for json in [r#"{"recourseResources":[]}"#, r#"{"recourseResources":["unknown"]}"#, r#"{"recourseResources":["ballast","ballast"]}"#,
        r#"{"recourseResources":["ballast"],"iterationLimit":0}"#, r#"{"recourseResources":["ballast"],"iterationLimit":10001}"#] {
        assert!(serde_json::from_str::<BppLoadingBendersOptions>(json).unwrap().validate(&context).is_err());
    }
    assert!(serde_json::from_str::<BppLoadingBendersOptions>(r#"{"recourseResources":["ballast"],"callback":"code"}"#).is_err());
    let registration = context.into_registration().unwrap();
    assert_eq!(LinearBendersPartition::build(registration.model(), partition).unwrap().recourse_variables().len(), 1);
}

#[test]
fn original_service_fallback_uses_a_fresh_registration_and_preserves_actual_proof() {
    let initial = context(); let source = initial.artifact(); let snapshot = initial.application_snapshot().unwrap();
    let partition = initial.benders_partition_options(&options().recourse_resources).unwrap(); let values = values(&initial);
    let mut initial = Some(initial); let mut factories = 0; let mut calls = 0;
    let result = DiscreteLoadingBendersApplicationService::solve(|| {
        factories += 1; initial.take().map_or_else(|| BppLoadingContext::from_artifact(&source).unwrap().into_registration(), BppLoadingContext::into_registration)
    }, DiscreteLoadingBendersOptions { partition, fallback_to_milp: true, ..Default::default() }, &SolveHandle::new(), |model, _, _| {
        calls += 1; if calls == 1 { return Err(ospf_rust_core::CoreError::NotImplemented("contract backend lacks decomposition".into())); }
        Ok(report(model, values.clone(), true))
    }).unwrap();
    assert_eq!(factories, 2); assert!(result.fallback_used); assert!(result.decomposition.is_none());
    assert_ne!(snapshot.revision.owner, result.result.registration.as_ref().unwrap().model().model_identity());
    let mut options = options(); options.fallback_to_milp = Some(true);
    let text = encode_bpp_loading_benders(&result, &source, Some(&snapshot), &options).unwrap();
    let decoded = decode_bpp_loading_benders(&text, &source).unwrap(); assert!(decoded.envelope.fallback_used);
    assert!(decoded.envelope.decomposition_error.is_some());
    assert!(serde_json::from_str::<SolveReport<f64, u64>>(&decoded.application.envelope.report_json).unwrap().is_optimal());
}

#[test]
fn decomposition_uses_initial_tokens_even_with_a_fresh_final_registration_and_rejects_proof_promotion() {
    let initial = context(); let source = initial.artifact(); let snapshot = initial.application_snapshot().unwrap(); let values = values(&initial);
    let fresh = BppLoadingContext::from_artifact(&source).unwrap(); let fresh_snapshot = fresh.application_snapshot().unwrap();
    let registration = fresh.into_registration().unwrap(); let model = snapshot.compile_flt64().unwrap().model;
    let mut child = report(&model, values.clone(), true); child.statistics.nodes = Some(usize::MAX);
    let solution = registration.context().extract_solution(registration.model(), &values, &TOLERANCE).unwrap();
    let mut result = DiscreteLoadingBendersResult { result: DiscreteLoadingSolveResult { registration: Some(registration), report: child.clone(), solution: Some(solution), objective: Some(-3.0) },
        decomposition: Some(FixedRecourseResult { backend_error: None, stop: Stop::IterationLimit,
            values: snapshot.variables.iter().zip(&values).map(|(state, value)| (state.variable, *value)).collect(), objective: Some(-3.0), lower_bound: Some(-4.0),
            master_report: Some(child.clone()), subproblem_report: Some(child.clone()), iterations: 1, feasibility_cuts: 0, optimality_cuts: 0 }),
        decomposition_error: None, fallback_report: Some(child), fallback_used: true };
    let mut options = options(); options.fallback_to_milp = Some(true);
    assert!(encode_bpp_loading_benders(&result, &source, Some(&fresh_snapshot), &options).is_err());
    let text = encode_bpp_loading_benders(&result, &source, Some(&snapshot), &options).unwrap();
    let report = decode_bpp_loading_benders(&text, &source).unwrap(); assert_eq!(report.decomposition.unwrap().summary.values, values);
    assert!(report.envelope.decomposition_report_json.unwrap().contains(&usize::MAX.to_string()));
    result.fallback_used = false; result.fallback_report = None; result.result.report = self::report(&model, values.clone(), false);
    result.result.report.termination_reason = TerminationReason::IterationLimit;
    let text = encode_bpp_loading_benders(&result, &source, Some(&snapshot), &options).unwrap();
    for variant in 0..9 {
        let mut envelope: BppLoadingBendersEnvelope = serde_json::from_str(&text).unwrap();
        match variant {
            0 => envelope.source_fingerprint = "foreign".into(), 1 => envelope.options.recourse_resources = vec!["unknown".into()],
            2 => envelope.fallback_used = true, 3 => envelope.decomposition_report_json = None,
            4 => { let mut application: BppLoadingApplicationEnvelope = serde_json::from_str(&envelope.application_report_json).unwrap();
                let mut report: SolveReport<f64, u64> = serde_json::from_str(&application.report_json).unwrap(); report.proof = Some(SolveProof::optimality());
                application.report_json = serde_json::to_string(&report).unwrap();
                envelope.application_report_json = serde_json::to_string(&application).unwrap(); },
            5 => envelope.schema_version = "2.0".into(), 6 => envelope.options.iteration_limit = Some(0),
            7 => envelope.decomposition_report_json = envelope.decomposition_report_json.map(|text| text.replace("\"tolerance\":1e-7", "\"tolerance\":0.25")),
            _ => envelope.decomposition_error = Some(BridgeError::new("BACKEND", "fallback was not invoked")),
        }
        assert!(decode_bpp_loading_benders(&serde_json::to_string(&envelope).unwrap(), &source).is_err(), "variant {variant}");
    }
    let mut worse = SolveSolution::vector(vec![0.0; values.len()]); worse.objective_value = Some(0.0);
    result.fallback_used = true;
    result.fallback_report = Some(SolveReport::builder(ProblemStatus::Feasible, TerminationReason::TimeLimit).solution(worse)
        .fingerprints(result.result.report.fingerprints.clone()).model_mapping(result.result.report.model_mapping.clone()).build().unwrap());
    result.result.report.termination_reason = TerminationReason::TimeLimit;
    let retained = encode_bpp_loading_benders(&result, &source, Some(&snapshot), &options).unwrap();
    let decoded = decode_bpp_loading_benders(&retained, &source).unwrap();
    assert_eq!(decoded.application.envelope.objective, Some(-3.0));
    assert_ne!(decoded.envelope.fallback_report_json.as_ref(), Some(&decoded.application.envelope.report_json));
    let mut envelope: BppLoadingBendersEnvelope = serde_json::from_str(&retained).unwrap();
    let mut application: BppLoadingApplicationEnvelope = serde_json::from_str(&envelope.application_report_json).unwrap();
    application.report_json = envelope.fallback_report_json.clone().unwrap(); application.objective = Some(0.0);
    envelope.application_report_json = serde_json::to_string(&application).unwrap();
    assert!(decode_bpp_loading_benders(&serde_json::to_string(&envelope).unwrap(), &source).is_err());
    result.fallback_report = None;
    result.fallback_used = true;
    result.decomposition_error = Some(ospf_rust_core::SolverError::NumericalError("fake fallback failed during cancellation".into()).into());
    result.result.report.termination_reason = TerminationReason::Cancelled;
    assert!(encode_bpp_loading_benders(&result, &source, Some(&snapshot), &options).is_ok());
}

#[test]
fn pre_solve_stop_never_invokes_factory_or_backend_and_keeps_unbound_report() {
    let source = context().artifact();
    for cancelled in [true, false] {
        let handle = SolveHandle::new(); if cancelled { handle.cancel("contract-cancel"); }
        let result = DiscreteLoadingBendersApplicationService::solve::<Unit, _, _>(|| panic!("no factory before pre-solve stop"),
            DiscreteLoadingBendersOptions { config: FixedRecourseConfig { time_limit: Duration::ZERO, ..Default::default() }, ..Default::default() },
            &handle, |_, _, _| panic!("no backend before pre-solve stop")).unwrap();
        let text = encode_bpp_loading_benders(&result, &source, None, &options()).unwrap();
        let report = decode_bpp_loading_benders(&text, &source).unwrap(); assert!(report.decomposition.is_none());
        assert!(!report.application.envelope.registration_present); assert!(report.application.solution.is_none());
    }
}
