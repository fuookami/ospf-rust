use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveHandle, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use ospf_rust_framework_bpp::application::{DiscreteLoadingApplicationService, DiscreteLoadingSolveOptions};
use super::*;

fn context() -> BppLoadingContext {
    BppLoadingContext::with_objective(super::super::tests::problem(), serde_json::from_str(r#"{"itemCosts":{"a":-10,"b":-20,"c":-3},"positionCosts":{"p":1,"r":2},"layoutCosts":[{"groupId":"g","layoutId":"outer","cost":4}]}"#).unwrap()).unwrap()
}

#[test]
fn preregistered_objectives_bind_full_source_and_reject_foreign_or_nonfinite_costs() {
    let context = context(); let source = context.artifact();
    assert_eq!(context.linear_model().model_json, BppLoadingContext::from_artifact(&source).unwrap().linear_model().model_json);
    let model = crate::decode_model(&context.linear_model()).unwrap();
    let values = context.solution_values(&super::super::tests::solution()).unwrap();
    assert_eq!(model.c.iter().zip(values).map(|(cost, value)| cost * value).sum::<f64>(), -23.0);
    assert_ne!(source.fingerprint, BppLoadingContext::new(super::super::tests::problem()).unwrap().artifact().fingerprint);
    for variant in 0..6 {
        let mut objective: super::super::BppLoadingObjective = serde_json::from_str(r#"{"itemCosts":{"a":-1},"positionCosts":{},"layoutCosts":[]}"#).unwrap();
        match variant {
            0 => { objective.item_costs.insert("foreign".into(), 1.0); },
            1 => { objective.item_costs.insert("a".into(), f64::INFINITY); },
            2 => { objective.position_costs.insert("foreign".into(), 0.0); },
            3 => { objective.position_costs.insert("p".into(), f64::NAN); },
            _ => {
                objective.layout_costs.push(super::super::BppLoadingLayoutCost { group_id: "g".into(), layout_id: if variant == 4 { "foreign".into() } else { "outer".into() }, cost: 1.0 });
                if variant == 5 { objective.layout_costs.push(objective.layout_costs[0].clone()); }
            }
        }
        assert!(BppLoadingContext::with_objective(super::super::tests::problem(), objective).is_err(), "variant {variant}");
    }
}

#[test]
fn actual_application_report_revalidates_original_registration_objective_and_independent_plan() {
    let context = context(); let source = context.artifact(); let values = context.solution_values(&super::super::tests::solution()).unwrap();
    let result = DiscreteLoadingApplicationService::solve(|| context.into_registration(), Default::default(), &SolveHandle::new(), |model, _, _| {
        let mut solution = SolveSolution::vector(values); solution.objective_value = Some(-23.0);
        let mut report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(model)?), ..Default::default() })
            .model_mapping(Some(linear_model_mapping(model)?)).build()?;
        report.statistics.nodes = Some(usize::MAX); Ok(report)
    }).unwrap();
    let actual = encode_model(&result.registration.as_ref().unwrap().model().linear_model_snapshot().unwrap().compile_flt64().unwrap().model).unwrap();
    let expected = BppLoadingContext::from_artifact(&source).unwrap().application_model().unwrap();
    assert_eq!(actual.model_json, expected.model_json);
    let text = encode_bpp_loading_application(&result, &source).unwrap(); let report = decode_bpp_loading_application(&text, &source).unwrap();
    assert_eq!(report.summary.objective_value, Some(-23.0)); assert_eq!(report.solution.unwrap().loaded_count, "2");
    assert!(report.envelope.report_json.contains(&usize::MAX.to_string())); assert!(report.envelope.registration_present);
    for variant in 0..6 {
        let mut envelope: BppLoadingApplicationEnvelope = serde_json::from_str(&text).unwrap();
        match variant { 0 => envelope.objective = Some(-22.0), 1 => envelope.objective = None, 2 => envelope.registration_present = false,
            3 => envelope.source_fingerprint = "foreign".into(), 4 => envelope.schema_version = "2.0".into(), _ => {
                let mut core: SolveReport<f64, u64> = serde_json::from_str(&envelope.report_json).unwrap(); core.fingerprints.model = None;
                envelope.report_json = serde_json::to_string(&core).unwrap();
            } }
        assert!(decode_bpp_loading_application(&serde_json::to_string(&envelope).unwrap(), &source).is_err(), "variant {variant}");
    }
}

#[test]
fn pre_solve_cancelled_or_time_limited_applications_do_not_fabricate_model_identity_or_zero_plans() {
    let source = context().artifact();
    for cancelled in [true, false] {
        let handle = SolveHandle::new(); if cancelled { handle.cancel("pre-solve-test"); }
        let options = DiscreteLoadingSolveOptions { time_limit: std::time::Duration::ZERO, ..Default::default() };
        let result = DiscreteLoadingApplicationService::solve::<Unit, _, _>(|| panic!("pre-solve stop must not create registration"), options, &handle,
            |_, _, _| panic!("pre-solve stop must not launch a backend")).unwrap();
        let text = encode_bpp_loading_application(&result, &source).unwrap(); let report = decode_bpp_loading_application(&text, &source).unwrap();
        assert!(!report.envelope.registration_present); assert!(report.solution.is_none());
        let core: SolveReport<f64, u64> = serde_json::from_str(&report.envelope.report_json).unwrap(); assert!(core.fingerprints.model.is_none());
        assert_eq!(report.summary.termination_reason, if cancelled { "CANCELLED" } else { "TIME_LIMIT" });
    }
}
