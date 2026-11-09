use std::cell::{Cell, RefCell};
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, linear_model_fingerprint, linear_model_mapping};
use super::*;

struct Control { observations: RefCell<Vec<BppLoadingPartitionObservation>>, cancelled: Cell<bool> }

impl Control {
    fn record(&self, terminal_check: bool) {
        self.observations.borrow_mut().push(BppLoadingPartitionObservation { terminal_check, remaining_nanos: "1000000000".into(), cancelled: self.cancelled.get() });
    }
}

impl PartitionColumnGenerationControl for Control {
    fn remaining(&self) -> ospf_rust_core::Result<Duration> { self.record(false); Ok(Duration::from_secs(1)) }
    fn terminal(&self) -> ospf_rust_core::Result<Option<TerminationReason>> { self.record(true); Ok(self.cancelled.get().then_some(TerminationReason::Cancelled)) }
}

fn fixture(cancel_final: bool, fail_final: bool) -> (ModelArtifact, String) {
    let problem = serde_json::from_str(r#"{"items":[],"positions":[{"id":"p","maxCount":"18446744073709551615","maxWeight":{"value":1,"unit":"kg"},"fixedOccupied":false,"enabled":true}],"allowedAssignments":[],"conflicts":[],"layoutGroups":[],"weightUnit":"kg"}"#).unwrap();
    let context = if fail_final { BppLoadingContext::with_resources(problem, None, serde_json::from_str(r#"[{"id":"ballast","positionId":"p","minimumWeight":{"value":0,"unit":"kg"},"maximumWeight":{"value":1,"unit":"kg"},"minimumPerLoadedWeight":0,"costPerWeight":0}]"#).unwrap()) }
        else { BppLoadingContext::new(problem) }.unwrap(); let source = context.artifact();
    let mut options: BppLoadingPartitionExecutionOptions = serde_json::from_str(r#"{"partition":{"partitions":[{"id":"whole","positionIds":["p"]}]}}"#).unwrap();
    if fail_final { options.partition.refinable_resources.push("ballast".into()); }
    let domain = options.partition.domain(&context).unwrap(); let registration = context.into_registration().unwrap();
    let control = Control { observations: Default::default(), cancelled: Cell::new(false) }; let mut calls = Vec::new(); let mut integers = 0;
    let result = PartitionColumnGenerationApplicationService::solve_with_control(&registration, domain, options.config(1000), &SolveHandle::new(), &control,
        |model, _, _| {
            if model.has_integer_variables() && model.variables.iter().any(|token| token.name() == "column_0") { integers += 1; }
            if fail_final && integers == 2 {
                let failure: ospf_rust_core::error::CoreError = ospf_rust_core::SolverError::SolveFailed("fake final backend failure".into()).into();
                calls.push(BppLoadingPartitionCall { model: encode_compiled_model(model).unwrap(), report_json: None, error: Some(BridgeError::new(failure.solver_error_class().code(), &failure)) });
                return Err(failure);
            }
            let mut values = vec![0.0; model.num_variables()];
            for (index, token) in model.variables.iter().enumerate() { if token.name() == "column_0" { values[index] = 1.0; } }
            let mut solution = SolveSolution::vector(values); solution.objective_value = Some(0.0);
            if !model.has_integer_variables() { solution.dual_solution = Some(vec![0.0; model.num_constraints()]); }
            let mut report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() }).model_mapping(Some(linear_model_mapping(model)?)).build()?;
            report.statistics.nodes = Some(usize::MAX);
            if cancel_final && model.has_integer_variables() && model.variables.iter().any(|token| token.name() == "column_0") { control.cancelled.set(true); }
            calls.push(BppLoadingPartitionCall { model: encode_compiled_model(model).unwrap(), report_json: Some(serde_json::to_string(&report).unwrap()), error: None }); Ok(report)
        }).unwrap();
    let text = encode_bpp_loading_partition(&source, BppLoadingPartitionEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
        options, time_limit_ms: 1000, observations: control.observations.into_inner(), calls, application_report_json: String::new(), certification: None }, &result).unwrap(); (source, text)
}

#[test]
fn original_service_replay_retains_root_convergence_actual_children_and_proof_free_source() {
    let (source, text) = fixture(false, false); let report = decode_bpp_loading_partition(&text, &source).unwrap();
    assert_eq!(report.json, text); assert!(report.summary.lp_converged); assert_eq!(report.summary.root_column_count, 1);
    assert_eq!(report.summary.trace.len(), 2); assert_eq!(report.application.solution.unwrap().loaded_count, "0");
    let core: serde_json::Value = serde_json::from_str(&report.application.envelope.report_json).unwrap(); assert!(core["proof"].is_null());
    assert!(report.envelope.calls.iter().any(|call| call.report_json.as_ref().unwrap().contains(&usize::MAX.to_string())));
}

#[test]
fn replay_rejects_reordered_children_budget_tampering_foreign_identity_and_proof_promotion() {
    let (source, text) = fixture(false, false);
    for variant in 0..9 {
        let mut envelope: BppLoadingPartitionEnvelope = serde_json::from_str(&text).unwrap();
        match variant {
            0 => envelope.calls.swap(0, 1),
            1 => { envelope.calls.pop(); },
            2 => envelope.observations[0].terminal_check = false,
            3 => envelope.observations[0].remaining_nanos = "01000000000".into(),
            4 => envelope.observations[1].remaining_nanos = "1000000001".into(),
            5 => envelope.source_fingerprint = "foreign".into(),
            6 => envelope.calls[0].model.fingerprint = "foreign".into(),
            7 => envelope.options.report_limit = Some(1),
            _ => { let mut app: BppLoadingApplicationEnvelope = serde_json::from_str(&envelope.application_report_json).unwrap();
                let mut report: serde_json::Value = serde_json::from_str(&app.report_json).unwrap(); report["proof"] = serde_json::to_value(SolveProof::<f64>::optimality()).unwrap();
                app.report_json = report.to_string(); envelope.application_report_json = serde_json::to_string(&app).unwrap(); }
        }
        assert!(decode_bpp_loading_partition(&serde_json::to_string(&envelope).unwrap(), &source).is_err(), "variant {variant}");
    }
}

#[test]
fn cancellation_observed_after_final_keeps_the_actual_validated_incumbent_without_source_proof() {
    let (source, text) = fixture(true, false); let report = decode_bpp_loading_partition(&text, &source).unwrap();
    assert!(report.application.solution.is_some()); assert_eq!(report.application.summary.termination_reason, "CANCELLED");
    assert!(report.summary.lp_converged);
    let core: serde_json::Value = serde_json::from_str(&report.application.envelope.report_json).unwrap(); assert!(core["proof"].is_null());
}

#[test]
fn recoverable_final_failure_keeps_the_verified_resource_incumbent_and_original_error_text() {
    let (source, text) = fixture(false, true); let report = decode_bpp_loading_partition(&text, &source).unwrap();
    assert!(report.application.solution.is_some()); assert_eq!(report.application.summary.termination_reason, "BACKEND_FAILURE");
    assert!(report.summary.refinement_call_index.is_some()); assert!(report.summary.final_call_index.is_none());
    assert!(report.envelope.calls.last().unwrap().error.as_ref().unwrap().message.contains("fake final backend failure"));
}

fn finishing_input() -> (ModelArtifact, String, ospf_rust_framework_bpp::application::DiscreteLoadingSolveResult<ospf_rust_quantities::unit::Unit>) {
    let (source, text) = fixture(false, false);
    let mut envelope: BppLoadingPartitionEnvelope = serde_json::from_str(&text).unwrap();
    envelope.options.source_milp_finishing = Some(BppLoadingSourceFinishingOptions { reserve_time_ms: 100 });
    envelope.time_limit_ms = 1100;
    let text = serde_json::to_string(&envelope).unwrap(); let decoded = decode_bpp_loading_partition(&text, &source).unwrap();
    let previous = ospf_rust_framework_bpp::application::DiscreteLoadingSolveResult {
        registration: Some(BppLoadingContext::from_artifact(&source).unwrap().into_registration().unwrap()),
        report: serde_json::from_str(&decoded.application.envelope.report_json).unwrap(),
        solution: None, objective: decoded.application.envelope.objective,
    };
    (source, text, previous)
}

#[test]
fn partition_finishing_retains_root_replay_and_only_uses_actual_complete_source_proofs() {
    use ospf_rust_framework_bpp::application::DiscreteLoadingCertificationService;
    let (source, text, previous) = finishing_input(); let values = previous.report.solution.as_ref().unwrap().values.clone();
    let result = DiscreteLoadingCertificationService::solve(previous, || BppLoadingContext::from_artifact(&source).unwrap().into_registration(),
        Default::default(), &SolveHandle::new(), |model, _, _| {
            assert_eq!(model.variables.iter().map(|token| token.get_result().unwrap()).collect::<Vec<_>>(), values);
            let mut solution = SolveSolution::vector(values.clone()); solution.objective_value = Some(0.0);
            let mut report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() }).model_mapping(Some(linear_model_mapping(model)?)).build()?;
            report.statistics.nodes = Some(usize::MAX); Ok(report)
        }).unwrap();
    let finished = encode_bpp_loading_partition_certification(&text, &result, &source).unwrap();
    let decoded = decode_bpp_loading_partition(&finished, &source).unwrap();
    assert_eq!(decoded.summary, decode_bpp_loading_partition(&text, &source).unwrap().summary);
    assert!(decoded.envelope.certification.as_ref().unwrap().report_json.as_ref().unwrap().contains(&usize::MAX.to_string()));
    let selected: SolveReport<f64, u64> = serde_json::from_str(&decoded.application.envelope.report_json).unwrap(); assert!(selected.is_optimal());
    for variant in 0..6 {
        let mut changed = decoded.envelope.clone();
        match variant {
            0 => changed.options.source_milp_finishing = None,
            1 => changed.certification.as_mut().unwrap().report_json = None,
            2 => changed.certification.as_mut().unwrap().previous_application_report_json = changed.application_report_json.clone(),
            3 => changed.options.source_milp_finishing.as_mut().unwrap().reserve_time_ms = 101,
            4 => changed.certification.as_mut().unwrap().error = Some(BridgeError::new("BACKEND", "conflicting evidence")),
            _ => changed.calls.clear(),
        }
        assert!(decode_bpp_loading_partition(&serde_json::to_string(&changed).unwrap(), &source).is_err(), "variant {variant}");
    }
    assert!(encode_bpp_loading_partition_certification(&finished, &result, &source).is_err());
}

#[test]
fn partition_finishing_preserves_validated_incumbents_after_empty_reports_errors_and_precancellation() {
    use ospf_rust_framework_bpp::application::{DiscreteLoadingCertificationService, DiscreteLoadingSolveOptions};
    for variant in 0..4 {
        let (source, text, previous) = finishing_input(); let handle = SolveHandle::new();
        if variant == 3 { handle.cancel("before-source-finishing"); }
        let result = DiscreteLoadingCertificationService::solve(previous, || { assert_ne!(variant, 3); BppLoadingContext::from_artifact(&source).unwrap().into_registration() },
            DiscreteLoadingSolveOptions::default(), &handle, |model, _, handle| {
                assert_ne!(variant, 3);
                if variant == 1 || variant == 2 {
                    if variant == 2 { handle.cancel("finishing-error-race"); }
                    return Err(ospf_rust_core::SolverError::NumericalError("original finishing error".into()).into());
                }
                SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit)
                    .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() }).model_mapping(Some(linear_model_mapping(model)?)).build()
            }).unwrap();
        let encoded = encode_bpp_loading_partition_certification(&text, &result, &source).unwrap();
        let report = decode_bpp_loading_partition(&encoded, &source).unwrap(); assert!(report.application.solution.is_some());
        let selected: SolveReport<f64, u64> = serde_json::from_str(&report.application.envelope.report_json).unwrap(); assert!(selected.proof.is_none());
        if variant == 0 { assert_eq!(selected.diagnostics.extensions.get("selected_incumbent").map(String::as_str), Some("validated_accelerator")); }
        if variant == 1 || variant == 2 { assert!(report.envelope.certification.unwrap().error.unwrap().message.contains("original finishing error")); }
        assert_eq!(report.application.summary.termination_reason, match variant { 0 => "TIME_LIMIT", 1 => "NUMERICAL_FAILURE", _ => "CANCELLED" });
    }
}

#[test]
fn reserved_full_source_can_follow_root_timeout_without_fabricating_root_registration_or_ignoring_cancellation() {
    use ospf_rust_framework_bpp::application::{DiscreteLoadingApplicationService, DiscreteLoadingCertificationResult};
    let (source, text, previous) = finishing_input(); let values = previous.report.solution.unwrap().values;
    for cancelled in [false, true] {
        let mut envelope: BppLoadingPartitionEnvelope = serde_json::from_str(&text).unwrap();
        envelope.calls.clear(); envelope.observations = vec![BppLoadingPartitionObservation { terminal_check: true, remaining_nanos: "0".into(), cancelled }];
        let report: SolveReport<f64> = SolveReport::builder(ProblemStatus::Unknown, if cancelled { TerminationReason::Cancelled } else { TerminationReason::TimeLimit }).build().unwrap();
        envelope.application_report_json = serde_json::to_string(&BppLoadingApplicationEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
            registration_present: false, report_json: serde_json::to_string(&report).unwrap(), objective: None }).unwrap();
        let acceleration = serde_json::to_string(&envelope).unwrap(); assert!(!decode_bpp_loading_partition(&acceleration, &source).unwrap().application.envelope.registration_present);
        let result = DiscreteLoadingApplicationService::solve(|| BppLoadingContext::from_artifact(&source).unwrap().into_registration(), Default::default(), &SolveHandle::new(), |model, _, _| {
            let mut solution = SolveSolution::vector(values.clone()); solution.objective_value = Some(0.0);
            SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
                .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() }).model_mapping(Some(linear_model_mapping(model)?)).build()
        }).unwrap();
        let finished = DiscreteLoadingCertificationResult { certification_report: Some(result.report.clone()), result, certification_error: None };
        let encoded = encode_bpp_loading_partition_certification(&acceleration, &finished, &source);
        if cancelled { assert!(encoded.is_err()); } else {
            let report = decode_bpp_loading_partition(&encoded.unwrap(), &source).unwrap(); assert!(report.application.envelope.registration_present);
            assert!(report.summary.trace.is_empty()); assert!(report.envelope.calls.is_empty());
        }
    }
}
