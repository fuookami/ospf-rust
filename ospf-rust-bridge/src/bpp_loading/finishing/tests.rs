use std::collections::BTreeMap;
use ospf_rust_core::solver::{SolveFingerprints, SolveHandle, SolveProof, SolveSolution, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework::solver::{FixedRecourseResult, FixedRecourseStop};
use ospf_rust_framework_bpp::application::{DiscreteLoadingApplicationService, DiscreteLoadingBendersApplicationService, DiscreteLoadingBendersOptions,
    DiscreteLoadingBendersResult, DiscreteLoadingCertificationService, DiscreteLoadingSolveOptions, DiscreteLoadingSolveResult};
use super::*;

fn context() -> BppLoadingContext {
    let problem = serde_json::from_str(r#"{"items":[{"id":"a","weight":{"value":1,"unit":"kg"},"required":false,"fixedPositionId":null}],"positions":[{"id":"p","maxCount":"1","maxWeight":{"value":2,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[{"itemId":"a","positionId":"p"}],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
    let objective = serde_json::from_str(r#"{"itemCosts":{"a":-5},"positionCosts":{"p":1},"layoutCosts":[]}"#).unwrap();
    let resources = serde_json::from_str(r#"[{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1,"unit":"kg"},"minimumPerLoadedWeight":0.5,"costPerWeight":2}]"#).unwrap();
    BppLoadingContext::with_resources(problem, Some(objective), resources).unwrap()
}

fn report(model: &ospf_rust_core::model::intermediate::LinearTriadModel, values: Option<Vec<f64>>, proof: bool) -> SolveReport<f64> {
    let solution = values.map(|values| { let objective = values.iter().zip(&model.c).map(|(v, c)| v * c).sum();
        let mut solution = SolveSolution::vector(values); solution.objective_value = Some(objective); solution });
    let mut builder = SolveReport::builder(if solution.is_some() { ProblemStatus::Feasible } else { ProblemStatus::Unknown }, if proof { TerminationReason::Completed } else { TerminationReason::TimeLimit })
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model).unwrap()), ..Default::default() }).model_mapping(Some(linear_model_mapping(model).unwrap()));
    if let Some(solution) = solution { builder = builder.solution(solution); }
    if proof { builder = builder.proof(SolveProof::optimality()); }
    builder.build().unwrap()
}

fn acceleration() -> (ModelArtifact, String, DiscreteLoadingSolveResult<Unit>) {
    let context = context(); let source = context.artifact(); let snapshot = context.application_snapshot().unwrap();
    let values = context.solution_values_with_resources(&serde_json::from_str(r#"{"assignments":{"a":"p"},"occupiedPositions":["p"],"selectedLayouts":{}}"#).unwrap(),
        &BTreeMap::from([("ballast".into(), crate::PhysicalQuantity { value: 0.5, unit: crate::QuantityUnit::Kilogram })])).unwrap();
    let registration = context.into_registration().unwrap();
    let solution = registration.context().extract_solution(registration.model(), &values, &TOLERANCE).unwrap();
    let mut child = report(&snapshot.compile_flt64().unwrap().model, Some(values.clone()), false); child.termination_reason = TerminationReason::IterationLimit;
    let result = DiscreteLoadingBendersResult { result: DiscreteLoadingSolveResult { registration: Some(registration), report: child.clone(), solution: Some(solution), objective: Some(-3.0) },
        decomposition: Some(FixedRecourseResult { backend_error: None, stop: FixedRecourseStop::IterationLimit, values: snapshot.variables.iter().zip(values).map(|(state, value)| (state.variable, value)).collect(),
            objective: Some(-3.0), lower_bound: Some(-4.0), master_report: Some(child.clone()), subproblem_report: Some(child), iterations: 1, feasibility_cuts: 0, optimality_cuts: 0 }),
        decomposition_error: None, fallback_report: None, fallback_used: false };
    let options = serde_json::from_str(r#"{"recourseResources":["ballast"],"sourceMilpFinishing":{"reserveTimeMs":1000}}"#).unwrap();
    let json = super::super::encode_bpp_loading_benders(&result, &source, Some(&snapshot), &options).unwrap(); (source, json, result.result)
}

#[test]
fn reserve_is_positive_and_strictly_inside_total_budget() {
    for reserve_time_ms in [0, 1000, u32::MAX] { assert!(BppLoadingSourceFinishingOptions { reserve_time_ms }.accelerator_budget(Duration::from_secs(1)).is_err()); }
    assert_eq!(BppLoadingSourceFinishingOptions { reserve_time_ms: 250 }.accelerator_budget(Duration::from_secs(1)).unwrap(), Duration::from_millis(750));
    assert!(serde_json::from_str::<BppLoadingSourceFinishingOptions>(r#"{"reserveTimeMs":1,"callback":true}"#).is_err());
}

#[test]
fn actual_fresh_source_proof_retains_accelerator_and_exact_original_counters() {
    let (source, acceleration, previous) = acceleration(); let owner = previous.registration.as_ref().unwrap().model().model_identity();
    let values = previous.report.solution.as_ref().unwrap().values.clone();
    let result = DiscreteLoadingCertificationService::solve(previous, || { let fresh = context().into_registration().unwrap(); assert_ne!(fresh.model().model_identity(), owner); Ok(fresh) },
        Default::default(), &SolveHandle::new(), |model, _, _| {
            assert_eq!(model.basic.variables.iter().map(|token| token.get_result().unwrap()).collect::<Vec<_>>(), values);
            let mut report = report(model, Some(values.clone()), true); report.statistics.nodes = Some(usize::MAX); Ok(report)
        }).unwrap();
    let json = encode_bpp_loading_certification(&acceleration, &result, &source).unwrap();
    let decoded = decode_bpp_loading_benders(&json, &source).unwrap(); let evidence = decoded.envelope.certification.unwrap();
    assert!(evidence.report_json.unwrap().contains(&usize::MAX.to_string()));
    assert_eq!(evidence.previous_application_report_json, decode_bpp_loading_benders(&acceleration, &source).unwrap().envelope.application_report_json);
    assert!(serde_json::from_str::<SolveReport<f64, u64>>(&decoded.application.envelope.report_json).unwrap().is_optimal());
    assert!(encode_bpp_loading_certification(&json, &result, &source).is_err());
}

#[test]
fn worse_or_missing_candidates_keep_old_plan_and_rebuild_audit_without_old_proofs() {
    for candidate in [false, true] {
        let (source, acceleration, previous) = acceleration(); let length = previous.report.solution.as_ref().unwrap().values.len();
        let result = DiscreteLoadingCertificationService::solve(previous, || context().into_registration(), Default::default(), &SolveHandle::new(),
            |model, _, _| Ok(report(model, candidate.then(|| vec![0.0; length]), false))).unwrap();
        let text = encode_bpp_loading_certification(&acceleration, &result, &source).unwrap();
        let decoded = decode_bpp_loading_benders(&text, &source).unwrap(); assert_eq!(decoded.application.envelope.objective, Some(-3.0));
        let selected: SolveReport<f64, u64> = serde_json::from_str(&decoded.application.envelope.report_json).unwrap();
        assert!(selected.proof.is_none()); assert_eq!(selected.diagnostics.extensions.get("selected_incumbent").map(String::as_str), Some("validated_accelerator"));
        let mut envelope = decoded.envelope; let mut application: BppLoadingApplicationEnvelope = serde_json::from_str(&envelope.application_report_json).unwrap();
        let mut altered = selected; altered.diagnostics.extensions.clear(); application.report_json = serde_json::to_string(&altered).unwrap();
        envelope.application_report_json = serde_json::to_string(&application).unwrap();
        assert!(decode_bpp_loading_benders(&serde_json::to_string(&envelope).unwrap(), &source).is_err());
    }
}

#[test]
fn recoverable_failure_and_cancellation_preserve_old_incumbent_and_error_evidence() {
    for cancel in [false, true] {
        let (source, acceleration, previous) = acceleration(); let handle = SolveHandle::new();
        let result = DiscreteLoadingCertificationService::solve(previous, || context().into_registration(), Default::default(), &handle, |_, _, handle| {
            if cancel { handle.cancel("cancel-versus-numerical"); }
            Err(ospf_rust_core::SolverError::NumericalError("contract failure".into()).into())
        }).unwrap();
        let json = encode_bpp_loading_certification(&acceleration, &result, &source).unwrap(); let decoded = decode_bpp_loading_benders(&json, &source).unwrap();
        assert_eq!(decoded.application.summary.termination_reason, if cancel { "CANCELLED" } else { "NUMERICAL_FAILURE" });
        assert!(decoded.envelope.certification.unwrap().error.is_some()); assert_eq!(decoded.application.envelope.objective, Some(-3.0));
    }
}

#[test]
fn timeout_or_cancel_does_not_invoke_fresh_factory() {
    for cancel in [false, true] {
        let (source, acceleration, previous) = acceleration(); let handle = SolveHandle::new(); if cancel { handle.cancel("before-finishing"); }
        let result = DiscreteLoadingCertificationService::solve(previous, || panic!("no fresh factory"), DiscreteLoadingSolveOptions { time_limit: Duration::ZERO, ..Default::default() },
            &handle, |_, _, _| panic!("no finishing backend")).unwrap();
        let text = encode_bpp_loading_certification(&acceleration, &result, &source).unwrap(); let decoded = decode_bpp_loading_benders(&text, &source).unwrap();
        assert!(decoded.envelope.certification.unwrap().report_json.is_none()); assert_eq!(decoded.application.envelope.objective, Some(-3.0));
    }
}

#[test]
fn certified_source_stays_identical_even_with_zero_budget_and_cancellation() {
    let (source, _, previous) = acceleration(); let values = previous.report.solution.as_ref().unwrap().values.clone();
    let first = DiscreteLoadingCertificationService::solve(previous, || context().into_registration(), Default::default(), &SolveHandle::new(),
        |model, _, _| Ok(report(model, Some(values.clone()), true))).unwrap();
    let old = encode_bpp_loading_application(&first.result, &source).unwrap();
    let handle = SolveHandle::new(); handle.cancel("after-source-proof");
    let next = DiscreteLoadingCertificationService::solve(first.result, || panic!("source proof needs no factory"),
        DiscreteLoadingSolveOptions { time_limit: Duration::ZERO, ..Default::default() }, &handle, |_, _, _| panic!("source proof needs no backend")).unwrap();
    let previous = decode_bpp_loading_application(&old, &source).unwrap();
    let selected = decode_bpp_loading_application(&encode_bpp_loading_application(&next.result, &source).unwrap(), &source).unwrap();
    let mut evidence = BppLoadingCertificationEvidence { previous_application_report_json: old, report_json: None, error: None };
    validate(&evidence, &previous, &selected, &source).unwrap();
    evidence.report_json = Some(selected.envelope.report_json.clone()); assert!(validate(&evidence, &previous, &selected, &source).is_err());
}

#[test]
fn finishing_gate_rejects_changed_identity_configuration_proof_and_evidence() {
    let (source, acceleration, previous) = acceleration(); let values = previous.report.solution.as_ref().unwrap().values.clone();
    let result = DiscreteLoadingCertificationService::solve(previous, || context().into_registration(), Default::default(), &SolveHandle::new(),
        |model, _, _| Ok(report(model, Some(values.clone()), true))).unwrap();
    let text = encode_bpp_loading_certification(&acceleration, &result, &source).unwrap();
    for variant in 0..7 {
        let mut envelope: BppLoadingBendersEnvelope = serde_json::from_str(&text).unwrap();
        match variant {
            0 => envelope.options.source_milp_finishing = None,
            1 => envelope.certification.as_mut().unwrap().error = Some(BridgeError::new("BACKEND", "cannot have both evidence forms")),
            2 => envelope.certification.as_mut().unwrap().report_json = None,
            3 => envelope.certification.as_mut().unwrap().previous_application_report_json = envelope.application_report_json.clone(),
            4 => {
                let evidence = envelope.certification.as_mut().unwrap();
                let mut report: SolveReport<f64, u64> = serde_json::from_str(evidence.report_json.as_ref().unwrap()).unwrap();
                report.fingerprints.model = None; evidence.report_json = Some(serde_json::to_string(&report).unwrap());
            },
            5 => envelope.options.source_milp_finishing.as_mut().unwrap().reserve_time_ms = 0,
            _ => envelope.source_fingerprint = "foreign".into(),
        }
        assert!(decode_bpp_loading_benders(&serde_json::to_string(&envelope).unwrap(), &source).is_err(), "variant {variant}");
    }
}

#[test]
fn reserved_source_solve_can_follow_an_unregistered_accelerator_timeout_but_not_cancellation() {
    let (source, _, plan) = acceleration(); let values = plan.report.solution.as_ref().unwrap().values.clone();
    for cancel in [false, true] {
        let handle = SolveHandle::new(); if cancel { handle.cancel("pre-registration-cancel"); }
        let result = DiscreteLoadingBendersApplicationService::solve::<Unit, _, _>(|| panic!("no accelerator registration"),
            DiscreteLoadingBendersOptions { config: ospf_rust_framework::solver::FixedRecourseConfig { time_limit: Duration::ZERO, ..Default::default() }, ..Default::default() },
            &handle, |_, _, _| panic!("no accelerator backend")).unwrap();
        let options = serde_json::from_str(r#"{"recourseResources":["ballast"],"sourceMilpFinishing":{"reserveTimeMs":1000}}"#).unwrap();
        let acceleration = super::super::encode_bpp_loading_benders(&result, &source, None, &options).unwrap();
        let result = DiscreteLoadingApplicationService::solve(|| context().into_registration(), Default::default(), &SolveHandle::new(),
            |model, _, _| Ok(report(model, Some(values.clone()), true))).unwrap();
        let finished = DiscreteLoadingCertificationResult { certification_report: Some(result.report.clone()), result, certification_error: None };
        let encoded = encode_bpp_loading_certification(&acceleration, &finished, &source);
        if cancel { assert!(encoded.is_err(), "cancelled acceleration cannot invoke finishing"); }
        else { let report = decode_bpp_loading_benders(&encoded.unwrap(), &source).unwrap(); assert!(report.application.envelope.registration_present); }
    }
}

#[test]
fn factory_budget_exhaustion_without_incumbent_retains_previous_registered_identity() {
    let (source, _, mut previous) = acceleration();
    previous.solution = None; previous.objective = None;
    let model = previous.registration.as_ref().unwrap().model().linear_model_snapshot().unwrap().compile_flt64().unwrap().model;
    previous.report = report(&model, None, false);
    let old = encode_bpp_loading_application(&previous, &source).unwrap();
    let result = DiscreteLoadingCertificationService::solve(previous, || { std::thread::sleep(Duration::from_millis(50)); context().into_registration() },
        DiscreteLoadingSolveOptions { time_limit: Duration::from_millis(10), ..Default::default() }, &SolveHandle::new(), |_, _, _| panic!("factory exhausted the deadline")).unwrap();
    assert!(result.result.registration.is_some()); assert!(result.certification_report.is_none());
    let previous = decode_bpp_loading_application(&old, &source).unwrap();
    let selected = decode_bpp_loading_application(&encode_bpp_loading_application(&result.result, &source).unwrap(), &source).unwrap();
    validate(&BppLoadingCertificationEvidence { previous_application_report_json: old, report_json: None, error: None }, &previous, &selected, &source).unwrap();
}
