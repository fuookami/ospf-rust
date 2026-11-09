use ospf_rust_core::solver::{SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
use super::*;
use crate::{Csp1dProblem, encode_csp1d_problem, generate_csp1d_patterns};

pub(super) fn fixture() -> (ModelArtifact, Vec<domain::CuttingPlan<f64>>, domain::Csp1dStageReport) {
    let problem: Csp1dProblem = serde_json::from_str(r#"{"products":[{"id":"p","name":"product","width":[{"value":30,"unit":"mm"}]}],"materials":[{"id":"m","name":"material","widthRange":{"lowerBound":{"value":0,"unit":"cm"},"upperBound":{"value":10,"unit":"cm"}},"machineId":"mc","availableBatches":"3"}],"machines":[{"id":"mc","name":"machine","widthRange":null,"maxBatchCount":"3"}],"demands":[{"productId":"p","mode":"roll","count":"5"}],"lengthUnit":"cm"}"#).unwrap();
    let artifact = encode_csp1d_problem(problem.clone()).unwrap();
    let source = problem.domain().unwrap();
    let plans = generate_csp1d_patterns(&problem, &Default::default()).unwrap().plans.iter()
        .map(|plan| decode_csp1d_pattern(&source, plan, problem.length_unit).unwrap()).collect::<Vec<_>>();
    let mut builder = domain::Csp1dProduceContextBuilder::new(domain::ProduceInput { cutting_plans: plans.clone(), demands: source.demands,
        materials: source.materials, machines: source.machines, warm_start_plan_usages: Vec::new() });
    builder.mode(domain::Csp1dModelingMode::MILP);
    let mut context = builder.build().unwrap();
    let mut model = MetaModel::new("actual-integer-child");
    context.register(&mut model).unwrap();
    let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
    let mut record = domain::Csp1dStageReport { stage: domain::Csp1dSolveStage::Milp, model: compiled.model,
        report: SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit).build().unwrap(), objective_constant: compiled.objective_constant };
    set_solution(&mut record, vec![0.0, 0.0, 2.0]);
    (artifact, plans, record)
}

fn set_solution(record: &mut domain::Csp1dStageReport, values: Vec<f64>) {
    let mut solution = SolveSolution::vector(values.clone());
    solution.objective_value = Some(values.iter().zip(&record.model.c).map(|(value, cost)| value * cost).sum());
    record.report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
        .model_mapping(Some(linear_model_mapping(&record.model).unwrap()))
        .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&record.model).unwrap()), ..Default::default() }).build().unwrap();
}

#[test]
fn source_context_binds_actual_columns_and_derives_production_without_relabeling_reports() {
    let (source, plans, record) = fixture();
    let text = encode_csp1d_production_report(&source, &plans, &record).unwrap();
    let result = decode_csp1d_production_report(&text, &source).unwrap();
    assert_eq!(result.artifact.child.solver_report_json, serde_json::to_string(&record.report).unwrap());
    assert_eq!(result.artifact.child.model.model_fingerprint, record.report.fingerprints.model.clone().unwrap());
    assert_eq!(result.production.as_ref().unwrap().batch_count, "2");
    assert_eq!(result.production.as_ref().unwrap().produced["p"], "6");
    assert_eq!(result.production.as_ref().unwrap().usages[0].plan.id, plans[2].id.as_str());
    assert!(result.summary.restricted_optimal); assert!(!result.summary.candidate_complete);
    assert_eq!(result.summary.objective_value, Some(2.0)); assert_eq!(result.summary.column_count, 3);
    let mut reordered = result.artifact.clone(); reordered.plans.reverse();
    let reordered = decode_csp1d_production_report(&serde_json::to_string(&reordered).unwrap(), &source).unwrap();
    assert_eq!(reordered.production, result.production);
    assert_eq!(reordered.artifact.child.solver_report_json, result.artifact.child.solver_report_json);
    let mut different = record.clone(); set_solution(&mut different, vec![0.0, 3.0, 0.0]);
    let other = decode_csp1d_production_report(&encode_csp1d_production_report(&source, &plans, &different).unwrap(), &source).unwrap();
    assert_eq!(other.production.as_ref().unwrap().batch_count, "3");
    assert_eq!(other.production.as_ref().unwrap().produced["p"], "6");
    assert_ne!(other.production.unwrap().usages, result.production.unwrap().usages);
    let mut final_record = record; final_record.stage = domain::Csp1dSolveStage::FinalMilp;
    let final_result = decode_csp1d_production_report(&encode_csp1d_production_report(&source, &plans, &final_record).unwrap(), &source).unwrap();
    assert_eq!(final_result.summary.stage, Csp1dStageKind::FinalMilp);
}

#[test]
fn independently_valid_child_models_cannot_be_attached_to_another_source_pool() {
    let (source, plans, record) = fixture();
    for variant in 0..7 {
        let mut different = record.clone();
        match variant {
            0 => different.model.c[2] = 2.0,
            1 => different.model.b[0] -= 1.0,
            2 => different.model.A.rows[0].entries[2].1 -= 1.0,
            3 => different.model.ub[2] = 10.0,
            4 => different.model.var_types[2] = ospf_rust_core::variable::VariableType::Continuous,
            5 => different.model.constraint_lazy_flags[0] = true,
            _ => different.objective_constant = 1.0,
        }
        set_solution(&mut different, vec![0.0, 0.0, 2.0]);
        encode_csp1d_stage_report(&different).unwrap();
        assert!(encode_csp1d_production_report(&source, &plans, &different).is_err(), "variant {variant}");
    }
    let mut other_problem = decode_csp1d_problem(&source).unwrap().0;
    other_problem.demands[0].count = "4".into();
    let other = encode_csp1d_problem(other_problem).unwrap();
    let text = encode_csp1d_production_report(&source, &plans, &record).unwrap();
    assert_eq!(decode_csp1d_production_report(&text, &other).unwrap_err().code, "FingerprintMismatch");
    let mut forged: Csp1dProductionReportArtifact = serde_json::from_str(&text).unwrap();
    forged.source_fingerprint = other.fingerprint.clone();
    assert!(decode_csp1d_production_report(&serde_json::to_string(&forged).unwrap(), &other).is_err());
}

#[test]
fn complete_pool_identity_and_schema_are_required_not_just_positive_selected_usages() {
    let (source, plans, record) = fixture();
    let text = encode_csp1d_production_report(&source, &plans, &record).unwrap();
    for variant in 0..9 {
        let mut artifact: Csp1dProductionReportArtifact = serde_json::from_str(&text).unwrap();
        match variant {
            0 => { artifact.plans.remove(0); },
            1 => artifact.plans[0].id = "foreign".into(),
            2 => artifact.plans[0].slices[0].amount = "2".into(),
            3 => artifact.plans[0].id = artifact.plans[1].id.clone(),
            4 => artifact.plans[0] = artifact.plans[1].clone(),
            5 => artifact.plans.clear(),
            6 => artifact.child.stage = Csp1dStageKind::Lp,
            7 => artifact.schema_version = "2.0".into(),
            _ => artifact.child.objective_constant = 7.0,
        }
        assert!(decode_csp1d_production_report(&serde_json::to_string(&artifact).unwrap(), &source).is_err(), "variant {variant}");
    }
    assert_eq!(decode_csp1d_production_report(&" ".repeat(MAX_BYTES + 1), &source).unwrap_err().code, "PayloadTooLarge");
    let injected = text.replacen("\"schemaVersion\":\"1.0\"", "\"schemaVersion\":\"1.0\",\"globallyOptimal\":true", 1);
    assert!(decode_csp1d_production_report(&injected, &source).is_err());
}

#[test]
fn empty_terminal_reports_preserve_reasons_and_never_fabricate_production_or_global_proofs() {
    let (source, plans, record) = fixture();
    for reason in [TerminationReason::Cancelled, TerminationReason::TimeLimit, TerminationReason::Interrupted, TerminationReason::BackendFailure] {
        let mut terminal = record.clone();
        terminal.report = SolveReport::builder(ProblemStatus::Unknown, reason).fingerprints(record.report.fingerprints.clone())
            .model_mapping(record.report.model_mapping.clone()).build().unwrap();
        let result = decode_csp1d_production_report(&encode_csp1d_production_report(&source, &plans, &terminal).unwrap(), &source).unwrap();
        assert!(result.production.is_none()); assert!(!result.summary.has_production); assert!(!result.summary.restricted_optimal);
        assert!(!result.summary.restricted_infeasible); assert!(!result.summary.candidate_complete);
        assert_eq!(result.summary.termination_reason, serde_json::to_value(reason).unwrap().as_str().unwrap());
    }
    let mut terminal = record.clone();
    terminal.report = SolveReport::builder(ProblemStatus::Infeasible, TerminationReason::Completed).proof(SolveProof::infeasibility())
        .fingerprints(record.report.fingerprints.clone()).model_mapping(record.report.model_mapping.clone()).build().unwrap();
    let result = decode_csp1d_production_report(&encode_csp1d_production_report(&source, &plans, &terminal).unwrap(), &source).unwrap();
    assert!(result.summary.restricted_infeasible); assert!(result.production.is_none()); assert!(!result.summary.candidate_complete);
}

#[test]
fn exact_raw_u64_metadata_is_retained_and_unsafe_production_counts_are_rejected() {
    let (source, plans, record) = fixture();
    let mut artifact: Csp1dProductionReportArtifact = serde_json::from_str(&encode_csp1d_production_report(&source, &plans, &record).unwrap()).unwrap();
    let mut portable: SolveReport<f64, u64> = serde_json::from_str(&artifact.child.solver_report_json).unwrap();
    portable.statistics.nodes = Some(u64::MAX);
    artifact.child.solver_report_json = serde_json::to_string(&portable).unwrap();
    let result = decode_csp1d_production_report(&serde_json::to_string(&artifact).unwrap(), &source).unwrap();
    assert_eq!(result.artifact.child.solver_report_json, artifact.child.solver_report_json);
    let (_, report) = decode_csp1d_stage_report(&result.artifact.child).unwrap();
    assert_eq!(report.statistics.nodes, Some(u64::MAX));
    let mut problem = decode_csp1d_problem(&source).unwrap().0;
    problem.materials[0].available_batches = None; problem.machines[0].max_batch_count = None;
    let large_source = encode_csp1d_problem(problem.clone()).unwrap();
    let domain = problem.domain().unwrap();
    let mut builder = domain::Csp1dProduceContextBuilder::new(domain::ProduceInput { cutting_plans: plans.clone(), demands: domain.demands,
        materials: domain.materials, machines: domain.machines, warm_start_plan_usages: Vec::new() });
    builder.mode(domain::Csp1dModelingMode::MILP);
    let mut context = builder.build().unwrap();
    let mut model = MetaModel::new("large-child"); context.register(&mut model).unwrap();
    let mut large = record; large.model = model.linear_model_snapshot().unwrap().compile_flt64().unwrap().model;
    set_solution(&mut large, vec![0.0, 0.0, MAX_SAFE_COUNT]);
    let result = decode_csp1d_production_report(&encode_csp1d_production_report(&large_source, &plans, &large).unwrap(), &large_source).unwrap();
    assert_eq!(result.production.unwrap().produced["p"], "27021597764222973");
    set_solution(&mut large, vec![0.0, 0.0, MAX_SAFE_COUNT + 1.0]);
    assert!(encode_csp1d_production_report(&large_source, &plans, &large).is_err());
}
