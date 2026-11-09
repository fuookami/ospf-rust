use ospf_rust_core::model::{MetaModel, SubObjective, flatten::{Linear, LinearMonomial}};
use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping, require_optimal_lp_certificate_for_linear_model};
use ospf_rust_core::variable::{Continuous, UInteger, VariableRange};
use super::*;

fn record(lp: bool) -> Csp1dStageReport {
    let mut model = MetaModel::new("actual-csp-child");
    let x = if lp {
        model.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 10.0)).unwrap()
    } else {
        model.register_auto_variable_with_range::<UInteger>("x", VariableRange::bounded(0.0, 10.0)).unwrap()
    };
    model.add_ge_constraint(&[(x, 1.0)], 2.0, "demand").unwrap();
    model.add_sub_objective(SubObjective::minimize(Linear::new(vec![LinearMonomial::new(1.0, x)], 7.0), "cost"));
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let mut solution = SolveSolution::vector(vec![2.0]);
    solution.objective_value = Some(2.0); solution.objective = Some(2.0);
    if lp { solution.dual_solution = Some(vec![-1.0]); }
    let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap()))
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() }).build().unwrap();
    Csp1dStageReport { stage: if lp { Csp1dSolveStage::Lp } else { Csp1dSolveStage::Milp }, model: compiled.model, report, objective_constant: compiled.objective_constant }
}

#[test]
fn actual_child_types_origins_constants_and_raw_report_identity_roundtrip() {
    for lp in [false, true] {
        let source = record(lp);
        let artifact = encode_csp1d_stage_report(&source).unwrap();
        let (model, report) = decode_csp1d_stage_report(&artifact).unwrap();
        assert_eq!(linear_model_fingerprint(&model).unwrap(), source.report.fingerprints.model.clone().unwrap());
        assert_eq!(model.constraint_names, source.model.constraint_names);
        assert_eq!(model.var_types, source.model.var_types);
        assert_eq!(model.variables[0].var_type(), source.model.variables[0].var_type());
        assert_eq!(report.model_mapping, source.report.model_mapping);
        assert_eq!(artifact.solver_report_json, serde_json::to_string(&source.report).unwrap());
        assert_eq!(artifact.objective_constant, 7.0); assert_eq!(report.solution.as_ref().unwrap().objective_value, Some(2.0));
        let rebuilt = record(lp);
        assert_ne!(source.report.fingerprints.model, rebuilt.report.fingerprints.model);
        let mut different = artifact.clone(); different.model = encode_compiled_linear_model(&rebuilt.model).unwrap();
        assert_eq!(decode_csp1d_stage_report(&different).unwrap_err().code, "FingerprintMismatch");
    }
    let mut final_record = record(false); final_record.stage = Csp1dSolveStage::FinalMilp;
    let final_artifact = encode_csp1d_stage_report(&final_record).unwrap();
    assert_eq!(final_artifact.stage, Csp1dStageKind::FinalMilp);
    decode_csp1d_stage_report(&final_artifact).unwrap();
}

#[test]
fn empty_terminal_reports_keep_actual_model_identity_and_reason_without_import_repair() {
    for reason in [TerminationReason::TimeLimit, TerminationReason::Cancelled, TerminationReason::Interrupted, TerminationReason::BackendFailure] {
        let mut source = record(false);
        source.report = SolveReport::builder(ProblemStatus::Unknown, reason)
            .fingerprints(source.report.fingerprints.clone()).model_mapping(source.report.model_mapping.clone()).build().unwrap();
        let mut artifact = encode_csp1d_stage_report(&source).unwrap();
        let (_, report) = decode_csp1d_stage_report(&artifact).unwrap();
        assert_eq!(report.termination_reason, reason); assert!(!report.has_incumbent()); assert!(report.proof.is_none());
        let mut missing = report; missing.fingerprints.model = None;
        artifact.solver_report_json = serde_json::to_string(&missing).unwrap();
        assert_eq!(decode_csp1d_stage_report(&artifact).unwrap_err().code, "FingerprintMismatch");
    }
}

#[test]
fn portable_lp_certificate_and_infeasibility_gates_keep_full_u64_counters() {
    let mut artifact = encode_csp1d_stage_report(&record(true)).unwrap();
    let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.solver_report_json).unwrap();
    report.statistics.nodes = Some(u64::MAX);
    report.provenance.thread_count = Some(9_007_199_254_740_993);
    artifact.solver_report_json = serde_json::to_string(&report).unwrap();
    let (model, exact) = decode_csp1d_stage_report(&artifact).unwrap();
    let certificate = require_optimal_lp_certificate_for_linear_model(&exact, &model).unwrap();
    assert_eq!(certificate.report.statistics.nodes, Some(u64::MAX));
    assert_eq!(certificate.report.provenance.thread_count, Some(9_007_199_254_740_993));
    let mut infeasible: SolveReport<f64> = SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
        .proof(SolveProof::infeasibility()).fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&model).unwrap()), ..Default::default() })
        .model_mapping(Some(linear_model_mapping(&model).unwrap())).build().unwrap();
    infeasible.statistics.nodes = Some(3);
    let mut portable: SolveReport<f64, u64> = serde_json::from_str(&serde_json::to_string(&infeasible).unwrap()).unwrap();
    portable.statistics.nodes = Some(u64::MAX);
    ospf_rust_core::solver::require_infeasibility_certificate_for_model(&portable, &linear_model_fingerprint(&model).unwrap()).unwrap();
    assert_eq!(portable.statistics.nodes, Some(u64::MAX));
}

#[test]
fn stage_reader_rejects_forged_vectors_proofs_objectives_and_model_identity_without_repair() {
    for lp in [false, true] {
        let original = encode_csp1d_stage_report(&record(lp)).unwrap();
        for variant in 0..11 {
            let mut artifact = original.clone();
            let mut report: SolveReport<f64, u64> = serde_json::from_str(&artifact.solver_report_json).unwrap();
            match variant {
                0 => report.solution.as_mut().unwrap().values.clear(),
                1 => report.solution.as_mut().unwrap().values = vec![1.0],
                2 => report.solution.as_mut().unwrap().values = vec![11.0],
                3 => { let solution = report.solution.as_mut().unwrap(); solution.values = vec![2.5]; solution.objective = Some(2.5); solution.objective_value = Some(2.5); },
                4 => report.solution.as_mut().unwrap().objective = Some(100.0),
                5 => { report.solution.as_mut().unwrap().stable_values.insert("foreign".into(), 2.0); },
                6 => report.fingerprints.model = None,
                7 => report.model_mapping = None,
                8 => report.solution.as_mut().unwrap().value = Some(2.0),
                9 => { if lp { report.solution.as_mut().unwrap().dual_solution = Some(vec![0.0]); } else { artifact.stage = Csp1dStageKind::Lp; } },
                _ => report.schema_version = "2.0".into(),
            }
            artifact.solver_report_json = serde_json::to_string(&report).unwrap();
            assert!(decode_csp1d_stage_report(&artifact).is_err(), "lp {lp}, variant {variant}");
        }
        let mut artifact = original.clone(); artifact.schema_version = "2.0".into();
        assert_eq!(decode_csp1d_stage_report(&artifact).unwrap_err().code, "UnsupportedProtocolVersion");
        artifact = original.clone(); artifact.model.schema_version = "2.0".into();
        assert_eq!(decode_csp1d_stage_report(&artifact).unwrap_err().code, "UnsupportedProtocolVersion");
        artifact = original.clone(); artifact.objective_constant = f64::INFINITY; assert!(decode_csp1d_stage_report(&artifact).is_err());
        artifact = original; artifact.solver_report_json = " ".repeat(8 * 1024 * 1024 + 1);
        assert_eq!(decode_csp1d_stage_report(&artifact).unwrap_err().code, "PayloadTooLarge");
    }
}
